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
const SEARCH_BASE: &str = "https://archive.org/advancedsearch.php";

/// One file inside an archive.org item.
#[derive(Debug, Clone)]
pub struct ArchiveFile {
    pub name: String,
    pub size: u64,
    /// Full download URL when this file lives inside a remote container.
    /// `None` means the URL is built from the item id and `name`.
    pub url: Option<String>,
}

/// One archive.org item returned by a title search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveItem {
    pub identifier: String,
    pub title: String,
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
            url: None,
        });
    }
    Ok(out)
}

/// Parse archive.org advanced-search JSON into item hits.
pub fn parse_search_items(document: &str) -> Result<Vec<ArchiveItem>> {
    let value: serde_json::Value = serde_json::from_str(document)
        .map_err(|error| crate::err(format!("search JSON: {error}")))?;
    let docs = value
        .pointer("/response/docs")
        .and_then(|node| node.as_array())
        .ok_or_else(|| err("search JSON has no response.docs[] array"))?;
    let mut out = Vec::with_capacity(docs.len());
    for doc in docs {
        let Some(identifier) = doc.get("identifier").and_then(|node| node.as_str()) else {
            continue;
        };
        let title = doc
            .get("title")
            .and_then(|node| node.as_str())
            .unwrap_or(identifier);
        out.push(ArchiveItem {
            identifier: identifier.to_string(),
            title: title.to_string(),
        });
    }
    Ok(out)
}

/// Search public archive.org uploads that carry Shamela books.
///
/// The query is a book title, not an item id. Results are limited to text
/// items whose titles mention a supported book format (`bok`, `mdb`, `mbok`,
/// `بوك`, `بصيغة بوك`) or the word `شاملة`, which targets the large keyless
/// Shamela mirrors on archive.org.
pub fn search_items(query: &str, rows: usize) -> Result<Vec<ArchiveItem>> {
    let clean: String = query
        .chars()
        .map(|ch| if matches!(ch, '"' | '\\') { ' ' } else { ch })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return Ok(Vec::new());
    }
    let rows = rows.clamp(1, 50);
    let lucene = format!(
        "title:(\"{clean}\") AND mediatype:texts AND (title:(bok) OR title:(mdb) OR title:(mbok) OR title:(\"بوك\") OR title:(\"بصيغة بوك\") OR title:(\"شاملة\"))"
    );
    let url = format!(
        "{SEARCH_BASE}?q={}&fl%5B%5D=identifier&fl%5B%5D=title&rows={rows}&page=1&output=json&sort%5B%5D=downloads%20desc",
        url_encode_segment(&lucene)
    );
    parse_search_items(&fetch_text_with(&url, 30)?)
}

/// Non-book files inside Shamela PC installs: the catalog in `Files/`
/// (`main.mdb`, `user.mdb`, `special.mdb`) and the search-index databases in
/// `Indices/`. They use the same extensions as books but are not readable.
fn is_library_metadata(name: &str) -> bool {
    let lower = name.to_ascii_lowercase().replace('\\', "/");
    let segments: Vec<&str> = lower.split('/').filter(|part| !part.is_empty()).collect();
    if segments.contains(&"indices") {
        return true;
    }
    let file = segments.last().copied().unwrap_or("");
    let in_files = segments.contains(&"files");
    in_files && matches!(file, "main.mdb" | "user.mdb" | "special.mdb")
}

/// Keep only `.bok` files (case-insensitive).
pub fn bok_files(files: &[ArchiveFile]) -> Vec<&ArchiveFile> {
    files
        .iter()
        .filter(|file| {
            file.name.to_ascii_lowercase().ends_with(".bok") && !is_library_metadata(&file.name)
        })
        .collect()
}

/// Keep every directly readable book layout (`.bok`, `.mdb`, `.mbok`).
pub fn book_files(files: &[ArchiveFile]) -> Vec<&ArchiveFile> {
    files
        .iter()
        .filter(|file| {
            let lower = file.name.to_ascii_lowercase();
            (lower.ends_with(".bok") || lower.ends_with(".mdb") || lower.ends_with(".mbok"))
                && !is_library_metadata(&file.name)
        })
        .collect()
}

/// Keep directly importable files: `.bok` / `.mdb` / `.mbok` plus natively
/// extractable archives (`.zip` / `.tar` / `.tar.gz` / `.rar` / `.7z`).
pub fn downloadable_files(files: &[ArchiveFile]) -> Vec<&ArchiveFile> {
    files
        .iter()
        .filter(|file| {
            crate::extract::is_importable(&file.name) && !is_library_metadata(&file.name)
        })
        .collect()
}

/// Download URL for one file, preferring the full URL recorded while
/// browsing a container.
pub fn file_url(identifier: &str, file: &ArchiveFile) -> String {
    file.url
        .clone()
        .unwrap_or_else(|| download_url(identifier, &file.name))
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
    fetch_text_with(url, 120)
}

/// Same as [`fetch_text`], but with a shorter deadline for calls that block
/// the interface (the archive.org title search).
fn fetch_text_with(url: &str, seconds: u64) -> Result<String> {
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(seconds))
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

