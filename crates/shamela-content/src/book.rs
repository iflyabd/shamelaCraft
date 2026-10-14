//! Reader for per-book SQLite databases.
//!
//! Schema (documented by the MIT-licensed
//! [ragaeeb/shamela](https://github.com/ragaeeb/shamela) `src/db/book.ts`):
//!
//! * `Page(id, content, part, page, number, services, is_deleted)`
//! * `Title(id, content, page, parent, is_deleted)`
//!
//! Rows with `is_deleted = '1'` are skipped, mirroring the patch protocol.

use rusqlite::{params, Connection, OpenFlags, Result};
use std::collections::HashMap;
use std::path::Path;

/// One book page: HTML content plus print position.
#[derive(Debug, Clone)]
pub struct Page {
    pub id: i64,
    pub content: String,
    pub part: Option<String>,
    pub page: Option<String>,
    pub number: Option<String>,
}

/// One table-of-contents entry; `parent` links to another title id.
#[derive(Debug, Clone)]
pub struct Title {
    pub id: i64,
    pub content: String,
    pub page: Option<String>,
    pub parent: Option<String>,
}

/// Nested table-of-contents node built from [`Title`] rows.
#[derive(Debug, Clone)]
pub struct TocNode {
    pub id: i64,
    pub content: String,
    pub page: Option<String>,
    pub children: Vec<TocNode>,
}

/// An open per-book database file. Always read-only.
pub struct BookDb {
    conn: Connection,
}

