//! Importers for ShamelaCraft.
//!
//! * [`bok`] — legacy `.bok` (MS Access/Jet MDB) books via pure-Rust
//!   [`jetdb`](https://docs.rs/jetdb).
//! * [`archive`] — keyless book acquisition from public archive.org items.
//! * [`convert`] — normalization of any source into the `Page`/`Title`
//!   SQLite format that [`shamela_content::book::BookDb`] reads.
//!
//! Schema facts (table/column names) were documented by
//! [shaybix/Raazi](https://github.com/shaybix/Raazi) (`shamela/extract.py`)
//! and the local-BOK workflow of
//! [yshalsager/shamela-epub-exporter](https://github.com/yshalsager/shamela-epub-exporter).
//! Only facts are used here; all code is original.

pub mod archive;
pub mod bok;
pub mod convert;

use std::fmt::{Display, Formatter};

/// Importer error with a human-readable message.
#[derive(Debug)]
pub struct ImportError(pub String);

impl Display for ImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ImportError {}

/// Crate result type.
pub type Result<T> = std::result::Result<T, ImportError>;

pub(crate) fn err(message: impl Into<String>) -> ImportError {
    ImportError(message.into())
}
