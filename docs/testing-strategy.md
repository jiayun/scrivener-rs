# Testing Strategy

## Test Organization

```
tests/
├── scrivx_tests.rs       # .scrivx XML parsing tests
├── project_tests.rs      # Project open/save integration tests
├── binder_tests.rs       # Binder traversal and mutation tests
├── document_tests.rs     # Document content read/write tests
├── search_tests.rs       # Search functionality tests
├── trash_tests.rs        # Trash operations tests
├── roundtrip_tests.rs    # End-to-end round-trip tests
└── fixtures/
    └── sample.scriv/
        ├── sample.scrivx
        ├── Files/
        │   └── Data/
        │       ├── 11111111-1111-1111-1111-111111111111/
        │       │   ├── content.rtf
        │       │   ├── notes.rtf
        │       │   └── synopsis.txt
        │       ├── 22222222-2222-2222-2222-222222222222/
        │       │   └── content.rtf
        │       └── 33333333-3333-3333-3333-333333333333/
        │           └── content.rtf
        └── Settings/
```

## Test Fixtures

### Sample .scrivx

A minimal but complete `.scrivx` for testing, with the following structure:

```
Binder:
├── Draft (DraftFolder)
│   ├── Chapter One (Text) — UUID 11111111-...
│   └── Chapter Two (Text) — UUID 22222222-...
├── Research (ResearchFolder)
│   └── Notes (Text) — UUID 33333333-...
└── Trash (TrashFolder)
    └── Deleted Scene (Text) — UUID 44444444-...
```

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject Identifier="sample" Version="2.0">
  <Binder>
    <BinderItem UUID="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"
                Type="DraftFolder"
                Created="2024-01-15T10:00:00Z"
                Modified="2024-01-15T10:00:00Z">
      <Title>Draft</Title>
      <MetaData>
        <IncludeInCompile>Yes</IncludeInCompile>
      </MetaData>
      <Children>
        <BinderItem UUID="11111111-1111-1111-1111-111111111111"
                    Type="Text"
                    Created="2024-01-15T10:30:00Z"
                    Modified="2024-06-20T14:22:00Z">
          <Title>Chapter One</Title>
          <MetaData>
            <IncludeInCompile>Yes</IncludeInCompile>
            <Keywords>
              <Keyword>protagonist</Keyword>
              <Keyword>opening</Keyword>
            </Keywords>
          </MetaData>
        </BinderItem>
        <BinderItem UUID="22222222-2222-2222-2222-222222222222"
                    Type="Text"
                    Created="2024-02-01T09:00:00Z"
                    Modified="2024-06-25T16:45:00Z">
          <Title>Chapter Two</Title>
          <MetaData>
            <IncludeInCompile>Yes</IncludeInCompile>
          </MetaData>
        </BinderItem>
      </Children>
    </BinderItem>
    <BinderItem UUID="BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"
                Type="ResearchFolder"
                Created="2024-01-15T10:00:00Z"
                Modified="2024-01-15T10:00:00Z">
      <Title>Research</Title>
      <Children>
        <BinderItem UUID="33333333-3333-3333-3333-333333333333"
                    Type="Text"
                    Created="2024-03-10T11:00:00Z"
                    Modified="2024-03-10T11:00:00Z">
          <Title>Character Notes</Title>
          <MetaData>
            <IncludeInCompile>No</IncludeInCompile>
          </MetaData>
        </BinderItem>
      </Children>
    </BinderItem>
    <BinderItem UUID="CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC"
                Type="TrashFolder"
                Created="2024-01-15T10:00:00Z"
                Modified="2024-05-01T08:00:00Z">
      <Title>Trash</Title>
      <Children>
        <BinderItem UUID="44444444-4444-4444-4444-444444444444"
                    Type="Text"
                    Created="2024-04-01T12:00:00Z"
                    Modified="2024-04-15T14:00:00Z">
          <Title>Deleted Scene</Title>
        </BinderItem>
      </Children>
    </BinderItem>
  </Binder>
  <ProjectProperties>
    <ProjectTitle>Sample Novel</ProjectTitle>
    <FullName>Test Author</FullName>
  </ProjectProperties>
</ScrivenerProject>
```

### Sample RTF Content Files

#### `Files/Data/11111111-.../content.rtf`
```rtf
{\rtf1\ansi\deff0{\fonttbl{\f0\fnil Helvetica;}}
\pard\f0\fs24 It was a dark and stormy night. The wind howled through the empty streets.\par
The protagonist stepped into the rain.\par}
```

#### `Files/Data/22222222-.../content.rtf`
```rtf
{\rtf1\ansi\deff0{\fonttbl{\f0\fnil Helvetica;}}
\pard\f0\fs24 Chapter two begins with a sunrise.\par}
```

## Unit Tests (in-module `#[cfg(test)]`)

