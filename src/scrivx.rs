use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use xmltree::{Element, EmitterConfig, XMLNode};

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
    let keywords = raw
        .metadata
        .as_ref()
        .and_then(|m| m.keywords.as_ref())
        .map(|k| k.keywords.clone())
        .unwrap_or_default();
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

    match raw.item_type.as_str() {
        "Folder" | "DraftFolder" | "ResearchFolder" | "TrashFolder" => {
            let folder_type = match raw.item_type.as_str() {
                "DraftFolder" => FolderType::DraftFolder,
                "ResearchFolder" => FolderType::ResearchFolder,
                "TrashFolder" => FolderType::TrashFolder,
                _ => FolderType::Folder,
            };

            Ok(BinderItem::Folder(Folder {
                uuid,
                title,
                children,
                synopsis: None,
                notes: None,
                keywords,
                content: DocumentContent::new(),
                metadata,
                folder_type,
            }))
        }
        item_type => Ok(BinderItem::Document(Document {
            uuid,
            title,
            children,
            doc_type: item_type.to_string(),
            synopsis: None,
            notes: None,
            keywords,
            content: DocumentContent::new(),
            metadata,
        })),
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
            item_type: doc.doc_type.clone(),
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
            children: if doc.children.is_empty() {
                None
            } else {
                Some(RawChildrenOut {
                    items: doc.children.iter().map(binder_item_to_raw).collect(),
                })
            },
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
                    keywords: if folder.keywords.is_empty() {
                        None
                    } else {
                        Some(RawKeywordsOut {
                            keywords: folder.keywords.clone(),
                        })
                    },
                    custom_metadata: if folder.metadata.custom_metadata.is_empty() {
                        None
                    } else {
                        Some(RawCustomMetaDataOut {
                            items: folder
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
    let mut all_items: Vec<RawBinderItemOut> = binder.root.iter().map(binder_item_to_raw).collect();

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

    let xml = quick_xml::se::to_string(&project).map_err(|e| ScrivenerError::ScrivxParseError {
        message: format!("XML serialization failed: {}", e),
    })?;

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}",
        xml
    ))
}

fn child_element<'a>(nodes: &'a [XMLNode], name: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|node| match node {
        XMLNode::Element(element) if element.name == name => Some(element),
        _ => None,
    })
}

fn set_element_text(element: &mut Element, text: impl Into<String>) {
    element
        .children
        .retain(|node| !matches!(node, XMLNode::Text(_) | XMLNode::CData(_)));
    element.children.insert(0, XMLNode::Text(text.into()));
}

