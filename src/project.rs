use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::binder::{Binder, BinderItem};
use crate::document::Document;
use crate::error::{Result, ScrivenerError};
use crate::metadata::ProjectMetadata;
use crate::search::{Match, SearchResult};
use crate::scrivx::{parse_scrivx_str, serialize_scrivx, serialize_scrivx_preserving};
use crate::statistics::ProjectStatistics;
use crate::trash::{Trash, TrashedItem};

/// A Scrivener 3 project loaded from a `.scriv` bundle.
#[derive(Debug, Clone)]
pub struct Project {
    pub path: PathBuf,
    pub binder: Binder,
    pub metadata: ProjectMetadata,
    pub trash: Trash,
    /// The raw XML content of the scrivx file, used to preserve structure on save.
    raw_xml: String,
}

impl Project {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Project> {
        let path = path.as_ref().to_path_buf();

        if !path.exists() || !path.is_dir() {
            return Err(ScrivenerError::ProjectNotFound { path });
        }

        let scrivx_path = find_scrivx_file(&path)?;
        let xml_content = std::fs::read_to_string(&scrivx_path)?;
        let (binder, metadata, trash) = parse_scrivx_str(&xml_content)?;

        Ok(Project {
            path,
            binder,
            metadata,
            trash,
            raw_xml: xml_content,
        })
    }

    pub fn save(&self) -> Result<()> {
        let scrivx_path = find_scrivx_file(&self.path)?;

        // If we have the original raw XML, preserve the full structure and only
        // replace the <Binder> section. Otherwise fall back to full serialization.
        let xml = if !self.raw_xml.is_empty() {
            serialize_scrivx_preserving(&self.raw_xml, &self.binder, &self.trash)?
        } else {
            serialize_scrivx(&self.binder, &self.metadata, &self.trash)?
        };

        let temp_path = scrivx_path.with_extension("scrivx.tmp");
        std::fs::write(&temp_path, &xml)?;
        std::fs::rename(&temp_path, &scrivx_path)?;

        Ok(())
    }

    pub fn save_as<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        let dest = dest.as_ref();
        copy_dir_recursive(&self.path, dest)?;
        let mut project = self.clone();
        project.path = dest.to_path_buf();
        project.save()
    }

    // -- Search --

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();

        for (item, _path) in self.binder.flatten() {
            if let BinderItem::Document(doc) = item {
                let content = match doc.read_content(&self.path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if let Some(text) = &content.plain_text {
                    let lower_text = text.to_lowercase();
                    let matches: Vec<Match> = lower_text
                        .match_indices(&lower_query)
                        .map(|(pos, _)| {
                            // Use char-boundary-safe slicing for multi-byte text
                            let start = floor_char_boundary(text, pos.saturating_sub(40));
                            let end = ceil_char_boundary(text, (pos + query.len() + 40).min(text.len()));
                            Match {
                                context: text[start..end].to_string(),
                                position: (pos, pos + query.len()),
                            }
                        })
                        .collect();

                    if !matches.is_empty() {
                        results.push(SearchResult {
                            document_uuid: doc.uuid,
                            document_title: doc.title.clone(),
                            matches,
                        });
                    }
                }
            }
        }

        results
    }

    pub fn search_regex(&self, pattern: &str) -> Result<Vec<SearchResult>> {
        let re = regex::Regex::new(pattern)?;
        let mut results = Vec::new();

        for (item, _path) in self.binder.flatten() {
            if let BinderItem::Document(doc) = item {
                let content = match doc.read_content(&self.path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if let Some(text) = &content.plain_text {
                    let matches: Vec<Match> = re
                        .find_iter(text)
                        .map(|m| {
                            let start = floor_char_boundary(text, m.start().saturating_sub(40));
                            let end = ceil_char_boundary(text, (m.end() + 40).min(text.len()));
                            Match {
                                context: text[start..end].to_string(),
                                position: (m.start(), m.end()),
                            }
                        })
                        .collect();

                    if !matches.is_empty() {
                        results.push(SearchResult {
                            document_uuid: doc.uuid,
                            document_title: doc.title.clone(),
                            matches,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    pub fn search_by_keyword(&self, keyword: &str) -> Vec<&Document> {
        let lower_kw = keyword.to_lowercase();
        let mut results = Vec::new();

        for (item, _) in self.binder.flatten() {
            if let BinderItem::Document(doc) = item {
                if doc.keywords.iter().any(|k| k.to_lowercase() == lower_kw) {
                    results.push(doc);
                }
            }
        }

        results
    }

    // -- Trash --

    pub fn list_trash(&self) -> &Trash {
        &self.trash
    }

    pub fn recover_from_trash(&mut self, uuid: Uuid) -> Result<()> {
        let index = self
            .trash
            .items
            .iter()
            .position(|item| item.uuid() == uuid)
            .ok_or(ScrivenerError::DocumentNotFound { uuid })?;

        let trashed_item = self.trash.items.remove(index);
        let binder_item = match trashed_item {
            TrashedItem::Document(doc) => BinderItem::Document(doc),
            TrashedItem::Folder(folder) => BinderItem::Folder(folder),
        };

        self.binder.root.push(binder_item);
        Ok(())
    }

    pub fn empty_trash(&mut self) -> Result<()> {
        for item in &self.trash.items {
            let uuid = item.uuid();
            let data_dir = self
                .path
                .join("Files")
                .join("Data")
                .join(uuid.to_string().to_uppercase());

            if data_dir.exists() {
                std::fs::remove_dir_all(&data_dir)?;
            }
        }

        self.trash.items.clear();
        Ok(())
    }

    // -- Statistics --

    pub fn statistics(&self) -> ProjectStatistics {
        let mut stats = ProjectStatistics {
            total_documents: 0,
            total_folders: 0,
            total_words: 0,
            total_characters: 0,
            words_by_document: HashMap::new(),
        };

        for (item, _) in self.binder.flatten() {
            match item {
                BinderItem::Document(doc) => {
                    stats.total_documents += 1;
                    if let Ok(content) = doc.read_content(&self.path) {
                        if let Some(formatted) = &content.formatted {
                            stats.total_words += formatted.word_count;
                            stats.total_characters += formatted.character_count;
                            stats.words_by_document.insert(doc.uuid, formatted.word_count);
                        }
                    }
                }
                BinderItem::Folder(_) => {
                    stats.total_folders += 1;
                }
            }
        }

        stats
    }
}

fn find_scrivx_file(project_dir: &Path) -> Result<PathBuf> {
    let mut scrivx_files: Vec<PathBuf> = std::fs::read_dir(project_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "scrivx"))
        .collect();

    match scrivx_files.len() {
        0 => Err(ScrivenerError::InvalidProject {
            message: "No .scrivx file found in project directory".into(),
        }),
        1 => Ok(scrivx_files.remove(0)),
        _ => Err(ScrivenerError::InvalidProject {
            message: "Multiple .scrivx files found".into(),
        }),
    }
}

/// Find the largest byte index <= `idx` that is a char boundary.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Find the smallest byte index >= `idx` that is a char boundary.
fn ceil_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

pub(crate) fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
