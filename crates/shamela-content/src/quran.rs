//! Uthmani Quran store: verses, sura navigation, tolerant search.
//!
//! Verse text comes from free public editions (e.g. the Tanzil-derived
//! mirrors); sura names are plain facts embedded below. Storage is a
//! local SQLite file; search matches normalized text.

use rusqlite::{params, Connection, Result};
use std::path::Path;

/// The 114 sura names in mushaf order. Asserted in tests.
pub const SURA_NAMES: [&str; 114] = [
    "الفاتحة",
    "البقرة",
    "آل عمران",
    "النساء",
    "المائدة",
    "الأنعام",
    "الأعراف",
    "الأنفال",
    "التوبة",
    "يونس",
    "هود",
    "يوسف",
    "الرعد",
    "إبراهيم",
    "الحجر",
    "النحل",
    "الإسراء",
    "الكهف",
    "مريم",
    "طه",
    "الأنبياء",
    "الحج",
    "المؤمنون",
    "النور",
    "الفرقان",
    "الشعراء",
    "النمل",
    "القصص",
    "العنكبوت",
    "الروم",
    "لقمان",
    "السجدة",
    "الأحزاب",
    "سبأ",
    "فاطر",
    "يس",
    "الصافات",
    "ص",
    "الزمر",
    "غافر",
    "فصلت",
    "الشورى",
    "الزخرف",
    "الدخان",
    "الجاثية",
    "الأحقاف",
    "محمد",
    "الفتح",
    "الحجرات",
    "ق",
    "الذاريات",
    "الطور",
    "النجم",
    "القمر",
    "الرحمن",
    "الواقعة",
    "الحديد",
    "المجادلة",
    "الحشر",
    "الممتحنة",
    "الصف",
    "الجمعة",
    "المنافقون",
    "التغابن",
    "الطلاق",
    "التحريم",
    "الملك",
    "القلم",
    "الحاقة",
    "المعارج",
    "نوح",
    "الجن",
    "المزمل",
    "المدثر",
    "القيامة",
    "الإنسان",
    "المرسلات",
    "النبأ",
    "النازعات",
    "عبس",
    "التكوير",
    "الانفطار",
    "المطففين",
    "الانشقاق",
    "البروج",
    "الطارق",
    "الأعلى",
    "الغاشية",
    "الفجر",
    "البلد",
    "الشمس",
    "الليل",
    "الضحى",
    "الشرح",
    "التين",
    "العلق",
    "القدر",
    "البينة",
    "الزلزلة",
    "العاديات",
    "القارعة",
    "التكاثر",
    "العصر",
    "الهمزة",
    "الفيل",
    "قريش",
    "الماعون",
    "الكوثر",
    "الكافرون",
    "النصر",
    "المسد",
    "الإخلاص",
    "الفلق",
    "الناس",
];

/// One verse.
#[derive(Debug, Clone)]
pub struct Verse {
    pub sura: i64,
    pub aya: i64,
    pub text: String,
}

/// Local verse store. Created empty; filled by an edition import.
pub struct QuranDb {
    conn: Connection,
}

impl QuranDb {
    /// Open (creating schema) a store file.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS verses (
                sura INTEGER, aya INTEGER, text TEXT, norm TEXT,
                PRIMARY KEY (sura, aya)
            );
            CREATE INDEX IF NOT EXISTS norm_idx ON verses (norm);",
        )?;
        Ok(Self { conn })
    }

    /// Bulk import `(sura, aya, text)` rows with precomputed normalized text.
    pub fn import(&self, verses: &[(i64, i64, String, String)]) -> Result<()> {
        self.conn.execute_batch("BEGIN;")?;
        let outcome = (|| {
            let mut stmt = self.conn.prepare(
                "INSERT OR REPLACE INTO verses (sura, aya, text, norm) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (sura, aya, text, norm) in verses {
                stmt.execute(params![sura, aya, text, norm])?;
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

    /// Total verse count (0 when no edition imported yet).
    pub fn count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM verses", [], |row| row.get(0))
    }

    /// All verses of one sura in order.
    pub fn sura(&self, sura: i64) -> Result<Vec<Verse>> {
        let mut stmt = self
            .conn
            .prepare("SELECT sura, aya, text FROM verses WHERE sura = ?1 ORDER BY aya")?;
        let rows = stmt.query_map(params![sura], |row| {
            Ok(Verse {
                sura: row.get(0)?,
                aya: row.get(1)?,
                text: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Substring search over normalized text.
    pub fn search(&self, normalized: &str, limit: i64) -> Result<Vec<Verse>> {
        let pattern = format!("%{normalized}%");
        let mut stmt = self.conn.prepare(
            "SELECT sura, aya, text FROM verses WHERE norm LIKE ?1 ESCAPE '\\' LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit], |row| {
            Ok(Verse {
                sura: row.get(0)?,
                aya: row.get(1)?,
                text: row.get(2)?,
            })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sura_names_complete() {
        assert_eq!(SURA_NAMES.len(), 114);
        assert_eq!(SURA_NAMES[0], "الفاتحة");
        assert_eq!(SURA_NAMES[113], "الناس");
    }

    #[test]
    fn store_round_trip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("sc-quran-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = QuranDb::open(&dir.join("q.db"))?;
        assert_eq!(db.count()?, 0);
        db.import(&[
            (1, 1, "بسم الله".to_string(), "بسم الله".to_string()),
            (1, 2, "الحمد لله".to_string(), "الحمد لله".to_string()),
        ])?;
        assert_eq!(db.count()?, 2);
        assert_eq!(db.sura(1)?.len(), 2);
        assert_eq!(db.search("الحمد", 10)?.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