/// List the books inside one remote container (ZIP/RAR/7z) using
/// archive.org's public "view archive" page. Inner URLs are recorded so the
/// downloader can fetch a single book without downloading the whole pack.
pub fn browse_archive(identifier: &str, archive: &str) -> Result<Vec<ArchiveFile>> {
    let encoded: String = archive
        .split('/')
        .map(url_encode_segment)
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("{DOWNLOAD_BASE}/{identifier}/{encoded}/");
    parse_archive_listing(&fetch_text(&url)?, identifier)
}

/// Parse the server-rendered archive listing page.
///
/// The page contains one `<tr>` per entry:
/// `<tr><td><a href="//archive.org/download/...">name</a><td>...<td id="size">123</tr>`.
/// Only importable book/archive files are kept.
pub fn parse_archive_listing(document: &str, identifier: &str) -> Result<Vec<ArchiveFile>> {
    let marker = format!("/download/{identifier}/");
    let mut out = Vec::new();
    let mut rest = document;
    while let Some(start) = rest.find("<tr>") {
        rest = &rest[start + 4..];
        let Some(end) = rest.find("</tr>") else {
            break;
        };
        let row = &rest[..end];
        rest = &rest[end + 5..];
        if !row.contains("id=\"size\"") {
            continue;
        }
        let Some(link_start) = row.find("<a href=\"") else {
            continue;
        };
        let link = &row[link_start + 9..];
        let Some(link_end) = link.find('"') else {
            continue;
        };
        let href = &link[..link_end];
        let after_href = &link[link_end + 1..];
        let Some(text_start) = after_href.find('>') else {
            continue;
        };
        let text = &after_href[text_start + 1..];
        let Some(text_end) = text.find("</a>") else {
            continue;
        };
        let name = decode_entities(text[..text_end].trim());
        if name.is_empty() || !crate::extract::is_importable(&name) || is_library_metadata(&name) {
            continue;
        }
        let size = row
            .split("id=\"size\"")
            .nth(1)
            .and_then(|tail| tail.split('>').nth(1))
            .map(|tail| {
                tail.chars()
                    .take_while(|ch| ch.is_ascii_digit())
                    .collect::<String>()
            })
            .and_then(|digits| digits.parse::<u64>().ok())
            .unwrap_or(0);
        let href = decode_entities(href);
        let full = if let Some(rest) = href.strip_prefix("//") {
            format!("https://{rest}")
        } else if href.starts_with('/') {
            format!("https://archive.org{href}")
        } else {
            href
        };
        // Rows from the page are always inside this item; keep the check to
        // avoid navigation rows on older page layouts.
        if !full.contains(&marker) {
            continue;
        }
        out.push(ArchiveFile {
            name,
            size,
            url: Some(full),
        });
    }
    Ok(out)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        rest = &rest[index..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|num| num.parse::<u32>().ok())
                })
                .and_then(char::from_u32),
        };
        if let Some(ch) = decoded {
            out.push(ch);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Download one URL to a local file (streamed, no size limit issues).
pub fn download_to(url: &str, dest: &std::path::Path) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| crate::err(format!("mkdir: {error}")))?;
    }
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(300))
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
    fn parses_search_fixture() {
        let document = r#"{
            "response": {
                "docs": [
                    {"identifier": "5201Bok", "title": "5201 Bok صحيح البخاري"},
                    {"identifier": "fallback-id"},
                    {"title": "no identifier"}
                ]
            }
        }"#;
        let items = parse_search_items(document).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].identifier, "5201Bok");
        assert_eq!(items[1].title, "fallback-id");
    }

    #[test]
    fn parses_browse_listing() {
        let html = r#"
        <tr><td><a href="//archive.org/download/item/pack.zip/nested/10.mdb">nested/10.mdb</a><td><td>2020<td id="size">116736</tr>
        <tr><td><a href="//archive.org/download/item/pack.zip/nested/notes.txt">nested/notes.txt</a><td><td>2020<td id="size">10</tr>
        <tr><td><a href="//archive.org/download/item/pack.zip/nested/11.bok">nested/11.bok</a><td><td>2020<td id="size">200</tr>
        "#;
        let files = parse_archive_listing(html, "item").unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "nested/10.mdb");
        assert_eq!(files[0].size, 116736);
        assert!(files[0].url.as_deref().unwrap().ends_with("nested/10.mdb"));
        assert_eq!(files[1].name, "nested/11.bok");
    }

    #[test]
    fn skips_library_metadata() {
        let html = r#"
        <tr><td><a href="//archive.org/download/item/pack.rar/X/Files/main.mdb">X/Files/main.mdb</a><td><td>2020<td id="size">10</tr>
        <tr><td><a href="//archive.org/download/item/pack.rar/X/Indices/1/2.mdb">X/Indices/1/2.mdb</a><td><td>2020<td id="size">10</tr>
        <tr><td><a href="//archive.org/download/item/pack.rar/X/Books/1/2.mdb">X/Books/1/2.mdb</a><td><td>2020<td id="size">10</tr>
        "#;
        let files = parse_archive_listing(html, "item").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "X/Books/1/2.mdb");
    }

    #[test]
    fn decodes_html_entities() {
        assert_eq!(decode_entities("a &amp; b &#x27;c&#x27;"), "a & b 'c'");
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
