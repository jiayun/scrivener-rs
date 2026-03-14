use scrivener::Project;

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
