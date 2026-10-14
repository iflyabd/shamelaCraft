//! ShamelaCraft — lightweight open-source Arabic library reader.
//!
//! Clean-room reimplementation: original code, original icons and artwork,
//! workflows inspired by the classic Shamela PC program (library tree,
//! tolerant search, tabs, bookmarks). No proprietary code, text, or assets
//! are included.
//!
//! v0.1 reads Shamela-4 `master.db` metadata (categories / books / authors),
//! legacy `.bok` files, and archive.org collections. Full-text content
//! providers beyond `.bok`/API databases are on the roadmap.

use iced::widget::pane_grid::{self, Axis};
use iced::widget::svg::{Handle, Svg};
use iced::widget::text::Shaping;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space, Stack};
use iced::{clipboard, Element, Font, Length, Task, Theme};
use iced::{keyboard, Background, Color};
use shamela_content::book::{BookDb, TocNode};
use shamela_content::html::content_to_text;
use shamela_content::quran::Verse;
use shamela_db::{Author, Book, Library};
mod i18n;
use i18n::{tr, Lang};
use shamela_index::{BookIndex, NormalizeOptions};
use std::path::PathBuf;

/// Bundled Arabic font (Amiri, SIL OFL — see assets/fonts/OFL.txt).
const AMIRI: Font = Font::with_name("Amiri");
const SEARCH_LIMIT: i64 = 100;
const MIN_QUERY_CHARS: usize = 2;
const SIDEBAR_WIDTH: f32 = 320.0;

fn about_text(lang: Lang) -> &'static str {
    match lang {
        Lang::Ar => {
            "v0.1 ShamelaCraft - lightweight Arabic library reader\n\n\
    - master.db read-only catalog (categories, books, authors)\n\
    - legacy BOK import, pure-Rust Jet reader\n\
    - archive.org keyless downloads (bok, zip, tar, rar)\n\
    - tolerant Arabic search (tashkeel, hamza, tatweel)\n\
    - tabs, bookmarks, themes, export to markdown\n\n\
    - idea: Abdelkader El Djazairi (Arab Falcon / Saqr Quraish)\n\
    - clean-room Rust, no proprietary code or assets"
        }
        Lang::En => {
            "v0.1 ShamelaCraft - lightweight Arabic library reader\n\n\
    - master.db read-only catalog (categories, books, authors)\n\
    - legacy BOK import, pure-Rust Jet reader\n\
    - archive.org keyless downloads (bok, zip, tar, rar)\n\
    - tolerant Arabic search (tashkeel, hamza, tatweel)\n\
    - tabs, bookmarks, themes, export to markdown\n\n\
    - idea: Abdelkader El Djazairi (Arab Falcon / Saqr Quraish)\n\
    - clean-room Rust, no proprietary code or assets"
        }
        Lang::Fr => {
            "v0.1 ShamelaCraft - lecteur arabe leger de bibliotheque\n\n\
    - catalogue master.db en lecture seule\n\
    - import BOK patrimonial, lecteur Jet 100% Rust\n\
    - telechargements archive.org sans cle\n\
    - recherche arabe tolerante\n\
    - onglets, favoris, themes, export markdown\n\n\
    - idee : Abdelkader El Djazairi (Arab Falcon / Saqr Quraish)\n\
    - Rust salle blanche, sans code proprietaire"
        }
    }
}

