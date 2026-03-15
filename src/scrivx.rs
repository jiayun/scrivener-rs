use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::binder::{Binder, BinderItem};
use crate::document::{Document, DocumentContent, Folder, FolderType};
use crate::error::{Result, ScrivenerError};
use crate::metadata::{DocumentMetadata, ProjectMetadata};
use crate::trash::{Trash, TrashedItem};

// -- Raw XML types (Deserialize) --

#[derive(Debug, Deserialize)]
#[serde(rename = "ScrivenerProject")]
struct RawScrivenerProject {
    #[serde(rename = "@Identifier")]
    _identifier: Option<String>,

    #[serde(rename = "@Version")]
    _version: Option<String>,

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

#[derive(Debug, Deserialize, Default)]
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

// -- Raw XML types (Serialize) --

#[derive(Debug, Serialize)]
#[serde(rename = "ScrivenerProject")]
struct RawScrivenerProjectOut {
    #[serde(rename = "@Identifier")]
    identifier: String,

    #[serde(rename = "@Version")]
    version: String,

    #[serde(rename = "Binder")]
    binder: RawBinderOut,

    #[serde(rename = "ProjectProperties")]
    project_properties: RawProjectPropertiesOut,
}

#[derive(Debug, Serialize)]
#[serde(rename = "Binder")]
struct RawBinderOut {
    #[serde(rename = "BinderItem")]
    items: Vec<RawBinderItemOut>,
}

#[derive(Debug, Serialize)]
struct RawBinderItemOut {
    #[serde(rename = "@UUID")]
    uuid: String,

    #[serde(rename = "@Type")]
    item_type: String,

    #[serde(rename = "@Created")]
    created: String,

    #[serde(rename = "@Modified")]
    modified: String,

    #[serde(rename = "Title")]
    title: String,

    #[serde(rename = "MetaData")]
    metadata: Option<RawMetaDataOut>,

    #[serde(rename = "Children", skip_serializing_if = "Option::is_none")]
    children: Option<RawChildrenOut>,
}

#[derive(Debug, Serialize)]
struct RawChildrenOut {
    #[serde(rename = "BinderItem")]
    items: Vec<RawBinderItemOut>,
}

#[derive(Debug, Serialize)]
struct RawMetaDataOut {
    #[serde(rename = "IncludeInCompile")]
    include_in_compile: String,

    #[serde(rename = "Keywords", skip_serializing_if = "Option::is_none")]
    keywords: Option<RawKeywordsOut>,

    #[serde(rename = "CustomMetaData", skip_serializing_if = "Option::is_none")]
    custom_metadata: Option<RawCustomMetaDataOut>,
}

#[derive(Debug, Serialize)]
struct RawKeywordsOut {
    #[serde(rename = "Keyword")]
    keywords: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RawCustomMetaDataOut {
    #[serde(rename = "MetaDataItem")]
    items: Vec<RawMetaDataItemOut>,
}

#[derive(Debug, Serialize)]
struct RawMetaDataItemOut {
    #[serde(rename = "FieldID")]
    field_id: String,

    #[serde(rename = "Value")]
    value: String,
}

#[derive(Debug, Serialize)]
struct RawProjectPropertiesOut {
    #[serde(rename = "ProjectTitle")]
    title: String,

