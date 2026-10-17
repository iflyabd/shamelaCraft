//! Keyless Quran tafsir fetching (spa5k/tafsir_api mirror on jsDelivr).
//!
//! Every edition is a per-sura JSON array of `{ "text": ... }` entries; files
//! are cached under the app data directory so repeat reads work offline.

use std::path::Path;

/// Curated Arabic commentaries: (slug, label).
pub const TAFSIRS: &[(&str, &str)] = &[
    ("ar-tafsir-muyassar", "التفسير الميسر"),
    ("ar-tafsir-ibn-kathir", "تفسير ابن كثير"),
    ("ar-tafseer-al-qurtubi", "تفسير القرطبي"),
    ("ar-tafsir-al-tabari", "تفسير الطبري"),
    ("ar-tafsir-al-baghawi", "تفسير البغوي"),
    ("ar-tafsir-al-jalalayn", "تفسير الجلالين"),
    ("ar-tafsir-as-saadi", "تفسير السعدي"),
    ("ar-tafsir-al-wasit", "التفسير الوسيط"),
    ("ar-tafsir-al-mukhtasar", "المختصر في التفسير"),
];

fn url(slug: &str, sura: i64) -> String {
    format!("https://cdn.jsdelivr.net/gh/spa5k/tafsir_api@main/tafsir/{slug}/{sura}.json")
}

fn parse(document: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(document).map_err(|error| format!("tafsir json: {error}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| String::from("tafsir json is not an array"))?;
    let mut lines = Vec::with_capacity(items.len());
    for item in items {
        if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
            lines.push(text.to_string());
        } else if let Some(text) = item.as_str() {
            lines.push(text.to_string());
        }
    }
    Ok(lines)
}

/// Fetch one sura tafsir (network).
pub fn fetch(slug: &str, sura: i64) -> Result<Vec<String>, String> {
    let response = ureq::get(&url(slug, sura))
        .call()
        .map_err(|error| format!("GET: {error}"))?;
    let mut document = String::new();
    use std::io::Read;
    response
        .into_reader()
        .read_to_string(&mut document)
        .map_err(|error| format!("read: {error}"))?;
    parse(&document)
}

/// Cached read: disk first, then network (the result is persisted).
pub fn load_cached(dir: &Path, slug: &str, sura: i64) -> Result<Vec<String>, String> {
    let file = dir.join(format!("{slug}-{sura}.json"));
    if let Ok(document) = std::fs::read_to_string(&file) {
        if let Ok(lines) = parse(&document) {
            if !lines.is_empty() {
                return Ok(lines);
            }
        }
    }
    let lines = fetch(slug, sura)?;
    if let Ok(document) = serde_json::to_string(&lines) {
        let _ = std::fs::write(&file, document);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_shapes() {
        let document = r#"[{"text":"one"},{"text":"two"}]"#;
        let lines = parse(document).unwrap();
        assert_eq!(lines, vec!["one".to_string(), "two".to_string()]);
        let plain = r#"["alpha","beta"]"#;
        assert_eq!(parse(plain).unwrap().len(), 2);
        assert!(parse("nope").is_err());
    }

    #[test]
    fn urls_are_keyless() {
        let link = url("ar-tafsir-muyassar", 1);
        assert!(link.starts_with("https://cdn.jsdelivr.net/"));
        assert!(link.ends_with("/1.json"));
    }
}
