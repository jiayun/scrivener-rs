use scrivener::{BinderItem, Project};

#[test]
fn open_sample_project() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    assert_eq!(project.metadata.title, "Sample Novel");
    assert_eq!(project.metadata.author.as_deref(), Some("Test Author"));
}

#[test]
fn open_nonexistent_project_error() {
    let result = Project::open("tests/fixtures/nonexistent.scriv");
    assert!(result.is_err());
}

#[test]
fn binder_structure_matches_fixture() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();

    // Should have Draft and Research at root (Trash is separate)
    assert_eq!(project.binder.root.len(), 2);

    // Draft should have 2 children
    if let BinderItem::Folder(draft) = &project.binder.root[0] {
        assert_eq!(draft.title, "Draft");
        assert_eq!(draft.children.len(), 2);
    } else {
        panic!("Expected Draft folder at root[0]");
    }

    // Trash should have 1 item
    assert_eq!(project.trash.items.len(), 1);
}