/// The three resizable panes: reader | books | side navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneKind {
    Reader,
    Books,
    Side,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Panel {
    #[default]
    Library,
    Authors,
    Quran,
    Control,
    Bookmarks,
    Downloads,
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
struct ReaderTab {
    pages: Vec<shamela_content::book::Page>,
    index: usize,
    toc: Vec<TocNode>,
    rendered: String,
    translation: Option<String>,
    show_translation: bool,
}

#[derive(Debug, Clone)]
enum TabKind {
    Card(String),
    Reader(ReaderTab),
    Quran { sura: i64, verses: Vec<Verse> },
}

#[derive(Debug, Clone)]
struct Tab {
    book_id: i64,
    title: String,
    kind: TabKind,
}

#[derive(Debug, Clone)]
struct Bookmark {
    id: i64,
    title: String,
}

#[derive(Debug, Clone)]
enum Message {
    OpenDatabase,
    ImportBok,
    About,
    SearchInput(String),
    CategorySelected(i64),
    AuthorSelected(i64),
    BookOpened(i64),
    TabSelected(usize),
    TabClosed(usize),
    PanelSelected(Panel),
    BookmarkAdd,
    BookmarkRemove(i64),
    ReaderPrev,
    ReaderNext,
    TocJump(usize),
    DownloadIdInput(String),
    DownloadList,
    DownloadFile(String),
    FontBigger,
    FontSmaller,
    LangCycle,
    ThemeToggle,
    FocusBook,
    SearchBook,
    ExportBook,
    OpenCard,
    PageFirst,
    PageLast,
    TranslatePage,
    CycleTarget,
    QuranSura(i64),
    QuranSearch(String),
    QuranDownload,
    NoteInput(String),
    NoteSave,
    NoteDelete(i64),
    CopyPage,
    LastSearch,
    PaneResized(pane_grid::ResizeEvent),
    ToggleToc,
    ToggleNotes,
    ToggleSettings,
    ToggleTashkeel,
    ToggleOptHamza,
    ToggleOptTashkeel,
    ToggleOptTatweel,
    ClearCache,
    ResetAll,
    CloseActiveTab,
    OpenImported(String),
    FindPage,
    DeleteBook(String),
    Hint(&'static str),
}

struct State {
    lib: Option<Library>,
    index: Option<BookIndex>,
    categories: Vec<shamela_db::Category>,
    active_category: Option<i64>,
    books: Vec<Book>,
    authors: Vec<Author>,
    active_author: Option<i64>,
    query: String,
    last_query: String,
    results: Vec<Book>,
    author_results: Vec<Author>,
    tabs: Vec<Tab>,
    active_tab: Option<usize>,
    bookmarks: Vec<Bookmark>,
    panel: Panel,
    lang: Lang,
    quran_sura: i64,
    quran_query: String,
    quran_results: Vec<shamela_content::quran::Verse>,
    note_input: String,
    notes: Vec<shamela_content::notes::Note>,
    panes: pane_grid::State<PaneKind>,
    show_toc: bool,
    show_notes: bool,
    show_tashkeel: bool,
    show_settings: bool,
    search_opts: NormalizeOptions,
    control_files: Vec<(String, u64)>,
    target_lang: String,
    dl_id: String,
    dl_files: Vec<shamela_import::archive::ArchiveFile>,
    font_size: f32,
    theme_kind: AppTheme,
    status: String,
}

impl Default for State {
    fn default() -> Self {
        let bookmarks = read_bookmarks();
        let mut state = Self {
            lib: None,
            index: None,
            categories: Vec::new(),
            active_category: None,
            books: Vec::new(),
            authors: Vec::new(),
            active_author: None,
            query: String::new(),
            last_query: String::new(),
            results: Vec::new(),
            author_results: Vec::new(),
            tabs: Vec::new(),
            active_tab: None,
            bookmarks,
            panel: Panel::Library,
            lang: Lang::Ar,
            quran_sura: 1,
            quran_query: String::new(),
            quran_results: Vec::new(),
            note_input: String::new(),
            notes: Vec::new(),
            panes: init_panes(),
            show_toc: true,
            show_notes: true,
            show_tashkeel: true,
            show_settings: false,
            search_opts: NormalizeOptions::default(),
            control_files: Vec::new(),
            target_lang: String::from("en"),
            dl_id: String::from("shamela-16241"),
            dl_files: Vec::new(),
            font_size: 22.0,
            theme_kind: AppTheme::Light,
            status: String::from("افتح ملف master.db لبدء التصفح"),
        };
        if let Ok(saved) = std::fs::read_to_string(last_db_path()) {
            let path = PathBuf::from(saved.trim());
            if path.is_file() {
                open_database_path(&mut state, Some(path));
            }
        }
        state
    }
}

/// Arabic text with proper complex shaping. Takes anything string-like and
/// returns an owned element, so callers may pass temporary labels freely.
fn ar(content: impl Into<String>, size: f32) -> Element<'static, Message> {
    text(content.into())
        .font(AMIRI)
        .size(size)
        .shaping(Shaping::Advanced)
        .into()
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

/// Original toolbar icon by name (24px line icons drawn for this project).
fn ticon(name: &'static str, px: f32) -> Element<'static, Message> {
    let handle = match name {
        "selbk" => Handle::from_memory(&include_bytes!("../assets/icons/selbk.svg")[..]),
        "display" => Handle::from_memory(&include_bytes!("../assets/icons/display.svg")[..]),
        "tree" => Handle::from_memory(&include_bytes!("../assets/icons/tree.svg")[..]),
        "com" => Handle::from_memory(&include_bytes!("../assets/icons/com.svg")[..]),
        "searchpage" => Handle::from_memory(&include_bytes!("../assets/icons/searchpage.svg")[..]),
        "searchbok" => Handle::from_memory(&include_bytes!("../assets/icons/searchbok.svg")[..]),
        "search" => Handle::from_memory(&include_bytes!("../assets/icons/search.svg")[..]),
        "lastsearch" => Handle::from_memory(&include_bytes!("../assets/icons/lastsearch.svg")[..]),
        "saved" => Handle::from_memory(&include_bytes!("../assets/icons/saved.svg")[..]),
        "first" => Handle::from_memory(&include_bytes!("../assets/icons/first.svg")[..]),
        "prev" => Handle::from_memory(&include_bytes!("../assets/icons/prev.svg")[..]),
        "next" => Handle::from_memory(&include_bytes!("../assets/icons/next.svg")[..]),
        "last" => Handle::from_memory(&include_bytes!("../assets/icons/last.svg")[..]),
        "dipquran" => Handle::from_memory(&include_bytes!("../assets/icons/dipquran.svg")[..]),
        "sharh" => Handle::from_memory(&include_bytes!("../assets/icons/sharh.svg")[..]),
        "import" => Handle::from_memory(&include_bytes!("../assets/icons/import.svg")[..]),
        "ctr" => Handle::from_memory(&include_bytes!("../assets/icons/ctr.svg")[..]),
        "authors" => Handle::from_memory(&include_bytes!("../assets/icons/authors.svg")[..]),
        "copy" => Handle::from_memory(&include_bytes!("../assets/icons/copy.svg")[..]),
        "bkmrk" => Handle::from_memory(&include_bytes!("../assets/icons/bkmrk.svg")[..]),
        "export" => Handle::from_memory(&include_bytes!("../assets/icons/export.svg")[..]),
        "betaka" => Handle::from_memory(&include_bytes!("../assets/icons/betaka.svg")[..]),
        "upgrade" => Handle::from_memory(&include_bytes!("../assets/icons/upgrade.svg")[..]),
        "options" => Handle::from_memory(&include_bytes!("../assets/icons/options.svg")[..]),
        "stop" => Handle::from_memory(&include_bytes!("../assets/icons/stop.svg")[..]),
        "sun" => Handle::from_memory(&include_bytes!("../assets/icons/sun.svg")[..]),
        "moon" => Handle::from_memory(&include_bytes!("../assets/icons/moon.svg")[..]),
        "x" => Handle::from_memory(&include_bytes!("../assets/icons/x.svg")[..]),
        "globe" => Handle::from_memory(&include_bytes!("../assets/icons/globe.svg")[..]),
        "lang" => Handle::from_memory(&include_bytes!("../assets/icons/lang.svg")[..]),
        "help" => Handle::from_memory(&include_bytes!("../assets/icons/help.svg")[..]),
        "plus" => Handle::from_memory(&include_bytes!("../assets/icons/plus.svg")[..]),
        _ => Handle::from_memory(&include_bytes!("../assets/icons/help.svg")[..]),
    };
    Svg::new(handle)
        .width(Length::Fixed(px))
        .height(Length::Fixed(px))
        .into()
}

/// List row in the classic browser style: transparent until hovered,
/// highlighted when selected. Used for the three browse lists
/// (categories, books, authors) and everywhere a list belongs.
fn list_row(label: String, selected: bool, msg: Message, size: f32) -> Element<'static, Message> {
    let content = button(ar(label, size))
        .on_press(msg)
        .width(Length::Fill)
        .padding(6);
    if selected {
        content.style(button::primary).into()
    } else {
        content.style(button::text).into()
    }
}

/// Icon toolbar button.
fn tool_btn(icon: &'static str, msg: Message) -> Element<'static, Message> {
    button(ticon(icon, 22.0)).on_press(msg).padding(5).into()
}

/// Reader | books | side navigation, user-resizable via drag handles.
fn init_panes() -> pane_grid::State<PaneKind> {
    let (mut panes, reader) = pane_grid::State::new(PaneKind::Reader);
    if let Some((books, first)) = panes.split(Axis::Vertical, reader, PaneKind::Books) {
        if let Some((_, second)) = panes.split(Axis::Vertical, books, PaneKind::Side) {
            panes.resize(first, 0.58);
            panes.resize(second, 0.42);
        }
    }
    panes
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let dir = std::path::Path::new(&home).join(".shamelacraft");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn last_db_path() -> PathBuf {
    data_dir().join("lastdb.txt")
}

fn remember_db(path: &std::path::Path) {
    let _ = std::fs::write(last_db_path(), path.to_string_lossy().as_bytes());
}

/// Headless end-to-end check used for smoke-testing builds:
/// opens a master.db (counts + full index), imports a .bok, prints results.
fn self_test(master: &str, bok: &str) -> i32 {
    let mut failures = 0;
    match Library::open(std::path::Path::new(master)) {
        Ok(lib) => match lib.counts() {
            Ok(counts) => {
                println!(
                    "MASTER OK books={} authors={} categories={}",
                    counts.books, counts.authors, counts.categories
                );
                match build_index(&lib) {
                    Ok(index) => {
                        let opts = NormalizeOptions::default();
                        match index.search("test", &opts, 5) {
                            Ok(hits) => println!("INDEX OK hits={}", hits.len()),
                            Err(err) => {
                                println!("INDEX SEARCH FAIL: {err}");
                                failures += 1;
                            }
                        }
                    }
                    Err(err) => {
                        println!("INDEX BUILD FAIL: {err}");
                        failures += 1;
                    }
                }
            }
            Err(err) => {
                println!("COUNTS FAIL: {err}");
                failures += 1;
            }
        },
        Err(err) => {
            println!("OPEN FAIL: {err}");
            failures += 1;
        }
    }
    match shamela_import::bok::open_bok(std::path::Path::new(bok)) {
        Ok(book) => println!(
            "BOK OK title={} pages={} toc={}",
            book.meta.title,
            book.pages.len(),
            book.toc.len()
        ),
        Err(err) => {
            println!("BOK FAIL: {err}");
            failures += 1;
        }
    }
    if std::env::var("TRANSLATE_TEST").as_deref() == Ok("1") {
        match shamela_import::translate::translate_chunk("Hello world", "en", "ar") {
            Ok(text) => println!("TRANSLATE OK {text}"),
            Err(err) => {
                println!("TRANSLATE FAIL: {err}");
                failures += 1;
            }
        }
    }
    if failures == 0 {
        println!("SELFTEST PASS");
    } else {
        println!("SELFTEST FAIL({failures})");
    }
    failures
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
    build_index_opts(lib, &NormalizeOptions::default())
}

fn build_index_opts(lib: &Library, opts: &NormalizeOptions) -> Result<BookIndex, String> {
    let index = BookIndex::in_memory().map_err(|err| err.to_string())?;
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
    open_database_path(state, None);
}

/// Open a master.db; with `Some(path)` it skips the file picker (used for
/// the remembered database and by tests).
fn open_database_path(state: &mut State, preset: Option<PathBuf>) {
    let path = match preset {
        Some(path) => path,
        None => {
            let dialog = rfd::FileDialog::new()
                .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
                .set_title("master.db");
            let Some(picked) = dialog.pick_file() else {
                state.status = String::from("no file selected");
                return;
            };
            picked
        }
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
                    remember_db(&path);
                    state.lib = Some(lib);
                    state.index = Some(index);
                    state.categories = state
                        .lib
                        .as_ref()
                        .and_then(|lib| lib.categories().ok())
                        .unwrap_or_default();
                    state.active_category = None;
                    state.books.clear();
                    state.authors.clear();
                    state.results.clear();
                    state.author_results.clear();
                    state.query.clear();
                    state.status = format!(
                        "{file_name} : books {} : authors {} : categories {}",
                        counts.books, counts.authors, counts.categories
                    );
                }
                _ => {
                    state.status = String::from("could not read database");
                }
            }
        }
        Err(err) => {
            state.status = format!("open failed: {err}");
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

fn load_author_books(state: &mut State, author: &Author) {
    state.books.clear();
    state.active_author = Some(author.id);
    state.active_category = None;
    if let Some(lib) = state.lib.as_ref() {
        if let Ok(books) = lib.books_of_author(author.id) {
            state.books = books;
        }
        state.status = format!("author {} : {} books", author.name, state.books.len());
    }
}

fn rebuild_index(state: &mut State) {
    if let Some(lib) = state.lib.as_ref() {
        state.index = build_index_opts(lib, &state.search_opts).ok();
    }
}

fn run_search(state: &mut State) {
    state.results.clear();
    state.author_results.clear();
    let query = state.query.trim().to_string();
    if query.chars().count() < MIN_QUERY_CHARS {
        return;
    }
    let Some(lib) = state.lib.as_ref() else {
        return;
    };
    if let Some(index) = state.index.as_ref() {
        if let Ok(hits) = index.search(&query, &state.search_opts, SEARCH_LIMIT as usize) {
            for (id, _score) in hits {
                if let Ok(Some(book)) = lib.book(id) {
                    state.results.push(book);
                }
            }
        }
    }
    if state.results.is_empty() {
        if let Ok(books) = lib.search_books(&query, SEARCH_LIMIT) {
            state.results = books;
        }
    }
    if let Ok(authors) = lib.search_authors(&query, SEARCH_LIMIT) {
        state.author_results = authors;
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
        String::from("-")
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
            .join(" ، ")
    };
    let printed = if book.printed == 1 { "yes" } else { "no" };
    format!(
        "{}\n\nby: {author_line}\ncategory: {category}\nprint-matched: {printed}\n\n---\nFull text needs a content provider (see roadmap). This card shows index data from master.db.",
        book.name
    )
}

fn open_about(state: &mut State) {
    if let Some(position) = state.tabs.iter().position(|tab| tab.title == "about") {
        state.active_tab = Some(position);
        return;
    }
    state.tabs.push(Tab {
        book_id: -1,
        title: String::from("about"),
        kind: TabKind::Card(about_text(state.lang).to_string()),
    });
    state.active_tab = Some(state.tabs.len() - 1);
}

/// Open a sura in the united reading pane.
fn open_quran_tab(state: &mut State, sura: i64) {
    let sura = sura.clamp(1, 114);
    if let Some(position) = state.tabs.iter().position(|tab| match &tab.kind {
        TabKind::Quran { sura: open, .. } => *open == sura,
        _ => false,
    }) {
        state.active_tab = Some(position);
        return;
    }
    let verses = shamela_content::quran::QuranDb::open(&quran_db())
        .map(|db| db.sura(sura).unwrap_or_default())
        .unwrap_or_default();
    let name = shamela_content::quran::SURA_NAMES
        .get(sura as usize - 1)
        .unwrap_or(&"?");
    state.tabs.push(Tab {
        book_id: -(100000 + sura),
        title: name.to_string(),
        kind: TabKind::Quran { sura, verses },
    });
    state.active_tab = Some(state.tabs.len() - 1);
}

fn open_tab(state: &mut State, book_id: i64) {
    if book_id <= -100000 {
        open_quran_tab(state, -book_id - 100000);
        return;
    }
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
        kind: TabKind::Card(body),
    });
    state.active_tab = Some(state.tabs.len() - 1);
}

fn books_dir() -> PathBuf {
    let dir = data_dir().join("books");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn import_bok(state: &mut State) {
    let dialog = rfd::FileDialog::new()
        .add_filter("Shamela legacy book", &["bok", "BOK"])
        .set_title("bok file");
    let Some(path) = dialog.pick_file() else {
        state.status = String::from("no file selected");
        return;
    };
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("book")
        .to_string();
    import_bok_file(state, &path, stem);
}

/// Import one local `.bok` file into a reader tab.
fn import_bok_file(state: &mut State, dest: &std::path::Path, stem: String) {
    let out = books_dir().join(format!("{stem}.db"));
    let book = match shamela_import::bok::open_bok(dest) {
        Ok(book) => book,
        Err(err) => {
            state.status = format!("BOK read failed: {err}");
            return;
        }
    };
    let title = if book.meta.title.is_empty() {
        stem
    } else {
        book.meta.title.clone()
    };
    let book_id = book.meta.id;
    match shamela_import::convert::bok_to_book_db(&book, &out) {
        Ok(()) => open_imported_db(state, &out, title, book_id),
        Err(err) => state.status = format!("convert failed: {err}"),
    }
}

/// Open converted pages + TOC in a reader tab.
fn open_imported_pages(
    state: &mut State,
    pages: Vec<shamela_content::book::Page>,
    toc: Vec<shamela_content::book::TocNode>,
    title: String,
    book_id: i64,
) {
    if pages.is_empty() {
        state.status = String::from("no pages");
        return;
    }
    let rendered = pages
        .first()
        .map(|page| content_to_text(&page.content))
        .unwrap_or_default();
    let count = pages.len();
    state.tabs.push(Tab {
        book_id,
        title: title.clone(),
        kind: TabKind::Reader(ReaderTab {
            pages,
            index: 0,
            toc,
            rendered,
            translation: None,
            show_translation: false,
        }),
    });
    state.active_tab = Some(state.tabs.len() - 1);
    state.status = format!("imported {title}: {count} pages");
    refresh_notes(state);
}

/// Open a converted `Page`/`Title` database file in a reader tab.
fn open_imported_db(state: &mut State, out: &std::path::Path, title: String, book_id: i64) {
    let (pages, toc) = match BookDb::open(out) {
        Ok(db) => (db.pages().unwrap_or_default(), db.toc().unwrap_or_default()),
        Err(err) => {
            state.status = format!("open converted failed: {err}");
            return;
        }
    };
    open_imported_pages(state, pages, toc, title, book_id);
}

/// Refresh the importable file list of the configured archive.org item.
fn refresh_download_list(state: &mut State) {
    let id = state.dl_id.trim().to_string();
    if id.is_empty() {
        state.status = String::from("enter an archive.org item id");
        return;
    }
    match shamela_import::archive::item_files(&id) {
        Ok(files) => {
            let items = shamela_import::archive::downloadable_files(&files);
            let count = items.len();
            state.dl_files = items.into_iter().cloned().collect();
            state.status = format!("item {id}: {count} importable files");
        }
        Err(err) => {
            state.dl_files.clear();
            state.status = format!("fetch failed: {err}");
        }
    }
}

/// Download one file from the configured item and import it.
/// `.bok` files import directly; archives are extracted first and every
/// contained book is imported (the first one opens automatically).
fn download_file(state: &mut State, name: String) {
    let id = state.dl_id.trim().to_string();
    let url = shamela_import::archive::download_url(&id, &name);
    let file_name = name.rsplit('/').next().unwrap_or(&name).to_string();
    let dest = books_dir().join(&file_name);
    state.status = format!("downloading: {name} ...");
    if let Err(err) = shamela_import::archive::download_to(&url, &dest) {
        state.status = format!("download failed: {err}");
        return;
    }
    if shamela_import::extract::archive_kind(&file_name).is_some() {
        let stem = dest
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or("book");
        let dir = books_dir().join(format!("{stem}_files"));
        match shamela_import::extract::extract_archive(&dest, &dir) {
            Ok(boks) if !boks.is_empty() => {
                let count = boks.len();
                let mut opened = false;
                for bok in &boks {
                    let inner_stem = bok
                        .file_stem()
                        .and_then(|part| part.to_str())
                        .unwrap_or("book")
                        .to_string();
                    let out = books_dir().join(format!("{inner_stem}.db"));
                    let Ok(book) = shamela_import::bok::open_bok(bok) else {
                        continue;
                    };
                    let title = if book.meta.title.is_empty() {
                        inner_stem
                    } else {
                        book.meta.title.clone()
                    };
                    if shamela_import::convert::bok_to_book_db(&book, &out).is_err() {
                        continue;
                    }
                    if !opened {
                        open_imported_db(state, &out, title, book.meta.id);
                        opened = true;
                    }
                }
                if opened {
                    state.status = format!("archive {name}: {count} books imported");
                } else {
                    state.status = format!("archive {name}: {count} books, none readable");
                }
            }
            Ok(_) => state.status = format!("archive {name}: no .bok inside"),
            Err(err) => state.status = format!("extract failed: {err}"),
        }
        return;
    }
    let stem = dest
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("book")
        .to_string();
    import_bok_file(state, &dest, stem);
}

/// Focus the first open book tab (mirrors "display active book").
fn focus_book(state: &mut State) {
    if let Some(position) = state
        .tabs
        .iter()
        .position(|tab| matches!(tab.kind, TabKind::Reader(_)))
    {
        state.active_tab = Some(position);
    } else {
        state.status = String::from("open a book first");
    }
}

/// Search the query inside the active book, jumping to the first match.
fn search_book(state: &mut State) {
    let query = state.query.trim().to_string();
    if query.chars().count() < MIN_QUERY_CHARS {
        state.status = String::from("type 2+ chars in the search box first");
        return;
    }
    let Some(index) = state.active_tab else {
        state.status = String::from("open a book first");
        return;
    };
    let Some(tab) = state.tabs.get(index) else {
        return;
    };
    let TabKind::Reader(reader) = &tab.kind else {
        state.status = String::from("open a book first");
        return;
    };
    if reader.pages.is_empty() {
        return;
    }
    let opts = state.search_opts;
    let needle = shamela_index::normalize_arabic(&query, &opts);
    let count = reader.pages.len();
    for offset in 0..count {
        let at = (reader.index + offset) % count;
        let hay = shamela_index::normalize_arabic(&reader.pages[at].content, &opts);
        if hay.contains(&needle) {
            jump_to_page(state, at);
            state.status = format!("match on page {}", at + 1);
            return;
        }
    }
    state.status = String::from("not found in this book");
}

/// Export the active book to a Markdown file.
/// Export one markdown text under a save dialog.
fn export_text(state: &mut State, title: &str, body: &str) {
    let safe_title: String = title
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let dialog = rfd::FileDialog::new()
        .add_filter("Markdown", &["md"])
        .set_file_name(format!("{safe_title}.md"));
    let Some(path) = dialog.save_file() else {
        state.status = String::from("export cancelled");
        return;
    };
    let out = format!("# {title}\n\n{body}\n");
    match std::fs::write(&path, out) {
        Ok(()) => state.status = String::from("exported"),
        Err(err) => state.status = format!("export failed: {err}"),
    }
}

fn export_book(state: &mut State) {
    let Some(index) = state.active_tab else {
        state.status = String::from("open a book first");
        return;
    };
    let Some(tab) = state.tabs.get(index) else {
        return;
    };
    let TabKind::Reader(reader) = &tab.kind else {
        state.status = String::from("open a book first");
        return;
    };
    if reader.pages.is_empty() {
        return;
    }
    let safe_title: String = tab
        .title
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let dialog = rfd::FileDialog::new()
        .add_filter("Markdown", &["md"])
        .set_file_name(format!("{safe_title}.md"));
    let Some(path) = dialog.save_file() else {
        state.status = String::from("export cancelled");
        return;
    };
    let mut out = format!("# {}\n\n", tab.title);
    for page in &reader.pages {
        out.push_str(&content_to_text(&page.content));
        out.push_str("\n\n---\n\n");
    }
    match std::fs::write(&path, out) {
        Ok(()) => state.status = format!("exported {}", reader.pages.len()),
        Err(err) => state.status = format!("export failed: {err}"),
    }
}

/// Push a card tab, focusing it when an identical one is open.
fn push_card(state: &mut State, book_id: i64, title: String, body: String) {
    if let Some(position) = state
        .tabs
        .iter()
        .position(|tab| tab.book_id == book_id && tab.title == title)
    {
        state.active_tab = Some(position);
        return;
    }
    state.tabs.push(Tab {
        book_id,
        title,
        kind: TabKind::Card(body),
    });
    state.active_tab = Some(state.tabs.len() - 1);
}

/// Open the catalog card of the active tab's book (or a sura card).
fn open_card(state: &mut State) {
    let Some(index) = state.active_tab else {
        state.status = String::from("open a book first");
        return;
    };
    let Some(tab) = state.tabs.get(index) else {
        return;
    };
    if let TabKind::Quran { sura, verses } = &tab.kind {
        let name = shamela_content::quran::SURA_NAMES
            .get(*sura as usize - 1)
            .unwrap_or(&"?");
        let body = format!(
            "{name}\n\nverses: {n}\n\n---\nQuran text, Uthmani edition.",
            n = verses.len()
        );
        push_card(state, -1, name.to_string(), body);
        return;
    }
    let book_id = tab.book_id;
    if book_id < 0 {
        state.status = String::from("open a book first");
        return;
    }
    open_tab(state, book_id);
}

fn translations_db() -> PathBuf {
    data_dir().join("translations.db")
}

/// Translate the active page (chunked, cached) or toggle back to original.
fn translate_current(state: &mut State) {
    let target = state.target_lang.clone();
    let from = "ar";
    if target == from {
        state.status = String::from("target is same as source");
        return;
    }
    let Some(index) = state.active_tab else {
        state.status = String::from("open a book first");
        return;
    };
    let cache = match shamela_content::cache::TranslationCache::open(&translations_db()) {
        Ok(cache) => cache,
        Err(err) => {
            state.status = format!("cache failed: {err}");
            return;
        }
    };
    let Some(tab) = state.tabs.get_mut(index) else {
        return;
    };
    let TabKind::Reader(reader) = &mut tab.kind else {
        state.status = String::from("open a book first");
        return;
    };
    if reader.pages.is_empty() {
        return;
    }
    if reader.show_translation {
        reader.show_translation = false;
        return;
    }
    let Some(page) = reader.pages.get(reader.index) else {
        return;
    };
    let source = content_to_text(&page.content);
    state.status = format!("translating to {target} ...");
    let mut fetch = |chunk: &str| -> shamela_import::Result<String> {
        let key = shamela_content::cache::cache_key(chunk, from, &target);
        if let Ok(Some(hit)) = cache.get(&key) {
            return Ok(hit);
        }
        let got = shamela_import::translate::translate_chunk(chunk, from, &target)?;
        let _ = cache.put(&key, &target, &got);
        Ok(got)
    };
    match shamela_import::translate::translate_page(&mut fetch, &source, from, &target) {
        Ok(text) => {
            reader.translation = Some(text);
            reader.show_translation = true;
            state.status = format!("translated to {target}");
        }
        Err(err) => {
            state.status = format!("translate failed: {err}");
        }
    }
}

fn move_page(state: &mut State, delta: isize) {
    let Some(index) = state.active_tab else {
        return;
    };
    let Some(tab) = state.tabs.get_mut(index) else {
        return;
    };
    match &mut tab.kind {
        TabKind::Reader(reader) => {
            if reader.pages.is_empty() {
                return;
            }
            let last = reader.pages.len() as isize - 1;
            reader.index = (reader.index as isize + delta).clamp(0, last) as usize;
            reader.translation = None;
            reader.show_translation = false;
            reader.rendered = reader
                .pages
                .get(reader.index)
                .map(|page| content_to_text(&page.content))
                .unwrap_or_default();
        }
        TabKind::Quran { sura, verses } => {
            let next = (*sura as isize + delta).clamp(1, 114) as i64;
            if next != *sura {
                *sura = next;
                *verses = shamela_content::quran::QuranDb::open(&quran_db())
                    .map(|db| db.sura(next).unwrap_or_default())
                    .unwrap_or_default();
                tab.title = shamela_content::quran::SURA_NAMES
                    .get(next as usize - 1)
                    .unwrap_or(&"?")
                    .to_string();
            }
        }
        TabKind::Card(_) => return,
    }
    refresh_notes(state);
}

fn jump_to_page(state: &mut State, target: usize) {
    let Some(index) = state.active_tab else {
        return;
    };
    let Some(tab) = state.tabs.get_mut(index) else {
        return;
    };
    let TabKind::Reader(reader) = &mut tab.kind else {
        return;
    };
    if target < reader.pages.len() {
        reader.index = target;
        reader.translation = None;
        reader.show_translation = false;
        reader.rendered = reader
            .pages
            .get(target)
            .map(|page| content_to_text(&page.content))
            .unwrap_or_default();
    }
    refresh_notes(state);
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

fn quran_db() -> PathBuf {
    data_dir().join("quran.db")
}

fn notes_db() -> PathBuf {
    data_dir().join("notes.db")
}

fn download_quran(state: &mut State) {
    state.status = String::from("downloading quran edition ...");
    let verses = match shamela_import::quran_dl::fetch_edition(shamela_import::quran_dl::QURAN_URL)
    {
        Ok(verses) => verses,
        Err(err) => {
            state.status = format!("quran download failed: {err}");
            return;
        }
    };
    let db = match shamela_content::quran::QuranDb::open(&quran_db()) {
        Ok(db) => db,
        Err(err) => {
            state.status = format!("quran store failed: {err}");
            return;
        }
    };
    let opts = NormalizeOptions::default();
    let rows: Vec<(i64, i64, String, String)> = verses
        .iter()
        .map(|v| {
            (
                v.sura,
                v.aya,
                v.text.clone(),
                shamela_index::normalize_arabic(&v.text, &opts),
            )
        })
        .collect();
    match db.import(&rows) {
        Ok(()) => {
            state.status = format!("quran imported: {} verses", rows.len());
            state.quran_results.clear();
        }
        Err(err) => state.status = format!("quran import failed: {err}"),
    }
}

fn run_quran_search(state: &mut State) {
    state.quran_results.clear();
    let query = state.quran_query.trim().to_string();
    if query.chars().count() < MIN_QUERY_CHARS {
        return;
    }
    let Ok(db) = shamela_content::quran::QuranDb::open(&quran_db()) else {
        return;
    };
    let norm = shamela_index::normalize_arabic(&query, &state.search_opts);
    if let Ok(hits) = db.search(&norm, 100) {
        state.quran_results = hits;
    }
}

/// Re-list imported `*.db` books with sizes for the control room.
fn refresh_control(state: &mut State) {
    state.control_files.clear();
    let dir = books_dir();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(String, u64)> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("db"))
        })
        .filter_map(|path| {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| (name.to_string(), size))
        })
        .collect();
    files.sort();
    state.control_files = files;
}

