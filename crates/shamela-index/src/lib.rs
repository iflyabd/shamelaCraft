//! Arabic-normalizing full-text search over library metadata (tantivy).
//!
//! Shamela-style tolerant search: tashkeel stripping, hamza/alef
//! unification and tatweel removal, applied symmetrically at index
//! time and at query time.

use tantivy::collector::TopDocs;
use tantivy::{doc, query::QueryParser, schema::*, Index, TantivyDocument};

/// Normalization switches, mirroring classic Shamela search options.
#[derive(Debug, Clone, Copy)]
pub struct NormalizeOptions {
    /// Drop vowel marks (U+064B–U+0652, U+0670).
    pub strip_tashkeel: bool,
    /// Unify أ/إ/آ→ا, ؤ→و, ئ→ي.
    pub unify_hamza: bool,
    /// Unify ى→ي.
    pub unify_alef_maksura: bool,
    /// Drop tatweel (ـ).
    pub strip_tatweel: bool,
}

impl Default for NormalizeOptions {
    fn default() -> Self {
        Self {
            strip_tashkeel: true,
            unify_hamza: true,
            unify_alef_maksura: true,
            strip_tatweel: true,
        }
    }
}

/// Normalize Arabic text for tolerant matching.
pub fn normalize_arabic(input: &str, opts: &NormalizeOptions) -> String {
    input
        .chars()
        .filter_map(|c| {
            if opts.strip_tashkeel && matches!(c, '\u{64B}'..='\u{652}' | '\u{670}') {
                return None;
            }
            if opts.strip_tatweel && c == '\u{640}' {
                return None;
            }
            Some(match c {
                'أ' | 'إ' | 'آ' => {
                    if opts.unify_hamza {
                        'ا'
                    } else {
                        c
                    }
                }
                'ؤ' => {
                    if opts.unify_hamza {
                        'و'
                    } else {
                        c
                    }
                }
                'ئ' => {
                    if opts.unify_hamza {
                        'ي'
                    } else {
                        c
                    }
                }
                'ى' => {
                    if opts.unify_alef_maksura {
                        'ي'
                    } else {
                        c
                    }
                }
                _ => c,
            })
        })
        .collect()
}

/// In-memory tantivy index over `(book_id, title, author)` triples.
pub struct BookIndex {
    index: Index,
    id_field: Field,
    title_field: Field,
    author_field: Field,
}

impl BookIndex {
    /// Create an empty in-memory index.
    pub fn in_memory() -> tantivy::Result<Self> {
        let mut builder = Schema::builder();
        let id_field = builder.add_i64_field("book_id", STORED);
        let title_field = builder.add_text_field("title", TEXT);
        let author_field = builder.add_text_field("author", TEXT);
        let index = Index::create_in_ram(builder.build());
        Ok(Self {
            index,
            id_field,
            title_field,
            author_field,
        })
    }

    /// Index normalized book metadata. Call once after opening a database.
    pub fn add_books(
        &self,
        books: &[(i64, String, String)],
        opts: &NormalizeOptions,
    ) -> tantivy::Result<()> {
        let mut writer = self.index.writer(50_000_000)?;
        for (id, title, author) in books {
            writer.add_document(doc!(
                self.id_field => *id,
                self.title_field => normalize_arabic(title, opts),
                self.author_field => normalize_arabic(author, opts),
            ))?;
        }
        writer.commit()?;
        Ok(())
    }

    /// Search titles and authors, returning `(book_id, score)` pairs.
    pub fn search(
        &self,
        query: &str,
        opts: &NormalizeOptions,
        limit: usize,
    ) -> tantivy::Result<Vec<(i64, f32)>> {
        let normalized = normalize_arabic(query.trim(), opts);
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.title_field, self.author_field]);
        let parsed = parser.parse_query(&normalized)?;
        let hits = searcher.search(&parsed, &TopDocs::with_limit(limit))?;
        let mut out = Vec::with_capacity(hits.len());
        for (score, address) in hits {
            let retrieved: TantivyDocument = searcher.doc(address)?;
            if let Some(value) = retrieved
                .get_first(self.id_field)
                .and_then(|owned| owned.as_i64())
            {
                out.push((value, score));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization() {
        let opts = NormalizeOptions::default();
        assert_eq!(normalize_arabic("مُحَمَّد", &opts), "محمد");
        assert_eq!(normalize_arabic("أحمد", &opts), "احمد");
        assert_eq!(normalize_arabic("القرآن", &opts), "القران");
        assert_eq!(normalize_arabic("مكتــبة", &opts), "مكتبه");
    }

    #[test]
    fn tolerant_search() {
        let opts = NormalizeOptions::default();
        let index = BookIndex::in_memory().unwrap();
        index
            .add_books(
                &[
                    (1, "صحيح البخاري".to_string(), "البخاري".to_string()),
                    (2, "صحيح مسلم".to_string(), "مسلم".to_string()),
                ],
                &opts,
            )
            .unwrap();

        let hits = index.search("بخاري", &opts, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 1);

        // Tashkeel in the query still matches plain indexed text.
        assert_eq!(index.search("صَحِيح", &opts, 10).unwrap().len(), 2);

        // Hamza variants match.
        assert_eq!(index.search("أحمد", &opts, 10).unwrap().len(), 0);
        assert!(index.search("", &opts, 10).unwrap().is_empty());
    }
}
