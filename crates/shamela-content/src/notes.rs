//! Per-page personal notes (the reader's own commentary layer).
//!
//! Stored locally in SQLite: one row per `(book_id, page_id)` note.

use rusqlite::{params, Connection, Result};
use std::path::Path;

/// One saved note.
#[derive(Debug, Clone)]
pub struct Note {
    pub id: i64,
    pub book_id: i64,
    pub page_id: i64,
    pub text: String,
}

/// Local notes store.
pub struct NoteStore {
    conn: Connection,
}

impl NoteStore {
    /// Open (creating schema) a notes database file.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id INTEGER NOT NULL,
                page_id INTEGER NOT NULL,
                text TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS note_page ON notes (book_id, page_id);",
        )?;
        Ok(Self { conn })
    }

    /// Add a note; empty text is ignored.
    pub fn add(&self, book_id: i64, page_id: i64, text: &str) -> Result<()> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO notes (book_id, page_id, text) VALUES (?1, ?2, ?3)",
            params![book_id, page_id, trimmed],
        )?;
        Ok(())
    }

    /// Notes for one page, oldest first.
    pub fn of_page(&self, book_id: i64, page_id: i64) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, book_id, page_id, text FROM notes
             WHERE book_id = ?1 AND page_id = ?2 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![book_id, page_id], |row| {
            Ok(Note {
                id: row.get(0)?,
                book_id: row.get(1)?,
                page_id: row.get(2)?,
                text: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Delete one note by id.
    pub fn remove(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_crud() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("sc-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = NoteStore::open(&dir.join("n.db"))?;
        store.add(7, 3, "  ")?;
        assert!(store.of_page(7, 3)?.is_empty());
        store.add(7, 3, "first")?;
        store.add(7, 3, "second")?;
        let notes = store.of_page(7, 3)?;
        assert_eq!(notes.len(), 2);
        store.remove(notes[0].id)?;
        assert_eq!(store.of_page(7, 3)?.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
