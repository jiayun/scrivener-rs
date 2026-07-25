# Project & Binder Operations Specification

## Project Operations

### `Project::open` Algorithm

Opens a `.scriv` bundle and returns a fully-constructed `Project`.

```
1. Validate path:
   - path must exist and be a directory
   - path should end with `.scriv` (warn if not)
2. Find .scrivx file:
   - Look for exactly one `.scrivx` file in the root of the bundle
   - Error if zero or multiple found
3. Parse .scrivx:
   - Read file contents
   - Deserialize with quick-xml + serde into RawScrivenerProject
   - Convert raw types to domain types (Binder, ProjectMetadata, Trash)
4. Resolve content paths lazily:
   - Documents and folders derive content paths from their UUID when read
5. Return Project { path, binder, metadata, trash }
```

```rust
impl Project {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Project> {
        let path = path.as_ref().to_path_buf();

        // Step 1: validate
        if !path.exists() || !path.is_dir() {
            return Err(ScrivenerError::ProjectNotFound {
                path: path.clone(),
            });
        }

        // Step 2: find .scrivx
        let scrivx_path = find_scrivx_file(&path)?;

        // Step 3: parse
        let xml_content = std::fs::read_to_string(&scrivx_path)?;
        let (binder, metadata, trash) = parse_scrivx_str(&xml_content)?;

        // Step 4: resolve paths (done lazily, not eagerly)
        Ok(Project { path, binder, metadata, trash })
    }
}

fn find_scrivx_file(project_dir: &Path) -> Result<PathBuf> {
    let mut scrivx_files: Vec<PathBuf> = std::fs::read_dir(project_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "scrivx"))
        .collect();

    match scrivx_files.len() {
        0 => Err(ScrivenerError::InvalidProject {
            message: "No .scrivx file found in project directory".into(),
        }),
        1 => Ok(scrivx_files.remove(0)),
        _ => Err(ScrivenerError::InvalidProject {
            message: "Multiple .scrivx files found".into(),
        }),
    }
}
```

### `Project::save` Algorithm

Saves the current project state back to disk.

```
1. Serialize binder, metadata, and trash back to XML
2. Write the .scrivx file (atomic: write to temp file, then rename)
3. For any documents with modified content:
   - Ensure Files/Data/{UUID}/ directory exists
   - Write content.rtf
   - Write notes.rtf (if modified)
   - Write synopsis.txt (if modified)
```

```rust
impl Project {
    pub fn save(&self) -> Result<()> {
        let scrivx_path = find_scrivx_file(&self.path)?;
        let xml = serialize_scrivx(&self.binder, &self.metadata, &self.trash)?;

        // Atomic write: temp file + rename
        let temp_path = scrivx_path.with_extension("scrivx.tmp");
        std::fs::write(&temp_path, &xml)?;
        std::fs::rename(&temp_path, &scrivx_path)?;

        Ok(())
    }

    pub fn save_as<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Copy entire .scriv bundle to new location, then save
        let dest = path.as_ref();
        copy_dir_recursive(&self.path, dest)?;
        let mut project = self.clone();
        project.path = dest.to_path_buf();
        project.save()
    }
}
```

---

## Binder Operations

### `find_by_uuid` — Recursive Tree Walk

Searches the binder tree for an item with the given UUID.

```rust
impl Binder {
    pub fn find_by_uuid(&self, uuid: Uuid) -> Option<&BinderItem> {
        for item in &self.root {
            if let Some(found) = find_by_uuid_recursive(item, uuid) {
                return Some(found);
            }
        }
        None
    }
}

fn find_by_uuid_recursive(item: &BinderItem, target: Uuid) -> Option<&BinderItem> {
    match item {
        BinderItem::Document(doc) => {
            if doc.uuid == target { Some(item) } else { None }
        }
        BinderItem::Folder(folder) => {
            if folder.uuid == target {
                return Some(item);
            }
            for child in &folder.children {
                if let Some(found) = find_by_uuid_recursive(child, target) {
                    return Some(found);
                }
            }
            None
        }
    }
}
```

### `find_by_title` — Title Search

Returns all items whose title matches the query (case-insensitive).

