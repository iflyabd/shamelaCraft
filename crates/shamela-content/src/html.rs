//! Behavior-compatible port of the Shamela HTML content pipeline.
//!
//! Semantics documented by the MIT-licensed
//! [ragaeeb/shamela](https://github.com/ragaeeb/shamela) (`src/content.ts`
//! and `src/utils/constants.ts`): title spans, hadeeth tags, narrator
//! links, footnote markers, page markers, and honorific ligatures.
//! Re-implemented here in Rust; no upstream code is copied.

/// Separator between page body and footnotes.
pub const FOOTNOTE_MARKER: &str = "_________";
/// Foreword marker stripped from content (U+821A).
pub const FOREWORD_MARKER: char = '\u{821a}';
/// Page-number span delimiters ⦗…⦘.
pub const PAGE_MARK_OPEN: char = '\u{2997}';
pub const PAGE_MARK_CLOSE: char = '\u{2998}';

/// Expand one honorific ligature, if it is one.
fn map_ligature(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{fd40}' => "رَحِمَهُ ٱللَّٰهُ",
        '\u{fd41}' => "رضي الله عنه",
        '\u{fd42}' => "رَضِيَ ٱللَّٰهُ عَنْهَا",
        '\u{fd43}' => "رَضِيَ اللَّهُ عَنْهُمْ",
        '\u{fd44}' => "رَضِيَ ٱللَّٰهُ عَنْهُمَا",
        '\u{fd45}' => "رَضِيَ اللَّهُ عَنْهُنَّ",
        '\u{fd47}' => "عَلَيْهِ ٱلسَّلَٰمُ",
        '\u{fd48}' => "عَلَيْهِمُ السَّلامُ",
        '\u{fd4a}' => "عليه الصلاة والسلام",
        '\u{fd4c}' => "صلى الله عليه وآله وسلم",
        '\u{fd4d}' => "عَلَيْهِ ٱلسَّلَٰمُ",
        '\u{fd4e}' => "تبارك وتعالى",
        '\u{fd4f}' => "رَحِمَهُمُ ٱللَّٰهُ",
        '\u{fdfd}' => "بِسْمِ اللَّهِ الرَّحْمَنِ الرَّحِيمِ",
        '\u{fdff}' => "عَزَّ وَجَلَّ",
        _ => return None,
    })
}

