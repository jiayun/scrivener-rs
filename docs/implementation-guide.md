# Implementation Guide

## Cargo.toml

```toml
[package]
name = "scrivener"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Rust library for reading and writing Scrivener 3 projects"
repository = "https://github.com/jiayun/scrivener-rs"
keywords = ["scrivener", "writing", "project", "xml"]
categories = ["parser-implementations", "text-processing"]

[dependencies]
scrivener-rtf = "0.1"
quick-xml = { version = "0.37", features = ["serialize"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
uuid = { version = "1.12", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
walkdir = "2.4"
regex = "1.0"

[dev-dependencies]
tempfile = "3.8"
pretty_assertions = "1.4"
```

## Implementation Order

Each step builds on the previous. Run `cargo check` after each step.

### Step 1: Project Scaffolding (~30 min)

- Create `Cargo.toml` as above
- Create `src/lib.rs` with module declarations (all modules as empty stubs)
- Create all module files: `error.rs`, `project.rs`, `binder.rs`, `document.rs`, `scrivx.rs`, `metadata.rs`, `search.rs`, `trash.rs`, `statistics.rs`
- Run `cargo check` to verify compilation

**Expected output**: Project compiles with empty modules.

### Step 2: Error Types (~30 min)

- Implement `error.rs` with all `ScrivenerError` variants
- Define `pub type Result<T>`
- Use `String` placeholders for forward references if needed

**Expected output**: `cargo check` passes.

### Step 3: Core Types (~1 hour)

- Implement `metadata.rs`: `ProjectMetadata`, `DocumentMetadata`
- Implement `document.rs`: `Document`, `Folder`, `DocumentContent`, `FormattedContent`
- Implement `binder.rs`: `Binder`, `BinderItem` enum (struct definitions only, no methods yet)
- Implement `trash.rs`: `Trash`, `TrashedItem` enum (struct only)
- Implement `search.rs`: `SearchResult`, `Match` (struct only)
- Implement `statistics.rs`: `ProjectStatistics` (struct only)
- Wire up `project.rs`: `Project` struct definition

**Expected output**: All types compile and can be used together.

### Step 4: .scrivx XML Serde Structs (~1 day)

- Implement raw XML types in `scrivx.rs`: `RawScrivenerProject`, `RawBinder`, `RawBinderItem`, `RawMetaData`, etc.
- Add `#[derive(Deserialize)]` with `#[serde(rename = "...")]` attributes
- Write a basic test: deserialize a minimal `.scrivx` XML string
- Verify field mapping with a real Scrivener project's `.scrivx`

**Expected output**: Raw XML structs deserialize from `.scrivx` XML. Basic deserialization test passes.

### Step 5: .scrivx Parser — Raw → Domain Conversion (~1-2 days)

- Implement `TryFrom<RawBinderItem> for BinderItem`
- Implement `From<RawProjectProperties> for ProjectMetadata`
- Implement `parse_scrivx()` public function: read file → deserialize → convert
- Handle the TrashFolder type separately → `Trash`
- Write tests comparing parsed output against known `.scrivx` content

**Expected output**: Full `.scrivx` parsing pipeline works. Can parse a real Scrivener project's index.

### Step 6: Project Open/Load (~1 day)

- Implement `Project::open()`:
  1. Validate `.scriv` directory exists
  2. Find `.scrivx` file in the directory
  3. Call `parse_scrivx()`
  4. Construct and return `Project`
- Implement `find_scrivx_file()` helper
- Write integration test with a fixture `.scriv` project

**Expected output**: `Project::open("tests/fixtures/sample.scriv")` returns a valid `Project`.

### Step 7: Binder Operations (~1 day)

- Implement `Binder::find_by_uuid()` — recursive tree walk
- Implement `Binder::find_by_title()` — case-insensitive search
- Implement `Binder::flatten()` — collect items with breadcrumb paths
- Implement `Binder::move_item()` — re-parent with cycle detection
- Write unit tests for each operation

**Expected output**: All binder traversal and mutation operations work correctly.

### Step 8: Document Content Read/Write (~1 day)

- Implement `Document::read_content()` — load RTF via `scrivener-rtf`, extract plain text
- Implement `extract_plain_text()` — walk RTF AST to collect text content
- Implement `count_words()` — whitespace-based word counting
- Implement `Document::write_content()` — generate minimal RTF, write to disk
- Implement `Document::update_synopsis()` — write synopsis.txt
- Implement `Document::update_notes()` — generate RTF for notes, write
- Implement keyword management (`add_keyword`, `remove_keyword`)
- Write tests using temp directories for write operations