```rust
impl Binder {
    pub fn find_by_title(&self, title: &str) -> Vec<&BinderItem> {
        let mut results = Vec::new();
        let lower_title = title.to_lowercase();
        for item in &self.root {
            find_by_title_recursive(item, &lower_title, &mut results);
        }
        results
    }
}

fn find_by_title_recursive<'a>(
    item: &'a BinderItem,
    target: &str,
    results: &mut Vec<&'a BinderItem>,
) {
    let item_title = match item {
        BinderItem::Document(doc) => &doc.title,
        BinderItem::Folder(folder) => &folder.title,
    };

    if item_title.to_lowercase().contains(target) {
        results.push(item);
    }

    if let BinderItem::Folder(folder) = item {
        for child in &folder.children {
            find_by_title_recursive(child, target, results);
        }
    }
}
```

### `flatten` — Collect All Items with Paths

Returns a flat list of all binder items with their "breadcrumb" path.

```rust
impl Binder {
    /// Returns all items with their path (e.g., ["Draft", "Chapter 1", "Scene 1"]).
    pub fn flatten(&self) -> Vec<(&BinderItem, Vec<String>)> {
        let mut results = Vec::new();
        for item in &self.root {
            flatten_recursive(item, &[], &mut results);
        }
        results
    }
}

fn flatten_recursive<'a>(
    item: &'a BinderItem,
    parent_path: &[String],
    results: &mut Vec<(&'a BinderItem, Vec<String>)>,
) {
    let title = match item {
        BinderItem::Document(doc) => &doc.title,
        BinderItem::Folder(folder) => &folder.title,
    };

    let mut path = parent_path.to_vec();
    path.push(title.clone());
    results.push((item, path.clone()));

    if let BinderItem::Folder(folder) = item {
        for child in &folder.children {
            flatten_recursive(child, &path, results);
        }
    }
}
```

### `move_item` — Re-parent with Validation

Moves a binder item to a new parent item (or to root if `new_parent` is
`None`). Scrivener allows documents as well as folders to contain children.

```rust
impl Binder {
    pub fn move_item(&mut self, uuid: Uuid, new_parent: Option<Uuid>) -> Result<()> {
        // Step 1: Validate the item exists
        if self.find_by_uuid(uuid).is_none() {
            return Err(ScrivenerError::DocumentNotFound { uuid });
        }

        // Step 2: If new_parent is specified, validate it exists
        if let Some(parent_uuid) = new_parent {
            if self.find_by_uuid(parent_uuid).is_none() {
                return Err(ScrivenerError::DocumentNotFound { uuid: parent_uuid });
            }
            // Step 3: Prevent moving an item into itself or its descendant
            if is_descendant(self, uuid, parent_uuid) {
                return Err(ScrivenerError::InvalidProject {
                    message: "Cannot move item into itself or its own descendant".into(),
                });
            }
        }

        // Step 4: Remove item from current location
        let item = remove_item_recursive(&mut self.root, uuid)
            .expect("item existence already validated");

        // Step 5: Insert into new parent
        match new_parent {
            None => self.root.push(item),
            Some(parent_uuid) => {
                self.find_by_uuid_mut(parent_uuid)
                    .expect("parent existence already validated")
                    .children_mut()
                    .push(item);
            }
        }

        Ok(())
    }
}
```

---

## Document Operations

### `read_content` — Load RTF Content

Reads the document's RTF file from disk and extracts plain text.

```rust
impl Document {
    pub fn read_content(&self, project_path: &Path) -> Result<DocumentContent> {
        let rtf_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string())
            .join("content.rtf");

        if !rtf_path.exists() {
            return Ok(DocumentContent {
                rtf_path: rtf_path.clone(),
                plain_text: Some(String::new()),
                formatted: None,
            });
        }

        // Parse RTF using scrivener-rtf
        let rtf_doc = scrivener_rtf::parse_file(&rtf_path)
            .map_err(|e| ScrivenerError::ContentError {
                message: format!("Failed to parse RTF for {}: {}", self.uuid, e),
            })?;

        // Extract plain text from AST
        let plain_text = extract_plain_text(&rtf_doc);
        let word_count = count_words(&plain_text);
        let char_count = plain_text.chars().count();

        Ok(DocumentContent {
            rtf_path,
            plain_text: Some(plain_text.clone()),
            formatted: Some(FormattedContent {
                text: plain_text,
                word_count,
                character_count: char_count,
            }),
        })
    }
}
```