    #[serde(rename = "FullName", skip_serializing_if = "Option::is_none")]
    full_name: Option<String>,
}

// -- Conversion: Raw → Domain --

fn parse_datetime(s: Option<&str>) -> DateTime<Utc> {
    s.and_then(|s| {
        // Try ISO 8601 / RFC 3339 first (e.g. "2024-01-15T10:00:00Z")
        s.parse::<DateTime<Utc>>().ok().or_else(|| {
            // Try Scrivener's native format (e.g. "2024-01-15 21:49:00 +0800")
            chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S %z")
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
    })
    .unwrap_or_else(Utc::now)
}

fn convert_metadata(raw: &RawBinderItem) -> DocumentMetadata {
    let include_in_compile = raw
        .metadata
        .as_ref()
        .and_then(|m| m.include_in_compile.as_deref())
        .map(|v| v == "Yes")
        .unwrap_or(true);

    let custom_metadata = raw
        .metadata
        .as_ref()
        .and_then(|m| m.custom_metadata.as_ref())
        .map(|cm| {
            cm.items
                .iter()
                .filter_map(|item| {
                    item.value
                        .as_ref()
                        .map(|v| (item.field_id.clone(), v.clone()))
                })
                .collect::<HashMap<String, String>>()
        })
        .unwrap_or_default();

    DocumentMetadata {
        created: parse_datetime(raw.created.as_deref()),
        modified: parse_datetime(raw.modified.as_deref()),
        include_in_compile,
        custom_metadata,
    }
}

fn convert_binder_item(raw: RawBinderItem) -> Result<BinderItem> {
    let uuid = Uuid::parse_str(&raw.uuid).map_err(|e| ScrivenerError::ScrivxParseError {
        message: format!("Invalid UUID '{}': {}", raw.uuid, e),
    })?;

    let title = raw.title.clone().unwrap_or_default();
    let metadata = convert_metadata(&raw);

    match raw.item_type.as_str() {
        "Text" | "Image" | "PDF" => {
            let keywords = raw
                .metadata
                .as_ref()
                .and_then(|m| m.keywords.as_ref())
                .map(|k| k.keywords.clone())
                .unwrap_or_default();

            Ok(BinderItem::Document(Document {
                uuid,
                title,
                synopsis: None,
                notes: None,
                keywords,
                content: DocumentContent::new(),
                metadata,
            }))
        }
        "Folder" | "DraftFolder" | "ResearchFolder" => {
            let folder_type = match raw.item_type.as_str() {
                "DraftFolder" => FolderType::DraftFolder,
                "ResearchFolder" => FolderType::ResearchFolder,
                _ => FolderType::Folder,
            };

            let children = raw
                .children
                .map(|c| {
                    c.items
                        .into_iter()
                        .map(convert_binder_item)
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .unwrap_or_default();

            Ok(BinderItem::Folder(Folder {
                uuid,
                title,
                children,
                metadata,
                folder_type,
            }))
        }
        "TrashFolder" => {
            // TrashFolder is handled separately; return as Folder for now
            let children = raw
                .children
                .map(|c| {
                    c.items
                        .into_iter()
                        .map(convert_binder_item)
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .unwrap_or_default();

            Ok(BinderItem::Folder(Folder {
                uuid,
                title,
                children,
                metadata,
                folder_type: FolderType::TrashFolder,
            }))
        }
        other => Err(ScrivenerError::ScrivxParseError {
            message: format!("Unknown BinderItem type: '{}'", other),
        }),
    }
}

fn binder_item_to_trashed(item: BinderItem) -> TrashedItem {
    match item {
        BinderItem::Document(doc) => TrashedItem::Document(doc),
        BinderItem::Folder(folder) => TrashedItem::Folder(folder),
    }
}

// -- Public API --

pub(crate) fn _parse_scrivx<P: AsRef<Path>>(path: P) -> Result<(Binder, ProjectMetadata, Trash)> {
    let xml_content = std::fs::read_to_string(path.as_ref())?;
    parse_scrivx_str(&xml_content)
}

pub fn parse_scrivx_str(xml: &str) -> Result<(Binder, ProjectMetadata, Trash)> {
    let raw: RawScrivenerProject =
        quick_xml::de::from_str(xml).map_err(|e| ScrivenerError::ScrivxParseError {
            message: format!("XML deserialization failed: {}", e),
        })?;

    let mut binder_items = Vec::new();
    let mut trash = Trash::default();

    for raw_item in raw.binder.items {
        if raw_item.item_type == "TrashFolder" {
            // Store the trash folder UUID
            if let Ok(trash_uuid) = Uuid::parse_str(&raw_item.uuid) {
                trash.uuid = Some(trash_uuid);
            }
            // Parse trash children
            if let Some(children) = raw_item.children {
                for child in children.items {
                    let item = convert_binder_item(child)?;
                    trash.items.push(binder_item_to_trashed(item));
                }
            }
        } else {
            let item = convert_binder_item(raw_item)?;
            binder_items.push(item);
        }
    }

    let metadata = if let Some(props) = raw.project_properties {
        ProjectMetadata {
            title: props.title.unwrap_or_default(),
            author: props.full_name,
            created: Utc::now(),
            modified: Utc::now(),
        }
    } else {
        ProjectMetadata::default()
    };

    Ok((Binder { root: binder_items }, metadata, trash))
}

// -- Serialization: Domain → XML --

fn format_datetime(dt: &DateTime<Utc>) -> String {
    use chrono::Local;
    let local = dt.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M:%S %z").to_string()
}

fn binder_item_to_raw(item: &BinderItem) -> RawBinderItemOut {
    match item {
        BinderItem::Document(doc) => RawBinderItemOut {
            uuid: doc.uuid.to_string().to_uppercase(),
            item_type: "Text".to_string(),
            created: format_datetime(&doc.metadata.created),
            modified: format_datetime(&doc.metadata.modified),
            title: doc.title.clone(),
            metadata: Some(RawMetaDataOut {
                include_in_compile: if doc.metadata.include_in_compile {
                    "Yes"
                } else {
                    "No"
                }
                .to_string(),
                keywords: if doc.keywords.is_empty() {
                    None
                } else {
                    Some(RawKeywordsOut {
                        keywords: doc.keywords.clone(),
                    })
                },
                custom_metadata: if doc.metadata.custom_metadata.is_empty() {
                    None
                } else {
                    Some(RawCustomMetaDataOut {
                        items: doc
                            .metadata
                            .custom_metadata
                            .iter()
                            .map(|(k, v)| RawMetaDataItemOut {
                                field_id: k.clone(),
                                value: v.clone(),
                            })
                            .collect(),
                    })
                },
            }),
            children: None,
        },
        BinderItem::Folder(folder) => {
            let children: Vec<RawBinderItemOut> =
                folder.children.iter().map(binder_item_to_raw).collect();

            RawBinderItemOut {
                uuid: folder.uuid.to_string().to_uppercase(),
                item_type: folder.folder_type.as_xml_type().to_string(),
                created: format_datetime(&folder.metadata.created),
                modified: format_datetime(&folder.metadata.modified),
                title: folder.title.clone(),
                metadata: Some(RawMetaDataOut {
                    include_in_compile: if folder.metadata.include_in_compile {
                        "Yes"
                    } else {
                        "No"
                    }
                    .to_string(),
                    keywords: None,
                    custom_metadata: None,
                }),
                children: if children.is_empty() {
                    None
                } else {
                    Some(RawChildrenOut { items: children })
                },
            }
        }
    }
}

fn trashed_item_to_raw(item: &TrashedItem) -> RawBinderItemOut {
    match item {
        TrashedItem::Document(doc) => binder_item_to_raw(&BinderItem::Document(doc.clone())),
        TrashedItem::Folder(folder) => binder_item_to_raw(&BinderItem::Folder(folder.clone())),
    }
}

pub fn serialize_scrivx(
    binder: &Binder,
    metadata: &ProjectMetadata,
    trash: &Trash,
) -> Result<String> {
    let mut all_items: Vec<RawBinderItemOut> =
        binder.root.iter().map(binder_item_to_raw).collect();

    // Add trash folder (always include it if there's a UUID, even if empty)
    {
        let trash_uuid = trash
            .uuid
            .map(|u| u.to_string().to_uppercase())
            .unwrap_or_else(|| "00000000-0000-0000-0000-000000000000".to_string());

        let trash_children: Vec<RawBinderItemOut> =
            trash.items.iter().map(trashed_item_to_raw).collect();

        all_items.push(RawBinderItemOut {
            uuid: trash_uuid,
            item_type: "TrashFolder".to_string(),
            created: format_datetime(&Utc::now()),
            modified: format_datetime(&Utc::now()),
            title: "Trash".to_string(),
            metadata: None,
            children: if trash_children.is_empty() {
                None
            } else {
                Some(RawChildrenOut {
                    items: trash_children,
                })
            },
        });
    }

    let project = RawScrivenerProjectOut {
        identifier: metadata.title.clone(),
        version: "2.0".to_string(),
        binder: RawBinderOut { items: all_items },
        project_properties: RawProjectPropertiesOut {
            title: metadata.title.clone(),
            full_name: metadata.author.clone(),
        },
    };

    let xml = quick_xml::se::to_string(&project)
        .map_err(|e| ScrivenerError::ScrivxParseError {
            message: format!("XML serialization failed: {}", e),
        })?;

    Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml))
}

/// Serialize only the `<Binder>...</Binder>` section as an XML string.
fn serialize_binder_xml(binder: &Binder, trash: &Trash) -> Result<String> {
    let mut all_items: Vec<RawBinderItemOut> =
        binder.root.iter().map(binder_item_to_raw).collect();

    // Add trash folder
    {
        let trash_uuid = trash
            .uuid
            .map(|u| u.to_string().to_uppercase())
            .unwrap_or_else(|| "00000000-0000-0000-0000-000000000000".to_string());

        let trash_children: Vec<RawBinderItemOut> =
            trash.items.iter().map(trashed_item_to_raw).collect();

        all_items.push(RawBinderItemOut {
            uuid: trash_uuid,
            item_type: "TrashFolder".to_string(),
            created: format_datetime(&Utc::now()),
            modified: format_datetime(&Utc::now()),
            title: "Trash".to_string(),
            metadata: None,
            children: if trash_children.is_empty() {
                None
            } else {
                Some(RawChildrenOut {
                    items: trash_children,
                })
            },
        });
    }

    let binder_out = RawBinderOut { items: all_items };
    let xml = quick_xml::se::to_string(&binder_out).map_err(|e| ScrivenerError::ScrivxParseError {
        message: format!("Binder XML serialization failed: {}", e),
    })?;
    Ok(xml)
}

/// Serialize the project by replacing only the `<Binder>...</Binder>` section in the
/// original raw XML, preserving all other elements (Collections, PrintSettings, etc.).
pub(crate) fn serialize_scrivx_preserving(
    raw_xml: &str,
    binder: &Binder,
    trash: &Trash,
) -> Result<String> {
    let binder_xml = serialize_binder_xml(binder, trash)?;

    // Find the <Binder> ... </Binder> region in the raw XML and replace it.
    // We look for the outermost <Binder> and </Binder> tags.
    let binder_start = raw_xml
        .find("<Binder")
        .ok_or_else(|| ScrivenerError::ScrivxParseError {
            message: "Could not find <Binder> tag in raw XML".into(),
        })?;

    let binder_end_tag = "</Binder>";
    let binder_end = raw_xml
        .find(binder_end_tag)
        .ok_or_else(|| ScrivenerError::ScrivxParseError {
            message: "Could not find </Binder> tag in raw XML".into(),
        })?;
    let binder_end = binder_end + binder_end_tag.len();

    // Build the replacement: splice the new binder XML into the original
    let mut result = String::with_capacity(raw_xml.len());
    result.push_str(&raw_xml[..binder_start]);
    result.push_str(&binder_xml);
    result.push_str(&raw_xml[binder_end..]);

    Ok(result)
}

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
            ..Default::default()
        };

        let item = convert_binder_item(raw).unwrap();
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

        assert!(convert_binder_item(raw).is_err());
    }

    #[test]
    fn parse_full_scrivx_str() {
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, metadata, trash) = parse_scrivx_str(xml).unwrap();

        assert_eq!(metadata.title, "Sample Novel");
        assert_eq!(metadata.author.as_deref(), Some("Test Author"));
        assert_eq!(binder.root.len(), 2); // Draft + Research (no Trash)
        assert_eq!(trash.items.len(), 1); // Deleted Scene
    }

    #[test]
    fn trash_folder_parsed_separately() {
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();

        assert!(!trash.items.is_empty());
        for item in &binder.root {
            if let BinderItem::Folder(f) = item {
                assert_ne!(f.title, "Trash");
            }
        }
    }

    #[test]
    fn serialize_roundtrip() {
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, metadata, trash) = parse_scrivx_str(xml).unwrap();

        let serialized = serialize_scrivx(&binder, &metadata, &trash).unwrap();
        let (binder2, metadata2, trash2) = parse_scrivx_str(&serialized).unwrap();

        assert_eq!(binder2.root.len(), binder.root.len());
        assert_eq!(metadata2.title, metadata.title);
        assert_eq!(trash2.items.len(), trash.items.len());
    }

