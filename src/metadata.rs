use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Project-level metadata from the .scrivx ProjectProperties section.
#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    pub title: String,
    pub author: Option<String>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: None,
            created: Utc::now(),
            modified: Utc::now(),
        }
    }
}

/// Document-level metadata from the .scrivx MetaData section.
#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub include_in_compile: bool,
    pub custom_metadata: HashMap<String, String>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            created: Utc::now(),
            modified: Utc::now(),
            include_in_compile: true,
            custom_metadata: HashMap::new(),
        }
    }
}
