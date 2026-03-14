use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::binder::{Binder, BinderItem};
use crate::document::{Document, DocumentContent, Folder};
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
    s.and_then(|s| s.parse::<DateTime<Utc>>().ok())
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
        "Text" => {
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
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
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
                item_type: "Folder".to_string(),
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

    // Add trash folder
    if !trash.items.is_empty() {
        let trash_children: Vec<RawBinderItemOut> =
            trash.items.iter().map(trashed_item_to_raw).collect();

        all_items.push(RawBinderItemOut {
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            item_type: "TrashFolder".to_string(),
            created: format_datetime(&Utc::now()),
            modified: format_datetime(&Utc::now()),
            title: "Trash".to_string(),
            metadata: None,
            children: Some(RawChildrenOut {
                items: trash_children,
            }),
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
}
