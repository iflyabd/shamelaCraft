//! Arabic root lookup over `S2.db`-shaped databases.
//!
//! The desktop `S2.db` table `roots(token BLOB, root BLOB)` stores
//! Windows-1256-encoded token→root mappings (3.2M rows). This module opens
//! such a database read-only and decodes on lookup.

use encoding_rs::WINDOWS_1256;
use rusqlite::{params, Connection, OpenFlags, Result};
use std::path::Path;

/// An open roots database file. Always read-only.
pub struct RootsDb {
    conn: Connection,
}

impl RootsDb {
    /// Open an existing roots database without write access.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self { conn })
    }

    /// Roots recorded for one surface token. Empty when the token is
    /// absent — or cannot be encoded in Windows-1256.
    pub fn lookup(&self, token: &str) -> Result<Vec<String>> {
        let (encoded, _, had_errors) = WINDOWS_1256.encode(token);
        if had_errors {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT root FROM roots WHERE token = ?1")?;
        let rows = stmt.query_map(params![encoded.as_ref()], |row| {
            let raw: Vec<u8> = row.get(0)?;
            Ok(WINDOWS_1256.decode(&raw).0.into_owned())
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(text: &str) -> Vec<u8> {
        WINDOWS_1256.encode(text).0.into_owned()
    }

    fn fixture() -> Result<RootsDb> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("CREATE TABLE roots (token BLOB, root BLOB);")?;
        conn.execute(
            "INSERT INTO roots VALUES (?1, ?2)",
            params![encode("الكتاب"), encode("كتب")],
        )?;
        Ok(RootsDb { conn })
    }

    #[test]
    fn lookup_round_trip() -> Result<()> {
        let db = fixture()?;
        assert_eq!(db.lookup("الكتاب")?, vec!["كتب".to_string()]);
        assert!(db.lookup("غيرموجود")?.is_empty());
        // Emoji is not encodable in Windows-1256.
        assert!(db.lookup("📖")?.is_empty());
        Ok(())
    }
}
