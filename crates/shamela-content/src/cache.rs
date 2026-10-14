//! SQLite cache for page translations.
//!
//! Keys are FNV-1a hashes of `source + from + to`, so repeated views and
//! shared books never re-hit the network.

use rusqlite::{params, Connection, Result};
use std::path::Path;

/// FNV-1a 64-bit hash rendered as hex.
pub fn cache_key(source: &str, from: &str, to: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in source.bytes().chain(from.bytes()).chain(to.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Key/value translation store.
pub struct TranslationCache {
    conn: Connection,
}

impl TranslationCache {
    /// Open (creating parent dirs and schema) a cache database file.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS translations (
                key TEXT PRIMARY KEY, target TEXT, text TEXT
            );",
        )?;
        Ok(Self { conn })
    }

    /// Look up a cached translation.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT text FROM translations WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        rows.next().transpose()
    }

    /// Store a translation.
    pub fn put(&self, key: &str, target: &str, text: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO translations (key, target, text) VALUES (?1, ?2, ?3)",
            params![key, target, text],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("sc-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = TranslationCache::open(&dir.join("t.db"))?;
        assert!(cache.get("nope")?.is_none());
        cache.put("k1", "en", "hello")?;
        assert_eq!(cache.get("k1")?.as_deref(), Some("hello"));
        assert_eq!(cache_key("a", "ar", "en").len(), 16);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