### .scrivx Parsing Tests (`scrivx.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_scrivx() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <ScrivenerProject Identifier="test" Version="2.0">
              <Binder>
                <BinderItem UUID="11111111-1111-1111-1111-111111111111" Type="Text">
                  <Title>Test Doc</Title>
                </BinderItem>
              </Binder>
            </ScrivenerProject>"#;

        let raw: RawScrivenerProject = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(raw.binder.items.len(), 1);
        assert_eq!(raw.binder.items[0].item_type, "Text");
        assert_eq!(raw.binder.items[0].title.as_deref(), Some("Test Doc"));
    }

    #[test]
    fn deserialize_nested_binder() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <ScrivenerProject Identifier="test" Version="2.0">
              <Binder>
                <BinderItem UUID="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA" Type="DraftFolder">
                  <Title>Draft</Title>
                  <Children>
                    <BinderItem UUID="11111111-1111-1111-1111-111111111111" Type="Text">
                      <Title>Chapter One</Title>
                    </BinderItem>
                  </Children>
                </BinderItem>
              </Binder>
            </ScrivenerProject>"#;

        let raw: RawScrivenerProject = quick_xml::de::from_str(xml).unwrap();
        let children = raw.binder.items[0].children.as_ref().unwrap();
        assert_eq!(children.items.len(), 1);
    }

    #[test]
    fn convert_raw_to_domain() {
        let raw = RawBinderItem {
            uuid: "11111111-1111-1111-1111-111111111111".into(),
            item_type: "Text".into(),
            title: Some("Test".into()),
            created: None,
            modified: None,
            metadata: None,
            children: None,
        };

        let item = BinderItem::try_from(raw).unwrap();
        assert!(matches!(item, BinderItem::Document(_)));
    }

    #[test]
    fn invalid_uuid_returns_error() {
        let raw = RawBinderItem {
            uuid: "not-a-valid-uuid".into(),
            item_type: "Text".into(),
            title: Some("Test".into()),
            ..Default::default()
        };

        assert!(BinderItem::try_from(raw).is_err());
    }

    #[test]
    fn trash_folder_parsed_separately() {
        // Verify TrashFolder type goes to Trash, not Binder
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();

        // Trash should have items
        assert!(!trash.items.is_empty());

        // Binder root should NOT contain the trash folder
        for item in &binder.root {
            if let BinderItem::Folder(f) = item {
                assert_ne!(f.title, "Trash");
            }
        }
    }
}
```

### Binder Tests (`binder.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_binder() -> Binder {
        Binder {
            root: vec![
                BinderItem::Folder(Folder {
                    uuid: Uuid::parse_str("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA").unwrap(),
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
                }),
            ],
        }
    }

    #[test]
    fn find_by_uuid_root_level() {
        let binder = sample_binder();
        let uuid = Uuid::parse_str("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA").unwrap();
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
        // Draft + Chapter One + Chapter Two = 3
        assert_eq!(flat.len(), 3);
        // Chapter One path should be ["Draft", "Chapter One"]
        assert_eq!(flat[1].1, vec!["Draft", "Chapter One"]);
    }

    #[test]
    fn move_item_to_root() {
        let mut binder = sample_binder();
        let uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        binder.move_item(uuid, None).unwrap();
        // Item should now be at root level
        assert_eq!(binder.root.len(), 2);
    }

    #[test]
    fn move_item_not_found_error() {
        let mut binder = sample_binder();
        let uuid = Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap();
        assert!(binder.move_item(uuid, None).is_err());
    }
}
```

### Document Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_add_no_duplicates() {
        let mut doc = Document::default();
        doc.add_keyword("test");
        doc.add_keyword("test");
        assert_eq!(doc.keywords.len(), 1);
    }

    #[test]
    fn keyword_remove() {
        let mut doc = Document::default();
        doc.add_keyword("alpha");
        doc.add_keyword("beta");
        doc.remove_keyword("alpha");
        assert_eq!(doc.keywords, vec!["beta"]);
    }
}
```

## Integration Tests (`tests/`)

### Project Open Tests (`tests/project_tests.rs`)

```rust
use scrivener::Project;

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
    if let scrivener::BinderItem::Folder(draft) = &project.binder.root[0] {
        assert_eq!(draft.title, "Draft");
        assert_eq!(draft.children.len(), 2);
    } else {
        panic!("Expected Draft folder at root[0]");
    }

    // Trash should have 1 item
    assert_eq!(project.trash.items.len(), 1);
}
```

### Content Read Tests (`tests/document_tests.rs`)

```rust
use scrivener::{Project, BinderItem};

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
    // UUID with no content.rtf file
    let uuid = uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();

    if let Some(BinderItem::Document(doc)) = project.binder.find_by_uuid(uuid) {
        let content = doc.read_content(&project.path).unwrap();
        assert_eq!(content.plain_text.as_deref(), Some(""));
    }
}
```

### Write Tests (`tests/document_tests.rs`, continued)

