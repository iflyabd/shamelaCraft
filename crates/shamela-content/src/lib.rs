//! Book content layer for ShamelaCraft.
//!
//! Three pieces, all working offline on files the user already has:
//!
//! * [`book`] — reader for per-book SQLite databases with the `Page` /
//!   `Title` schema (`id, content, part, page, number`, patch-aware).
//! * [`decoder`] — port of the Shamela substitution-cipher decoder for the
//!   desktop `S1.db` / `S2.db` service databases.
//! * [`html`] — behavior-compatible port of the Shamela HTML content
//!   pipeline (title spans, hadeeth tags, footnotes, page markers).
//! * [`roots`] — Arabic root lookup over `S2.db`-shaped databases
//!   (Windows-1256 blobs).
//!
//! ## Attribution
//!
//! The decoding tables, the `Page`/`Title` schema, and the pipeline semantics
//! were documented by the MIT-licensed project
//! [ragaeeb/shamela](https://github.com/ragaeeb/shamela) (scripts/
//! reverse-engineering work and `src/` content utilities). They are
//! re-implemented here in Rust; book texts themselves are never bundled.

pub mod book;
pub mod cache;
pub mod decoder;
pub mod html;
pub mod notes;
pub mod quran;
pub mod roots;
