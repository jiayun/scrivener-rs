use std::collections::HashMap;
use uuid::Uuid;

/// Aggregate statistics for the entire project.
#[derive(Debug, Clone)]
pub struct ProjectStatistics {
    pub total_documents: usize,
    pub total_folders: usize,
    pub total_words: usize,
    pub total_characters: usize,
    pub words_by_document: HashMap<Uuid, usize>,
}
