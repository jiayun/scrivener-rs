use scrivener::Project;

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
fn list_trash_items() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let trash = project.list_trash();
    assert_eq!(trash.items.len(), 1);
}

#[test]
fn recover_from_trash() {
    let temp = tempfile::TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    let mut project = Project::open(temp.path().join("sample.scriv")).unwrap();
    let initial_root_count = project.binder.root.len();
    let trash_uuid = uuid::Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();

    project.recover_from_trash(trash_uuid).unwrap();

    assert_eq!(project.binder.root.len(), initial_root_count + 1);
    assert!(project.trash.items.is_empty());
}

#[test]
fn empty_trash_removes_files() {
    let temp = tempfile::TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    // Create the data directory for the trashed item
    let trash_data_dir = temp
        .path()
        .join("sample.scriv")
        .join("Files")
        .join("Data")
        .join("44444444-4444-4444-4444-444444444444");
    std::fs::create_dir_all(&trash_data_dir).unwrap();
    std::fs::write(trash_data_dir.join("content.rtf"), "test").unwrap();

    let mut project = Project::open(temp.path().join("sample.scriv")).unwrap();
    project.empty_trash().unwrap();

    assert!(project.trash.items.is_empty());
    assert!(!trash_data_dir.exists());
}