**Expected output**: Can read existing content and write new content. Round-trip: write → read → verify.

### Step 9: Search (~1 day)

- Implement `Project::search()` — plain text search across all documents
- Implement `Project::search_regex()` — regex-based search
- Implement `Project::search_by_keyword()` — keyword filter
- Context extraction: 40 chars before/after match
- Write tests with known content and expected match positions

**Expected output**: Search returns correct results with context snippets.

### Step 10: Trash Management (~0.5 day)

- Implement `Project::list_trash()` — return trash contents
- Implement `Project::recover_from_trash()` — move item back to binder root
- Implement `Project::empty_trash()` — delete files and clear trash list
- Write tests using temp directories

**Expected output**: Trash operations work. Recover restores item to binder, empty deletes files.

### Step 11: Project Save/Write-back (~1 day)

- Implement domain → raw XML conversion (reverse of step 5)
- Implement `serialize_scrivx()` — generate XML string from domain types
- Implement `Project::save()` — atomic write (temp file + rename)
- Implement `Project::save_as()` — copy bundle + save
- Round-trip test: open → modify → save → re-open → verify
- Preserve unrecognized XML elements through the round-trip

**Expected output**: Modified projects save correctly. Round-trip produces equivalent `.scrivx`.

### Step 12: Statistics & Polish (~0.5 day)

- Implement `Project::statistics()` — aggregate word/character counts
- Add doc comments on all public items
- Wire up `lib.rs` re-exports
- Update README with usage examples
- Final `cargo clippy` and `cargo test` pass

**Expected output**: Complete, documented public API. All tests pass.

## Known Challenges & Solutions

### 1. XML Namespace Handling

**Problem**: Some Scrivener versions include XML namespace declarations or processing instructions that `quick-xml` + serde may not handle by default.

**Solution**: Use `quick-xml`'s `Config` to configure namespace handling. Strip or ignore namespace prefixes during deserialization. Test against real `.scrivx` files from different Scrivener versions (macOS vs Windows).

### 2. Scrivener Version Compatibility

**Problem**: Scrivener 3 has evolved its `.scrivx` format across minor versions. Older projects may have slightly different element structures.

**Solution**: Make all fields optional with sensible defaults. Use `#[serde(default)]` extensively. Do NOT use `#[serde(deny_unknown_fields)]`. Log warnings for unknown elements rather than erroring. Only error on truly structural incompatibilities (e.g., Version="1.0" which is Scrivener 2).

### 3. UUID Path Mapping

**Problem**: UUIDs in `.scrivx` must exactly match directory names under `Files/Data/`. Case sensitivity and formatting (hyphenated vs non-hyphenated) matter.

**Solution**: Always use `Uuid::to_string()` (lowercase hyphenated) for path construction. Scrivener uses uppercase hyphenated UUIDs in XML, but the filesystem paths use the same format. On macOS (case-insensitive HFS+/APFS by default) this is not an issue, but on Linux it could be — normalize UUID formatting to match Scrivener's convention.

### 4. RTF ↔ Plain Text Conversion

**Problem**: Extracting plain text from RTF requires walking the AST and understanding which control words represent content vs formatting. Generating RTF from plain text requires wrapping in a valid document structure.

**Solution**: The `extract_plain_text()` function walks the AST, collecting `Content::Text` nodes and converting `\par` to newlines. Destination groups (metadata) are skipped. For generation, use a minimal RTF template with a single font and default formatting.

### 5. Concurrent File Access Safety

**Problem**: Scrivener may have the project open while our library reads/writes files. This could cause data corruption or read inconsistency.

**Solution**: For v0.1.0, document that the project should not be open in Scrivener during write operations. For reads, files are read atomically (single `fs::read_to_string`). Future versions could implement file locking or check for Scrivener's lock files (`.lock` files in the `.scriv` bundle).

### 6. Large Project Performance

**Problem**: A Scrivener project with thousands of documents could be slow to search if every document's RTF is loaded and parsed.

**Solution**: Lazy content loading is already the default — `Project::open()` only parses the `.scrivx`. Search operations load content on-demand. Future optimization: parallel content loading with `rayon`, content caching, or an index file.

### 7. Preserving Unknown XML Elements

**Problem**: When round-tripping a `.scrivx` file, we must preserve XML elements we don't parse (Collections, LabelSettings, StatusSettings, etc.) to avoid data loss.

**Solution**: Store unparsed sections as raw XML strings during deserialization. Re-insert them during serialization. This requires a hybrid approach: serde for known elements, raw XML passthrough for unknown ones. This is a known complexity with `quick-xml` + serde and may require custom `Deserialize` implementations.