fn refresh_notes(state: &mut State) {
    state.notes.clear();
    state.note_input.clear();
    let Some(index) = state.active_tab else {
        return;
    };
    let Some(tab) = state.tabs.get(index) else {
        return;
    };
    let TabKind::Reader(reader) = &tab.kind else {
        return;
    };
    let Some(page) = reader.pages.get(reader.index) else {
        return;
    };
    if let Ok(store) = shamela_content::notes::NoteStore::open(&notes_db()) {
        state.notes = store.of_page(tab.book_id, page.id).unwrap_or_default();
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::OpenDatabase => open_database(state),
        Message::ImportBok => import_bok(state),
        Message::About => open_about(state),
        Message::SearchInput(query) => {
            state.query = query.clone();
            if query.trim().chars().count() >= MIN_QUERY_CHARS {
                state.last_query = query;
            }
            run_search(state);
        }
        Message::CategorySelected(id) => {
            state.active_category = Some(id);
            state.active_author = None;
            state.query.clear();
            state.results.clear();
            state.author_results.clear();
            load_category(state);
        }
        Message::AuthorSelected(id) => {
            if let Some(lib) = state.lib.as_ref() {
                if let Ok(Some(full)) = lib.author(id) {
                    load_author_books(state, &full);
                }
            }
        }
        Message::BookOpened(id) => open_tab(state, id),
        Message::TabSelected(index) => {
            state.active_tab = Some(index);
        }
        Message::TabClosed(index) => close_tab(state, index),
        Message::PanelSelected(panel) => {
            state.panel = panel;
            if panel == Panel::Authors && state.query.trim().chars().count() < MIN_QUERY_CHARS {
                if let Some(lib) = state.lib.as_ref() {
                    state.authors = lib.search_authors("", 200).unwrap_or_default();
                }
            }
            if panel == Panel::Control {
                refresh_control(state);
            }
        }
        Message::BookmarkAdd => add_bookmark(state),
        Message::BookmarkRemove(id) => remove_bookmark(state, id),
        Message::ReaderPrev => move_page(state, -1),
        Message::ReaderNext => move_page(state, 1),
        Message::TocJump(target) => jump_to_page(state, target),
        Message::DownloadIdInput(id) => {
            state.dl_id = id;
        }
        Message::DownloadList => refresh_download_list(state),
        Message::DownloadFile(name) => download_file(state, name),
        Message::FontBigger => {
            state.font_size = (state.font_size + 1.0).min(34.0);
        }
        Message::LangCycle => {
            state.lang = state.lang.next();
        }
        Message::FontSmaller => {
            state.font_size = (state.font_size - 1.0).max(14.0);
        }
        Message::ThemeToggle => {
            state.theme_kind = match state.theme_kind {
                AppTheme::Light => AppTheme::Dark,
                AppTheme::Dark => AppTheme::Light,
            };
        }
        Message::FocusBook => focus_book(state),
        Message::SearchBook => {
            let is_reader = matches!(
                state.active_tab.and_then(|i| state.tabs.get(i)),
                Some(tab) if matches!(tab.kind, TabKind::Reader(_))
            );
            if is_reader {
                search_book(state);
                return Task::none();
            }
            let query = state.query.trim().to_string();
            if query.chars().count() < MIN_QUERY_CHARS {
                state.status = String::from("type 2+ chars in the search box first");
                return Task::none();
            }
            let needle = shamela_index::normalize_arabic(&query, &state.search_opts);
            let Some((_, title, body)) = active_text(state) else {
                state.status = String::from("open a book first");
                return Task::none();
            };
            let hay = shamela_index::normalize_arabic(&body, &state.search_opts);
            let count = hay.matches(&needle).count();
            state.status = format!("{count} matches in {title}");
            return Task::none();
        }
        Message::ExportBook => {
            let is_reader = matches!(
                state.active_tab.and_then(|i| state.tabs.get(i)),
                Some(tab) if matches!(tab.kind, TabKind::Reader(_))
            );
            if is_reader {
                export_book(state);
            } else if let Some((_, title, body)) = active_text(state) {
                export_text(state, &title, &body);
            } else {
                state.status = String::from("open a book first");
            }
        }
        Message::OpenCard => open_card(state),
        Message::PageFirst => move_page(state, isize::MIN),
        Message::PageLast => move_page(state, isize::MAX),
        Message::TranslatePage => translate_current(state),
        Message::CopyPage => {
            let Some((_, title, body)) = active_text(state) else {
                state.status = String::from("open a book first");
                return Task::none();
            };
            let cited = format!("{body}\n[{title}]");
            return clipboard::write(cited);
        }
        Message::PaneResized(event) => {
            state.panes.resize(event.split, event.ratio);
        }
        Message::ToggleToc => {
            state.show_toc = !state.show_toc;
        }
        Message::ToggleNotes => {
            state.show_notes = !state.show_notes;
        }
        Message::ToggleSettings => {
            state.show_settings = !state.show_settings;
        }
        Message::ToggleTashkeel => {
            state.show_tashkeel = !state.show_tashkeel;
        }
        Message::ToggleOptHamza => {
            state.search_opts.unify_hamza = !state.search_opts.unify_hamza;
            rebuild_index(state);
        }
        Message::ToggleOptTashkeel => {
            state.search_opts.strip_tashkeel = !state.search_opts.strip_tashkeel;
            rebuild_index(state);
        }
        Message::ToggleOptTatweel => {
            state.search_opts.strip_tatweel = !state.search_opts.strip_tatweel;
            rebuild_index(state);
        }
        Message::ClearCache => {
            let path = translations_db();
            match std::fs::remove_file(&path) {
                Ok(()) => state.status = String::from("translation cache cleared"),
                Err(err) => state.status = format!("cache clear failed: {err}"),
            }
        }
        Message::ResetAll => {
            state.font_size = 22.0;
            state.theme_kind = AppTheme::Light;
            state.show_tashkeel = true;
            state.search_opts = NormalizeOptions::default();
            state.status = String::from("settings reset");
        }
        Message::CloseActiveTab => {
            if let Some(index) = state.active_tab {
                close_tab(state, index);
            }
        }
        Message::OpenImported(name) => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let path = books_dir().join(&name);
            let title = path
                .file_stem()
                .and_then(|part| part.to_str())
                .unwrap_or("book")
                .to_string();
            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
            let book_id = hasher.finish() as i64;
            open_imported_db(state, &path, title, book_id);
        }
        Message::FindPage => {
            let query = state.query.trim().to_string();
            if query.chars().count() < MIN_QUERY_CHARS {
                state.status = String::from("type 2+ chars in the search box first");
                return Task::none();
            }
            let count = match state.active_tab.and_then(|i| state.tabs.get(i)) {
                Some(tab) => match &tab.kind {
                    TabKind::Reader(reader) => match reader.pages.get(reader.index) {
                        Some(page) => {
                            let needle =
                                shamela_index::normalize_arabic(&query, &state.search_opts);
                            let hay =
                                shamela_index::normalize_arabic(&page.content, &state.search_opts);
                            hay.matches(&needle).count()
                        }
                        None => 0,
                    },
                    _ => 0,
                },
                None => 0,
            };
            state.status = format!("{count} matches on this page");
        }
        Message::DeleteBook(name) => {
            let path = books_dir().join(&name);
            let _ = std::fs::remove_file(&path);
            refresh_control(state);
        }
        Message::LastSearch => {
            if state.last_query.trim().chars().count() >= MIN_QUERY_CHARS {
                state.query = state.last_query.clone();
                run_search(state);
                state.status = String::from("last search restored");
            } else {
                state.status = String::from("no previous search");
            }
        }
        Message::QuranSura(sura) => {
            state.quran_sura = sura.clamp(1, 114);
            open_quran_tab(state, state.quran_sura);
        }
        Message::QuranSearch(query) => {
            state.quran_query = query;
            run_quran_search(state);
        }
        Message::QuranDownload => download_quran(state),
        Message::NoteInput(text) => {
            state.note_input = text;
        }
        Message::NoteSave => {
            let (book_id, page_id) = match state.active_tab.and_then(|i| state.tabs.get(i)) {
                Some(tab) => match &tab.kind {
                    TabKind::Reader(reader) => match reader.pages.get(reader.index) {
                        Some(page) => (tab.book_id, page.id),
                        None => (tab.book_id, reader.index as i64),
                    },
                    _ => (tab.book_id, 0),
                },
                None => {
                    state.status = String::from("open a book first");
                    return Task::none();
                }
            };
            if let Ok(store) = shamela_content::notes::NoteStore::open(&notes_db()) {
                let _ = store.add(book_id, page_id, &state.note_input.clone());
            }
            refresh_notes(state);
        }
        Message::NoteDelete(id) => {
            if let Ok(store) = shamela_content::notes::NoteStore::open(&notes_db()) {
                let _ = store.remove(id);
            }
            refresh_notes(state);
        }
        Message::CycleTarget => {
            state.target_lang = match state.target_lang.as_str() {
                "en" => String::from("fr"),
                "fr" => String::from("ar"),
                _ => String::from("en"),
            };
        }
        Message::Hint(text) => {
            state.status = text.to_string();
        }
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn title_size(base: f32) -> f32 {
    (base - 2.0).max(12.0)
}

fn item_size(base: f32) -> f32 {
    (base - 4.0).max(12.0)
}

/// Icon toolbar mirroring the classic Shamela PC button order.
/// Buttons needing an active book explain so, exactly like the original.
fn tool_bar(state: &State) -> Element<'_, Message> {
    let theme_icon = match state.theme_kind {
        AppTheme::Light => "moon",
        AppTheme::Dark => "sun",
    };
    let row_one = row![
        tool_btn("selbk", Message::PanelSelected(Panel::Library)),
        tool_btn("display", Message::FocusBook),
        tool_btn("tree", Message::ToggleToc),
        tool_btn("com", Message::ToggleNotes),
        tool_btn("searchpage", Message::FindPage),
        tool_btn("searchbok", Message::SearchBook),
        tool_btn("search", Message::PanelSelected(Panel::Library)),
        tool_btn("lastsearch", Message::LastSearch),
        tool_btn("saved", Message::PanelSelected(Panel::Bookmarks)),
        tool_btn("first", Message::PageFirst),
        tool_btn("prev", Message::ReaderPrev),
        tool_btn("next", Message::ReaderNext),
        tool_btn("last", Message::PageLast),
        tool_btn("dipquran", Message::PanelSelected(Panel::Quran)),
        tool_btn("sharh", Message::Hint("sharh linking: roadmap")),
    ]
    .spacing(2)
    .padding(2);
    let row_two = row![
        tool_btn("import", Message::ImportBok),
        tool_btn("ctr", Message::PanelSelected(Panel::Control)),
        tool_btn("authors", Message::PanelSelected(Panel::Authors)),
        tool_btn("copy", Message::CopyPage),
        tool_btn("bkmrk", Message::PanelSelected(Panel::Bookmarks)),
        tool_btn("export", Message::ExportBook),
        tool_btn("betaka", Message::OpenCard),
        tool_btn("upgrade", Message::PanelSelected(Panel::Downloads)),
        tool_btn("options", Message::ToggleSettings),
        tool_btn("stop", Message::Hint("no background task")),
        tool_btn("globe", Message::PanelSelected(Panel::Downloads)),
        tool_btn(theme_icon, Message::ThemeToggle),
        button(ar(state.lang.code(), 14.0)).on_press(Message::LangCycle),
        tool_btn("help", Message::About),
        tool_btn("plus", Message::OpenDatabase),
        button(ar("A+", 14.0)).on_press(Message::FontBigger),
        button(ar("A-", 14.0)).on_press(Message::FontSmaller),
    ]
    .spacing(2)
    .padding(2);
    column![row_one, row_two].into()
}

