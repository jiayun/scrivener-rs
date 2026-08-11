# scrivener

[![Crates.io](https://img.shields.io/crates/v/scrivener.svg)](https://crates.io/crates/scrivener)
[![docs.rs](https://img.shields.io/docsrs/scrivener)](https://docs.rs/scrivener)
[![CI](https://github.com/jiayun/scrivener-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/jiayun/scrivener-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A Rust library for reading and writing [Scrivener 3](https://www.literatureandlatte.com/scrivener/overview) `.scriv` projects.

## Features

- **Open & save** — load `.scriv` bundles, modify them, and write changes back (atomic save via temp file)
- **Binder navigation** — traverse children of any binder item, find items by UUID or title, flatten the tree with paths
- **Move items** — reorganize documents and folders under either document or folder parents with cycle protection
- **Lossless binder round-tripping** — preserve document children, original item types, and unmodeled Scrivener XML such as `TextSettings`
- **Compile-state fidelity** — treat Scrivener's omitted `IncludeInCompile` element as excluded and preserve the omitted form on save
- **RTF content** — read and write document or folder content, notes, and synopses with plain text extraction (via [`scrivener-rtf`](https://crates.io/crates/scrivener-rtf))
- **Full-text search** — plain text search, regex search, and keyword-based filtering
- **Statistics** — word counts, character counts, per-document breakdowns
- **Trash management** — list, recover, and permanently delete trashed items

## Quick Start

```sh
cargo add scrivener
```

```rust
use scrivener::Project;

fn main() -> scrivener::Result<()> {
    // Open a Scrivener project
    let mut project = Project::open("my-novel.scriv")?;

    // Navigate the binder
    for (item, path) in project.binder.flatten() {
        println!("{}", path.join(" / "));
    }

    // Find a document by title
    let docs = project.binder.find_by_title("Chapter One");

    // Every binder item, including folders, can have body text.
    if let Some(item) = docs.first() {
        let content = item.read_content(&project.path)?;
        if let Some(text) = &content.plain_text {
            println!("{text}");
        }
    }

    // Search across all documents
    let results = project.search("important phrase");
    for result in &results {
        println!("{}: {} matches", result.document_title, result.matches.len());
    }

    // Project statistics
    let stats = project.statistics();
    println!("Documents: {}, Words: {}", stats.total_documents, stats.total_words);

    // Save changes
    project.save()?;

    Ok(())
}
```

## Upgrading to 0.2

Scrivener permits text-like documents to contain binder children. Version 0.2
models that structure explicitly and preserves the original XML `Type` for
non-folder items. Starting with 0.2.1, saves also merge managed binder changes
into the original XML by UUID, preserving unmodeled Scrivener-owned fields such
as `TextSettings` and extension attributes. As a result, the public `Document`
and `Folder` structs gained fields. Code that constructs either type should use
`..Default::default()` so it continues to compile as the model evolves.

Version 0.2.2 recognizes Scrivener 3's normalized compile-state convention:
an omitted `IncludeInCompile` element is excluded rather than included.

## License

MIT
