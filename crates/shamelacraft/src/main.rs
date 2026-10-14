//! ShamelaCraft — lightweight open-source Arabic library reader.
//!
//! Clean-room reimplementation: original code, original layout inspired by
//! classic Shamela workflows (library tree, tolerant search, tabs,
//! bookmarks). No proprietary code, text, or assets are included.
//!
//! v0.1 reads Shamela-4 `master.db` metadata (categories / books / authors)
//! and renders catalog cards. Full-text content providers (Lucene stored
//! fields reader, BOK importer, online API) are on the roadmap.

use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Font, Length, Task, Theme};
use shamela_db::{Book, Library};
use shamela_index::{BookIndex, NormalizeOptions};
use std::path::PathBuf;

/// Bundled Arabic font (Amiri, SIL OFL — see assets/fonts/OFL.txt).
const AMIRI: Font = Font::with_name("Amiri");
const SEARCH_LIMIT: i64 = 100;
const MIN_QUERY_CHARS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Panel {
    #[default]
    Library,
    Bookmarks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppTheme {
    Light,
    Dark,
}

impl std::fmt::Display for AppTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Light => "فاتح",
            Self::Dark => "داكن",
        })
    }
}

#[derive(Debug, Clone)]
struct Tab {
    book_id: i64,
    title: String,
    body: String,
}

#[derive(Debug, Clone)]
struct Bookmark {
    id: i64,
    title: String,
}

#[derive(Debug, Clone)]
enum Message {
    OpenDatabase,
    SearchInput(String),
    CategorySelected(i64),
    BookOpened(i64),
    TabSelected(usize),
    TabClosed(usize),
    PanelSelected(Panel),
    BookmarkAdd,
    BookmarkRemove(i64),
    FontBigger,
    FontSmaller,
    ThemeSelected(AppTheme),
}

struct State {
    lib: Option<Library>,
    index: Option<BookIndex>,
    categories: Vec<shamela_db::Category>,
    active_category: Option<i64>,
    books: Vec<Book>,
    query: String,
    results: Vec<Book>,
    tabs: Vec<Tab>,
    active_tab: Option<usize>,
    bookmarks: Vec<Bookmark>,
    panel: Panel,
    font_size: f32,
    theme_kind: AppTheme,
    status: String,
}

