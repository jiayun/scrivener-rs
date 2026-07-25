# Complete Type Definitions

## Project (`project.rs`)

```rust
/// A Scrivener 3 project loaded from a `.scriv` bundle.
///
/// The project is the top-level entry point. It holds the binder tree,
/// project metadata, and the path to the `.scriv` directory on disk.
#[derive(Debug, Clone)]
pub struct Project {
    /// Path to the `.scriv` bundle directory.
    pub path: PathBuf,

    /// The binder tree containing all documents and folders.
    pub binder: Binder,

    /// Project-level metadata (title, author, dates).
    pub metadata: ProjectMetadata,

    /// Trash folder contents (separate from the main binder tree).
    pub trash: Trash,
}
```

## Binder (`binder.rs`)

```rust
/// The binder tree — Scrivener's hierarchical document structure.
///
/// The root contains top-level items (typically "Draft", "Research", "Trash").
/// Each item can be a Document or Folder, and either may have children.
#[derive(Debug, Clone)]
pub struct Binder {
    /// Top-level binder items (Draft, Research, etc.)
    pub root: Vec<BinderItem>,
}

/// A single item in the binder tree.
///
/// Uses an enum to retain the logical document/folder distinction while
/// preserving Scrivener's document-with-children model.
#[derive(Debug, Clone)]
pub enum BinderItem {
    /// A text-like document that may contain child items.
    Document(Document),

    /// A folder that may contain child items.
    Folder(Folder),
}
```

### BinderItem Notes

- `BinderItem` is recursive through both `Document.children` and `Folder.children`
- The Scrivener "Draft" folder is a `Folder` at the root level
- The "Trash" folder is parsed separately into `Project.trash`
- Each item has a UUID that matches the filesystem path under `Files/Data/`

## Document (`document.rs`)

```rust
/// A text document in the binder.
///
/// Content is loaded lazily — the struct is populated from .scrivx metadata,
/// but RTF content is only read from disk when `read_content()` is called.
#[derive(Debug, Clone)]
pub struct Document {
    /// Unique identifier, maps to `Files/Data/{uuid}/` on disk.
    pub uuid: Uuid,

    /// Document title as shown in the binder.
    pub title: String,

    /// Child binder items.
    pub children: Vec<BinderItem>,

    /// Original XML Type value, such as Text, Image, PDF, or WebArchive.
    pub doc_type: String,

    /// Short synopsis/summary text.
    pub synopsis: Option<String>,

    /// Document notes (separate from main content).
    pub notes: Option<String>,

    /// Keywords/tags assigned to this document.
    pub keywords: Vec<String>,

    /// Content reference (RTF path, cached plain text).
    pub content: DocumentContent,

    /// Document-level metadata (dates, word count, compile flag).
    pub metadata: DocumentMetadata,
}
```

### Folder

```rust
/// A folder in the binder that can contain child items.
///
/// Folders can also have their own content (synopsis, notes),
/// but their primary purpose is organizing child items.
#[derive(Debug, Clone)]
pub struct Folder {
    /// Unique identifier.
    pub uuid: Uuid,

    /// Folder title as shown in the binder.
    pub title: String,

    /// Child items (documents and sub-folders).
    pub children: Vec<BinderItem>,

    pub synopsis: Option<String>,
    pub notes: Option<String>,
    pub keywords: Vec<String>,
    pub content: DocumentContent,

    /// Folder-level metadata.
    pub metadata: DocumentMetadata,
}
```

### DocumentContent

```rust
/// Content associated with a document.
///
/// The `rtf_path` is always set (resolved from UUID).
/// Plain text and formatted content are populated lazily.
#[derive(Debug, Clone)]
pub struct DocumentContent {
    /// Path to the content.rtf file on disk.
    pub rtf_path: PathBuf,

    /// Cached plain text extracted from RTF (populated by `read_content()`).
    pub plain_text: Option<String>,

    /// Cached formatted content (populated by `read_content()`).
    pub formatted: Option<FormattedContent>,
}

/// Formatted content extracted from RTF.
///
/// Provides structured access to text with formatting information.
#[derive(Debug, Clone)]
pub struct FormattedContent {
    /// The full document text with formatting stripped.
    pub text: String,

    /// Word count of the content.
    pub word_count: usize,

    /// Character count of the content.
    pub character_count: usize,
}
```

## Metadata (`metadata.rs`)

### ProjectMetadata

```rust
/// Project-level metadata from the .scrivx ProjectProperties section.
#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    /// Project title.
    pub title: String,

    /// Author name.
    pub author: Option<String>,

    /// Template the project was created from.
    pub template: Option<String>,

    /// When the project was created.
    pub created: DateTime<Utc>,

    /// When the project was last modified.
    pub modified: DateTime<Utc>,
}
```

