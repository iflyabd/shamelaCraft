//! Read-only access to Shamela-4 `master.db` metadata databases.
//!
//! Clean-room implementation: only the on-disk SQLite schema shape is used
//! (tables `category`, `author`, `book`, `author_book`, `coauthor_book`).
//! No proprietary code, text content, or assets are referenced or copied.

use rusqlite::{params, Connection, OpenFlags, Result};
use std::path::Path;

/// A top-level library section (e.g. العقيدة، الفقه).
#[derive(Debug, Clone)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub order: i64,
}

/// An author with an optional Hijri death year.
#[derive(Debug, Clone)]
pub struct Author {
    pub id: i64,
    pub name: String,
    pub death_number: Option<i64>,
    pub death_text: String,
}

/// A book entry with its catalog metadata.
///
/// Full text content is intentionally out of scope for this crate; see the
/// `ContentProvider` roadmap in the top-level README.
#[derive(Debug, Clone)]
pub struct Book {
    pub id: i64,
    pub name: String,
    pub category: i64,
    pub book_type: i64,
    pub authors: String,
    pub main_author: i64,
    pub printed: i64,
    pub parent: Option<i64>,
    pub pdf_links: String,
    pub meta_data: String,
}

/// Row counts for the library header/status bar.
#[derive(Debug, Clone, Copy, Default)]
pub struct Counts {
    pub books: i64,
    pub authors: i64,
    pub categories: i64,
}

/// An open `master.db` file. Always opened read-only; user data is never modified.
pub struct Library {
    conn: Connection,
}

