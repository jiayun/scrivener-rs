# Architecture & Module Design

## Overview

scrivener is a Rust library for reading and writing Scrivener 3 projects (`.scriv` bundles). It parses the `.scrivx` XML index, navigates the binder tree, reads/writes document content (via `scrivener-rtf` for RTF handling), and supports search, trash management, and project statistics.

## Data Flow

```
.scriv/           ──▶  Find .scrivx   ──▶  quick-xml + serde  ──▶  Project struct
(directory bundle)      (walkdir)           (XML deserialize)       ├── Binder (tree)
                                                                    ├── ProjectMetadata
                                                                    └── documents...

Project struct    ──▶  Modify binder/docs  ──▶  Serialize XML  ──▶  Write .scrivx + files
(in memory)            (API calls)               (quick-xml)        (.scriv/ directory)
```

### Content Read Path

```
Project.scriv/Files/Data/{UUID}/content.rtf
    │
    ▼
scrivener-rtf::parse()  ──▶  RTF Document AST  ──▶  extract plain text
                                                      (for search/stats)
```

### Content Write Path

```
plain text / formatted content
    │
    ▼
scrivener-rtf::Document  ──▶  .to_rtf()  ──▶  write to Files/Data/{UUID}/content.rtf
```

## Module Structure

```
src/
├── lib.rs          # Public API, re-exports
├── error.rs        # Error types (thiserror)
├── project.rs      # Project struct, open/save
├── binder.rs       # Binder tree, BinderItem, traversal
├── document.rs     # Document/Folder types, content read/write
├── scrivx.rs       # .scrivx XML parsing (quick-xml + serde)
├── metadata.rs     # ProjectMetadata, DocumentMetadata
├── search.rs       # Search functionality
├── trash.rs        # Trash management
└── statistics.rs   # Word count, project stats

tests/
├── project_tests.rs
├── binder_tests.rs
├── scrivx_tests.rs
├── search_tests.rs
├── roundtrip_tests.rs
└── fixtures/
    └── sample.scriv/       # Sample Scrivener project
        ├── sample.scrivx
        └── Files/Data/...
```

### Module Responsibilities

| Module | Visibility | Responsibility |
|--------|-----------|----------------|
| `lib.rs` | public | Entry point (`Project::open`), re-exports all public types |
| `error.rs` | public | `ScrivenerError` enum, `Result<T>` type alias |
| `project.rs` | public | `Project` struct, `open()`, `save()`, `save_as()` |
| `binder.rs` | public | `Binder`, `BinderItem` enum, tree traversal operations |
| `document.rs` | public | `Document`, `Folder`, `DocumentContent`, content read/write |
| `scrivx.rs` | internal | Raw XML serde structs, `.scrivx` deserialization/serialization |
| `metadata.rs` | public | `ProjectMetadata`, `DocumentMetadata` structs |
| `search.rs` | public | `SearchResult`, `Match`, plain text/regex/keyword search |
| `trash.rs` | public | `Trash`, `TrashedItem`, recover/empty operations |
| `statistics.rs` | public | `ProjectStatistics`, word/character counting |

## Key Design Decisions

### 1. Serde Deserialization for XML

The `.scrivx` file is parsed using `quick-xml` with serde `Deserialize`. Raw XML types in `scrivx.rs` map directly to the XML schema, then convert to domain types (`Binder`, `BinderItem`, etc.) via `From`/`TryFrom` impls.

This two-layer approach keeps the XML schema details isolated from the public API. If Scrivener changes its XML format, only `scrivx.rs` needs updating.

Alternative considered: manual `quick-xml` event-based parsing — more flexible but significantly more code and harder to maintain.

### 2. UUID-based File Paths

Scrivener stores document content at `Files/Data/{UUID}/content.rtf`. The UUID from the `.scrivx` binder entry maps directly to a filesystem path. This mapping is deterministic and requires no index lookup.

The `Document` struct stores the UUID, and content paths are resolved lazily via `Project.path.join("Files/Data").join(uuid.to_string())`.

### 3. Lazy Content Loading

`Project::open()` parses the `.scrivx` and builds the binder tree, but does NOT load document RTF content into memory. Content is loaded on-demand via `Document::read_content()`.

This keeps initial project load fast — a Scrivener project with hundreds of documents only needs to parse one XML file on open.

### 4. Owned SearchResult

`SearchResult` owns its data (`String` fields, not `&str` references) to avoid lifetime complexity. Search results may outlive the borrow of the document content that produced them. The performance cost of cloning strings is negligible for search result sets.

### 5. BinderItem as Enum

`BinderItem` is an enum with `Document` and `Folder` variants rather than a single struct with an `item_type` field. This makes the type system enforce the structural constraint: only `Folder` variants can have `children`.

### 6. scrivener-rtf Integration

RTF parsing and generation is delegated entirely to the `scrivener-rtf` crate. This crate handles:
- Parsing RTF bytes into an AST
- Extracting plain text from the AST (for search and word count)
- Generating RTF bytes from modified content

The separation keeps this crate focused on project structure and the `.scrivx` format.

## Public API

```rust
// Project lifecycle
impl Project {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Project>;
    pub fn save(&self) -> Result<()>;
    pub fn save_as<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}

// Binder navigation
impl Binder {
    pub fn find_by_uuid(&self, uuid: Uuid) -> Option<&BinderItem>;
    pub fn find_by_title(&self, title: &str) -> Vec<&BinderItem>;
    pub fn flatten(&self) -> Vec<(&BinderItem, Vec<String>)>;
    pub fn move_item(&mut self, uuid: Uuid, new_parent: Option<Uuid>) -> Result<()>;
}

// Document operations
impl Document {
    pub fn read_content(&self) -> Result<DocumentContent>;
    pub fn write_content(&mut self, content: &str) -> Result<()>;
    pub fn update_synopsis(&mut self, synopsis: &str) -> Result<()>;
    pub fn update_notes(&mut self, notes: &str) -> Result<()>;
    pub fn add_keyword(&mut self, keyword: &str);
    pub fn remove_keyword(&mut self, keyword: &str);
}

// Search
impl Project {
    pub fn search(&self, query: &str) -> Vec<SearchResult>;
    pub fn search_regex(&self, pattern: &str) -> Result<Vec<SearchResult>>;
    pub fn search_by_keyword(&self, keyword: &str) -> Vec<&Document>;
}

// Trash
impl Project {
    pub fn list_trash(&self) -> Trash;
    pub fn recover_from_trash(&mut self, uuid: Uuid) -> Result<()>;
    pub fn empty_trash(&mut self) -> Result<()>;
}

// Statistics
impl Project {
    pub fn statistics(&self) -> ProjectStatistics;
}
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `scrivener-rtf` | workspace | RTF parsing and generation |
| `quick-xml` | 0.37 | XML parsing with serde support |
| `serde` | 1.0 | Serialization/deserialization framework |
| `thiserror` | 2.0 | Error type derivation |
| `uuid` | 1.12 | UUID type with serde support |
| `chrono` | 0.4 | DateTime types with serde support |
| `walkdir` | 2.4 | Recursive directory traversal |
| `regex` | 1.0 | (optional) Regex search support |
| `tempfile` | 3.8 | (dev) Temporary directories for write tests |
| `pretty_assertions` | 1.4 | (dev) Readable test diffs |
