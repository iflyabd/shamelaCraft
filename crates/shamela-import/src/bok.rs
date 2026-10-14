//! Legacy `.bok` reader.
//!
//! A `.bok` file is a Jet3 MS Access database (Windows-1256 text, cf. the
//! `MDB_JET3_CHARSET=cp1256` convention used by mdbtools-based tooling).
//! Layout facts (from `shaybix/Raazi` `shamela/extract.py`):
//!
//! * `main(BkId, Bk, Betaka, Inf, Auth, AuthInf, cat, AD)` — one catalog row.
//! * `b{id}(nass, id, part, page)` — body pages for book id.
//! * `t{id}(tit, lvl, sub, id)` — headings; `id` points at a body row.
//!
//! Read with pure-Rust `jetdb`; Arabic bytes are recovered by re-decoding
//! jetdb's Latin-1 text as Windows-1256 (lossless round-trip).

use crate::{err, Result};
use encoding_rs::WINDOWS_1256;
use jetdb::{read_catalog, read_table_def, read_table_rows, PageReader, Value};
use std::collections::HashMap;
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

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Text(text) => fix_encoding(text),
        Value::Long(num) => num.to_string(),
        Value::Int(num) => num.to_string(),
        Value::BigInt(num) => num.to_string(),
        Value::Byte(num) => num.to_string(),
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
        Value::Long(num) => Some(*num),
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

fn read_table(reader: &mut PageReader, name: &str) -> Result<Table> {
    let catalog = read_catalog(reader).map_err(|err| crate::err(format!("catalog: {err}")))?;
    let entry = catalog
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| err(format!("table '{name}' not found"))?;
    let def =
        read_table_def(reader, &entry.name, entry.table_page).map_err(|err| err(format!("{err}")))?;
    let columns = def
        .columns
        .iter()
        .enumerate()
        .map(|(index, col)| (col.name.to_ascii_lowercase(), index))
        .collect();
    let result =
        read_table_rows(reader, &def).map_err(|err| err(format!("rows of {name}: {err}")))?;
    Ok(Table {
        columns,
        rows: result.rows,
    })
}

fn text_of(table: &Table, row: &[Value], name: &str) -> String {
    value_text(table.get(row, name)).trim().to_string()
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

/// Open and fully read a legacy `.bok` file.
pub fn open_bok(path: &Path) -> Result<BokBook> {
    let path_str = path
        .to_str()
        .ok_or_else(|| err("book path is not valid UTF-8"))?;
    let mut reader =
        PageReader::open(path_str).map_err(|err| crate::err(format!("open .bok: {err}")))?;

    let main = read_table(&mut reader, "main")?;
    let row = main.rows.first().ok_or_else(|| err("main table is empty"))?;
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
    let body = read_table(&mut reader, &body_table)?;
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
    if let Ok(headings) = read_table(&mut reader, &toc_table) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_fix_round_trip() {
        // cp1256 bytes for ع ب (0xE3 0xC8) surface via Latin-1 as ãÈ.
        assert_eq!(fix_encoding("ãÈ"), "عب");
        assert_eq!(fix_encoding("plain ascii"), "plain ascii");
        // Non-Latin-1 input passes through untouched.
        assert_eq!(fix_encoding("عب"), "عب");
    }

    #[test]
    fn value_helpers() {
        assert_eq!(value_text(&Value::Null), "");
        assert_eq!(value_text(&Value::Long(42)), "42");
        assert_eq!(value_int(&Value::Text("7".to_string())), Some(7));
        assert_eq!(value_int(&Value::Null), None);
    }
}
