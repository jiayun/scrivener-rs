pub mod binder;
pub mod document;
pub mod error;
pub mod metadata;
pub mod search;
pub mod statistics;
pub mod trash;

mod scrivx;

pub use binder::{Binder, BinderItem};
pub use document::{Document, DocumentContent, Folder, FormattedContent};
pub use error::{Result, ScrivenerError};
pub use metadata::{DocumentMetadata, ProjectMetadata};
pub use search::{Match, SearchResult};
pub use statistics::ProjectStatistics;
pub use trash::{Trash, TrashedItem};

mod project;
pub use project::Project;
