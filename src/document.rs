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

/// The type of folder in the Scrivener binder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderType {
    /// Normal user-created folder.
    Folder,
    /// The top-level Draft (Manuscript) folder.
    DraftFolder,
    /// The top-level Research folder.
    ResearchFolder,
    /// The Trash folder (handled separately, but tracked for serialization).
    TrashFolder,
}

impl FolderType {
    /// Returns the XML `Type` attribute value for this folder type.
    pub fn as_xml_type(&self) -> &'static str {
        match self {
            FolderType::Folder => "Folder",
            FolderType::DraftFolder => "DraftFolder",
            FolderType::ResearchFolder => "ResearchFolder",
            FolderType::TrashFolder => "TrashFolder",
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
    /// The original folder type from the scrivx XML.
    pub folder_type: FolderType,
}

impl Default for Folder {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            title: String::new(),
            children: Vec::new(),
            metadata: DocumentMetadata::default(),
            folder_type: FolderType::Folder,
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

/// Header control words whose groups should be skipped during text extraction.
const HEADER_DESTINATIONS: &[&str] = &[
    "fonttbl",
    "colortbl",
    "stylesheet",
    "listtable",
    "listoverridetable",
    "info",
    "generator",
    "expandedcolortbl",
];

/// Check if a group is a known header/destination group that should be skipped.
fn is_header_group(group: &scrivener_rtf::Group) -> bool {
    if group.is_destination {
        return true;
    }
    // Check if the first content item is a header control word
    matches!(
        group.content.first(),
        Some(scrivener_rtf::Content::ControlWord(name, _))
            if HEADER_DESTINATIONS.iter().any(|h| h == name)
    )
}

fn extract_text_from_group(group: &scrivener_rtf::Group, text: &mut String) {
    if is_header_group(group) {
        return;
    }
    let mut skip_next_text = false;
    let mut pending_high_surrogate: Option<u16> = None;
    for content in &group.content {
        match content {
            scrivener_rtf::Content::Text(s) => {
                if skip_next_text {
                    skip_next_text = false;
                    // Skip the first char (ANSI fallback '?' after \uN)
                    if s.len() > 1 {
                        text.push_str(&s[1..]);
                    }
                } else {
                    text.push_str(s);
                }
            }
            scrivener_rtf::Content::ControlWord(name, Some(code)) if name == "u" => {
                let code = *code;
                let unsigned = if code < 0 { (code + 0x10000) as u16 } else { code as u16 };

                if (0xD800..=0xDBFF).contains(&unsigned) {
                    // High surrogate — store and wait for low surrogate
                    pending_high_surrogate = Some(unsigned);
                } else if (0xDC00..=0xDFFF).contains(&unsigned) {
                    // Low surrogate — combine with pending high surrogate
                    if let Some(hi) = pending_high_surrogate.take() {
                        let code_point =
                            ((hi as u32 - 0xD800) << 10) + (unsigned as u32 - 0xDC00) + 0x10000;
                        if let Some(c) = char::from_u32(code_point) {
                            text.push(c);
                        }
                    }
                } else {
                    pending_high_surrogate = None;
                    if let Some(c) = char::from_u32(unsigned as u32) {
                        text.push(c);
                    }
                }
                skip_next_text = true;
            }
            scrivener_rtf::Content::ControlWord(name, _) if name == "par" => {
                text.push('\n');
                skip_next_text = false;
            }
            scrivener_rtf::Content::Group(sub) => {
                extract_text_from_group(sub, text);
                skip_next_text = false;
            }
            _ => {
                skip_next_text = false;
            }
        }
    }
}

pub(crate) fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn generate_rtf_from_text(text: &str) -> String {
    let mut body = String::new();
    for ch in text.chars() {
        match ch {
            '\\' => body.push_str("\\\\"),
            '{' => body.push_str("\\{"),
            '}' => body.push_str("\\}"),
            '\n' => body.push_str("\\par\n"),
            c if c as u32 > 127 => {
                let code_point = c as u32;
                if code_point > 0xFFFF {
                    // Supplementary plane → UTF-16 surrogate pair
                    let adjusted = code_point - 0x10000;
                    let high = (0xD800 + (adjusted >> 10)) as i16;
                    let low = (0xDC00 + (adjusted & 0x3FF)) as i16;
                    body.push_str(&format!("\\u{}?\\u{}?", high, low));
                } else {
                    // BMP: i16 wrapping handles values > 0x7FFF per RTF spec
                    body.push_str(&format!("\\u{}?", code_point as i16));
                }
            }
            c => body.push(c),
        }
    }
    format!(
        "{{\\rtf1\\ansi\\ansicpg1252\\deff0{{\\fonttbl{{\\f0\\fnil Helvetica;}}}}\n\\pard\\f0\\fs24 {}\\par}}",
        body
    )
}

/// Returns the uppercase UUID string for use in file paths (matches Scrivener convention).
fn data_dir_name(uuid: &Uuid) -> String {
    uuid.to_string().to_uppercase()
}

// -- Document methods --

impl Document {
    pub fn read_content(&self, project_path: &Path) -> Result<DocumentContent> {
        let rtf_path = project_path
            .join("Files")
            .join("Data")
            .join(data_dir_name(&self.uuid))
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
            .join(data_dir_name(&self.uuid));

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
            .join(data_dir_name(&self.uuid));

        std::fs::create_dir_all(&dir_path)?;
        std::fs::write(dir_path.join("synopsis.txt"), synopsis)?;
        self.synopsis = Some(synopsis.to_string());
        Ok(())
    }

    pub fn update_notes(&mut self, project_path: &Path, notes: &str) -> Result<()> {
        let dir_path = project_path
            .join("Files")
            .join("Data")
            .join(data_dir_name(&self.uuid));

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