/// Normalize line endings to Unix style.
pub fn normalize_line_endings(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

/// Apply character mapping rules: drop `<img …>` tags, the foreword marker,
/// and expand honorific ligatures.
pub fn map_content(input: &str) -> String {
    // Pass 1: strip <img ...> plus one optional trailing '>'.
    let mut no_img = String::with_capacity(input.len());
    let mut rest = input;
    loop {
        let Some(start) = rest.find("<img") else {
            no_img.push_str(rest);
            break;
        };
        no_img.push_str(&rest[..start]);
        let after = &rest[start..];
        match after.find('>') {
            Some(end) => {
                let mut tail = &after[end + 1..];
                if let Some(stripped) = tail.strip_prefix('>') {
                    tail = stripped;
                }
                rest = tail;
            }
            None => {
                no_img.push_str(after);
                break;
            }
        }
    }
    // Pass 2: foreword marker + ligatures.
    let mut out = String::with_capacity(no_img.len());
    for c in no_img.chars() {
        if c == FOREWORD_MARKER {
            continue;
        }
        match map_ligature(c) {
            Some(expanded) => out.push_str(expanded),
            None => out.push(c),
        }
    }
    out
}

/// Split page content into `(body, footer)` on the footnote marker.
pub fn split_body_footer(content: &str) -> (&str, &str) {
    match content.split_once(FOOTNOTE_MARKER) {
        Some((body, footer)) => (body, footer),
        None => (content, ""),
    }
}

/// Remove ⦗…⦘ page-number markers.
pub fn remove_page_markers(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == PAGE_MARK_OPEN {
            for inner in chars.by_ref() {
                if inner == PAGE_MARK_CLOSE {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Walk HTML tags, calling `on_tag` with each raw tag body (without `<>`).
/// Returns the byte index just past `>`, or `None` when unterminated.
fn tag_end(input: &str, start: usize) -> Option<usize> {
    input[start..].find('>').map(|offset| start + offset + 1)
}

/// Normalize `<hadeeth-N>` … `</hadeeth>` into plain styled spans.
pub fn normalize_hadeeth(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        let Some(end) = tag_end(rest, start) else {
            break;
        };
        out.push_str(&rest[..start]);
        let body = rest[start + 1..end - 1].trim();
        if body == "/hadeeth" {
            out.push_str("</span>");
        } else if body
            .strip_prefix("hadeeth-")
            .is_some_and(|num| !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()))
        {
            out.push_str("<span class=\"hadeeth\">");
        } else {
            out.push_str(&rest[start..end]);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Strip every `<…>` tag, keeping inner text.
pub fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        let Some(end) = tag_end(rest, start) else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// A `<span data-type="title">` heading found in page HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanTitle {
    /// Raw `id` attribute (e.g. `toc-123`), when present.
    pub id: Option<String>,
    /// Visible heading text, tags stripped.
    pub text: String,
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(start) = tag.find(&needle) {
            let value_start = start + needle.len();
            if let Some(end) = tag[value_start..].find(quote) {
                return Some(tag[value_start..value_start + end].to_string());
            }
        }
    }
    None
}

fn is_title_span(tag: &str) -> bool {
    tag.starts_with("span")
        && (tag.contains("data-type=\"title\"") || tag.contains("data-type='title'"))
}

/// Extract title spans in document order.
pub fn extract_title_spans(input: &str) -> Vec<SpanTitle> {
    let mut titles = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        let Some(end) = tag_end(rest, start) else {
            break;
        };
        let body = rest[start + 1..end - 1].trim().to_string();
        rest = &rest[end..];
        if !is_title_span(&body) {
            continue;
        }
        let id = attr_value(&body, "id");
        let close = rest.find("</span>").unwrap_or(rest.len());
        let text = strip_html_tags(&rest[..close]).trim().to_string();
        titles.push(SpanTitle { id, text });
        rest = &rest[close.min(rest.len())..];
        if let Some(stripped) = rest.strip_prefix("</span>") {
            rest = stripped;
        }
    }
    titles
}

fn is_block_tag(body: &str) -> bool {
    let name = body
        .trim_start_matches('/')
        .split([' ', '\t', '\n', '\r', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "p" | "div" | "br" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

/// Convert page HTML to readable plain text: title spans become `##`
/// headings, hadeeth tags are normalized away, links keep their text.
pub fn content_to_text(input: &str) -> String {
    let normalized = normalize_hadeeth(input);
    let mut out = String::with_capacity(normalized.len());
    let mut rest = normalized.as_str();
    let mut in_title = false;
    while let Some(start) = rest.find('<') {
        let Some(end) = tag_end(rest, start) else {
            break;
        };
        out.push_str(&rest[..start]);
        let body = rest[start + 1..end - 1].trim().to_string();
        if is_title_span(&body) {
            out.push_str("\n## ");
            in_title = true;
        } else if body == "/span" && in_title {
            out.push('\n');
            in_title = false;
        } else if is_block_tag(&body) && !body.starts_with('/') {
            out.push('\n');
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    let mapped = map_content(&out);
    let unmarked = remove_page_markers(&mapped);
    let endings = normalize_line_endings(&unmarked);
    // Trim lines, collapse blanks, drop edges.
    let mut lines: Vec<&str> = Vec::new();
    for line in endings.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !lines.last().is_some_and(|last: &&str| last.is_empty()) {
                lines.push("");
            }
        } else {
            lines.push(trimmed);
        }
    }
    while lines.first() == Some(&"") {
        lines.remove(0);
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings() {
        assert_eq!(normalize_line_endings("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn strips_tags_but_keeps_text() {
        let clean = strip_html_tags("<p>نص <b>عريض</b></p>");
        assert!(!clean.contains('<'));
        assert!(clean.contains("نص"));
    }

    #[test]
    fn hadeeth_tags_normalize() {
        let out = normalize_hadeeth("<hadeeth-3>نص</hadeeth>");
        assert!(out.contains("<span class=\"hadeeth\">"));
        assert!(out.contains("</span>"));
        assert!(!out.contains("hadeeth-3"));
    }

    #[test]
    fn title_spans_extract_and_head() {
        let html = "<span data-type=\"title\" id=\"toc-7\">الباب</span><p>متن</p>";
        let titles = extract_title_spans(html);
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].id.as_deref(), Some("toc-7"));
        assert!(titles[0].text.contains("الباب"));
        let text = content_to_text(html);
        assert!(text.contains("##"));
        assert!(text.contains("متن"));
    }

    #[test]
    fn footnotes_split() {
        let (body, footer) = split_body_footer("متن _________ حاشية");
        assert_eq!((body, footer), ("متن ", " حاشية"));
        assert_eq!(split_body_footer("بلا حاشية"), ("بلا حاشية", ""));
    }

    #[test]
    fn page_markers_removed() {
        let clean = remove_page_markers("نص ⦗123⦘ نص");
        assert!(!clean.contains('⦗'));
        assert!(clean.contains("نص"));
    }

    #[test]
    fn ligatures_expand_and_img_drops() {
        let expanded = map_content("\u{fdfd}");
        assert!(!expanded.contains('\u{fdfd}'));
        assert!(expanded.len() > "x".len());
        let clean = map_content("<img src=\"x\">>نص");
        assert!(!clean.contains("<img"));
        assert!(clean.contains("نص"));
        assert!(!map_content("نص舄").contains(FOREWORD_MARKER));
    }
}