impl Default for State {
    fn default() -> Self {
        let bookmarks = read_bookmarks();
        Self {
            lib: None,
            index: None,
            categories: Vec::new(),
            active_category: None,
            books: Vec::new(),
            query: String::new(),
            results: Vec::new(),
            tabs: Vec::new(),
            active_tab: None,
            bookmarks,
            panel: Panel::Library,
            font_size: 20.0,
            theme_kind: AppTheme::Light,
            status: String::from("افتح ملف master.db لبدء التصفح"),
        }
    }
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let dir = std::path::Path::new(&home).join(".shamelacraft");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn bookmarks_path() -> PathBuf {
    data_dir().join("bookmarks.db")
}

fn read_bookmarks() -> Vec<Bookmark> {
    let Ok(conn) = rusqlite::Connection::open(bookmarks_path()) else {
        return Vec::new();
    };
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS bookmarks (book_id INTEGER PRIMARY KEY, title TEXT NOT NULL)",
    );
    conn.prepare("SELECT book_id, title FROM bookmarks ORDER BY rowid")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(Bookmark {
                    id: row.get(0)?,
                    title: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<Bookmark>>>()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Library operations
// ---------------------------------------------------------------------------

fn build_index(lib: &Library) -> Result<BookIndex, String> {
    let index = BookIndex::in_memory().map_err(|err| err.to_string())?;
    let opts = NormalizeOptions::default();
    let mut docs = Vec::new();
    let categories = lib.categories().map_err(|err| err.to_string())?;
    for category in &categories {
        let books = lib
            .books_in_category(category.id)
            .map_err(|err| err.to_string())?;
        for book in &books {
            let author = lib
                .author(book.main_author)
                .ok()
                .flatten()
                .map(|item| item.name)
                .unwrap_or_default();
            docs.push((book.id, book.name.clone(), author));
        }
    }
    index
        .add_books(&docs, &opts)
        .map_err(|err| err.to_string())?;
    Ok(index)
}

fn open_database(state: &mut State) {
    let dialog = rfd::FileDialog::new()
        .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
        .set_title("اختر ملف master.db");
    let Some(path) = dialog.pick_file() else {
        state.status = String::from("لم يتم اختيار ملف");
        return;
    };
    match Library::open(&path) {
        Ok(lib) => {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_string();
            match (lib.counts(), build_index(&lib)) {
                (Ok(counts), Ok(index)) => {
                    state.lib = Some(lib);
                    state.index = Some(index);
                    state.categories = state
                        .lib
                        .as_ref()
                        .and_then(|lib| lib.categories().ok())
                        .unwrap_or_default();
                    state.active_category = None;
                    state.books.clear();
                    state.results.clear();
                    state.query.clear();
                    state.status = format!(
                        "{file_name} — كتب: {} — مؤلفون: {} — أقسام: {}",
                        counts.books, counts.authors, counts.categories
                    );
                }
                _ => {
                    state.status = String::from("تعذر قراءة القاعدة أو بناء الفهرس");
                }
            }
        }
        Err(err) => {
            state.status = format!("تعذر فتح القاعدة: {err}");
        }
    }
}

fn load_category(state: &mut State) {
    state.books.clear();
    if let (Some(lib), Some(id)) = (state.lib.as_ref(), state.active_category) {
        if let Ok(books) = lib.books_in_category(id) {
            state.books = books;
        }
    }
}

fn run_search(state: &mut State) {
    state.results.clear();
    let query = state.query.trim().to_string();
    if query.chars().count() < MIN_QUERY_CHARS {
        return;
    }
    let Some(lib) = state.lib.as_ref() else {
        return;
    };
    if let Some(index) = state.index.as_ref() {
        let opts = NormalizeOptions::default();
        if let Ok(hits) = index.search(&query, &opts, SEARCH_LIMIT as usize) {
            for (id, _score) in hits {
                if let Ok(Some(book)) = lib.book(id) {
                    state.results.push(book);
                }
            }
            if !state.results.is_empty() {
                return;
            }
        }
    }
    if let Ok(books) = lib.search_books(&query, SEARCH_LIMIT) {
        state.results = books;
    }
}

fn render_book_card(state: &State, book: &Book) -> String {
    let Some(lib) = state.lib.as_ref() else {
        return book.name.clone();
    };
    let category = state
        .categories
        .iter()
        .find(|item| item.id == book.category)
        .map(|item| item.name.clone())
        .unwrap_or_else(|| book.category.to_string());
    let authors = lib.authors_of_book(book.id).unwrap_or_default();
    let author_line = if authors.is_empty() {
        String::from("—")
    } else {
        authors
            .iter()
            .map(|item| {
                if item.death_text.is_empty() {
                    item.name.clone()
                } else {
                    format!("{} ({})", item.name, item.death_text)
                }
            })
            .collect::<Vec<_>>()
            .join("، ")
    };
    let printed = if book.printed == 1 { "نعم" } else { "لا" };
    format!(
        "{}\n\nالمؤلف: {author_line}\nالقسم: {category}\nموافق للمطبوع: {printed}\n\n---\nعرض النص الكامل يتطلب موفر محتوى (خارطة الطريق: قارئ فهارس Lucene، مستورد BOK). هذه البطاقة تعرض بيانات الفهرس من master.db."
    , book.name)
}

fn open_tab(state: &mut State, book_id: i64) {
    if let Some(position) = state.tabs.iter().position(|tab| tab.book_id == book_id) {
        state.active_tab = Some(position);
        return;
    }
    let Some(lib) = state.lib.as_ref() else {
        return;
    };
    let Ok(Some(book)) = lib.book(book_id) else {
        return;
    };
    let body = render_book_card(state, &book);
    state.tabs.push(Tab {
        book_id,
        title: book.name.clone(),
        body,
    });
    state.active_tab = Some(state.tabs.len() - 1);
}

fn close_tab(state: &mut State, index: usize) {
    if index >= state.tabs.len() {
        return;
    }
    state.tabs.remove(index);
    state.active_tab = match state.active_tab {
        Some(active) if active == index => {
            if state.tabs.is_empty() {
                None
            } else {
                Some(active.min(state.tabs.len() - 1))
            }
        }
        Some(active) if active > index => Some(active - 1),
        other => other,
    };
}

fn add_bookmark(state: &mut State) {
    let Some(index) = state.active_tab else {
        return;
    };
    let Some(tab) = state.tabs.get(index) else {
        return;
    };
    if let Ok(conn) = rusqlite::Connection::open(bookmarks_path()) {
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bookmarks (book_id INTEGER PRIMARY KEY, title TEXT NOT NULL)",
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO bookmarks (book_id, title) VALUES (?1, ?2)",
            rusqlite::params![tab.book_id, tab.title],
        );
    }
    state.bookmarks = read_bookmarks();
}

fn remove_bookmark(state: &mut State, book_id: i64) {
    if let Ok(conn) = rusqlite::Connection::open(bookmarks_path()) {
        let _ = conn.execute(
            "DELETE FROM bookmarks WHERE book_id = ?1",
            rusqlite::params![book_id],
        );
    }
    state.bookmarks = read_bookmarks();
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::OpenDatabase => open_database(state),
        Message::SearchInput(query) => {
            state.query = query;
            run_search(state);
        }
        Message::CategorySelected(id) => {
            state.active_category = Some(id);
            state.query.clear();
            state.results.clear();
            load_category(state);
        }
        Message::BookOpened(id) => open_tab(state, id),
        Message::TabSelected(index) => {
            state.active_tab = Some(index);
        }
        Message::TabClosed(index) => close_tab(state, index),
        Message::PanelSelected(panel) => {
            state.panel = panel;
        }
        Message::BookmarkAdd => add_bookmark(state),
        Message::BookmarkRemove(id) => remove_bookmark(state, id),
        Message::FontBigger => {
            state.font_size = (state.font_size + 1.0).min(34.0);
        }
        Message::FontSmaller => {
            state.font_size = (state.font_size - 1.0).max(14.0);
        }
        Message::ThemeSelected(theme) => {
            state.theme_kind = theme;
        }
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn book_buttons(state: &State, books: &[Book]) -> Element<'_, Message> {
    if books.is_empty() {
        return text("لا نتائج — جرّب كلمة أخرى").font(AMIRI).into();
    }
    let items: Vec<Element<'_, Message>> = books
        .iter()
        .take(200)
        .map(|book| {
            button(
                text(book.name.as_str())
                    .font(AMIRI)
                    .size((state.font_size - 4.0).max(12.0)),
            )
            .on_press(Message::BookOpened(book.id))
            .width(Length::Fill)
            .into()
        })
        .collect();
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn library_tree(state: &State) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    items.push(
        text("الأقسام")
            .font(AMIRI)
            .size((state.font_size - 2.0).max(12.0))
            .into(),
    );
    for category in &state.categories {
        let label = if Some(category.id) == state.active_category {
            format!("> {}", category.name)
        } else {
            category.name.clone()
        };
        items.push(
            button(
                text(label)
                    .font(AMIRI)
                    .size((state.font_size - 4.0).max(12.0)),
            )
            .on_press(Message::CategorySelected(category.id))
            .width(Length::Fill)
            .into(),
        );
    }
    items.push(
        text("الكتب")
            .font(AMIRI)
            .size((state.font_size - 2.0).max(12.0))
            .into(),
    );
    if state.active_category.is_none() {
        items.push(
            text("اختر قسما لعرض كتبه")
                .font(AMIRI)
                .size((state.font_size - 4.0).max(12.0))
                .into(),
        );
    }
    for book in state.books.iter().take(200) {
        items.push(
            button(
                text(book.name.as_str())
                    .font(AMIRI)
                    .size((state.font_size - 4.0).max(12.0)),
            )
            .on_press(Message::BookOpened(book.id))
            .width(Length::Fill)
            .into(),
        );
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn bookmark_list(state: &State) -> Element<'_, Message> {
    if state.bookmarks.is_empty() {
        return text("لا توجد عناصر في المفضلة بعد").font(AMIRI).into();
    }
    let items: Vec<Element<'_, Message>> = state
        .bookmarks
        .iter()
        .map(|mark| {
            row![
                button(
                    text(mark.title.as_str())
                        .font(AMIRI)
                        .size((state.font_size - 4.0).max(12.0)),
                )
                .on_press(Message::BookOpened(mark.id))
                .width(Length::Fill),
                button(text("حذف").font(AMIRI)).on_press(Message::BookmarkRemove(mark.id)),
            ]
            .spacing(4)
            .into()
        })
        .collect();
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn sidebar(state: &State) -> Element<'_, Message> {
    let search = text_input("بحث في الكتب والمؤلفين …", state.query.as_str())
        .on_input(Message::SearchInput)
        .padding(8);
    let list = if state.query.trim().chars().count() >= MIN_QUERY_CHARS {
        book_buttons(state, &state.results)
    } else if state.panel == Panel::Bookmarks {
        bookmark_list(state)
    } else {
        library_tree(state)
    };
    column![search, list]
        .spacing(8)
        .padding(8)
        .width(Length::FillPortion(2))
        .into()
}

fn reader(state: &State) -> Element<'_, Message> {
    let tabs: Vec<Element<'_, Message>> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let label = if Some(index) == state.active_tab {
                format!("> {}", tab.title)
            } else {
                tab.title.clone()
            };
            button(text(label).font(AMIRI).size(14.0))
                .on_press(Message::TabSelected(index))
                .into()
        })
        .collect();
    let tab_bar = row(tabs).spacing(4);
    let content: Element<'_, Message> =
        match state.active_tab.and_then(|index| state.tabs.get(index)) {
            Some(tab) => column![
                row![
                    button(text("إضافة للمفضلة").font(AMIRI)).on_press(Message::BookmarkAdd),
                    button(text("إغلاق التبويب").font(AMIRI))
                        .on_press(Message::TabClosed(state.active_tab.unwrap_or(0))),
                ]
                .spacing(8),
                scrollable(
                    text(tab.body.as_str())
                        .font(AMIRI)
                        .size(state.font_size)
                        .width(Length::Fill),
                )
                .height(Length::Fill),
            ]
            .spacing(8)
            .into(),
            None => text("اختر كتابا من القائمة الجانبية لفتحه هنا")
                .font(AMIRI)
                .size(state.font_size)
                .into(),
        };
    column![tab_bar, content]
        .spacing(8)
        .padding(8)
        .width(Length::FillPortion(3))
        .into()
}

