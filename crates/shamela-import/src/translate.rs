//! Page translation through the free MyMemory API (no key).
//!
//! Long pages are split into line-bounded chunks (the anonymous quota is
//! small), each chunk is cached (see `shamela_content::cache`), and HTML
//! entities in responses are unescaped. Only facts about the public API
//! are used here; all code is original.

use crate::{err, Result};

const ENDPOINT: &str = "https://api.mymemory.translated.net/get";
const CHUNK_LIMIT: usize = 400;

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn split_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.split('\n') {
        if current.len() + line.len() + 1 > CHUNK_LIMIT && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.trim().is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Translate one chunk with MyMemory (`from`/`to` like `ar`, `en`, `fr`).
pub fn translate_chunk(chunk: &str, from: &str, to: &str) -> Result<String> {
    if chunk.trim().is_empty() {
        return Ok(String::new());
    }
    let url = format!(
        "{ENDPOINT}?q={}&langpair={from}|{to}",
        percent_encode(chunk)
    );
    let body = ureq::get(&url)
        .call()
        .map_err(|error| err(format!("translate request: {error}")))?
        .into_string()
        .map_err(|error| err(format!("translate read: {error}")))?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| err(format!("translate json: {error}")))?;
    let status = value
        .get("responseStatus")
        .and_then(|node| node.as_i64())
        .unwrap_or(0);
    let text = value
        .pointer("/responseData/translatedText")
        .and_then(|node| node.as_str())
        .unwrap_or("");
    if status != 200 {
        return Err(err(format!("translate status {status}")));
    }
    if text.contains("MYMEMORY WARNING")
        || text.contains("QUERY LENGTH LIMIT")
        || text.contains("INVALID TARGET LANGUAGE")
    {
        return Err(err("translate quota or language error"));
    }
    Ok(unescape(text))
}

/// Translate a full page, chunk by chunk. The `fetch` callback receives one
/// chunk and returns its translation (callers wire caching around it).
pub fn translate_page(
    fetch: &mut dyn FnMut(&str) -> Result<String>,
    text: &str,
    _from: &str,
    _to: &str,
) -> Result<String> {
    let mut out = Vec::new();
    for chunk in split_chunks(text) {
        out.push(fetch(&chunk)?);
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_respect_lines() {
        let text = "aaa\n".repeat(200);
        let chunks = split_chunks(&text);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= CHUNK_LIMIT + 10));
    }

    #[test]
    fn encoding_and_unescape() {
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(unescape("a &quot;b&quot;"), "a \"b\"");
    }

    #[test]
    fn page_joins_fetched_chunks() {
        let out = translate_page(
            &mut |chunk| Ok(format!("[{chunk}]")),
            "one\ntwo",
            "ar",
            "en",
        )
        .unwrap();
        assert!(out.contains("[one") && out.contains("two]"));
    }
}
