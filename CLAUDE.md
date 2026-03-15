# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust library (`scrivener` crate) for reading and writing Scrivener 3 `.scriv` project bundles. Parses the `.scrivx` XML file, provides binder navigation, RTF content reading/writing (via `scrivener-rtf`), full-text search, statistics, and trash management.

## Build & Test Commands

```sh
cargo build              # Build the library
cargo test               # Run all tests
cargo test <test_name>   # Run a single test by name
cargo clippy --all-targets -- -D warnings   # Lint (CI runs this)
```

CI runs `cargo clippy --all-targets -- -D warnings` followed by `cargo test` on ubuntu-latest.

## Architecture

### Core Data Flow

`Project::open()` → reads `.scrivx` XML → `scrivx::parse_scrivx_str()` deserializes into raw serde types → converts to domain types (`Binder`, `ProjectMetadata`, `Trash`).

`Project::save()` uses **preserving serialization**: replaces only the `<Binder>` section in the original raw XML, keeping all other XML sections (Collections, PrintSettings, etc.) intact. Falls back to full serialization if no raw XML is stored.

### Module Responsibilities

- **`project.rs`** — `Project` struct: open/save, search (plain text, regex, keyword), trash operations, statistics. Entry point for the public API.
- **`scrivx.rs`** (internal) — XML parsing/serialization. Two-layer type system: `Raw*` serde types for XML ↔ `Domain` types. Contains both `Deserialize` and `Serialize` variants of raw types.
- **`binder.rs`** — `Binder` tree with recursive traversal: find by UUID/title, flatten with paths, move items between folders.
- **`document.rs`** — `Document`, `Folder`, `DocumentContent`, `FolderType`. RTF content read/write via filesystem (`Files/Data/{UUID}/content.rtf`).
- **`trash.rs`** — `Trash` and `TrashedItem`, mirrors binder item types.
- **`search.rs`** / **`statistics.rs`** / **`metadata.rs`** / **`error.rs`** — Supporting types.

### Key Conventions

- UUIDs are stored uppercase in filesystem paths and serialized XML (e.g., `Files/Data/11111111-1111-1111-1111-111111111111/`).
- Scrivener date format: `"%Y-%m-%d %H:%M:%S %z"` (parsed alongside ISO 8601).
- Text search uses char-boundary-safe slicing for multi-byte content.
- Test fixtures live in `tests/fixtures/sample.scriv/`.