### `write_content` — Generate and Save RTF

Wraps plain text in a minimal RTF document and writes to disk.

```rust
impl Document {
    pub fn write_content(&mut self, project_path: &Path, content: &str) -> Result<()> {
        let dir_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string());

        // Ensure directory exists
        std::fs::create_dir_all(&dir_path)?;

        let rtf_path = dir_path.join("content.rtf");

        // Generate RTF wrapping the plain text
        let rtf = generate_rtf_from_text(content);
        std::fs::write(&rtf_path, rtf)?;

        // Update cached content
        self.content.plain_text = Some(content.to_string());

        Ok(())
    }
}

fn generate_rtf_from_text(text: &str) -> String {
    // Minimal RTF wrapper using scrivener-rtf types
    // {\rtf1\ansi\deff0{\fonttbl{\f0\fnil Helvetica;}}
    //  \pard\f0\fs24 {text}\par}
    todo!("Use scrivener-rtf Document builder")
}
```

### Synopsis and Notes

```rust
impl Document {
    pub fn update_synopsis(&mut self, project_path: &Path, synopsis: &str) -> Result<()> {
        let synopsis_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string())
            .join("synopsis.txt");

        std::fs::create_dir_all(synopsis_path.parent().unwrap())?;
        std::fs::write(&synopsis_path, synopsis)?;
        self.synopsis = Some(synopsis.to_string());
        Ok(())
    }

    pub fn update_notes(&mut self, project_path: &Path, notes: &str) -> Result<()> {
        let notes_path = project_path
            .join("Files")
            .join("Data")
            .join(self.uuid.to_string())
            .join("notes.rtf");

        std::fs::create_dir_all(notes_path.parent().unwrap())?;
        let rtf = generate_rtf_from_text(notes);
        std::fs::write(&notes_path, rtf)?;
        self.notes = Some(notes.to_string());
        Ok(())
    }
}
```

### Keyword Management

```rust
impl Document {
    pub fn add_keyword(&mut self, keyword: &str) {
        let kw = keyword.to_string();
        if !self.keywords.contains(&kw) {
            self.keywords.push(kw);
        }
    }

    pub fn remove_keyword(&mut self, keyword: &str) {
        self.keywords.retain(|k| k != keyword);
    }
}
```

---

## Search Implementation

### Plain Text Search

```rust
impl Project {
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();

        for (item, _path) in self.binder.flatten() {
            if let BinderItem::Document(doc) = item {
                let content = match doc.read_content(&self.path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if let Some(text) = &content.plain_text {
                    let lower_text = text.to_lowercase();
                    let matches: Vec<Match> = lower_text
                        .match_indices(&lower_query)
                        .map(|(pos, _)| {
                            let start = pos.saturating_sub(40);
                            let end = (pos + query.len() + 40).min(text.len());
                            Match {
                                context: text[start..end].to_string(),
                                position: (pos, pos + query.len()),
                            }
                        })
                        .collect();

                    if !matches.is_empty() {
                        results.push(SearchResult {
                            document_uuid: doc.uuid,
                            document_title: doc.title.clone(),
                            matches,
                        });
                    }
                }
            }
        }

        results
    }
}
```

### Regex Search

```rust
impl Project {
    pub fn search_regex(&self, pattern: &str) -> Result<Vec<SearchResult>> {
        let re = regex::Regex::new(pattern)?;
        let mut results = Vec::new();

        for (item, _path) in self.binder.flatten() {
            if let BinderItem::Document(doc) = item {
                let content = match doc.read_content(&self.path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if let Some(text) = &content.plain_text {
                    let matches: Vec<Match> = re.find_iter(text)
                        .map(|m| {
                            let start = m.start().saturating_sub(40);
                            let end = (m.end() + 40).min(text.len());
                            Match {
                                context: text[start..end].to_string(),
                                position: (m.start(), m.end()),
                            }
                        })
                        .collect();

                    if !matches.is_empty() {
                        results.push(SearchResult {
                            document_uuid: doc.uuid,
                            document_title: doc.title.clone(),
                            matches,
                        });
                    }
                }
            }
        }

        Ok(results)
    }
}
```

