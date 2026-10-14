//! Keyless book acquisition from public archive.org items.
//!
//! Many archive.org items mirror full Shamela libraries (notably `.bok`
//! collections). Everything here uses the public metadata API — no keys.
//!
//! ```text
//! GET https://archive.org/metadata/{identifier}  -> files[] { name, size }
//! GET https://archive.org/download/{identifier}/{name}
//! ```

use crate::{err, Result};

const METADATA_BASE: &str = "https://archive.org/metadata";
const DOWNLOAD_BASE: &str = "https://archive.org/download";

/// One file inside an archive.org item.
#[derive(Debug, Clone)]
pub struct ArchiveFile {
    pub name: String,
    pub size: u64,
}

/// Parse an archive.org metadata document into its file list.
pub fn parse_item_files(document: &str) -> Result<Vec<ArchiveFile>> {
    let value: serde_json::Value = serde_json::from_str(document)
        .map_err(|error| crate::err(format!("metadata JSON: {error}")))?;
    let files = value
        .get("files")
        .and_then(|node| node.as_array())
        .ok_or_else(|| err("metadata has no files[] array"))?;
    let mut out = Vec::with_capacity(files.len());
    for file in files {
        let Some(name) = file.get("name").and_then(|node| node.as_str()) else {
            continue;
        };
        let size = file
            .get("size")
            .and_then(|node| node.as_str())
            .and_then(|raw| raw.parse::<u64>().ok())
            .or_else(|| file.get("size").and_then(|node| node.as_u64()))
            .unwrap_or(0);
        out.push(ArchiveFile {
            name: name.to_string(),
            size,
        });
    }
    Ok(out)
}

/// Keep only `.bok` files (case-insensitive).
pub fn bok_files(files: &[ArchiveFile]) -> Vec<&ArchiveFile> {
    files
        .iter()
        .filter(|file| file.name.to_ascii_lowercase().ends_with(".bok"))
        .collect()
}

/// Direct download URL for one item file.
pub fn download_url(identifier: &str, name: &str) -> String {
    let encoded: String = name
        .split('/')
        .map(url_encode_segment)
        .collect::<Vec<_>>()
        .join("/");
    format!("{DOWNLOAD_BASE}/{identifier}/{encoded}")
}

fn url_encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn fetch_text(url: &str) -> Result<String> {
    ureq::get(url)
        .call()
        .map_err(|error| err(format!("GET {url}: {error}")))?
        .into_string()
        .map_err(|error| err(format!("read {url}: {error}")))
}

/// List all files of a public archive.org item.
pub fn item_files(identifier: &str) -> Result<Vec<ArchiveFile>> {
    let url = format!("{METADATA_BASE}/{identifier}");
    parse_item_files(&fetch_text(&url)?)
}

/// Download one URL to a local file (streamed, no size limit issues).
pub fn download_to(url: &str, dest: &std::path::Path) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| crate::err(format!("mkdir: {error}")))?;
    }
    let response = ureq::get(url)
        .call()
        .map_err(|error| err(format!("GET {url}: {error}")))?;
    let mut reader = response.into_reader();
    let mut file =
        std::fs::File::create(dest).map_err(|error| crate::err(format!("create file: {error}")))?;
    std::io::copy(&mut reader, &mut file).map_err(|error| err(format!("write: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "files": [
            {"name": "book1.bok", "size": "12345"},
            {"name": "notes.TXT", "size": 99},
            {"name": "dir/book2.BOK"},
            {"no-name": true}
        ]
    }"#;

    #[test]
    fn parses_metadata_fixture() {
        let files = parse_item_files(FIXTURE).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].size, 12345);
        assert_eq!(bok_files(&files).len(), 2);
    }

    #[test]
    fn builds_download_urls() {
        assert_eq!(
            download_url("some-item", "dir/my book.bok"),
            "https://archive.org/download/some-item/dir/my%20book.bok"
        );
    }

    #[test]
    fn rejects_bad_json() {
        assert!(parse_item_files("not json").is_err());
        assert!(parse_item_files("{}").is_err());
    }
}
