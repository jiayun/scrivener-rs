use scrivener::{BinderItem, Project};

#[test]
fn read_document_content() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let uuid = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

    if let Some(BinderItem::Document(doc)) = project.binder.find_by_uuid(uuid) {
        let content = doc.read_content(&project.path).unwrap();
        let text = content.plain_text.unwrap();
        assert!(text.contains("dark and stormy night"));
        assert!(text.contains("protagonist"));
    } else {
        panic!("Document not found");
    }
}

#[test]
fn read_missing_content_returns_empty() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let uuid = uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();

    if let Some(BinderItem::Document(doc)) = project.binder.find_by_uuid(uuid) {
        let content = doc.read_content(&project.path).unwrap();
        assert_eq!(content.plain_text.as_deref(), Some(""));
    } else {
        panic!("Document not found");
    }
}

#[test]
fn write_and_read_content() {
    let temp = tempfile::TempDir::new().unwrap();
    let project_dir = temp.path().join("test.scriv");
    std::fs::create_dir_all(&project_dir).unwrap();

    let uuid = uuid::Uuid::new_v4();
    let mut doc = scrivener::Document {
        uuid,
        title: "Test".into(),
        ..Default::default()
    };

    doc.write_content(&project_dir, "Hello, world!").unwrap();

    let content = doc.read_content(&project_dir).unwrap();
    assert!(content.plain_text.unwrap().contains("Hello, world!"));
}

#[test]
fn write_synopsis() {
    let temp = tempfile::TempDir::new().unwrap();
    let project_dir = temp.path().join("test.scriv");

    let uuid = uuid::Uuid::new_v4();
    let mut doc = scrivener::Document {
        uuid,
        title: "Test".into(),
        ..Default::default()
    };

    doc.update_synopsis(&project_dir, "A brief summary.")
        .unwrap();
    assert_eq!(doc.synopsis.as_deref(), Some("A brief summary."));

    let synopsis_path = project_dir
        .join("Files")
        .join("Data")
        .join(uuid.to_string().to_uppercase())
        .join("synopsis.txt");
    assert!(synopsis_path.exists());
    assert_eq!(
        std::fs::read_to_string(&synopsis_path).unwrap(),
        "A brief summary."
    );
}

#[test]
fn folder_content_synopsis_and_notes_roundtrip() {
    let temp = tempfile::TempDir::new().unwrap();
    let project_dir = temp.path().join("test.scriv");
    let mut folder = scrivener::Folder {
        title: "Content Folder".into(),
        ..Default::default()
    };

    folder
        .write_content(&project_dir, "Folder body text.")
        .unwrap();
    folder
        .update_synopsis(&project_dir, "Folder synopsis.")
        .unwrap();
    folder.update_notes(&project_dir, "Folder notes.").unwrap();

    assert_eq!(
        folder
            .read_content(&project_dir)
            .unwrap()
            .plain_text
            .as_deref(),
        Some("Folder body text.\n")
    );
    assert_eq!(
        folder.read_synopsis(&project_dir).unwrap().as_deref(),
        Some("Folder synopsis.")
    );
    assert_eq!(
        folder.read_notes(&project_dir).unwrap().as_deref(),
        Some("Folder notes.\n")
    );
}
