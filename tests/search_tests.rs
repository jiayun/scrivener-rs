use scrivener::{BinderItem, Project};

#[test]
fn search_finds_matching_documents() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let results = project.search("protagonist");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document_title, "Chapter One");
    assert!(!results[0].matches.is_empty());
}

#[test]
fn search_case_insensitive() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let results = project.search("STORMY");
    assert_eq!(results.len(), 1);
}

#[test]
fn search_no_results() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let results = project.search("xyzzy_nonexistent");
    assert!(results.is_empty());
}

#[test]
fn search_by_keyword() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let docs = project.search_by_keyword("protagonist");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].title, "Chapter One");
}

#[test]
fn search_and_statistics_include_folder_content() {
    let temp = tempfile::TempDir::new().unwrap();
    let project_path = temp.path().join("sample.scriv");
    Project::open("tests/fixtures/sample.scriv")
        .unwrap()
        .save_as(&project_path)
        .unwrap();

    let mut project = Project::open(&project_path).unwrap();
    let folder_uuid = project.binder.root[0].uuid();
    if let Some(BinderItem::Folder(folder)) = project.binder.find_by_uuid_mut(folder_uuid) {
        folder
            .write_content(&project_path, "Unique folder body phrase.")
            .unwrap();
    } else {
        panic!("Expected Draft folder");
    }

    let results = project.search("folder body phrase");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document_title, "Draft");

    let stats = project.statistics();
    assert_eq!(
        stats.words_by_document.get(&folder_uuid),
        Some(&4),
        "folder body should have a per-item word count"
    );
}