impl Library {
    /// Open an existing database file without write access.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self { conn })
    }

    /// All categories ordered for display.
    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT category_id, category_name, category_order
             FROM category
             ORDER BY category_order, category_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                order: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    fn book_from_row(row: &rusqlite::Row) -> Result<Book> {
        Ok(Book {
            id: row.get("book_id")?,
            name: row.get("book_name")?,
            category: row.get("book_category")?,
            book_type: row.get("book_type")?,
            authors: row.get::<_, Option<String>>("authors")?.unwrap_or_default(),
            main_author: row.get::<_, Option<i64>>("main_author")?.unwrap_or(0),
            printed: row.get::<_, Option<i64>>("printed")?.unwrap_or(0),
            parent: row.get("parent")?,
            pdf_links: row
                .get::<_, Option<String>>("pdf_links")?
                .unwrap_or_default(),
            meta_data: row
                .get::<_, Option<String>>("meta_data")?
                .unwrap_or_default(),
        })
    }

    const BOOK_COLS: &'static str = "book_id, book_name, book_category, book_type, \
        authors, main_author, printed, parent, pdf_links, meta_data";

    /// Visible books of one category, in catalog order.
    pub fn books_in_category(&self, category_id: i64) -> Result<Vec<Book>> {
        let sql = format!(
            "SELECT {} FROM book
             WHERE book_category = ?1 AND (hidden IS NULL OR hidden = 0)
             ORDER BY alpha, book_id",
            Self::BOOK_COLS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![category_id], Self::book_from_row)?;
        rows.collect()
    }

    /// A single book by id, if present.
    pub fn book(&self, book_id: i64) -> Result<Option<Book>> {
        let sql = format!("SELECT {} FROM book WHERE book_id = ?1", Self::BOOK_COLS);
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map(params![book_id], Self::book_from_row)?;
        Ok(rows.next().transpose()?)
    }

    /// A single author by id, if present.
    pub fn author(&self, author_id: i64) -> Result<Option<Author>> {
        let mut stmt = self.conn.prepare(
            "SELECT author_id, author_name, death_number, death_text
             FROM author
             WHERE author_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![author_id], |row| {
            Ok(Author {
                id: row.get(0)?,
                name: row.get(1)?,
                death_number: row.get(2)?,
                death_text: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    /// Main author plus co-authors of a book.
    pub fn authors_of_book(&self, book_id: i64) -> Result<Vec<Author>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.author_id, a.author_name, a.death_number, a.death_text
             FROM author a
             WHERE a.author_id IN (
                 SELECT author_id FROM author_book WHERE book_id = ?1
                 UNION
                 SELECT author_id FROM coauthor_book WHERE book_id = ?1
             )
             ORDER BY a.author_id",
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok(Author {
                id: row.get(0)?,
                name: row.get(1)?,
                death_number: row.get(2)?,
                death_text: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    fn escape_like(query: &str) -> String {
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    }

    /// Fallback substring search over book titles (the indexed search in
    /// `shamela-index` is preferred for Arabic-tolerant queries).
    pub fn search_books(&self, query: &str, limit: i64) -> Result<Vec<Book>> {
        let pattern = format!("%{}%", Self::escape_like(query));
        let sql = format!(
            "SELECT {} FROM book
             WHERE book_name LIKE ?1 ESCAPE '\\'
             ORDER BY alpha, book_id
             LIMIT ?2",
            Self::BOOK_COLS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![pattern, limit], Self::book_from_row)?;
        rows.collect()
    }

    /// Fallback substring search over author names.
    pub fn search_authors(&self, query: &str, limit: i64) -> Result<Vec<Author>> {
        let pattern = format!("%{}%", Self::escape_like(query));
        let mut stmt = self.conn.prepare(
            "SELECT author_id, author_name, death_number, death_text
             FROM author
             WHERE author_name LIKE ?1 ESCAPE '\\'
             ORDER BY alpha, author_id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit], |row| {
            Ok(Author {
                id: row.get(0)?,
                name: row.get(1)?,
                death_number: row.get(2)?,
                death_text: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    /// Library totals for the header/status bar.
    pub fn counts(&self) -> Result<Counts> {
        let books: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM book", [], |row| row.get(0))?;
        let authors: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM author", [], |row| row.get(0))?;
        let categories: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM category", [], |row| row.get(0))?;
        Ok(Counts {
            books,
            authors,
            categories,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Result<Library> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE category (category_id INTEGER PRIMARY KEY, category_name TEXT, category_order INTEGER);
             CREATE TABLE author (author_id INTEGER PRIMARY KEY, author_name TEXT, death_number INTEGER, death_text TEXT, alpha INTEGER);
             CREATE TABLE book (book_id INTEGER PRIMARY KEY, book_name TEXT, book_category INTEGER, book_type INTEGER,
                 book_date INTEGER, authors TEXT, main_author INTEGER, printed INTEGER, group_id INTEGER, hidden INTEGER,
                 major_online INTEGER, minor_online INTEGER, major_ondisk INTEGER, minor_ondisk INTEGER,
                 pdf_links TEXT, pdf_ondisk INTEGER, pdf_online INTEGER,
                 cover_ondisk INTEGER, cover_online INTEGER,
                 meta_data TEXT, parent INTEGER, alpha INTEGER, group_order INTEGER, book_up INTEGER);
             CREATE TABLE author_book (author_id INTEGER, book_id INTEGER);
             CREATE TABLE coauthor_book (author_id INTEGER, book_id INTEGER);
             INSERT INTO category VALUES (1, 'العقيدة', 1), (2, 'الفقه', 2);
             INSERT INTO author VALUES (10, 'ابن تيمية', 728, 'ت 728هـ', 1), (11, 'النووي', 676, 'ت 676هـ', 2);
             INSERT INTO book VALUES
                 (100, 'مجموع الفتاوى', 1, 1, NULL, NULL, 10, 1, NULL, NULL, NULL, NULL, NULL, NULL, '', NULL, NULL, NULL, NULL, '', NULL, 1, NULL, NULL),
                 (101, 'المنهاج', 2, 1, NULL, NULL, 11, 1, NULL, NULL, NULL, NULL, NULL, NULL, '', NULL, NULL, NULL, NULL, '', NULL, 2, NULL, NULL);
             INSERT INTO author_book VALUES (10, 100), (11, 101);",
        )?;
        Ok(Library { conn })
    }

    #[test]
    fn browse_and_search() -> Result<()> {
        let lib = fixture()?;
        let counts = lib.counts()?;
        assert_eq!(counts.books, 2);
        assert_eq!(counts.authors, 2);
        assert_eq!(counts.categories, 2);
        assert_eq!(lib.categories()?.len(), 2);
        assert_eq!(lib.books_in_category(1)?.len(), 1);
        assert_eq!(lib.search_books("الفتاوى", 10)?.len(), 1);
        assert_eq!(lib.search_authors("النووي", 10)?.len(), 1);
        assert_eq!(lib.authors_of_book(100)?[0].name, "ابن تيمية");
        assert!(lib.book(999)?.is_none());
        Ok(())
    }
}