fn view(state: &State) -> Element<'_, Message> {
    let header = row![
        text("شاملة كرافت").font(AMIRI).size(24.0),
        button(text("فتح قاعدة البيانات").font(AMIRI)).on_press(Message::OpenDatabase),
        button(text("المكتبة").font(AMIRI)).on_press(Message::PanelSelected(Panel::Library)),
        button(text("المفضلة").font(AMIRI)).on_press(Message::PanelSelected(Panel::Bookmarks)),
        button(text("A+").font(AMIRI)).on_press(Message::FontBigger),
        button(text("A-").font(AMIRI)).on_press(Message::FontSmaller),
        pick_list(
            vec![AppTheme::Light, AppTheme::Dark],
            Some(state.theme_kind),
            Message::ThemeSelected,
        ),
    ]
    .spacing(8)
    .padding(8);
    let body = row![reader(state), sidebar(state)]
        .spacing(8)
        .padding(8)
        .height(Length::Fill);
    let status = container(text(state.status.as_str()).font(AMIRI).size(14.0))
        .padding(8)
        .width(Length::Fill);
    column![header, body, status].into()
}

fn theme(state: &State) -> Theme {
    match state.theme_kind {
        AppTheme::Light => Theme::Light,
        AppTheme::Dark => Theme::Dark,
    }
}

fn main() -> iced::Result {
    iced::application("ShamelaCraft — شاملة كرافت", update, view)
        .theme(theme)
        .font(include_bytes!("../assets/fonts/Amiri-Regular.ttf"))
        .run()
}