    #[test]
    fn preserving_serialize_keeps_non_binder_sections() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject Identifier="test" Version="2.0" Creator="14.3 (3192073)" Device="JiayunMBP" ModID="A1B2C3">
  <Binder>
    <BinderItem UUID="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA" Type="DraftFolder" Created="2024-01-15T10:00:00Z" Modified="2024-01-15T10:00:00Z">
      <Title>Draft</Title>
    </BinderItem>
    <BinderItem UUID="CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC" Type="TrashFolder" Created="2024-01-15T10:00:00Z" Modified="2024-01-15T10:00:00Z">
      <Title>Trash</Title>
    </BinderItem>
  </Binder>
  <Collections>
    <Collection Type="RecentSearch" ID="1234">
      <Title>Search Results</Title>
    </Collection>
  </Collections>
  <PrintSettings>
    <PaperSize>612.0, 792.0</PaperSize>
  </PrintSettings>
  <ProjectProperties>
    <ProjectTitle>Test</ProjectTitle>
  </ProjectProperties>
</ScrivenerProject>"#;

        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();
        let result = serialize_scrivx_preserving(xml, &binder, &trash).unwrap();

        // The non-binder sections must be preserved
        assert!(result.contains("Creator=\"14.3 (3192073)\""), "Creator attribute lost");
        assert!(result.contains("Device=\"JiayunMBP\""), "Device attribute lost");
        assert!(result.contains("ModID=\"A1B2C3\""), "ModID attribute lost");
        assert!(result.contains("<Collections>"), "Collections section lost");
        assert!(result.contains("<PrintSettings>"), "PrintSettings section lost");
        assert!(result.contains("<PaperSize>612.0, 792.0</PaperSize>"), "PaperSize lost");

