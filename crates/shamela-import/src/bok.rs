//! Legacy Shamela book readers.
//!
//! Three layouts are supported, all read-only and pure Rust:
//!
//! * `.bok` — Jet3 MS Access, `main(BkId, Bk, Betaka, Inf, Auth, AuthInf,
//!   cat, AD)` plus `b{id}`/`t{id}` tables.
//! * Mobile `.mdb` — Jet3 MS Access with a `book(id, nass, part, page)`
//!   table and a `title(id, tit, lvl, sub)` table.
//! * Mobile `.mbok` — SQLite with `caption` plus `page_content` (FTS3
//!   shadow tables are ignored).
//!
//! The mobile formats are what Shamela's Android/portable libraries store
//! inside large ZIP/RAR collections. `.bok` and `.mdb` are both Jet3
//! databases; only the table layout differs.
//!
//! Arabic Jet3 text is recovered by re-decoding jetdb's Latin-1 view as
//! Windows-1256 (lossless round-trip).

use crate::{err, Result};
use encoding_rs::WINDOWS_1256;
use jetdb::{read_catalog, read_table_def, read_table_rows, PageReader, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// Catalog row of a `.bok` file.
#[derive(Debug, Clone)]
pub struct BokMeta {
    pub id: i64,
    pub title: String,
    pub card: String,
    pub info: String,
    pub author: String,
    pub author_bio: String,
    pub category: String,
    pub died: Option<i64>,
}

/// One body row of table `b{id}`.
#[derive(Debug, Clone)]
pub struct BokPage {
    pub id: i64,
    pub text: String,
    pub part: Option<String>,
    pub page: Option<String>,
}

/// One heading row of table `t{id}`.
#[derive(Debug, Clone)]
pub struct BokToc {
    pub heading: String,
    pub level: i64,
    pub sub: i64,
    pub page_id: i64,
}

/// A fully read legacy book.
#[derive(Debug, Clone)]
pub struct BokBook {
    pub meta: BokMeta,
    pub pages: Vec<BokPage>,
    pub toc: Vec<BokToc>,
}

/// Recover Arabic text: jetdb decodes Jet3 bytes as Latin-1, which is a
/// lossless mapping of the original Windows-1256 bytes.
pub fn fix_encoding(text: &str) -> String {
    if text.is_ascii() {
        return text.to_string();
    }
    let mut bytes = Vec::with_capacity(text.len());
    for c in text.chars() {
        let value = c as u32;
        if value > 0xFF {
            return text.to_string();
        }
        bytes.push(value as u8);
    }
    WINDOWS_1256.decode(&bytes).0.into_owned()
}

fn value_text(value: &Value, legacy: bool) -> String {
    match value {
        Value::Null => String::new(),
        Value::Text(text) => {
            if legacy {
                fix_encoding(text)
            } else {
                text.clone()
            }
        }
        Value::Long(num) => num.to_string(),
        Value::Int(num) => num.to_string(),
        Value::BigInt(num) => num.to_string(),
        Value::Byte(num) => num.to_string(),
        Value::DateTimeExtended(_) => String::new(),
        Value::Float(num) => num.to_string(),
        Value::Double(num) => num.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Binary(_) => String::new(),
        Value::Timestamp(_) => String::new(),
        Value::Money(text) | Value::Numeric(text) | Value::Guid(text) => text.clone(),
    }
}

fn value_int(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Long(num) => Some(i64::from(*num)),
        Value::Int(num) => Some(i64::from(*num)),
        Value::BigInt(num) => Some(*num),
        Value::Byte(num) => Some(i64::from(*num)),
        Value::Float(num) => Some(*num as i64),
        Value::Double(num) => Some(*num as i64),
        Value::Text(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

struct Table {
    columns: HashMap<String, usize>,
    rows: Vec<Vec<Value>>,
    /// Jet3 stores text as cp1256 bytes viewed through Latin-1; Jet4+ is
    /// already decoded as UTF-16 by jetdb.
    legacy: bool,
}

impl Table {
    fn get<'a>(&'a self, row: &'a [Value], name: &str) -> &'a Value {
        static NULL: Value = Value::Null;
        self.columns
            .get(&name.to_ascii_lowercase())
            .and_then(|index| row.get(*index))
            .unwrap_or(&NULL)
    }
}

fn read_table(reader: &mut PageReader, name: &str, legacy: bool) -> Result<Table> {
    let catalog = read_catalog(reader).map_err(|error| crate::err(format!("catalog: {error}")))?;
    let entry = catalog
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| err(format!("table '{name}' not found")))?;
    let def = read_table_def(reader, &entry.name, entry.table_page)
        .map_err(|error| crate::err(format!("{error}")))?;
    let columns = def
        .columns
        .iter()
        .enumerate()
        .map(|(index, col)| (col.name.to_ascii_lowercase(), index))
        .collect();
    let result = read_table_rows(reader, &def)
        .map_err(|error| crate::err(format!("rows of {name}: {error}")))?;
    Ok(Table {
        columns,
        rows: result.rows,
        legacy,
    })
}

fn text_of(table: &Table, row: &[Value], name: &str) -> String {
    value_text(table.get(row, name), table.legacy)
        .trim()
        .to_string()
}

fn int_of(table: &Table, row: &[Value], name: &str) -> Option<i64> {
    value_int(table.get(row, name))
}

fn opt_text(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn positive_text(value: Option<i64>) -> Option<String> {
    value.filter(|num| *num > 0).map(|num| num.to_string())
}

fn is_sqlite(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 16];
    file.read_exact(&mut header).is_ok() && &header == b"SQLite format 3\0"
}

fn is_jet3(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    let mut header = [0u8; 0x15];
    if file.read_exact(&mut header).is_err() {
        return true;
    }
    header[0x14] == 0
}

/// Strip a leading catalog number such as `085 - صلة الأرحام`.
///
/// Only a real separator after the digits is removed; titles that simply
/// start with a number (`1000 سؤال`) are kept intact.
fn clean_title(raw: &str) -> String {
    let text = raw.trim();
    let mut digits_end = 0;
    for (index, ch) in text.char_indices() {
        if ch.is_numeric() {
            digits_end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    if digits_end > 0 {
        let rest = text[digits_end..].trim_start();
        for separator in ['-', '–', '—', ':', '.', '|', 'ـ'] {
            if let Some(after) = rest.strip_prefix(separator) {
                let clean = after.trim_start();
                if !clean.is_empty() {
                    return clean.to_string();
                }
            }
        }
    }
    text.to_string()
}

fn fallback_title(first_page: &str, path: &Path) -> String {
    let line = first_page
        .lines()
        .map(|line| line.trim().trim_matches('_').trim())
        .find(|line| line.chars().count() >= 3)
        .unwrap_or("");
    if line.is_empty() {
        return path
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or("book")
            .to_string();
    }
    let mut title: String = line.chars().take(80).collect();
    if line.chars().count() > 80 {
        title.push('…');
    }
    title
}

/// Open and fully read a legacy Shamela book in any supported layout.
pub fn open_bok(path: &Path) -> Result<BokBook> {
    if is_sqlite(path) {
        return open_mbok(path);
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| err("book path is not valid UTF-8"))?;
    let mut reader = PageReader::open(path_str)
        .map_err(|error| crate::err(format!("open .bok/.mdb: {error}")))?;
    let catalog =
        read_catalog(&mut reader).map_err(|error| crate::err(format!("catalog: {error}")))?;
    let legacy = is_jet3(path);
    if catalog
        .iter()
        .any(|item| item.name.eq_ignore_ascii_case("main"))
    {
        open_legacy(&mut reader, legacy)
    } else if catalog
        .iter()
        .any(|item| item.name.eq_ignore_ascii_case("book"))
    {
        open_mobile_mdb(&mut reader, path, legacy)
    } else {
        Err(err(
            "unknown book layout: neither 'main' nor 'book' table found",
        ))
    }
}

fn open_legacy(reader: &mut PageReader, legacy: bool) -> Result<BokBook> {
    let main = read_table(reader, "main", legacy)?;
    let row = main
        .rows
        .first()
        .ok_or_else(|| err("main table is empty"))?;
    let id = int_of(&main, row, "BkId").ok_or_else(|| err("main.BkId missing"))?;

    let meta = BokMeta {
        id,
        title: text_of(&main, row, "Bk"),
        card: text_of(&main, row, "Betaka"),
        info: text_of(&main, row, "Inf"),
        author: text_of(&main, row, "Auth"),
        author_bio: text_of(&main, row, "AuthInf"),
        category: text_of(&main, row, "cat"),
        died: int_of(&main, row, "AD"),
    };

    let body_table = format!("b{id}");
    let body = read_table(reader, &body_table, legacy)?;
    let mut pages = Vec::with_capacity(body.rows.len());
    for (index, body_row) in body.rows.iter().enumerate() {
        let text = text_of(&body, body_row, "nass");
        if text.is_empty() {
            continue;
        }
        pages.push(BokPage {
            id: int_of(&body, body_row, "id").unwrap_or(index as i64 + 1),
            text,
            part: opt_text(text_of(&body, body_row, "part")),
            page: opt_text(text_of(&body, body_row, "page")),
        });
    }

    let toc_table = format!("t{id}");
    let mut toc = Vec::new();
    if let Ok(headings) = read_table(reader, &toc_table, legacy) {
        for heading_row in &headings.rows {
            let heading = text_of(&headings, heading_row, "tit");
            if heading.is_empty() {
                continue;
            }
            toc.push(BokToc {
                heading,
                level: int_of(&headings, heading_row, "lvl").unwrap_or(1).max(1),
                sub: int_of(&headings, heading_row, "sub").unwrap_or(0),
                page_id: int_of(&headings, heading_row, "id").unwrap_or(0),
            });
        }
    }

    Ok(BokBook { meta, pages, toc })
}

fn open_mobile_mdb(reader: &mut PageReader, path: &Path, legacy: bool) -> Result<BokBook> {
    let book = read_table(reader, "book", legacy)?;
    let mut pages = Vec::with_capacity(book.rows.len());
    for (index, row) in book.rows.iter().enumerate() {
        let text = text_of(&book, row, "nass");
        if text.is_empty() {
            continue;
        }
        pages.push(BokPage {
            id: int_of(&book, row, "id").unwrap_or(index as i64 + 1),
            text,
            part: positive_text(int_of(&book, row, "part")),
            page: positive_text(int_of(&book, row, "page")),
        });
    }

    let mut toc = Vec::new();
    if let Ok(headings) = read_table(reader, "title", legacy) {
        for row in &headings.rows {
            let heading = clean_title(&text_of(&headings, row, "tit"));
            if heading.is_empty() {
                continue;
            }
            toc.push(BokToc {
                heading,
                level: int_of(&headings, row, "lvl").unwrap_or(1).max(1),
                sub: int_of(&headings, row, "sub").unwrap_or(0),
                page_id: int_of(&headings, row, "id").unwrap_or(0),
            });
        }
    }

    let first_page = pages.first().map(|page| page.text.as_str()).unwrap_or("");
    let title = toc
        .iter()
        .find(|heading| heading.level == 1)
        .or_else(|| toc.first())
        .map(|heading| heading.heading.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| fallback_title(first_page, path));
    let id = path
        .file_stem()
        .and_then(|part| part.to_str())
        .and_then(|stem| stem.parse::<i64>().ok())
        .unwrap_or(0);

    Ok(BokBook {
        meta: BokMeta {
            id,
            title,
            card: String::new(),
            info: String::new(),
            author: String::new(),
            author_bio: String::new(),
            category: String::new(),
            died: None,
        },
        pages,
        toc,
    })
}

struct Caption {
    parent_id: i64,
    page_id: i64,
    title: String,
    level: i64,
}

fn sqlite_table_exists(conn: &rusqlite::Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .is_ok()
}

fn load_captions(conn: &rusqlite::Connection, captions: &mut Vec<Caption>) -> Result<()> {
    let with_level = conn.prepare(
        "SELECT CAST(parent_id AS INTEGER), CAST(page_id AS INTEGER),
                CAST(title AS TEXT), COALESCE(CAST(level AS INTEGER), 1)
         FROM caption ORDER BY cid",
    );
    if let Ok(mut statement) = with_level {
        let rows = statement
            .query_map([], |row| {
                Ok(Caption {
                    parent_id: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    page_id: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    title: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    level: row.get::<_, Option<i64>>(3)?.unwrap_or(1).max(1),
                })
            })
            .map_err(|error| err(format!("caption query: {error}")))?;
        for row in rows {
            captions.push(row.map_err(|error| err(format!("caption row: {error}")))?);
        }
        return Ok(());
    }

    // Older mobile libraries omit the level column; default to a flat TOC.
    let mut statement = conn
        .prepare(
            "SELECT CAST(parent_id AS INTEGER), CAST(page_id AS INTEGER),
                    CAST(title AS TEXT)
             FROM caption ORDER BY cid",
        )
        .map_err(|error| err(format!("caption query: {error}")))?;
    let rows = statement
        .query_map([], |row| {
            Ok(Caption {
                parent_id: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                page_id: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                title: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                level: 1,
            })
        })
        .map_err(|error| err(format!("caption query: {error}")))?;
    for row in rows {
        captions.push(row.map_err(|error| err(format!("caption row: {error}")))?);
    }
    Ok(())
}

fn open_mbok(path: &Path) -> Result<BokBook> {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| err(format!("open .mbok: {error}")))?;

    let mut captions = Vec::new();
    if sqlite_table_exists(&conn, "caption") {
        load_captions(&conn, &mut captions)?;
    }

    let mut pages = read_mbok_pages(&conn)?;
    if pages.is_empty() {
        pages = read_mbok_fts_pages(&conn)?;
    }

    let mut toc = Vec::new();
    for caption in &captions {
        let heading = clean_title(&caption.title);
        if heading.is_empty() {
            continue;
        }
        toc.push(BokToc {
            heading,
            level: caption.level,
            sub: caption.parent_id,
            page_id: caption.page_id,
        });
    }

    let first_page = pages.first().map(|page| page.text.as_str()).unwrap_or("");
    let title = captions
        .iter()
        .find(|caption| caption.parent_id == 0 && !caption.title.trim().is_empty())
        .or_else(|| {
            captions
                .iter()
                .find(|caption| !caption.title.trim().is_empty())
        })
        .map(|caption| clean_title(&caption.title))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| fallback_title(first_page, path));

    Ok(BokBook {
        meta: BokMeta {
            id: 0,
            title,
            card: String::new(),
            info: String::new(),
            author: String::new(),
            author_bio: String::new(),
            category: String::new(),
            died: None,
        },
        pages,
        toc,
    })
}

fn read_mbok_pages(conn: &rusqlite::Connection) -> Result<Vec<BokPage>> {
    if !sqlite_table_exists(conn, "page_content") {
        return Ok(Vec::new());
    }
    let mut statement = conn
        .prepare(
            "SELECT CAST(docid AS INTEGER), CAST(c0id AS INTEGER),
                    CAST(c2number AS TEXT), CAST(c3part AS TEXT),
                    CAST(c5content AS TEXT), CAST(c6content_plain AS TEXT)
             FROM page_content ORDER BY docid",
        )
        .map_err(|error| err(format!("page_content query: {error}")))?;
    let rows = statement
        .query_map([], |row| {
            let docid = row.get::<_, i64>(0)?;
            let c0id = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
            let number = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let part = row.get::<_, Option<String>>(3)?.unwrap_or_default();
            let rich = row.get::<_, Option<String>>(4)?.unwrap_or_default();
            let plain = row.get::<_, Option<String>>(5)?.unwrap_or_default();
            Ok((docid, c0id, number, part, rich, plain))
        })
        .map_err(|error| err(format!("page_content rows: {error}")))?;
    let mut pages = Vec::new();
    for row in rows {
        let (docid, c0id, number, part, rich, plain) =
            row.map_err(|error| err(format!("page_content row: {error}")))?;
        let text = if rich.trim().is_empty() { plain } else { rich };
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        pages.push(BokPage {
            id: if c0id > 0 { c0id } else { docid },
            text,
            part: positive_sql_text(part),
            page: positive_sql_text(number),
        });
    }
    Ok(pages)
}

fn read_mbok_fts_pages(conn: &rusqlite::Connection) -> Result<Vec<BokPage>> {
    if !sqlite_table_exists(conn, "page") {
        return Ok(Vec::new());
    }
    let mut statement = conn
        .prepare(
            "SELECT CAST(id AS INTEGER), CAST(number AS TEXT), CAST(part AS TEXT),
                    CAST(content AS TEXT), CAST(content_plain AS TEXT)
             FROM page ORDER BY id",
        )
        .map_err(|error| err(format!("page query: {error}")))?;
    let rows = statement
        .query_map([], |row| {
            let id = row.get::<_, i64>(0)?;
            let number = row.get::<_, Option<String>>(1)?.unwrap_or_default();
            let part = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let rich = row.get::<_, Option<String>>(3)?.unwrap_or_default();
            let plain = row.get::<_, Option<String>>(4)?.unwrap_or_default();
            Ok((id, number, part, rich, plain))
        })
        .map_err(|error| err(format!("page rows: {error}")))?;
    let mut pages = Vec::new();
    for row in rows {
        let (id, number, part, rich, plain) =
            row.map_err(|error| err(format!("page row: {error}")))?;
        let text = if rich.trim().is_empty() { plain } else { rich };
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        pages.push(BokPage {
            id,
            text,
            part: positive_sql_text(part),
            page: positive_sql_text(number),
        });
    }
    Ok(pages)
}

fn positive_sql_text(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "0" {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_fix_round_trip() {
        // Simulate jetdb's Latin-1 view of cp1256 bytes and recover it.
        let original = "عب";
        let bytes = encoding_rs::WINDOWS_1256.encode(original).0;
        let latin1: String = bytes.iter().map(|byte| *byte as char).collect();
        assert_eq!(fix_encoding(&latin1), original);
        assert_eq!(fix_encoding("plain ascii"), "plain ascii");
    }

    #[test]
    fn value_helpers() {
        assert_eq!(value_text(&Value::Null, true), "");
        assert_eq!(value_text(&Value::Long(42), true), "42");
        assert_eq!(value_text(&Value::Text("مرحبا".into()), false), "مرحبا");
        assert_eq!(value_int(&Value::Text("7".to_string())), Some(7));
        assert_eq!(value_int(&Value::Null), None);
    }

    #[test]
    fn catalog_number_is_removed_only_with_separator() {
        assert_eq!(clean_title("085 - صلة الأرحام"), "صلة الأرحام");
        assert_eq!(clean_title("12 – باب الطهارة"), "باب الطهارة");
        assert_eq!(clean_title("1000 سؤال وجواب"), "1000 سؤال وجواب");
        assert_eq!(clean_title("١٢٣ - كتاب"), "كتاب");
    }
}
