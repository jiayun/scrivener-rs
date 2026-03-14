use scrivener::{BinderItem, Project};

fn copy_fixture_to_temp(temp: &tempfile::TempDir) {
    let src = std::path::Path::new("tests/fixtures/sample.scriv");
    let dest = temp.path().join("sample.scriv");
    copy_dir_recursive(src, &dest).unwrap();
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[test]
fn roundtrip_open_save_open() {
    let temp = tempfile::TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    let project_path = temp.path().join("sample.scriv");

    // Open and modify
    let mut project = Project::open(&project_path).unwrap();
    let uuid = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    if let Some(BinderItem::Document(doc)) = project.binder.find_by_uuid_mut(uuid) {
        doc.add_keyword("new-keyword");
    }

    // Save
    project.save().unwrap();

    // Re-open and verify
    let project2 = Project::open(&project_path).unwrap();
    if let Some(BinderItem::Document(doc)) = project2.binder.find_by_uuid(uuid) {
        assert!(doc.keywords.contains(&"new-keyword".to_string()));
    } else {
        panic!("Document not found after round-trip");
    }
}

#[test]
fn roundtrip_preserves_structure() {
    let temp = tempfile::TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    let project_path = temp.path().join("sample.scriv");

    let project1 = Project::open(&project_path).unwrap();
    let flat1 = project1.binder.flatten();

    project1.save().unwrap();

    let project2 = Project::open(&project_path).unwrap();
    let flat2 = project2.binder.flatten();

    assert_eq!(flat1.len(), flat2.len());

    for (a, b) in flat1.iter().zip(flat2.iter()) {
        assert_eq!(a.1, b.1);
    }
}
