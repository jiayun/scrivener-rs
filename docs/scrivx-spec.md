# .scrivx XML Parsing Specification

## Overview

The `.scrivx` file is an XML document that serves as the index for a Scrivener 3 project. It describes the binder tree structure, document metadata, project properties, and compilation settings. This is the single most important file in a `.scriv` bundle — all other files (RTF content, notes, snapshots) are referenced from the binder entries via UUID-based paths.

## Public Interface

```rust
pub fn parse_scrivx<P: AsRef<Path>>(path: P) -> Result<(Binder, ProjectMetadata, Trash)>

pub fn write_scrivx<P: AsRef<Path>>(
    path: P,
    binder: &Binder,
    metadata: &ProjectMetadata,
    trash: &Trash,
) -> Result<()>
```

## XML Structure

### Top-level

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject Identifier="..." Version="2.0">
  <Binder>
    <BinderItem UUID="XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX" Type="DraftFolder" Created="2024-01-15T10:30:00Z" Modified="2024-01-15T10:30:00Z">
      <Title>Draft</Title>
      <MetaData>
        <IncludeInCompile>Yes</IncludeInCompile>
      </MetaData>
      <Children>
        <!-- child BinderItems -->
      </Children>
    </BinderItem>
    <BinderItem UUID="..." Type="ResearchFolder" ...>
      <!-- Research folder -->
    </BinderItem>
    <BinderItem UUID="..." Type="TrashFolder" ...>
      <!-- Trash folder -->
    </BinderItem>
  </Binder>
  <Collections>
    <!-- Search collections, custom collections -->
  </Collections>
  <LabelSettings>
    <!-- Label/status definitions -->
  </LabelSettings>
  <StatusSettings>
    <!-- Status definitions -->
  </StatusSettings>
  <ProjectProperties>
    <ProjectTitle>My Novel</ProjectTitle>
    <FullName>Author Name</FullName>
  </ProjectProperties>
</ScrivenerProject>
```

### BinderItem Element

```xml
<BinderItem UUID="A1B2C3D4-E5F6-7890-ABCD-EF1234567890"
            Type="Text"
            Created="2024-01-15T10:30:00Z"
            Modified="2024-06-20T14:22:00Z">
  <Title>Chapter One</Title>
  <MetaData>
    <IncludeInCompile>Yes</IncludeInCompile>
    <StatusID>2</StatusID>
    <LabelID>3</LabelID>
    <Keywords>
      <Keyword>protagonist</Keyword>
      <Keyword>opening</Keyword>
    </Keywords>
    <CustomMetaData>
      <MetaDataItem>
        <FieldID>POV</FieldID>
        <Value>First Person</Value>
      </MetaDataItem>
    </CustomMetaData>
    <NotesSelection>0, 0</NotesSelection>
  </MetaData>
  <TextSettings>
    <TextSelection>0, 0</TextSelection>
  </TextSettings>
  <Children>
    <!-- nested BinderItem elements for sub-documents -->
  </Children>
</BinderItem>
```

### BinderItem Type Values

| Type Value | Description | Maps To |
|-----------|-------------|---------|
| `Text` | A text document | `BinderItem::Document` |
| `Image` | An image item | `BinderItem::Document` with preserved type |
| `PDF` | A PDF item | `BinderItem::Document` with preserved type |
| Other values | Web archives and future item types | `BinderItem::Document` with preserved type |
| `Folder` | A generic folder | `BinderItem::Folder` |
| `DraftFolder` | The top-level Draft/Manuscript folder | `BinderItem::Folder` (root) |
| `ResearchFolder` | The top-level Research folder | `BinderItem::Folder` (root) |
| `TrashFolder` | The Trash folder | Parsed into `Trash` |
| `Root` | Invisible root container | Skipped |

## Serde Deserialization Structs (`scrivx.rs`)

### Raw XML Types

These structs map directly to the XML schema. They are internal to `scrivx.rs` and convert to domain types.

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename = "ScrivenerProject")]
struct RawScrivenerProject {
    #[serde(rename = "@Identifier")]
    identifier: Option<String>,

    #[serde(rename = "@Version")]
    version: Option<String>,

    #[serde(rename = "Binder")]
    binder: RawBinder,

    #[serde(rename = "ProjectProperties")]
    project_properties: Option<RawProjectProperties>,
}

#[derive(Debug, Deserialize)]
struct RawBinder {
    #[serde(rename = "BinderItem", default)]
    items: Vec<RawBinderItem>,
}

#[derive(Debug, Deserialize)]
struct RawBinderItem {
    #[serde(rename = "@UUID")]
    uuid: String,

    #[serde(rename = "@Type")]
    item_type: String,

    #[serde(rename = "@Created")]
    created: Option<String>,

    #[serde(rename = "@Modified")]
    modified: Option<String>,

    #[serde(rename = "Title")]
    title: Option<String>,

    #[serde(rename = "MetaData")]
    metadata: Option<RawMetaData>,

    #[serde(rename = "Children")]
    children: Option<RawChildren>,
}

#[derive(Debug, Deserialize)]
struct RawChildren {
    #[serde(rename = "BinderItem", default)]
    items: Vec<RawBinderItem>,
}

#[derive(Debug, Deserialize)]
struct RawMetaData {
    #[serde(rename = "IncludeInCompile")]
    include_in_compile: Option<String>,

    #[serde(rename = "Keywords")]
    keywords: Option<RawKeywords>,

    #[serde(rename = "CustomMetaData")]
    custom_metadata: Option<RawCustomMetaData>,
}

#[derive(Debug, Deserialize)]
struct RawKeywords {
    #[serde(rename = "Keyword", default)]
    keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawCustomMetaData {
    #[serde(rename = "MetaDataItem", default)]
    items: Vec<RawMetaDataItem>,
}

#[derive(Debug, Deserialize)]
struct RawMetaDataItem {
    #[serde(rename = "FieldID")]
    field_id: String,

    #[serde(rename = "Value")]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawProjectProperties {
    #[serde(rename = "ProjectTitle")]
    title: Option<String>,

    #[serde(rename = "FullName")]
    full_name: Option<String>,
}
```