fn book_buttons<'a>(state: &'a State, books: &'a [Book]) -> Element<'a, Message> {
    if books.is_empty() {
        return ar(tr(state.lang, "no_results"), state.font_size);
    }
    let items: Vec<Element<'static, Message>> = books
        .iter()
        .take(200)
        .map(|book| {
            list_row(
                book.name.clone(),
                false,
                Message::BookOpened(book.id),
                item_size(state.font_size),
            )
        })
        .collect();
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn library_tree(state: &State) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    items.push(ar(tr(state.lang, "sections"), title_size(state.font_size)));
    for category in &state.categories {
        items.push(list_row(
            category.name.clone(),
            Some(category.id) == state.active_category,
            Message::CategorySelected(category.id),
            item_size(state.font_size),
        ));
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn authors_panel(state: &State) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    items.push(ar(tr(state.lang, "authors"), title_size(state.font_size)));
    let list: &[Author] = if state.query.trim().chars().count() >= MIN_QUERY_CHARS {
        &state.author_results
    } else {
        &state.authors
    };
    if list.is_empty() {
        items.push(ar(
            tr(state.lang, "type_author"),
            item_size(state.font_size),
        ));
    }
    for author in list.iter().take(200) {
        let label = if author.death_text.is_empty() {
            author.name.clone()
        } else {
            format!("{} ({})", author.name, author.death_text)
        };
        items.push(list_row(
            label,
            Some(author.id) == state.active_author,
            Message::AuthorSelected(author.id),
            item_size(state.font_size),
        ));
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn bookmark_list(state: &State) -> Element<'_, Message> {
    if state.bookmarks.is_empty() {
        return ar(tr(state.lang, "no_bookmarks"), state.font_size);
    }
    let items: Vec<Element<'_, Message>> = state
        .bookmarks
        .iter()
        .map(|mark| {
            row![
                list_row(
                    mark.title.clone(),
                    false,
                    Message::BookOpened(mark.id),
                    item_size(state.font_size),
                ),
                button(ticon("x", 16.0)).on_press(Message::BookmarkRemove(mark.id)),
            ]
            .spacing(4)
            .into()
        })
        .collect();
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn downloads_panel(state: &State) -> Element<'_, Message> {
    let input = text_input("archive.org item id ...", state.dl_id.as_str())
        .on_input(Message::DownloadIdInput)
        .padding(8);
    let fetch = button(ar(tr(state.lang, "fetch_books"), 14.0))
        .on_press(Message::DownloadList)
        .width(Length::Fill);
    let mut items: Vec<Element<'_, Message>> = vec![input.into(), fetch.into()];
    for file in &state.dl_files {
        let megabytes = file.size as f64 / 1_048_576.0;
        let label = format!("{} ({megabytes:.1} MB)", file.name);
        items.push(list_row(
            label,
            false,
            Message::DownloadFile(file.name.clone()),
            item_size(state.font_size),
        ));
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

fn quran_panel(state: &State) -> Element<'_, Message> {
    let empty = shamela_content::quran::QuranDb::open(&quran_db())
        .map(|db| db.count().unwrap_or(0) == 0)
        .unwrap_or(true);
    if empty {
        return column![
            ar(tr(state.lang, "quran"), title_size(state.font_size)),
            button(ar(tr(state.lang, "download_quran"), 14.0))
                .on_press(Message::QuranDownload)
                .width(Length::Fill),
        ]
        .spacing(8)
        .into();
    }
    let search = text_input(tr(state.lang, "quran_search"), state.quran_query.as_str())
        .on_input(Message::QuranSearch)
        .padding(8);
    if !state.quran_results.is_empty() {
        let hits: Vec<Element<'_, Message>> = state
            .quran_results
            .iter()
            .map(|verse| {
                let label = format!("S{}:{} {}", verse.sura, verse.aya, verse.text);
                list_row(
                    label,
                    false,
                    Message::QuranSura(verse.sura),
                    item_size(state.font_size),
                )
            })
            .collect();
        return column![
            search,
            scrollable(column(hits).spacing(4)).height(Length::Fill)
        ]
        .spacing(8)
        .height(Length::Fill)
        .into();
    }
    let suras: Vec<Element<'_, Message>> = shamela_content::quran::SURA_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let id = index as i64 + 1;
            let label = format!("{id}. {name}");
            list_row(label, id == state.quran_sura, Message::QuranSura(id), 13.0)
        })
        .collect();
    column![
        search,
        scrollable(column(suras).spacing(2)).height(Length::Fill)
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

fn sidebar(state: &State) -> Element<'_, Message> {
    let show_search = matches!(state.panel, Panel::Library | Panel::Authors);
    let search: Element<'_, Message> = if show_search {
        text_input(tr(state.lang, "search_hint"), state.query.as_str())
            .on_input(Message::SearchInput)
            .padding(8)
            .into()
    } else {
        Space::with_height(Length::Fixed(0.0)).into()
    };
    let list =
        if state.query.trim().chars().count() >= MIN_QUERY_CHARS && state.panel == Panel::Library {
            book_buttons(state, &state.results)
        } else {
            match state.panel {
                Panel::Library => library_tree(state),
                Panel::Authors => authors_panel(state),
                Panel::Bookmarks => bookmark_list(state),
                Panel::Quran => quran_panel(state),
                Panel::Control => control_panel(state),
                Panel::Downloads => downloads_panel(state),
            }
        };
    column![search, list]
        .spacing(8)
        .padding(8)
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .into()
}

/// Strip tashkeel marks when the user hides them (real Shamela option).
fn apply_tashkeel(text: &str, show: bool) -> std::borrow::Cow<'_, str> {
    if show {
        std::borrow::Cow::Borrowed(text)
    } else {
        std::borrow::Cow::Owned(
            text.chars()
                .filter(|&c| !matches!(c, '\u{64b}'..='\u{652}' | '\u{670}'))
                .collect(),
        )
    }
}

