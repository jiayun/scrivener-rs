use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ScrivenerError {
    #[error("Project not found: {path}")]
    ProjectNotFound { path: PathBuf },

    #[error("Invalid project: {message}")]
    InvalidProject { message: String },

    #[error("Failed to parse .scrivx: {message}")]
    ScrivxParseError { message: String },

    #[error("Document not found: UUID {uuid}")]
    DocumentNotFound { uuid: Uuid },

    #[error("Content error: {message}")]
    ContentError { message: String },

    #[error("Invalid regex pattern: {0}")]
    RegexError(#[from] regex::Error),

    #[error("RTF error: {0}")]
    RtfError(#[from] scrivener_rtf::RtfError),

    #[error("XML error: {0}")]
    XmlError(#[from] quick_xml::DeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ScrivenerError>;