### Keyword Search

```rust
impl Project {
    pub fn search_by_keyword(&self, keyword: &str) -> Vec<&Document> {
        let lower_kw = keyword.to_lowercase();
        let mut results = Vec::new();

        for (item, _) in self.binder.flatten() {
            if let BinderItem::Document(doc) = item {
                if doc.keywords.iter().any(|k| k.to_lowercase() == lower_kw) {
                    results.push(doc);
                }
            }
        }

        results
    }
}
```

---

## Trash Operations

### List Trash

```rust
impl Project {
    /// Returns the current trash contents.
    /// The trash is parsed from the TrashFolder in .scrivx during Project::open.
    pub fn list_trash(&self) -> &Trash {
        &self.trash
    }
}
```

### Recover from Trash

```rust
impl Project {
    /// Moves an item from trash back to the root of the binder.
    pub fn recover_from_trash(&mut self, uuid: Uuid) -> Result<()> {
        let index = self.trash.items.iter().position(|item| {
            match item {
                TrashedItem::Document(doc) => doc.uuid == uuid,
                TrashedItem::Folder(folder) => folder.uuid == uuid,
            }
        }).ok_or(ScrivenerError::DocumentNotFound { uuid })?;

        let trashed_item = self.trash.items.remove(index);

        // Convert TrashedItem back to BinderItem and add to root
        let binder_item = match trashed_item {
            TrashedItem::Document(doc) => BinderItem::Document(doc),
            TrashedItem::Folder(folder) => BinderItem::Folder(folder),
        };

        self.binder.root.push(binder_item);
        Ok(())
    }
}
```

### Empty Trash

```rust
impl Project {
    /// Permanently removes all trashed items (deletes files from disk).
    pub fn empty_trash(&mut self) -> Result<()> {
        for item in &self.trash.items {
            let uuid = match item {
                TrashedItem::Document(doc) => doc.uuid,
                TrashedItem::Folder(folder) => folder.uuid,
            };

            let data_dir = self.path
                .join("Files")
                .join("Data")
                .join(uuid.to_string());

            if data_dir.exists() {
                std::fs::remove_dir_all(&data_dir)?;
            }
        }

        self.trash.items.clear();
        Ok(())
    }
}
```

---

## Statistics Collection

```rust
impl Project {
    pub fn statistics(&self) -> ProjectStatistics {
        let mut stats = ProjectStatistics {
            total_documents: 0,
            total_folders: 0,
            total_words: 0,
            total_characters: 0,
            words_by_document: HashMap::new(),
        };

        for (item, _) in self.binder.flatten() {
            match item {
                BinderItem::Document(doc) => {
                    stats.total_documents += 1;

                    if let Ok(content) = doc.read_content(&self.path) {
                        if let Some(formatted) = &content.formatted {
                            stats.total_words += formatted.word_count;
                            stats.total_characters += formatted.character_count;
                            stats.words_by_document.insert(doc.uuid, formatted.word_count);
                        }
                    }
                }
                BinderItem::Folder(_) => {
                    stats.total_folders += 1;
                }
            }
        }

        stats
    }
}
```

### Word Counting

```rust
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn extract_plain_text(doc: &scrivener_rtf::Document) -> String {
    let mut text = String::new();
    for group in &doc.groups {
        extract_text_from_group(group, &mut text);
    }
    text
}

fn extract_text_from_group(group: &scrivener_rtf::Group, text: &mut String) {
    if group.is_destination {
        return; // Skip destination groups (metadata, not content)
    }

    for content in &group.content {
        match content {
            scrivener_rtf::Content::Text(s) => text.push_str(s),
            scrivener_rtf::Content::ControlWord(name, _) if name == "par" => {
                text.push('\n');
            }
            scrivener_rtf::Content::Group(sub) => {
                extract_text_from_group(sub, text);
            }
            _ => {}
        }
    }
}
```