        // The binder should still be parseable
        let (binder2, _, _) = parse_scrivx_str(&result).unwrap();
        assert_eq!(binder2.root.len(), binder.root.len());
    }

    #[test]
    fn folder_types_preserved_on_parse() {
        use crate::document::FolderType;

        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, _, _) = parse_scrivx_str(xml).unwrap();

        // First folder should be DraftFolder
        if let BinderItem::Folder(f) = &binder.root[0] {
            assert_eq!(f.folder_type, FolderType::DraftFolder);
        } else {
            panic!("Expected DraftFolder");
        }

        // Second folder should be ResearchFolder
        if let BinderItem::Folder(f) = &binder.root[1] {
            assert_eq!(f.folder_type, FolderType::ResearchFolder);
        } else {
            panic!("Expected ResearchFolder");
        }
    }

    #[test]
    fn folder_types_preserved_in_serialized_binder() {
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();

        let result = serialize_scrivx_preserving(xml, &binder, &trash).unwrap();

        // DraftFolder and ResearchFolder types must be in the output
        assert!(result.contains("Type=\"DraftFolder\""), "DraftFolder type lost in serialized binder");
        assert!(result.contains("Type=\"ResearchFolder\""), "ResearchFolder type lost in serialized binder");
        assert!(result.contains("Type=\"TrashFolder\""), "TrashFolder type lost in serialized binder");
    }

    #[test]
    fn trash_uuid_preserved() {
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (_, _, trash) = parse_scrivx_str(xml).unwrap();

        assert_eq!(
            trash.uuid,
            Some(Uuid::parse_str("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC").unwrap())
        );
    }

    #[test]
    fn preserving_serialize_roundtrip_with_fixture() {
        let xml = include_str!("../tests/fixtures/sample.scriv/sample.scrivx");
        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();

        let result = serialize_scrivx_preserving(xml, &binder, &trash).unwrap();
        let (binder2, metadata2, trash2) = parse_scrivx_str(&result).unwrap();

        assert_eq!(binder2.root.len(), binder.root.len());
        assert_eq!(metadata2.title, "Sample Novel");
        assert_eq!(trash2.items.len(), trash.items.len());

        // Verify ProjectProperties is still there
        assert!(result.contains("<ProjectTitle>Sample Novel</ProjectTitle>"));
        assert!(result.contains("<FullName>Test Author</FullName>"));
    }
}