### Conversion: Raw → Domain

```rust
impl TryFrom<RawBinderItem> for BinderItem {
    type Error = ScrivenerError;

    fn try_from(raw: RawBinderItem) -> Result<Self> {
        let uuid = Uuid::parse_str(&raw.uuid)
            .map_err(|e| ScrivenerError::ScrivxParseError {
                message: format!("Invalid UUID '{}': {}", raw.uuid, e),
            })?;

        let title = raw.title.unwrap_or_default();
        let metadata = convert_metadata(&raw)?;

        match raw.item_type.as_str() {
            "Text" => {
                let keywords = raw.metadata
                    .as_ref()
                    .and_then(|m| m.keywords.as_ref())
                    .map(|k| k.keywords.clone())
                    .unwrap_or_default();

                Ok(BinderItem::Document(Document {
                    uuid,
                    title,
                    synopsis: None,  // loaded lazily
                    notes: None,     // loaded lazily
                    keywords,
                    content: DocumentContent::new(uuid),
                    metadata,
                }))
            }
            "Folder" | "DraftFolder" | "ResearchFolder" => {
                let children = raw.children
                    .map(|c| c.items.into_iter()
                        .map(BinderItem::try_from)
                        .collect::<Result<Vec<_>>>())
                    .transpose()?
                    .unwrap_or_default();

                Ok(BinderItem::Folder(Folder {
                    uuid,
                    title,
                    children,
                    metadata,
                }))
            }
            other => Err(ScrivenerError::ScrivxParseError {
                message: format!("Unknown BinderItem type: '{}'", other),
            }),
        }
    }
}
```

## File Path Resolution

Document content files are stored in a predictable path based on the UUID:

```
Project.scriv/
├── Files/
│   └── Data/
│       ├── A1B2C3D4-E5F6-7890-ABCD-EF1234567890/
│       │   ├── content.rtf      # Main document content
│       │   ├── notes.rtf        # Document notes
│       │   └── synopsis.txt     # Synopsis text
│       └── B2C3D4E5-F6A7-8901-BCDE-F12345678901/
│           ├── content.rtf
│           └── notes.rtf
└── project.scrivx
```

```rust
impl DocumentContent {
    fn new(uuid: Uuid) -> Self {
        Self {
            rtf_path: PathBuf::new(), // resolved when Project sets base_path
            plain_text: None,
            formatted: None,
        }
    }
}

impl Project {
    fn resolve_content_path(&self, uuid: &Uuid) -> PathBuf {
        self.path
            .join("Files")
            .join("Data")
            .join(uuid.to_string())
            .join("content.rtf")
    }

    fn resolve_notes_path(&self, uuid: &Uuid) -> PathBuf {
        self.path
            .join("Files")
            .join("Data")
            .join(uuid.to_string())
            .join("notes.rtf")
    }

    fn resolve_synopsis_path(&self, uuid: &Uuid) -> PathBuf {
        self.path
            .join("Files")
            .join("Data")
            .join(uuid.to_string())
            .join("synopsis.txt")
    }
}
```

## Handling Scrivener Version Differences

### Version Detection

The `<ScrivenerProject>` root element includes a `Version` attribute:

```xml
<ScrivenerProject Identifier="..." Version="2.0">
```

| Version | Scrivener | Notes |
|---------|-----------|-------|
| `2.0` | Scrivener 3.x | Current format, primary target |
| `1.0` | Scrivener 2.x | Legacy — different XML schema, not supported in v0.1.0 |

### Forward Compatibility

- Unknown XML elements are ignored during deserialization (`#[serde(deny_unknown_fields)]` is NOT used)
- Unknown `BinderItem` types are represented as documents with their original XML `Type`
- Missing optional elements default to sensible values

## Error Handling

### Parse Errors

| Scenario | Error |
|----------|-------|
| `.scrivx` file not found | `ScrivenerError::ProjectNotFound` |
| Invalid XML syntax | `ScrivenerError::XmlError` (from `quick-xml`) |
| Missing required field (UUID) | `ScrivenerError::ScrivxParseError` |
| Invalid UUID format | `ScrivenerError::ScrivxParseError` |
| Unsupported project version | `ScrivenerError::InvalidProject` |

### Serialization Back to XML

For a newly constructed project, domain types convert to raw XML types and
serialize via `quick-xml::se::to_string`. When saving a project opened from
disk, the library instead merges the current binder tree into an `xmltree` DOM
of the original `<Binder>` section. Existing items are matched by UUID, so
structural changes and modeled values are updated while Scrivener-owned XML
that the domain model does not understand remains attached to the item.

The serialization preserves:
- Element ordering (Binder items in their current order)
- UUID formatting (uppercase hyphenated)
- Date formatting (ISO 8601)
- Unmodeled Binder/BinderItem attributes and elements, including `TextSettings`
- Unmodeled metadata and children-container extensions

Elements outside `<Binder>` (Collections, LabelSettings, StatusSettings, and
similar project settings) remain byte-for-byte unchanged.