fn rebuild_repeated_text_children(
    mut container: Element,
    child_name: &str,
    values: &[String],
) -> Element {
    let mut existing = container
        .children
        .iter()
        .filter_map(|node| match node {
            XMLNode::Element(element) if element.name == child_name => Some(element.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_iter();
    let mut children = container
        .children
        .into_iter()
        .filter(|node| {
            !matches!(
                node,
                XMLNode::Element(element) if element.name == child_name
            )
        })
        .collect::<Vec<_>>();

    for value in values {
        let mut element = existing.next().unwrap_or_else(|| Element::new(child_name));
        set_element_text(&mut element, value);
        children.push(XMLNode::Element(element));
    }
    container.children = children;
    container
}

fn metadata_to_element(item: &BinderItem, existing: Option<&Element>) -> Element {
    let mut metadata = existing
        .cloned()
        .unwrap_or_else(|| Element::new("MetaData"));
    let old_children = std::mem::take(&mut metadata.children);
    let include_existing = child_element(&old_children, "IncludeInCompile").cloned();
    let keywords_existing = child_element(&old_children, "Keywords").cloned();
    let custom_existing = child_element(&old_children, "CustomMetaData").cloned();

    let mut include = include_existing.unwrap_or_else(|| Element::new("IncludeInCompile"));
    set_element_text(
        &mut include,
        if item.metadata().include_in_compile {
            "Yes"
        } else {
            "No"
        },
    );

    let mut children = vec![XMLNode::Element(include)];
    children.extend(old_children.into_iter().filter(|node| {
        !matches!(
            node,
            XMLNode::Element(element)
                if matches!(
                    element.name.as_str(),
                    "IncludeInCompile" | "Keywords" | "CustomMetaData"
                )
        )
    }));

    if keywords_existing.is_some() || !item.keywords().is_empty() {
        let keywords = rebuild_repeated_text_children(
            keywords_existing.unwrap_or_else(|| Element::new("Keywords")),
            "Keyword",
            item.keywords(),
        );
        if !keywords.attributes.is_empty() || !keywords.children.is_empty() {
            children.push(XMLNode::Element(keywords));
        }
    }

    if custom_existing.is_some() || !item.metadata().custom_metadata.is_empty() {
        let mut custom = custom_existing.unwrap_or_else(|| Element::new("CustomMetaData"));
        let old_items = std::mem::take(&mut custom.children);
        let mut by_field_id = HashMap::new();
        let mut unknown = Vec::new();

        for node in old_items {
            match node {
                XMLNode::Element(element) if element.name == "MetaDataItem" => {
                    let field_id = element
                        .get_child("FieldID")
                        .and_then(Element::get_text)
                        .map(|value| value.into_owned());
                    let has_value = element.get_child("Value").is_some();
                    if let Some(field_id) = field_id.filter(|_| has_value) {
                        by_field_id.insert(field_id, element);
                    } else {
                        unknown.push(XMLNode::Element(element));
                    }
                }
                node => unknown.push(node),
            }
        }

        let mut items = unknown;
        let mut fields = item.metadata().custom_metadata.iter().collect::<Vec<_>>();
        fields.sort_by_key(|(field_id, _)| *field_id);
        for (field_id, value) in fields {
            let mut metadata_item = by_field_id
                .remove(field_id)
                .unwrap_or_else(|| Element::new("MetaDataItem"));
            let item_children = std::mem::take(&mut metadata_item.children);
            let mut field_element = child_element(&item_children, "FieldID")
                .cloned()
                .unwrap_or_else(|| Element::new("FieldID"));
            let mut value_element = child_element(&item_children, "Value")
                .cloned()
                .unwrap_or_else(|| Element::new("Value"));
            set_element_text(&mut field_element, field_id);
            set_element_text(&mut value_element, value);

            metadata_item.children = vec![
                XMLNode::Element(field_element),
                XMLNode::Element(value_element),
            ];
            metadata_item
                .children
                .extend(item_children.into_iter().filter(|node| {
                    !matches!(
                        node,
                        XMLNode::Element(element)
                            if matches!(element.name.as_str(), "FieldID" | "Value")
                    )
                }));
            items.push(XMLNode::Element(metadata_item));
        }
        custom.children = items;
        if !custom.attributes.is_empty() || !custom.children.is_empty() {
            children.push(XMLNode::Element(custom));
        }
    }

    metadata.children = children;
    metadata
}

fn collect_existing_binder_items(element: &Element, items: &mut HashMap<Uuid, Element>) {
    if element.name == "BinderItem" {
        if let Some(uuid) = element
            .attributes
            .get("UUID")
            .and_then(|value| Uuid::parse_str(value).ok())
        {
            items.insert(uuid, element.clone());
        }
    }

    for child in &element.children {
        if let XMLNode::Element(child) = child {
            collect_existing_binder_items(child, items);
        }
    }
}

fn children_to_element(
    existing: Option<&Element>,
    children: &[BinderItem],
    existing_items: &HashMap<Uuid, Element>,
) -> Option<Element> {
    let mut container = existing
        .cloned()
        .unwrap_or_else(|| Element::new("Children"));
    let mut nodes = std::mem::take(&mut container.children)
        .into_iter()
        .filter(|node| {
            !matches!(
                node,
                XMLNode::Element(element) if element.name == "BinderItem"
            )
        })
        .collect::<Vec<_>>();
    nodes.extend(
        children
            .iter()
            .map(|child| XMLNode::Element(binder_item_to_element(child, existing_items))),
    );
    container.children = nodes;

    if children.is_empty() && container.attributes.is_empty() && container.children.is_empty() {
        None
    } else {
        Some(container)
    }
}

fn binder_item_to_element(item: &BinderItem, existing_items: &HashMap<Uuid, Element>) -> Element {
    let mut element = existing_items
        .get(&item.uuid())
        .cloned()
        .unwrap_or_else(|| Element::new("BinderItem"));
    element.name = "BinderItem".to_string();
    element
        .attributes
        .insert("UUID".to_string(), item.uuid().to_string().to_uppercase());
    element
        .attributes
        .insert("Type".to_string(), item.item_type().to_string());
    element.attributes.insert(
        "Created".to_string(),
        format_datetime(&item.metadata().created),
    );
    element.attributes.insert(
        "Modified".to_string(),
        format_datetime(&item.metadata().modified),
    );

    let old_children = std::mem::take(&mut element.children);
    let mut title = child_element(&old_children, "Title")
        .cloned()
        .unwrap_or_else(|| Element::new("Title"));
    set_element_text(&mut title, item.title());
    let metadata_existing = child_element(&old_children, "MetaData");
    let children_existing = child_element(&old_children, "Children").cloned();
    let mut children = vec![
        XMLNode::Element(title),
        XMLNode::Element(metadata_to_element(item, metadata_existing)),
    ];
    children.extend(old_children.into_iter().filter(|node| {
        !matches!(
            node,
            XMLNode::Element(element)
                if matches!(element.name.as_str(), "Title" | "MetaData" | "Children")
        )
    }));

    if let Some(child_container) =
        children_to_element(children_existing.as_ref(), item.children(), existing_items)
    {
        children.push(XMLNode::Element(child_container));
    }
    element.children = children;
    element
}

fn trash_to_element(trash: &Trash, existing_items: &HashMap<Uuid, Element>) -> Element {
    let trash_uuid = trash.uuid.unwrap_or_else(Uuid::nil);
    let mut element = existing_items
        .get(&trash_uuid)
        .cloned()
        .unwrap_or_else(|| Element::new("BinderItem"));
    element.name = "BinderItem".to_string();
    element
        .attributes
        .insert("UUID".to_string(), trash_uuid.to_string().to_uppercase());
    element
        .attributes
        .insert("Type".to_string(), "TrashFolder".to_string());
    element
        .attributes
        .entry("Created".to_string())
        .or_insert_with(|| format_datetime(&Utc::now()));
    element
        .attributes
        .entry("Modified".to_string())
        .or_insert_with(|| format_datetime(&Utc::now()));

    let old_children = std::mem::take(&mut element.children);
    let mut title = child_element(&old_children, "Title")
        .cloned()
        .unwrap_or_else(|| Element::new("Title"));
    set_element_text(&mut title, "Trash");
    let children_existing = child_element(&old_children, "Children").cloned();
    let mut children = old_children
        .into_iter()
        .filter(|node| {
            !matches!(
                node,
                XMLNode::Element(element)
                    if matches!(element.name.as_str(), "Title" | "Children")
            )
        })
        .collect::<Vec<_>>();
    children.insert(0, XMLNode::Element(title));

    let trash_children = trash
        .items
        .iter()
        .map(|item| match item {
            TrashedItem::Document(document) => BinderItem::Document(document.clone()),
            TrashedItem::Folder(folder) => BinderItem::Folder(folder.clone()),
        })
        .collect::<Vec<_>>();
    if let Some(child_container) =
        children_to_element(children_existing.as_ref(), &trash_children, existing_items)
    {
        children.push(XMLNode::Element(child_container));
    }
    element.children = children;
    element
}

/// Serialize the `<Binder>` section while preserving unmodeled XML on existing
/// binder items. Structural changes are applied by UUID, so moves and nested
/// document children retain Scrivener-owned fields such as `TextSettings`.
fn serialize_binder_xml_preserving(
    raw_xml: &str,
    binder: &Binder,
    trash: &Trash,
) -> Result<String> {
    let root =
        Element::parse(raw_xml.as_bytes()).map_err(|error| ScrivenerError::ScrivxParseError {
            message: format!("XML tree parse failed: {}", error),
        })?;
    let mut binder_element =
        root.get_child("Binder")
            .cloned()
            .ok_or_else(|| ScrivenerError::ScrivxParseError {
                message: "Could not find <Binder> element".into(),
            })?;

    let mut existing_items = HashMap::new();
    collect_existing_binder_items(&binder_element, &mut existing_items);

    let preserved_root_nodes = std::mem::take(&mut binder_element.children)
        .into_iter()
        .filter(|node| {
            !matches!(
                node,
                XMLNode::Element(element) if element.name == "BinderItem"
            )
        });
    binder_element.children = preserved_root_nodes
        .chain(
            binder
                .root
                .iter()
                .map(|item| XMLNode::Element(binder_item_to_element(item, &existing_items))),
        )
        .chain(std::iter::once(XMLNode::Element(trash_to_element(
            trash,
            &existing_items,
        ))))
        .collect();

    let mut output = Vec::new();
    binder_element
        .write_with_config(
            &mut output,
            EmitterConfig::new()
                .write_document_declaration(false)
                .perform_indent(true),
        )
        .map_err(|error| ScrivenerError::ScrivxParseError {
            message: format!("Binder XML serialization failed: {}", error),
        })?;
    String::from_utf8(output).map_err(|error| ScrivenerError::ScrivxParseError {
        message: format!("Binder XML was not UTF-8: {}", error),
    })
}

/// Serialize the project by replacing only the `<Binder>...</Binder>` section in the
/// original raw XML, preserving all other elements (Collections, PrintSettings, etc.).
pub(crate) fn serialize_scrivx_preserving(
    raw_xml: &str,
    binder: &Binder,
    trash: &Trash,
) -> Result<String> {
    let binder_xml = serialize_binder_xml_preserving(raw_xml, binder, trash)?;

    // Find the <Binder> ... </Binder> region in the raw XML and replace it.
    // We look for the outermost <Binder> and </Binder> tags.
    let binder_start = raw_xml
        .find("<Binder")
        .ok_or_else(|| ScrivenerError::ScrivxParseError {
            message: "Could not find <Binder> tag in raw XML".into(),
        })?;

    let binder_end_tag = "</Binder>";
    let binder_end =
        raw_xml
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
    fn document_children_and_unknown_types_roundtrip() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject Identifier="test" Version="2.0">
  <Binder>
    <BinderItem UUID="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA" Type="DraftFolder">
      <Title>Draft</Title>
      <Children>
        <BinderItem UUID="11111111-1111-1111-1111-111111111111" Type="Text">
          <Title>Parent Document</Title>
          <Children>
            <BinderItem UUID="22222222-2222-2222-2222-222222222222" Type="Image">
              <Title>Image Child</Title>
            </BinderItem>
            <BinderItem UUID="33333333-3333-3333-3333-333333333333" Type="WebArchive">
              <Title>Web Child</Title>
            </BinderItem>
          </Children>
        </BinderItem>
      </Children>
    </BinderItem>
    <BinderItem UUID="CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC" Type="TrashFolder">
      <Title>Trash</Title>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#;

        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();
        let parent_uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let parent = binder.find_by_uuid(parent_uuid).unwrap();
        assert_eq!(parent.children().len(), 2);
        assert_eq!(parent.children()[0].item_type(), "Image");
        assert_eq!(parent.children()[1].item_type(), "WebArchive");

        let serialized = serialize_scrivx_preserving(xml, &binder, &trash).unwrap();
        assert!(serialized.contains("Type=\"Image\""));
        assert!(serialized.contains("Type=\"WebArchive\""));

        let (reopened, _, _) = parse_scrivx_str(&serialized).unwrap();
        let reopened_parent = reopened.find_by_uuid(parent_uuid).unwrap();
        assert_eq!(reopened_parent.children().len(), 2);
        assert_eq!(
            reopened.flatten()[3].1,
            vec!["Draft", "Parent Document", "Web Child"]
        );
    }

    fn find_xml_item(element: &Element, uuid: Uuid) -> Option<&Element> {
        if element.name == "BinderItem"
            && element
                .attributes
                .get("UUID")
                .is_some_and(|value| Uuid::parse_str(value).ok() == Some(uuid))
        {
            return Some(element);
        }
        element
            .children
            .iter()
            .filter_map(XMLNode::as_element)
            .find_map(|child| find_xml_item(child, uuid))
    }

    #[test]
    fn unmodeled_binder_xml_survives_updates_and_moves() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject Identifier="test" Version="2.0">
  <Binder BinderExtension="keep">
    <BinderItem UUID="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA" Type="DraftFolder">
      <Title>Draft</Title>
      <MetaData><IncludeInCompile>Yes</IncludeInCompile></MetaData>
      <Children SourceExtension="keep">
        <FutureContainer>source extension</FutureContainer>
        <BinderItem UUID="11111111-1111-1111-1111-111111111111" Type="Text" FutureAttribute="keep">
          <Title Language="en">Original</Title>
          <MetaData MetadataExtension="keep">
            <IncludeInCompile IncludeExtension="keep">Yes</IncludeInCompile>
            <LabelID>label-id</LabelID>
            <Keywords KeywordsExtension="keep"><Keyword>original</Keyword><KeywordExtension>keep</KeywordExtension></Keywords>
            <CustomMetaData CustomExtension="keep">
              <MetaDataItem><FieldID>Known</FieldID><Value>old</Value><ItemExtension>keep</ItemExtension></MetaDataItem>
              <MetaDataItem><FieldID>Opaque</FieldID><FutureValue>keep</FutureValue></MetaDataItem>
            </CustomMetaData>
          </MetaData>
          <TextSettings TextExtension="keep">
            <TextSelection>3,4</TextSelection>
            <FutureSetting Enabled="Yes">keep</FutureSetting>
          </TextSettings>
          <FutureItemField Flag="1"><Nested>keep</Nested></FutureItemField>
        </BinderItem>
        <BinderItem UUID="22222222-2222-2222-2222-222222222222" Type="Folder">
          <Title>Target</Title>
          <MetaData><IncludeInCompile>Yes</IncludeInCompile></MetaData>
          <Children TargetExtension="keep"><FutureContainer>target extension</FutureContainer></Children>
        </BinderItem>
      </Children>
    </BinderItem>
    <BinderItem UUID="CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC" Type="TrashFolder">
      <Title>Trash</Title>
      <TextSettings><TextSelection>9,9</TextSelection></TextSettings>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#;

        let (mut binder, _, trash) = parse_scrivx_str(xml).unwrap();
        let document_uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let target_uuid = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let item = binder.find_by_uuid_mut(document_uuid).unwrap();
        *item.title_mut() = "Renamed".to_string();
        item.add_keyword("added");
        item.metadata_mut()
            .custom_metadata
            .insert("Known".to_string(), "updated".to_string());
        binder.move_item(document_uuid, Some(target_uuid)).unwrap();

        let serialized = serialize_scrivx_preserving(xml, &binder, &trash).unwrap();
        let root = Element::parse(serialized.as_bytes()).unwrap();
        let binder_xml = root.get_child("Binder").unwrap();
        assert_eq!(
            binder_xml
                .attributes
                .get("BinderExtension")
                .map(String::as_str),
            Some("keep")
        );

        let document = find_xml_item(binder_xml, document_uuid).unwrap();
        assert_eq!(
            document
                .attributes
                .get("FutureAttribute")
                .map(String::as_str),
            Some("keep")
        );
        let title = document.get_child("Title").unwrap();
        assert_eq!(title.get_text().as_deref(), Some("Renamed"));
        assert_eq!(
            title.attributes.get("Language").map(String::as_str),
            Some("en")
        );

        let metadata = document.get_child("MetaData").unwrap();
        assert_eq!(
            metadata
                .attributes
                .get("MetadataExtension")
                .map(String::as_str),
            Some("keep")
        );
        assert_eq!(
            metadata
                .get_child("LabelID")
                .and_then(Element::get_text)
                .as_deref(),
            Some("label-id")
        );
        let include = metadata.get_child("IncludeInCompile").unwrap();
        assert_eq!(
            include
                .attributes
                .get("IncludeExtension")
                .map(String::as_str),
            Some("keep")
        );

        let keywords = metadata.get_child("Keywords").unwrap();
        assert_eq!(
            keywords
                .attributes
                .get("KeywordsExtension")
                .map(String::as_str),
            Some("keep")
        );
        let keyword_values = keywords
            .children
            .iter()
            .filter_map(XMLNode::as_element)
            .filter(|element| element.name == "Keyword")
            .filter_map(Element::get_text)
            .map(|value| value.into_owned())
            .collect::<Vec<_>>();
        assert_eq!(keyword_values, vec!["original", "added"]);
        assert!(keywords.get_child("KeywordExtension").is_some());

        let custom = metadata.get_child("CustomMetaData").unwrap();
        assert_eq!(
            custom.attributes.get("CustomExtension").map(String::as_str),
            Some("keep")
        );
        assert!(custom.children.iter().any(|node| {
            node.as_element().is_some_and(|element| {
                element.name == "MetaDataItem"
                    && element
                        .get_child("FieldID")
                        .and_then(Element::get_text)
                        .as_deref()
                        == Some("Opaque")
                    && element.get_child("FutureValue").is_some()
            })
        }));
        assert!(custom.children.iter().any(|node| {
            node.as_element().is_some_and(|element| {
                element.name == "MetaDataItem"
                    && element
                        .get_child("FieldID")
                        .and_then(Element::get_text)
                        .as_deref()
                        == Some("Known")
                    && element
                        .get_child("Value")
                        .and_then(Element::get_text)
                        .as_deref()
                        == Some("updated")
                    && element.get_child("ItemExtension").is_some()
            })
        }));

        let text_settings = document.get_child("TextSettings").unwrap();
        assert_eq!(
            text_settings
                .attributes
                .get("TextExtension")
                .map(String::as_str),
            Some("keep")
        );
        assert_eq!(
            text_settings
                .get_child("TextSelection")
                .and_then(Element::get_text)
                .as_deref(),
            Some("3,4")
        );
        assert!(text_settings.get_child("FutureSetting").is_some());
        assert!(document.get_child("FutureItemField").is_some());

        let target = find_xml_item(binder_xml, target_uuid).unwrap();
        let target_children = target.get_child("Children").unwrap();
        assert_eq!(
            target_children
                .attributes
                .get("TargetExtension")
                .map(String::as_str),
            Some("keep")
        );
        assert!(target_children.get_child("FutureContainer").is_some());
        assert!(find_xml_item(target_children, document_uuid).is_some());

        let trash_uuid = Uuid::parse_str("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC").unwrap();
        let trash_xml = find_xml_item(binder_xml, trash_uuid).unwrap();
        assert_eq!(
            trash_xml
                .get_child("TextSettings")
                .and_then(|settings| settings.get_child("TextSelection"))
                .and_then(Element::get_text)
                .as_deref(),
            Some("9,9")
        );
    }

    #[test]
    fn folder_keywords_and_custom_metadata_roundtrip() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject Identifier="test" Version="2.0">
  <Binder>
    <BinderItem UUID="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA" Type="DraftFolder">
      <Title>Draft</Title>
      <MetaData>
        <IncludeInCompile>Yes</IncludeInCompile>
        <Keywords><Keyword>folder-keyword</Keyword></Keywords>
        <CustomMetaData>
          <MetaDataItem><FieldID>Status</FieldID><Value>Drafting</Value></MetaDataItem>
        </CustomMetaData>
      </MetaData>
    </BinderItem>
    <BinderItem UUID="CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC" Type="TrashFolder">
      <Title>Trash</Title>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#;

        let (binder, _, trash) = parse_scrivx_str(xml).unwrap();
        let folder = &binder.root[0];
        assert_eq!(folder.keywords(), &["folder-keyword"]);
        assert_eq!(
            folder.metadata().custom_metadata.get("Status"),
            Some(&"Drafting".to_string())
        );

        let serialized = serialize_scrivx_preserving(xml, &binder, &trash).unwrap();
        let (reopened, _, _) = parse_scrivx_str(&serialized).unwrap();
        let folder = &reopened.root[0];
        assert_eq!(folder.keywords(), &["folder-keyword"]);
        assert_eq!(
            folder.metadata().custom_metadata.get("Status"),
            Some(&"Drafting".to_string())
        );
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
        assert!(
            result.contains("Creator=\"14.3 (3192073)\""),
            "Creator attribute lost"
        );
        assert!(
            result.contains("Device=\"JiayunMBP\""),
            "Device attribute lost"
        );
        assert!(result.contains("ModID=\"A1B2C3\""), "ModID attribute lost");
        assert!(result.contains("<Collections>"), "Collections section lost");
        assert!(
            result.contains("<PrintSettings>"),
            "PrintSettings section lost"
        );
        assert!(
            result.contains("<PaperSize>612.0, 792.0</PaperSize>"),
            "PaperSize lost"
        );

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
        assert!(
            result.contains("Type=\"DraftFolder\""),
            "DraftFolder type lost in serialized binder"
        );
        assert!(
            result.contains("Type=\"ResearchFolder\""),
            "ResearchFolder type lost in serialized binder"
        );
        assert!(
            result.contains("Type=\"TrashFolder\""),
            "TrashFolder type lost in serialized binder"
        );
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
