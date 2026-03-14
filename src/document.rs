use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::Result;
use crate::metadata::DocumentMetadata;

/// A text document in the binder.
#[derive(Debug, Clone)]
pub struct Document {
    pub uuid: Uuid,
    pub title: String,
    pub synopsis: Option<String>,
    pub notes: Option<String>,
    pub keywords: Vec<String>,
    pub content: DocumentContent,
    pub metadata: DocumentMetadata,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            title: String::new(),
            synopsis: None,
            notes: None,
            keywords: Vec::new(),
            content: DocumentContent::new(),
            metadata: DocumentMetadata::default(),
        }
    }
}

/// A folder in the binder that can contain child items.
#[derive(Debug, Clone)]
pub struct Folder {
    pub uuid: Uuid,
    pub title: String,
    pub children: Vec<crate::binder::BinderItem>,
    pub metadata: DocumentMetadata,
}

impl Default for Folder {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            title: String::new(),
            children: Vec::new(),
            metadata: DocumentMetadata::default(),
        }
    }
}

/// Content associated with a document.
#[derive(Debug, Clone)]
pub struct DocumentContent {
    pub rtf_path: PathBuf,
    pub plain_text: Option<String>,
    pub formatted: Option<FormattedContent>,
}

impl DocumentContent {
    pub fn new() -> Self {
        Self {
            rtf_path: PathBuf::new(),
            plain_text: None,
            formatted: None,
        }
    }
}

impl Default for DocumentContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Formatted content extracted from RTF.
#[derive(Debug, Clone)]
pub struct FormattedContent {
    pub text: String,
    pub word_count: usize,
    pub character_count: usize,
}

// -- RTF helpers --

pub(crate) fn extract_plain_text(doc: &scrivener_rtf::Document) -> String {
    let mut text = String::new();
    for group in &doc.groups {
        extract_text_from_group(group, &mut text);
    }
    text
}

fn extract_text_from_group(group: &scrivener_rtf::Group, text: &mut String) {
    if group.is_destination {
        return;
    }
    for content in &group.content {
        match content {
            scrivener_rtf::Content::Text(s) => text.push_str(s),
            scrivener_rtf::Content::ControlWord(name, _) if name == "par" => {
                text.push('\n');
            }
            scrivener_rtf::Content::Group(sub) => {
                extract_text_from_group(sub, text);
            }
            _ => {}
        }
    }
}

pub(crate) fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn generate_rtf_from_text(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}");
    let body = escaped.replace('\n', "\\par\n");
    format!(
        "{{\\rtf1\\ansi\\deff0{{\\fonttbl{{\\f0\\fnil Helvetica;}}}}\n\\pard\\f0\\fs24 {}\\par}}",
        body
    )
}

// -- Document methods --

impl Document {
    pub fn read_content(&self, project_path: &Path) -> Result<DocumentContent> {
        let rtf_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string())
            .join("content.rtf");

        if !rtf_path.exists() {
            return Ok(DocumentContent {
                rtf_path,
                plain_text: Some(String::new()),
                formatted: None,
            });
        }

        let rtf_doc = scrivener_rtf::parse_file(&rtf_path)
            .map_err(|e| crate::error::ScrivenerError::ContentError {
                message: format!("Failed to parse RTF for {}: {}", self.uuid, e),
            })?;

        let plain_text = extract_plain_text(&rtf_doc);
        let word_count = count_words(&plain_text);
        let char_count = plain_text.chars().count();

        Ok(DocumentContent {
            rtf_path,
            plain_text: Some(plain_text.clone()),
            formatted: Some(FormattedContent {
                text: plain_text,
                word_count,
                character_count: char_count,
            }),
        })
    }

    pub fn write_content(&mut self, project_path: &Path, content: &str) -> Result<()> {
        let dir_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string());

        std::fs::create_dir_all(&dir_path)?;

        let rtf_path = dir_path.join("content.rtf");
        let rtf = generate_rtf_from_text(content);
        std::fs::write(&rtf_path, rtf)?;

        self.content.plain_text = Some(content.to_string());
        Ok(())
    }

    pub fn update_synopsis(&mut self, project_path: &Path, synopsis: &str) -> Result<()> {
        let dir_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string());

        std::fs::create_dir_all(&dir_path)?;
        std::fs::write(dir_path.join("synopsis.txt"), synopsis)?;
        self.synopsis = Some(synopsis.to_string());
        Ok(())
    }

    pub fn update_notes(&mut self, project_path: &Path, notes: &str) -> Result<()> {
        let dir_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string());

        std::fs::create_dir_all(&dir_path)?;
        let rtf = generate_rtf_from_text(notes);
        std::fs::write(dir_path.join("notes.rtf"), rtf)?;
        self.notes = Some(notes.to_string());
        Ok(())
    }

    pub fn add_keyword(&mut self, keyword: &str) {
        let kw = keyword.to_string();
        if !self.keywords.contains(&kw) {
            self.keywords.push(kw);
        }
    }

    pub fn remove_keyword(&mut self, keyword: &str) {
        self.keywords.retain(|k| k != keyword);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_add_no_duplicates() {
        let mut doc = Document::default();
        doc.add_keyword("test");
        doc.add_keyword("test");
        assert_eq!(doc.keywords.len(), 1);
    }

    #[test]
    fn keyword_remove() {
        let mut doc = Document::default();
        doc.add_keyword("alpha");
        doc.add_keyword("beta");
        doc.remove_keyword("alpha");
        assert_eq!(doc.keywords, vec!["beta"]);
    }
}
