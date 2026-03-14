use uuid::Uuid;

/// A search result from a single document.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub document_uuid: Uuid,
    pub document_title: String,
    pub matches: Vec<Match>,
}

/// A single match within a document's content.
#[derive(Debug, Clone)]
pub struct Match {
    pub context: String,
    pub position: (usize, usize),
}
