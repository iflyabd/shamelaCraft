//! Normalization of any source into the `Page`/`Title` SQLite format.
//!
//! Whatever the origin (legacy `.bok`, EPUB chapters tomorrow, API
//! downloads), imported books land in identical `.db` files that
//! [`shamela_content::book::BookDb`] already reads.

use crate::bok::BokBook;
use shamela_content::book::{BookWriter, NewPage, NewTitle};
use std::path::Path;

/// Convert a legacy book into a `Page`/`Title` database.
///
/// Heading levels become a tree: each heading's parent is the nearest
/// previous heading with a smaller level; `Title.page` keeps the source
/// body-row id so the reader can jump to the page.
pub fn bok_to_book_db(book: &BokBook, out: &Path) -> rusqlite::Result<()> {
    let writer = BookWriter::create(out)?;
    let pages: Vec<NewPage> = book
        .pages
        .iter()
        .map(|page| NewPage {
            id: page.id,
            content: page.text.clone(),
            part: page.part.clone(),
            page: page.page.clone(),
            number: page.page.clone(),
        })
        .collect();
    writer.insert_pages(&pages)?;

    // Assign sequential title ids, then wire parents by level.
    let mut titles: Vec<(i64, NewTitle)> = Vec::with_capacity(book.toc.len());
    let mut stack: Vec<(i64, i64)> = Vec::new(); // (level, title id)
    for (index, heading) in book.toc.iter().enumerate() {
        let id = index as i64 + 1;
        while stack.last().is_some_and(|(level, _)| *level >= heading.level) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, parent_id)| parent_id.to_string());
        stack.push((heading.level, id));
        titles.push((
            id,
            NewTitle {
                content: heading.heading.clone(),
                page: Some(heading.page_id.to_string()),
                parent,
            },
        ));
    }
    writer.insert_titles(&titles)?;
    Ok(())
}