/// Render a sura as plain text lines.
fn render_sura(verses: &[Verse], show_tashkeel: bool) -> String {
    verses
        .iter()
        .map(|verse| {
            format!(
                "({}) {}",
                verse.aya,
                apply_tashkeel(&verse.text, show_tashkeel)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Display text of the active tab, if any: (book_id, title, body).
fn active_text(state: &State) -> Option<(i64, String, String)> {
    let tab = state.active_tab.and_then(|index| state.tabs.get(index))?;
    match &tab.kind {
        TabKind::Card(body) => Some((
            tab.book_id,
            tab.title.clone(),
            apply_tashkeel(body, state.show_tashkeel).into_owned(),
        )),
        TabKind::Reader(reader) => {
            let page = reader.pages.get(reader.index)?;
            let shown = if reader.show_translation {
                reader
                    .translation
                    .as_deref()
                    .unwrap_or(reader.rendered.as_str())
            } else {
                reader.rendered.as_str()
            };
            Some((
                tab.book_id,
                tab.title.clone(),
                apply_tashkeel(shown, state.show_tashkeel).into_owned(),
            ))
        }
        TabKind::Quran { sura: _, verses } => {
            let name = current_sura_name(state);
            Some((-1, name, render_sura(verses, state.show_tashkeel)))
        }
    }
}

fn current_sura_name(state: &State) -> String {
    match state.active_tab.and_then(|index| state.tabs.get(index)) {
        Some(tab) => match &tab.kind {
            TabKind::Quran { sura, .. } => shamela_content::quran::SURA_NAMES
                .get(*sura as usize - 1)
                .unwrap_or(&"?")
                .to_string(),
            _ => String::new(),
        },
        None => String::new(),
    }
}

fn reader_tab_view<'a>(state: &'a State, tab: &'a ReaderTab) -> Element<'a, Message> {
    let font_size = state.font_size;
    let target = state.target_lang.as_str();
    let label = match tab.pages.get(tab.index) {
        Some(page) => {
            let position = format!("page {} / {}", tab.index + 1, tab.pages.len());
            let origin = match (&page.part, &page.page) {
                (Some(part), Some(number)) => format!(" (v{part} p{number})"),
                _ => String::new(),
            };
            format!("{position}{origin}")
        }
        None => String::from("no pages"),
    };
    let shown = if tab.show_translation {
        tab.translation
            .as_deref()
            .unwrap_or(tab.rendered.as_str())
            .to_string()
    } else {
        tab.rendered.clone()
    };
    let shown = apply_tashkeel(&shown, state.show_tashkeel).into_owned();
    let nav = row![
        button(ticon("next", 18.0)).on_press(Message::ReaderNext),
        text(label).font(AMIRI).size(16.0),
        button(ticon("prev", 18.0)).on_press(Message::ReaderPrev),
        button(ticon("lang", 18.0)).on_press(Message::TranslatePage),
        button(ar(target.to_uppercase().as_str(), 13.0)).on_press(Message::CycleTarget),
    ]
    .spacing(8);
    let mut note_items: Vec<Element<'_, Message>> = vec![ar(tr(state.lang, "notes"), 14.0)];
    for note in &state.notes {
        note_items.push(
            row![
                ar(note.text.as_str(), item_size(font_size)),
                button(ticon("x", 14.0)).on_press(Message::NoteDelete(note.id)),
            ]
            .spacing(4)
            .into(),
        );
    }
    note_items.push(
        row![
            text_input(tr(state.lang, "note_hint"), state.note_input.as_str())
                .on_input(Message::NoteInput)
                .padding(6)
                .width(Length::Fill),
            button(ticon("plus", 16.0)).on_press(Message::NoteSave),
        ]
        .spacing(4)
        .into(),
    );
    let page_scroll: Element<'_, Message> = scrollable(
        text(shown)
            .font(AMIRI)
            .size(font_size)
            .shaping(Shaping::Advanced)
            .width(Length::Fill),
    )
    .height(if state.show_notes {
        Length::FillPortion(3)
    } else {
        Length::Fill
    })
    .into();
    let mut page_parts: Vec<Element<'_, Message>> = vec![nav.into(), page_scroll];
    if state.show_notes {
        page_parts.push(
            scrollable(column(note_items).spacing(2))
                .height(Length::FillPortion(1))
                .into(),
        );
    }
    let page_view = column(page_parts).spacing(8).width(Length::Fill);

    fn flatten<'a>(nodes: &'a [TocNode], depth: usize, out: &mut Vec<(usize, &'a TocNode)>) {
        for node in nodes {
            out.push((depth, node));
            flatten(&node.children, depth + 1, out);
        }
    }
    let mut flat: Vec<(usize, &TocNode)> = Vec::new();
    flatten(&tab.toc, 0, &mut flat);
    let here = tab
        .pages
        .get(tab.index)
        .map(|page| page.id.to_string())
        .unwrap_or_default();
    let entries: Vec<Element<'static, Message>> = flat
        .into_iter()
        .map(|(depth, node)| {
            let label = format!("{}{}", "  ".repeat(depth), node.content);
            let selected = !here.is_empty() && node.page.as_deref() == Some(here.as_str());
            match node
                .page
                .as_deref()
                .and_then(|raw| tab.pages.iter().position(|page| page.id.to_string() == raw))
            {
                Some(index) => list_row(
                    label,
                    selected,
                    Message::TocJump(index),
                    (font_size - 4.0).max(12.0),
                ),
                None => ar(label.as_str(), (font_size - 4.0).max(12.0)),
            }
        })
        .collect();
    if !state.show_toc {
        return column![page_view].height(Length::Fill).into();
    }
    let toc_view = scrollable(column(entries).spacing(2)).height(Length::Fill);
    row![page_view, toc_view]
        .spacing(8)
        .height(Length::Fill)
        .into()
}

fn reader(state: &State) -> Element<'_, Message> {
    let tabs: Vec<Element<'_, Message>> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let label = format!("#{num}", num = index + 1);
            row![
                button(ticon("display", 14.0)).on_press(Message::TabSelected(index)),
                button(ar(label.as_str(), 13.0)).on_press(Message::TabSelected(index)),
                button(ticon("x", 14.0)).on_press(Message::TabClosed(index)),
            ]
            .spacing(2)
            .into()
        })
        .collect();
    let tab_bar = row(tabs).spacing(4);
    let content: Element<'_, Message> =
        match state.active_tab.and_then(|index| state.tabs.get(index)) {
            Some(tab) => match &tab.kind {
                TabKind::Quran { sura, verses } => {
                    let name = shamela_content::quran::SURA_NAMES
                        .get(*sura as usize - 1)
                        .unwrap_or(&"?");
                    let head = format!("{name} ({})", verses.len());
                    let body_text = render_sura(verses, state.show_tashkeel);
                    column![
                        row![
                            tool_btn("bkmrk", Message::BookmarkAdd),
                            tool_btn("x", Message::TabClosed(state.active_tab.unwrap_or(0))),
                        ]
                        .spacing(8),
                        row![
                            button(ticon("prev", 18.0)).on_press(Message::ReaderPrev),
                            button(ticon("next", 18.0)).on_press(Message::ReaderNext),
                        ]
                        .spacing(8),
                        scrollable(
                            text(format!("{head}\n\n{body_text}"))
                                .font(AMIRI)
                                .size(state.font_size)
                                .shaping(Shaping::Advanced)
                                .width(Length::Fill),
                        )
                        .height(Length::Fill),
                    ]
                    .spacing(8)
                    .into()
                }
                TabKind::Card(body) => column![
                    row![
                        tool_btn("bkmrk", Message::BookmarkAdd),
                        tool_btn("x", Message::TabClosed(state.active_tab.unwrap_or(0))),
                    ]
                    .spacing(8),
                    scrollable(
                        text(apply_tashkeel(body, state.show_tashkeel).into_owned())
                            .font(AMIRI)
                            .size(state.font_size)
                            .shaping(Shaping::Advanced)
                            .width(Length::Fill),
                    )
                    .height(Length::Fill),
                ]
                .spacing(8)
                .into(),
                TabKind::Reader(reader_tab) => column![
                    row![
                        tool_btn("bkmrk", Message::BookmarkAdd),
                        tool_btn("x", Message::TabClosed(state.active_tab.unwrap_or(0))),
                    ]
                    .spacing(8),
                    reader_tab_view(state, reader_tab),
                ]
                .spacing(8)
                .into(),
            },
            None => ar(tr(state.lang, "choose_book"), state.font_size),
        };
    column![tab_bar, content]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .into()
}