impl BookDb {
    /// Open an existing per-book `.db` file without write access.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self { conn })
    }

    fn table_exists(&self, table: &str) -> bool {
        self.conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |_| Ok(()),
            )
            .is_ok()
    }

    /// All live pages in order.
    pub fn pages(&self) -> Result<Vec<Page>> {
        if !self.table_exists("Page") {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, content, part, page, number FROM Page
             WHERE COALESCE(is_deleted, '0') != '1'
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Page {
                id: row.get(0)?,
                content: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                part: row.get(2)?,
                page: row.get(3)?,
                number: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// One page by id, if live.
    pub fn page(&self, id: i64) -> Result<Option<Page>> {
        if !self.table_exists("Page") {
            return Ok(None);
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, content, part, page, number FROM Page
             WHERE id = ?1 AND COALESCE(is_deleted, '0') != '1'",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Page {
                id: row.get(0)?,
                content: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                part: row.get(2)?,
                page: row.get(3)?,
                number: row.get(4)?,
            })
        })?;
        rows.next().transpose()
    }

    /// All live titles in order.
    pub fn titles(&self) -> Result<Vec<Title>> {
        if !self.table_exists("Title") {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, content, page, parent FROM Title
             WHERE COALESCE(is_deleted, '0') != '1'
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Title {
                id: row.get(0)?,
                content: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                page: row.get(2)?,
                parent: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Titles nested into a tree via `parent` ids. Dangling or
    /// non-numeric parents become roots, preserving document order.
    pub fn toc(&self) -> Result<Vec<TocNode>> {
        let titles = self.titles()?;
        let mut children: HashMap<i64, Vec<usize>> = HashMap::new();
        let mut order: Vec<usize> = Vec::with_capacity(titles.len());
        for (index, title) in titles.iter().enumerate() {
            match title
                .parent
                .as_deref()
                .and_then(|raw| raw.trim().parse::<i64>().ok())
            {
                Some(parent) => children.entry(parent).or_default().push(index),
                None => order.push(index),
            }
        }
        // Titles whose parent id does not exist are roots too.
        let known: std::collections::HashSet<i64> = titles.iter().map(|t| t.id).collect();
        for (index, title) in titles.iter().enumerate() {
            if let Some(parent) = title
                .parent
                .as_deref()
                .and_then(|raw| raw.trim().parse::<i64>().ok())
            {
                if !known.contains(&parent) {
                    order.push(index);
                }
            }
        }
        fn build(titles: &[Title], children: &HashMap<i64, Vec<usize>>, index: usize) -> TocNode {
            let title = &titles[index];
            TocNode {
                id: title.id,
                content: title.content.clone(),
                page: title.page.clone(),
                children: children
                    .get(&title.id)
                    .map(|list| {
                        list.iter()
                            .map(|child| build(titles, children, *child))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        }
        Ok(order
            .iter()
            .map(|index| build(&titles, &children, *index))
            .collect())
    }

    /// Live page count.
    pub fn page_count(&self) -> Result<i64> {
        if !self.table_exists("Page") {
            return Ok(0);
        }
        self.conn.query_row(
            "SELECT COUNT(*) FROM Page WHERE COALESCE(is_deleted, '0') != '1'",
            [],
            |row| row.get(0),
        )
    }
}

/// A page row to write with [`BookWriter`].
#[derive(Debug, Clone)]
pub struct NewPage {
    pub id: i64,
    pub content: String,
    pub part: Option<String>,
    pub page: Option<String>,
    pub number: Option<String>,
}

/// A title row to write with [`BookWriter`].
#[derive(Debug, Clone)]
pub struct NewTitle {
    pub content: String,
    pub page: Option<String>,
    pub parent: Option<String>,
}

/// Creates `Page`/`Title` databases (the same schema [`BookDb`] reads),
/// used by every importer so all sources converge on one format.
pub struct BookWriter {
    conn: Connection,
}

impl BookWriter {
    /// Create (or open) a book database file, ensuring the schema.
    pub fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS Page (
                id INTEGER, content TEXT, part TEXT, page TEXT,
                number TEXT, services TEXT, is_deleted TEXT
            );
            CREATE TABLE IF NOT EXISTS Title (
                id INTEGER, content TEXT, page INTEGER,
                parent INTEGER, is_deleted TEXT
            );",
        )?;
        Ok(Self { conn })
    }

    /// Bulk-insert pages inside one transaction.
    pub fn insert_pages(&self, pages: &[NewPage]) -> Result<()> {
        self.conn.execute_batch("BEGIN;")?;
        let outcome = (|| {
            let mut stmt = self.conn.prepare(
                "INSERT INTO Page (id, content, part, page, number)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for page in pages {
                stmt.execute(params![
                    page.id,
                    page.content,
                    page.part,
                    page.page,
                    page.number
                ])?;
            }
            Ok::<(), rusqlite::Error>(())
        })();
        match outcome {
            Ok(()) => self.conn.execute_batch("COMMIT;"),
            Err(err) => {
                let _ = self.conn.execute_batch("ROLLBACK;");
                Err(err)
            }
        }
    }

    /// Bulk-insert titles with explicit ids inside one transaction.
    pub fn insert_titles(&self, titles: &[(i64, NewTitle)]) -> Result<()> {
        self.conn.execute_batch("BEGIN;")?;
        let outcome = (|| {
            let mut stmt = self
                .conn
                .prepare("INSERT INTO Title (id, content, page, parent) VALUES (?1, ?2, ?3, ?4)")?;
            for (id, title) in titles {
                stmt.execute(params![id, title.content, title.page, title.parent])?;
            }
            Ok::<(), rusqlite::Error>(())
        })();
        match outcome {
            Ok(()) => self.conn.execute_batch("COMMIT;"),
            Err(err) => {
                let _ = self.conn.execute_batch("ROLLBACK;");
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Result<BookDb> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE Page (id INTEGER, content TEXT, part TEXT, page TEXT, number TEXT, services TEXT, is_deleted TEXT);
             CREATE TABLE Title (id INTEGER, content TEXT, page INTEGER, parent INTEGER, is_deleted TEXT);
             INSERT INTO Page VALUES
                 (1, '<p>مقدمة</p>', '1', '1', '1', NULL, '0'),
                 (2, '<p>نص محذوف</p>', '1', '2', '2', NULL, '1'),
                 (3, '<p>باب أول</p>', '1', '3', '3', NULL, NULL);
             INSERT INTO Title VALUES
                 (10, 'مقدمة الكتاب', 1, NULL, '0'),
                 (11, 'الباب الأول', 3, 10, '0'),
                 (12, 'عنوان محذوف', 2, NULL, '1');",
        )?;
        Ok(BookDb { conn })
    }

    #[test]
    fn pages_skip_deleted() -> Result<()> {
        let db = fixture()?;
        assert_eq!(db.page_count()?, 2);
        let pages = db.pages()?;
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].id, 1);
        assert!(db.page(2)?.is_none());
        assert!(db.page(3)?.is_some());
        Ok(())
    }

    #[test]
    fn toc_nests_children() -> Result<()> {
        let db = fixture()?;
        let titles = db.titles()?;
        assert_eq!(titles.len(), 2);
        let toc = db.toc()?;
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].content, "مقدمة الكتاب");
        assert_eq!(toc[0].children.len(), 1);
        assert_eq!(toc[0].children[0].content, "الباب الأول");
        Ok(())
    }

    #[test]
    fn writer_round_trip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("shamelacraft-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("book.db");
        {
            let writer = BookWriter::create(&path)?;
            writer.insert_pages(&[NewPage {
                id: 1,
                content: "<p>نص</p>".to_string(),
                part: Some("1".to_string()),
                page: Some("5".to_string()),
                number: Some("5".to_string()),
            }])?;
            writer.insert_titles(&[(
                1,
                NewTitle {
                    content: "باب".to_string(),
                    page: Some("1".to_string()),
                    parent: None,
                },
            )])?;
        }
        let db = BookDb::open(&path)?;
        assert_eq!(db.page_count()?, 1);
        assert_eq!(db.titles()?.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
