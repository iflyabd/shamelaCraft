//! Free Quran edition download (Tanzil-derived public mirrors, no key).
//!
//! Only the transport is implemented here; parsing targets the
//! `{chapter, verse, text}` shape and normalization reuses the index
//! crate's Arabic normalizer.

use crate::{err, Result};

/// Uthmani edition mirror (returns `{quran: [{chapter, verse, text}]}`).
pub const QURAN_URL: &str =
    "https://cdn.jsdelivr.net/gh/fawazahmed0/quran-api@1/editions/ara-quranuthmanihaf.min.json";

/// One downloaded verse.
#[derive(Debug, Clone)]
pub struct EditionVerse {
    pub sura: i64,
    pub aya: i64,
    pub text: String,
}

/// Download and parse an edition document.
pub fn fetch_edition(url: &str) -> Result<Vec<EditionVerse>> {
    let body = ureq::get(url)
        .call()
        .map_err(|error| err(format!("quran download: {error}")))?
        .into_string()
        .map_err(|error| err(format!("quran read: {error}")))?;
    parse_edition(&body)
}

/// Parse an edition document.
pub fn parse_edition(document: &str) -> Result<Vec<EditionVerse>> {
    let value: serde_json::Value =
        serde_json::from_str(document).map_err(|error| err(format!("quran json: {error}")))?;
    let list = value
        .get("quran")
        .and_then(|node| node.as_array())
        .ok_or_else(|| err("quran json has no quran[] array"))?;
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let sura = item.get("chapter").and_then(|n| n.as_i64()).unwrap_or(0);
        let aya = item.get("verse").and_then(|n| n.as_i64()).unwrap_or(0);
        let text = item
            .get("text")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        if sura > 0 && aya > 0 && !text.is_empty() {
            out.push(EditionVerse { sura, aya, text });
        }
    }
    if out.is_empty() {
        return Err(err("quran edition parsed to zero verses"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_edition() {
        let doc = r#"{"quran": [
            {"chapter": 1, "verse": 1, "text": "abc"},
            {"chapter": 0, "verse": 0, "text": ""},
            {"chapter": 114, "verse": 6, "text": "xyz"}
        ]}"#;
        let verses = parse_edition(doc).unwrap();
        assert_eq!(verses.len(), 2);
        assert_eq!((verses[0].sura, verses[0].aya), (1, 1));
        assert!(parse_edition("{}").is_err());
        assert!(parse_edition("{\"quran\": []}").is_err());
    }
}