```rust
use tempfile::TempDir;

#[test]
fn write_and_read_content() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("test.scriv");
    std::fs::create_dir_all(&project_dir).unwrap();

    // Create a document and write content
    let uuid = uuid::Uuid::new_v4();
    let mut doc = scrivener::Document {
        uuid,
        title: "Test".into(),
        ..Default::default()
    };

    doc.write_content(temp.path().join("test.scriv").as_ref(), "Hello, world!").unwrap();

    // Read it back
    let content = doc.read_content(temp.path().join("test.scriv").as_ref()).unwrap();
    assert!(content.plain_text.unwrap().contains("Hello, world!"));
}

#[test]
fn write_synopsis() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("test.scriv");

    let uuid = uuid::Uuid::new_v4();
    let mut doc = scrivener::Document {
        uuid,
        title: "Test".into(),
        ..Default::default()
    };

    doc.update_synopsis(&project_dir, "A brief summary.").unwrap();
    assert_eq!(doc.synopsis.as_deref(), Some("A brief summary."));

    // Verify file exists on disk
    let synopsis_path = project_dir
        .join("Files").join("Data").join(uuid.to_string()).join("synopsis.txt");
    assert!(synopsis_path.exists());
    assert_eq!(std::fs::read_to_string(&synopsis_path).unwrap(), "A brief summary.");
}
```

### Search Tests (`tests/search_tests.rs`)

```rust
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
fn search_regex() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let results = project.search_regex(r"Chapter \w+").unwrap();
    // Should match content containing "Chapter" patterns
    assert!(!results.is_empty());
}

#[test]
fn search_by_keyword() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let docs = project.search_by_keyword("protagonist");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].title, "Chapter One");
}
```

### Trash Tests (`tests/trash_tests.rs`)

```rust
use scrivener::Project;
use tempfile::TempDir;

#[test]
fn list_trash_items() {
    let project = Project::open("tests/fixtures/sample.scriv").unwrap();
    let trash = project.list_trash();
    assert_eq!(trash.items.len(), 1);
}

#[test]
fn recover_from_trash() {
    let temp = TempDir::new().unwrap();
    // Copy fixture to temp dir for mutation
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
    let temp = TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    let mut project = Project::open(temp.path().join("sample.scriv")).unwrap();
    project.empty_trash().unwrap();

    assert!(project.trash.items.is_empty());
    // Verify data directory was deleted
    let deleted_dir = temp.path()
        .join("sample.scriv")
        .join("Files").join("Data")
        .join("44444444-4444-4444-4444-444444444444");
    assert!(!deleted_dir.exists());
}
```

### Round-trip Tests (`tests/roundtrip_tests.rs`)

```rust
use scrivener::Project;
use tempfile::TempDir;

#[test]
fn roundtrip_open_save_open() {
    let temp = TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    let project_path = temp.path().join("sample.scriv");

    // Open and modify
    let mut project = Project::open(&project_path).unwrap();
    let uuid = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    if let Some(scrivener::BinderItem::Document(doc)) = project.binder.find_by_uuid_mut(uuid) {
        doc.add_keyword("new-keyword");
    }

    // Save
    project.save().unwrap();

    // Re-open and verify
    let project2 = Project::open(&project_path).unwrap();
    if let Some(scrivener::BinderItem::Document(doc)) = project2.binder.find_by_uuid(uuid) {
        assert!(doc.keywords.contains(&"new-keyword".to_string()));
    } else {
        panic!("Document not found after round-trip");
    }
}

#[test]
fn roundtrip_preserves_structure() {
    let temp = TempDir::new().unwrap();
    copy_fixture_to_temp(&temp);

    let project_path = temp.path().join("sample.scriv");

    let project1 = Project::open(&project_path).unwrap();
    let flat1 = project1.binder.flatten();

    project1.save().unwrap();

    let project2 = Project::open(&project_path).unwrap();
    let flat2 = project2.binder.flatten();

    // Same number of items
    assert_eq!(flat1.len(), flat2.len());

    // Same titles in same order
    for (a, b) in flat1.iter().zip(flat2.iter()) {
        assert_eq!(a.1, b.1); // paths match
    }
}
```

## Test Coverage Goals

| Module | Coverage Target | Key Scenarios |
|--------|----------------|---------------|
| `scrivx.rs` | 95%+ | Deserialize all element types, missing fields, invalid UUIDs |
| `binder.rs` | 90%+ | find_by_uuid, find_by_title, flatten, move_item, cycle detection |
| `project.rs` | 85%+ | open, save, save_as, missing project, invalid structure |
| `document.rs` | 90%+ | read_content, write_content, synopsis, notes, keywords |
| `search.rs` | 85%+ | plain text, regex, keyword, empty results, context extraction |
| `trash.rs` | 90%+ | list, recover, empty, not-found errors |
| `metadata.rs` | 80%+ | Date parsing, default values |
| `statistics.rs` | 85%+ | Word count, character count, per-document counts |
| Round-trip | N/A | Structure preservation, content preservation |

## Temp Directory Pattern

All tests that write to disk use `tempfile::TempDir` to ensure isolation:

```rust
fn copy_fixture_to_temp(temp: &TempDir) {
    let src = Path::new("tests/fixtures/sample.scriv");
    let dest = temp.path().join("sample.scriv");
    copy_dir_recursive(src, &dest).unwrap();
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
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
```

This ensures:
- Tests don't modify the fixture files
- Tests don't interfere with each other
- Temp directories are cleaned up automatically on drop