### DocumentMetadata

```rust
/// Document-level metadata from the .scrivx MetaData section.
#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    /// When the document was created.
    pub created: DateTime<Utc>,

    /// When the document was last modified.
    pub modified: DateTime<Utc>,

    /// Word count (from Scrivener's cached count).
    pub word_count: usize,

    /// Character count (from Scrivener's cached count).
    pub character_count: usize,

    /// Whether this document is included in Compile output.
    pub include_in_compile: bool,

    /// Custom metadata key-value pairs defined by the user.
    pub custom_metadata: HashMap<String, String>,
}
```

### Metadata Notes

- `DateTime<Utc>` uses the `chrono` crate with serde support
- `custom_metadata` stores user-defined fields as string key-value pairs
- Word/character counts may be stale — Scrivener caches these in the `.scrivx`; use `statistics()` for fresh counts

## Search (`search.rs`)

```rust
/// A search result from a single document.
///
/// Owns all its data (no lifetimes) so results can outlive
/// the borrow of document content that produced them.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// UUID of the document containing the match.
    pub document_uuid: Uuid,

    /// Title of the matching document.
    pub document_title: String,

    /// Individual matches within the document.
    pub matches: Vec<Match>,
}

/// A single match within a document's content.
#[derive(Debug, Clone)]
pub struct Match {
    /// Surrounding text context for display.
    pub context: String,

    /// Byte offset range (start, end) within the document's plain text.
    pub position: (usize, usize),
}
```

## Trash (`trash.rs`)

```rust
/// Contents of the Scrivener Trash folder.
///
/// The Trash is a special binder section. Items in trash retain
/// their full structure (content, metadata) and can be recovered.
#[derive(Debug, Clone, Default)]
pub struct Trash {
    /// Items currently in the trash.
    pub items: Vec<TrashedItem>,
}

/// An item in the trash — mirrors BinderItem but tracks origin.
#[derive(Debug, Clone)]
pub enum TrashedItem {
    /// A trashed document.
    Document(Document),

    /// A trashed folder (may contain children).
    Folder(Folder),
}
```

## Statistics (`statistics.rs`)

```rust
/// Aggregate statistics for the entire project.
#[derive(Debug, Clone)]
pub struct ProjectStatistics {
    /// Total number of documents (excluding trash).
    pub total_documents: usize,

    /// Total number of folders (excluding trash).
    pub total_folders: usize,

    /// Total word count across all documents.
    pub total_words: usize,

    /// Total character count across all documents.
    pub total_characters: usize,

    /// Per-document word counts, keyed by UUID.
    pub words_by_document: HashMap<Uuid, usize>,
}
```

## Error Types (`error.rs`)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScrivenerError {
    #[error("Project not found: {path}")]
    ProjectNotFound {
        path: PathBuf,
    },

    #[error("Invalid project: {message}")]
    InvalidProject {
        message: String,
    },

    #[error("Failed to parse .scrivx: {message}")]
    ScrivxParseError {
        message: String,
    },

    #[error("Document not found: UUID {uuid}")]
    DocumentNotFound {
        uuid: Uuid,
    },

    #[error("Content error: {message}")]
    ContentError {
        message: String,
    },

    #[error("Invalid regex pattern: {0}")]
    RegexError(#[from] regex::Error),

    #[error("RTF error: {0}")]
    RtfError(#[from] scrivener_rtf::RtfError),

    #[error("XML error: {0}")]
    XmlError(#[from] quick_xml::DeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ScrivenerError>;
```

### Error Notes

- `ScrivenerError` wraps errors from all dependencies (`quick-xml`, `scrivener-rtf`, `regex`, `std::io`)
- `ProjectNotFound` is returned when the `.scriv` directory doesn't exist or lacks a `.scrivx` file
- `InvalidProject` covers structural issues (missing `Files/Data/` directory, malformed binder)
- `DocumentNotFound` is returned by binder lookup operations when a UUID doesn't match any item

## Key Design Choices

### UUID Usage
- `uuid::Uuid` type (not `String`) for type safety and validation
- Serde support enabled via `uuid = { features = ["serde"] }`
- UUIDs are parsed from `.scrivx` XML and used for filesystem path resolution

### HashMap for Custom Metadata
- Scrivener allows users to define custom metadata fields per document
- These are stored as `HashMap<String, String>` since the keys are user-defined
- More structured metadata (dates, counts) uses typed fields

### chrono DateTime
- All timestamps use `chrono::DateTime<Utc>` for timezone-aware date handling
- Scrivener stores dates in ISO 8601 format in `.scrivx`, which chrono parses natively
- Serde support via `chrono = { features = ["serde"] }`