fn welcome(state: &State) -> Element<'static, Message> {
    let open = button(ar(tr(state.lang, "open_db"), 18.0))
        .on_press(Message::OpenDatabase)
        .width(Length::Fixed(300.0));
    let import = button(ar(tr(state.lang, "import_bok"), 18.0))
        .on_press(Message::ImportBok)
        .width(Length::Fixed(300.0));
    let downloads = button(ar(tr(state.lang, "download_from_net"), 18.0))
        .on_press(Message::PanelSelected(Panel::Downloads))
        .width(Length::Fixed(300.0));
    let logo = Svg::new(Handle::from_memory(
        &include_bytes!("../assets/icons/display.svg")[..],
    ))
    .width(Length::Fixed(110.0))
    .height(Length::Fixed(110.0));
    let body = column![
        logo,
        ar(tr(state.lang, "welcome"), 15.0),
        open,
        import,
        downloads,
    ]
    .spacing(12)
    .width(Length::Shrink);
    row![
        Space::with_width(Length::Fill),
        container(body).padding(24),
        Space::with_width(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

fn status_bar(state: &State) -> Element<'_, Message> {
    row![
        Space::with_width(Length::Fill),
        ar(state.status.as_str(), 13.0),
    ]
    .padding(6)
    .into()
}

/// Settings popup: every option works (display, search, data).
fn settings_modal(state: &State) -> Element<'_, Message> {
    let title = row![
        ticon("options", 22.0),
        ar(tr(state.lang, "settings"), 18.0),
        Space::with_width(Length::Fill),
        button(ticon("x", 18.0)).on_press(Message::ToggleSettings),
    ]
    .spacing(8);
    let font_row = row![
        button(ar("A-", 14.0)).on_press(Message::FontSmaller),
        ar(format!("{}", state.font_size as i64), 15.0),
        button(ar("A+", 14.0)).on_press(Message::FontBigger),
        button(ticon(
            match state.theme_kind {
                AppTheme::Light => "moon",
                AppTheme::Dark => "sun",
            },
            20.0
        ))
        .on_press(Message::ThemeToggle),
        button(ar(state.lang.code(), 14.0)).on_press(Message::LangCycle),
    ]
    .spacing(8);
    let card = column![
        title,
        ar(tr(state.lang, "display"), 16.0),
        font_row,
        toggle_row(
            tr(state.lang, "show_tashkeel").to_string(),
            state.show_tashkeel,
            Message::ToggleTashkeel
        ),
        ar(tr(state.lang, "search_options"), 16.0),
        toggle_row(
            tr(state.lang, "unify_hamza").to_string(),
            state.search_opts.unify_hamza,
            Message::ToggleOptHamza
        ),
        toggle_row(
            tr(state.lang, "ignore_tashkeel").to_string(),
            state.search_opts.strip_tashkeel,
            Message::ToggleOptTashkeel
        ),
        toggle_row(
            tr(state.lang, "strip_tatweel").to_string(),
            state.search_opts.strip_tatweel,
            Message::ToggleOptTatweel
        ),
        ar(tr(state.lang, "data_section"), 16.0),
        button(ar(tr(state.lang, "clear_cache"), 14.0))
            .on_press(Message::ClearCache)
            .width(Length::Fill),
        button(ar(tr(state.lang, "reset_all"), 14.0))
            .on_press(Message::ResetAll)
            .width(Length::Fill),
    ]
    .spacing(8)
    .padding(4)
    .width(Length::Fixed(430.0));
    let boxed = container(card)
        .padding(18)
        .width(Length::Fixed(466.0))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 1.0))),
            ..container::Style::default()
        });
    container(column![
        Space::with_height(Length::Fill),
        row![
            Space::with_width(Length::Fill),
            boxed,
            Space::with_width(Length::Fill),
        ],
        Space::with_height(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
        ..container::Style::default()
    })
    .into()
}

