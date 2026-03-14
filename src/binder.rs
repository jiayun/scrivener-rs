use uuid::Uuid;

use crate::document::{Document, Folder};
use crate::error::{Result, ScrivenerError};

/// The binder tree — Scrivener's hierarchical document structure.
#[derive(Debug, Clone)]
pub struct Binder {
    pub root: Vec<BinderItem>,
}

/// A single item in the binder tree.
#[derive(Debug, Clone)]
pub enum BinderItem {
    Document(Document),
    Folder(Folder),
}

impl BinderItem {
    pub fn uuid(&self) -> Uuid {
        match self {
            BinderItem::Document(doc) => doc.uuid,
            BinderItem::Folder(folder) => folder.uuid,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            BinderItem::Document(doc) => &doc.title,
            BinderItem::Folder(folder) => &folder.title,
        }
    }
}

impl Binder {
    pub fn find_by_uuid(&self, uuid: Uuid) -> Option<&BinderItem> {
        for item in &self.root {
            if let Some(found) = find_by_uuid_recursive(item, uuid) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_by_uuid_mut(&mut self, uuid: Uuid) -> Option<&mut BinderItem> {
        for item in &mut self.root {
            if let Some(found) = find_by_uuid_recursive_mut(item, uuid) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_by_title(&self, title: &str) -> Vec<&BinderItem> {
        let mut results = Vec::new();
        let lower_title = title.to_lowercase();
        for item in &self.root {
            find_by_title_recursive(item, &lower_title, &mut results);
        }
        results
    }

    pub fn flatten(&self) -> Vec<(&BinderItem, Vec<String>)> {
        let mut results = Vec::new();
        for item in &self.root {
            flatten_recursive(item, &[], &mut results);
        }
        results
    }

    pub fn move_item(&mut self, uuid: Uuid, new_parent: Option<Uuid>) -> Result<()> {
        if self.find_by_uuid(uuid).is_none() {
            return Err(ScrivenerError::DocumentNotFound { uuid });
        }

        if let Some(parent_uuid) = new_parent {
            match self.find_by_uuid(parent_uuid) {
                Some(BinderItem::Folder(_)) => {}
                Some(_) => {
                    return Err(ScrivenerError::InvalidProject {
                        message: "Cannot move item into a document".into(),
                    })
                }
                None => return Err(ScrivenerError::DocumentNotFound { uuid: parent_uuid }),
            }
            if is_descendant(&self.root, uuid, parent_uuid) {
                return Err(ScrivenerError::InvalidProject {
                    message: "Cannot move folder into its own descendant".into(),
                });
            }
        }

        let item = remove_item_recursive(&mut self.root, uuid)
            .expect("item existence already validated");

        match new_parent {
            None => self.root.push(item),
            Some(parent_uuid) => {
                insert_into_folder(&mut self.root, parent_uuid, item);
            }
        }

        Ok(())
    }
}

fn find_by_uuid_recursive(item: &BinderItem, target: Uuid) -> Option<&BinderItem> {
    if item.uuid() == target {
        return Some(item);
    }
    if let BinderItem::Folder(folder) = item {
        for child in &folder.children {
            if let Some(found) = find_by_uuid_recursive(child, target) {
                return Some(found);
            }
        }
    }
    None
}

fn find_by_uuid_recursive_mut(item: &mut BinderItem, target: Uuid) -> Option<&mut BinderItem> {
    if item.uuid() == target {
        return Some(item);
    }
    if let BinderItem::Folder(folder) = item {
        for child in &mut folder.children {
            if let Some(found) = find_by_uuid_recursive_mut(child, target) {
                return Some(found);
            }
        }
    }
    None
}

fn find_by_title_recursive<'a>(
    item: &'a BinderItem,
    target: &str,
    results: &mut Vec<&'a BinderItem>,
) {
    if item.title().to_lowercase().contains(target) {
        results.push(item);
    }
    if let BinderItem::Folder(folder) = item {
        for child in &folder.children {
            find_by_title_recursive(child, target, results);
        }
    }
}

fn flatten_recursive<'a>(
    item: &'a BinderItem,
    parent_path: &[String],
    results: &mut Vec<(&'a BinderItem, Vec<String>)>,
) {
    let mut path = parent_path.to_vec();
    path.push(item.title().to_string());
    results.push((item, path.clone()));

    if let BinderItem::Folder(folder) = item {
        for child in &folder.children {
            flatten_recursive(child, &path, results);
        }
    }
}

fn remove_item_recursive(items: &mut Vec<BinderItem>, uuid: Uuid) -> Option<BinderItem> {
    if let Some(pos) = items.iter().position(|i| i.uuid() == uuid) {
        return Some(items.remove(pos));
    }
    for item in items.iter_mut() {
        if let BinderItem::Folder(folder) = item {
            if let Some(removed) = remove_item_recursive(&mut folder.children, uuid) {
                return Some(removed);
            }
        }
    }
    None
}

fn find_folder_mut(items: &mut [BinderItem], target: Uuid) -> Option<&mut Folder> {
    for item in items.iter_mut() {
        if let BinderItem::Folder(folder) = item {
            if folder.uuid == target {
                return Some(folder);
            }
            if let Some(found) = find_folder_mut(&mut folder.children, target) {
                return Some(found);
            }
        }
    }
    None
}

fn insert_into_folder(items: &mut [BinderItem], parent_uuid: Uuid, new_item: BinderItem) {
    if let Some(folder) = find_folder_mut(items, parent_uuid) {
        folder.children.push(new_item);
    }
}

fn is_descendant(items: &[BinderItem], ancestor_uuid: Uuid, target_uuid: Uuid) -> bool {
    for item in items {
        if item.uuid() == ancestor_uuid {
            if let BinderItem::Folder(folder) = item {
                return contains_uuid(&folder.children, target_uuid);
            }
            return false;
        }
        if let BinderItem::Folder(folder) = item {
            if is_descendant(&folder.children, ancestor_uuid, target_uuid) {
                return true;
            }
        }
    }
    false
}

fn contains_uuid(items: &[BinderItem], target: Uuid) -> bool {
    for item in items {
        if item.uuid() == target {
            return true;
        }
        if let BinderItem::Folder(folder) = item {
            if contains_uuid(&folder.children, target) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Folder};

    fn sample_binder() -> Binder {
        Binder {
            root: vec![BinderItem::Folder(Folder {
                uuid: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
                title: "Draft".into(),
                children: vec![
                    BinderItem::Document(Document {
                        uuid: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                        title: "Chapter One".into(),
                        ..Default::default()
                    }),
                    BinderItem::Document(Document {
                        uuid: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                        title: "Chapter Two".into(),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            })],
        }
    }

    #[test]
    fn find_by_uuid_root_level() {
        let binder = sample_binder();
        let uuid = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        assert!(binder.find_by_uuid(uuid).is_some());
    }

    #[test]
    fn find_by_uuid_nested() {
        let binder = sample_binder();
        let uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let found = binder.find_by_uuid(uuid);
        assert!(found.is_some());
        if let Some(BinderItem::Document(doc)) = found {
            assert_eq!(doc.title, "Chapter One");
        }
    }

    #[test]
    fn find_by_uuid_not_found() {
        let binder = sample_binder();
        let uuid = Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap();
        assert!(binder.find_by_uuid(uuid).is_none());
    }

    #[test]
    fn find_by_title_case_insensitive() {
        let binder = sample_binder();
        let results = binder.find_by_title("chapter");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn flatten_returns_all_items_with_paths() {
        let binder = sample_binder();
        let flat = binder.flatten();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[1].1, vec!["Draft", "Chapter One"]);
    }

    #[test]
    fn move_item_to_root() {
        let mut binder = sample_binder();
        let uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        binder.move_item(uuid, None).unwrap();
        assert_eq!(binder.root.len(), 2);
    }

    #[test]
    fn move_item_into_second_folder() {
        let folder1_uuid = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let folder2_uuid = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let doc_uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

        let mut binder = Binder {
            root: vec![
                BinderItem::Folder(Folder {
                    uuid: folder1_uuid,
                    title: "Folder 1".into(),
                    children: vec![BinderItem::Document(Document {
                        uuid: doc_uuid,
                        title: "Doc".into(),
                        ..Default::default()
                    })],
                    ..Default::default()
                }),
                BinderItem::Folder(Folder {
                    uuid: folder2_uuid,
                    title: "Folder 2".into(),
                    children: vec![],
                    ..Default::default()
                }),
            ],
        };

        binder.move_item(doc_uuid, Some(folder2_uuid)).unwrap();

        // Doc should be in Folder 2, not Folder 1
        if let BinderItem::Folder(f1) = &binder.root[0] {
            assert!(f1.children.is_empty(), "Folder 1 should be empty");
        }
        if let BinderItem::Folder(f2) = &binder.root[1] {
            assert_eq!(f2.children.len(), 1, "Folder 2 should have the doc");
            assert_eq!(f2.children[0].uuid(), doc_uuid);
        }
    }

    #[test]
    fn move_item_not_found_error() {
        let mut binder = sample_binder();
        let uuid = Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap();
        assert!(binder.move_item(uuid, None).is_err());
    }
}
