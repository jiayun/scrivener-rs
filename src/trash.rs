use uuid::Uuid;

use crate::document::{Document, Folder};

/// Contents of the Scrivener Trash folder.
#[derive(Debug, Clone, Default)]
pub struct Trash {
    /// The UUID of the TrashFolder BinderItem in the original scrivx.
    pub uuid: Option<Uuid>,
    pub items: Vec<TrashedItem>,
}

/// An item in the trash — mirrors BinderItem but tracks origin.
#[derive(Debug, Clone)]
pub enum TrashedItem {
    Document(Document),
    Folder(Folder),
}

impl TrashedItem {
    pub fn uuid(&self) -> uuid::Uuid {
        match self {
            TrashedItem::Document(doc) => doc.uuid,
            TrashedItem::Folder(folder) => folder.uuid,
        }
    }
}