/// Toggle row with ASCII state box (always renders) plus label.
fn toggle_row(label: String, on: bool, msg: Message) -> Element<'static, Message> {
    let mark = if on { "[x]" } else { "[ ]" };
    button(ar(format!("{mark} {label}"), 15.0))
        .on_press(msg)
        .style(button::text)
        .width(Length::Fill)
        .padding(6)
        .into()
}

/// Global hotkeys mirroring the classic program (F5/F6 pages, F3 search,
/// F10 import dialog, Esc closes the current tab).
fn hotkey(key: keyboard::Key, _mods: keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::key::Named;
    match key {
        keyboard::Key::Named(Named::F5) => Some(Message::ReaderNext),
        keyboard::Key::Named(Named::F6) => Some(Message::ReaderPrev),
        keyboard::Key::Named(Named::F3) => Some(Message::PanelSelected(Panel::Library)),
        keyboard::Key::Named(Named::F10) => Some(Message::ImportBok),
        keyboard::Key::Named(Named::Escape) => Some(Message::CloseActiveTab),
        _ => None,
    }
}

fn view(state: &State) -> Element<'_, Message> {
    let top = tool_bar(state);
    let body: Element<'_, Message> = if state.lib.is_none() && state.tabs.is_empty() {
        welcome(state)
    } else {
        pane_grid::PaneGrid::new(&state.panes, |_pane, content, _| {
            pane_grid::Content::new(match content {
                PaneKind::Reader => reader(state),
                PaneKind::Books => books_panel(state),
                PaneKind::Side => sidebar(state),
            })
        })
        .spacing(6)
        .height(Length::Fill)
        .on_resize(10.0, Message::PaneResized)
        .into()
    };
    let content = column![top, body, status_bar(state)];
    if state.show_settings {
        Stack::new()
            .push(content)
            .push(settings_modal(state))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        content.into()
    }
}

fn theme(state: &State) -> Theme {
    match state.theme_kind {
        AppTheme::Light => Theme::Light,
        AppTheme::Dark => Theme::Dark,
    }
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--self-test" {
        std::process::exit(self_test(&args[2], &args[3]));
    }
    iced::application("ShamelaCraft", update, view)
        .subscription(|_| iced::keyboard::on_key_press(hotkey))
        .theme(theme)
        .font(include_bytes!("../assets/fonts/Amiri-Regular.ttf"))
        .run()
}
