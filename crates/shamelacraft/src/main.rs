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
use iced::widget::text::{LineHeight, Shaping};
use iced::widget::tooltip::{Position, Tooltip};
use iced::widget::{
    button, column, container, progress_bar, row, scrollable, text, text_input, Space, Stack,
};
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
use std::sync::Arc;

/// Project pages opened by Help / Live update.
const HELP_URL: &str = "https://github.com/iflyabd/shamelaCraft#readme";
const RELEASES_URL: &str = "https://github.com/iflyabd/shamelaCraft/releases";

/// Bundled Arabic font (Amiri, SIL OFL — see assets/fonts/OFL.txt).
const AMIRI: Font = Font::with_name("Amiri");
const SEARCH_LIMIT: i64 = 100;
const MIN_QUERY_CHARS: usize = 2;

fn about_text(lang: Lang) -> &'static str {
    match lang {
        Lang::Ar => {
            "v0.2 ShamelaCraft - قارئ المكتبة العربية الخفيف\n\n\
    - فهرس master.db للقراءة فقط (تصنيفات، كتب، مؤلفون)\n\
    - استيراد BOK القديم بقارئ Jet خالص بلغة Rust\n\
    - تحميل من archive.org بلا مفاتيح (bok, zip, tar, rar)\n\
    - مدير تحميلات: قائمة انتظار، تقدم، إلغاء وإعادة\n\
    - بحث عربي متسامح (تشكيل، همزة، تطويل)\n\
    - تبويبات، إشارات مرجعية، ملاحظات، تصدير Markdown\n\
    - قرآن عثماني في نافذة القراءة + تسعة تفاسير\n\
    - إعدادات محفوظة: خط، ألوان، تباعد، سمة، خيارات البحث\n\n\
    - الفكرة: عبد القادر الجزائري (الصقر القرشي)\n\
    - Rust نظيف، بلا كود أو أصول مملوكة"
        }
        Lang::En => {
            "v0.2 ShamelaCraft - lightweight Arabic library reader\n\n\
    - master.db read-only catalog (categories, books, authors)\n\
    - legacy BOK import, pure-Rust Jet reader\n\
    - archive.org keyless downloads (bok, zip, tar, rar)\n\
    - download manager: queue, progress, cancel, retry\n\
    - tolerant Arabic search (tashkeel, hamza, tatweel)\n\
    - tabs, bookmarks, notes, markdown export\n\
    - Uthmani Quran in the reading pane + nine tafsirs\n\
    - persistent settings: font, colors, spacing, theme, search\n\n\
    - idea: Abdelkader El Djazairi (Arab Falcon / Saqr Quraish)\n\
    - clean-room Rust, no proprietary code or assets"
        }
        Lang::Fr => {
            "v0.2 ShamelaCraft - lecteur arabe leger de bibliotheque\n\n\
    - catalogue master.db en lecture seule\n\
    - import BOK patrimonial, lecteur Jet 100% Rust\n\
    - telechargements archive.org sans cle\n\
    - gestionnaire: file d'attente, progression, annulation\n\
    - recherche arabe tolerante\n\
    - onglets, favoris, notes, export markdown\n\
    - Coran uthmani dans la zone de lecture + neuf tafsirs\n\
    - reglages persistants: police, couleurs, interligne, theme\n\n\
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

/// Interface accent presets (all white-text safe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Accent {
    #[default]
    Royal,
    Emerald,
    Maroon,
    Amber,
}

impl Accent {
    fn color(self) -> Color {
        match self {
            Self::Royal => Color::from_rgb(0.20, 0.35, 0.86),
            Self::Emerald => Color::from_rgb(0.05, 0.45, 0.28),
            Self::Maroon => Color::from_rgb(0.55, 0.13, 0.18),
            Self::Amber => Color::from_rgb(0.72, 0.53, 0.10),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Royal => "Royal",
            Self::Emerald => "Emerald",
            Self::Maroon => "Maroon",
            Self::Amber => "Amber",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Royal => Self::Emerald,
            Self::Emerald => Self::Maroon,
            Self::Maroon => Self::Amber,
            Self::Amber => Self::Royal,
        }
    }
}

/// Resolved chrome: accent color + corner radius.
#[derive(Debug, Clone, Copy)]
struct Chrome {
    accent: Color,
    radius: f32,
}

/// Accent button style used for toolbar, tabs and selected rows.
fn accent_style(accent: Color, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, _| button::Style {
        background: Some(Background::Color(accent)),
        text_color: Color::WHITE,
        border: iced::border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: iced::border::Radius::new(radius),
        },
        ..button::Style::default()
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
    /// Local SQLite file when the book came from a downloaded/imported file.
    db: Option<PathBuf>,
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
    DownloadAll,
    DownloadOpen,
    DlTick,
    DlToggle,
    DlRemove(usize),
    DlRetry(usize),
    DlCancel,
    DownloadCurrent,
    DlToggleFile(String),
    DlSelectAll,
    DlSelectNone,
    DlDownloadSelected,
    DlFilterInput(String),
    DlClearFinished,
    DlOpenFolder,
    DlPresetSource(String),
    AccentNext,
    ToggleCorners,
    LineBigger,
    LineSmaller,
    ReaderFgNext,
    ReaderBgNext,
    TafsirCycle,
    TafsirToggle,
    OpenReleases,
    OpenHelp,
    SharhLink,
    EscapePressed,
    ToggleReopenLast,
    ToggleNotifySearch,
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
    OpenImported(String),
    FindPage,
    MenuToggle(usize),
    QuitApp,
    GoToToggle,
    GoToClose,
    GoToInput(String),
    GoToSubmit,
    FindToggle,
    FindClose,
    FindInput(String),
    FindNext,
    FindPrev,
    DeleteBook(String),
}

/// What to do with a file once its download finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterDl {
    ImportBok,
    ImportArchive,
}

/// One background download job (progress shared with its worker thread).
struct QueueItem {
    id: usize,
    label: String,
    url: String,
    dest: PathBuf,
    after: AfterDl,
    progress: Arc<shamela_import::download::DlProgress>,
    started: bool,
    done: bool,
    failed: Option<String>,
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
    tafsir_ix: usize,
    show_tafsir: bool,
    tafsir_for: Option<i64>,
    tafsir_lines: Vec<String>,
    note_input: String,
    notes: Vec<shamela_content::notes::Note>,
    panes: pane_grid::State<PaneKind>,
    show_toc: bool,
    show_notes: bool,
    show_tashkeel: bool,
    show_settings: bool,
    accent: Accent,
    rounded: bool,
    line_height: f32,
    reader_text: Color,
    reader_bg: Color,
    notify_search: bool,
    reopen_last: bool,
    search_opts: NormalizeOptions,
    control_files: Vec<(String, u64)>,
    author_names: std::collections::HashMap<i64, String>,
    target_lang: String,
    dl_id: String,
    dl_files: Vec<shamela_import::archive::ArchiveFile>,
    dl_selected: std::collections::BTreeSet<String>,
    dl_filter: String,
    dl_queue: Vec<QueueItem>,
    dl_next_id: usize,
    show_downloads: bool,
    menu_open: Option<usize>,
    show_goto: bool,
    goto_input: String,
    show_find: bool,
    find_input: String,
    find_hits: Vec<usize>,
    find_pos: usize,
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
            tafsir_ix: 0,
            show_tafsir: false,
            tafsir_for: None,
            tafsir_lines: Vec::new(),
            note_input: String::new(),
            notes: Vec::new(),
            panes: init_panes(),
            show_toc: true,
            show_notes: true,
            show_tashkeel: true,
            show_settings: false,
            accent: Accent::Royal,
            rounded: true,
            line_height: 1.6,
            reader_text: Color::BLACK,
            reader_bg: Color::WHITE,
            notify_search: true,
            reopen_last: true,
            search_opts: NormalizeOptions::default(),
            control_files: Vec::new(),
            author_names: std::collections::HashMap::new(),
            target_lang: String::from("en"),
            dl_id: String::from("mutunulhadith"),
            dl_queue: Vec::new(),
            dl_next_id: 1,
            show_downloads: false,
            menu_open: None,
            show_goto: false,
            goto_input: String::new(),
            show_find: false,
            find_input: String::new(),
            find_hits: Vec::new(),
            find_pos: 0,
            dl_files: Vec::new(),
            dl_selected: std::collections::BTreeSet::new(),
            dl_filter: String::new(),
            font_size: 24.0,
            theme_kind: AppTheme::Light,
            status: String::from("افتح ملف master.db لبدء التصفح"),
        };
        load_settings(&mut state);
        if state.reopen_last {
            if let Ok(saved) = std::fs::read_to_string(last_db_path()) {
                let path = PathBuf::from(saved.trim());
                if path.is_file() {
                    open_database_path(&mut state, Some(path));
                }
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

/// Rough rendered width (px) of a mixed Arabic/Latin string in Amiri at
/// `size`. Overestimating only adds harmless padding.
fn text_width_est(content: &str, size: f32) -> f32 {
    let mut units = 0.0f32;
    for ch in content.chars() {
        units += match ch {
            ' ' => 0.30,
            '\u{0600}'..='\u{06FF}' | '\u{FB50}'..='\u{FEFF}' => 0.52,
            'a'..='z' | 'A'..='Z' => 0.62,
            '0'..='9' => 0.55,
            _ => 0.45,
        };
    }
    (units * size + 12.0).max(size * 2.0)
}

/// Label for shrink-sized controls (button content, tabs, rows followed by
/// more elements). iced lays RTL glyphs out right-aligned inside the layout
/// limits the text receives; a shrink-sized text can be measured against a
/// much wider limit than its own bounds, pushing the glyphs outside the
/// control. A fixed estimated width keeps the glyphs inside the box.
fn ar_boxed(content: impl Into<String>, size: f32) -> Element<'static, Message> {
    let content = content.into();
    let width = text_width_est(&content, size);
    text(content)
        .font(AMIRI)
        .size(size)
        .shaping(Shaping::Advanced)
        .width(Length::Fixed(width))
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
fn list_row(
    label: String,
    selected: bool,
    msg: Message,
    size: f32,
    chrome: Chrome,
) -> Element<'static, Message> {
    let content = button(ar(label, size))
        .on_press(msg)
        .width(Length::Fill)
        .padding(6);
    if selected {
        content
            .style(accent_style(chrome.accent, chrome.radius))
            .into()
    } else {
        content.style(button::text).into()
    }
}

impl State {
    fn chrome(&self) -> Chrome {
        Chrome {
            accent: self.accent.color(),
            radius: if self.rounded { 8.0 } else { 1.5 },
        }
    }
}

/// Icon toolbar button with bilingual hover label.
fn tool_btn(
    icon: &'static str,
    tip: (&'static str, &'static str),
    msg: Message,
    chrome: Chrome,
) -> Element<'static, Message> {
    let label = format!("{} / {}", tip.0, tip.1);
    Tooltip::new(
        button(ticon(icon, 22.0))
            .on_press(msg)
            .padding(5)
            .style(accent_style(chrome.accent, chrome.radius)),
        ar(label, 13.0),
        Position::Bottom,
    )
    .gap(8.0)
    .into()
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

fn settings_path() -> PathBuf {
    data_dir().join("settings.txt")
}

/// Persist every user-tweakable option (mirrors the original options dialog).
fn save_settings(state: &State) {
    let theme = match state.theme_kind {
        AppTheme::Light => "light",
        AppTheme::Dark => "dark",
    };
    let lang = match state.lang {
        Lang::Ar => "ar",
        Lang::En => "en",
        Lang::Fr => "fr",
    };
    let accent = match state.accent {
        Accent::Royal => "royal",
        Accent::Emerald => "emerald",
        Accent::Maroon => "maroon",
        Accent::Amber => "amber",
    };
    let body = format!(
        "font={}\ntheme={theme}\nlang={lang}\naccent={accent}\nrounded={}\nline={:.1}\n\ntashkeel={}\nhamza={}\nsearch_tashkeel={}\ntatweel={}\nreopen={}\nnotify={}\n",
        state.font_size,
        u8::from(state.rounded),
        state.line_height,
        u8::from(state.show_tashkeel),
        u8::from(state.search_opts.unify_hamza),
        u8::from(state.search_opts.strip_tashkeel),
        u8::from(state.search_opts.strip_tatweel),
        u8::from(state.reopen_last),
        u8::from(state.notify_search),
    );
    let _ = std::fs::write(settings_path(), body);
}

fn load_settings(state: &mut State) {
    let Ok(text) = std::fs::read_to_string(settings_path()) else {
        return;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "font" => {
                if let Ok(size) = value.parse::<f32>() {
                    state.font_size = size.clamp(14.0, 34.0);
                }
            }
            "theme" => {
                state.theme_kind = if value == "dark" {
                    AppTheme::Dark
                } else {
                    AppTheme::Light
                };
            }
            "lang" => {
                state.lang = match value {
                    "en" => Lang::En,
                    "fr" => Lang::Fr,
                    _ => Lang::Ar,
                };
            }
            "accent" => {
                state.accent = match value {
                    "emerald" => Accent::Emerald,
                    "maroon" => Accent::Maroon,
                    "amber" => Accent::Amber,
                    _ => Accent::Royal,
                };
            }
            "rounded" => state.rounded = value == "1",
            "line" => {
                if let Ok(height) = value.parse::<f32>() {
                    state.line_height = height.clamp(1.0, 2.4);
                }
            }
            "tashkeel" => state.show_tashkeel = value == "1",
            "hamza" => state.search_opts.unify_hamza = value == "1",
            "search_tashkeel" => state.search_opts.strip_tashkeel = value == "1",
            "tatweel" => state.search_opts.strip_tatweel = value == "1",
            "reopen" => state.reopen_last = value == "1",
            "notify" => state.notify_search = value == "1",
            _ => {}
        }
    }
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
        .add_books(&docs, opts)
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
                    let author_names: std::collections::HashMap<i64, String> =
                        lib.author_names().unwrap_or_default().into_iter().collect();
                    state.lib = Some(lib);
                    state.author_names = author_names;
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
    if state.notify_search {
        state.status = format!(
            "{} books / {} authors",
            state.results.len(),
            state.author_results.len()
        );
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
        db: None,
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
        db: None,
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
        db: None,
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
        Ok(()) => open_imported_db(state, &out, title, book_id, None),
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
    db_path: Option<PathBuf>,
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
        db: db_path,
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
fn open_imported_db(
    state: &mut State,
    out: &std::path::Path,
    title: String,
    book_id: i64,
    origin: Option<String>,
) {
    if let Some(url) = origin {
        if let Some(stem) = out.file_stem().and_then(|s| s.to_str()) {
            let _ = std::fs::write(books_dir().join(format!("{stem}.db.url")), url);
        }
    }
    let (pages, toc) = match BookDb::open(out) {
        Ok(db) => (db.pages().unwrap_or_default(), db.toc().unwrap_or_default()),
        Err(err) => {
            state.status = format!("open converted failed: {err}");
            return;
        }
    };
    open_imported_pages(state, pages, toc, title, book_id, Some(out.to_path_buf()));
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
            state.dl_selected.clear();
            state.dl_filter.clear();
            state.status = format!("item {id}: {count} importable files");
        }
        Err(err) => {
            state.dl_files.clear();
            state.status = format!("fetch failed: {err}");
        }
    }
}

/// Queue a download; starts it immediately when the lane is free.
fn enqueue(state: &mut State, label: String, url: String, dest: PathBuf, after: AfterDl) {
    let id = state.dl_next_id;
    state.dl_next_id += 1;
    state.dl_queue.push(QueueItem {
        id,
        label,
        url,
        dest,
        after,
        progress: shamela_import::download::DlProgress::shared(),
        started: false,
        done: false,
        failed: None,
    });
    start_next(state);
}

/// Start the first queued job when no job is active.
///
/// `started` is set before the worker is spawned; polling the byte counter
/// instead would leave a window in which a second worker truncates and
/// rewrites the same file while the first one is already running.
fn start_next(state: &mut State) {
    if state
        .dl_queue
        .iter()
        .any(|job| job.started && !job.done && job.failed.is_none())
    {
        return;
    }
    if let Some(job) = state
        .dl_queue
        .iter_mut()
        .find(|job| !job.started && !job.done && job.failed.is_none())
    {
        job.started = true;
        let url = job.url.clone();
        let dest = job.dest.clone();
        let prog = job.progress.clone();
        shamela_import::download::start_download(url, dest, prog);
    }
}

/// Anything still to do?
fn dl_active(state: &State) -> bool {
    state
        .dl_queue
        .iter()
        .any(|job| job.started && !job.done && job.failed.is_none())
}

/// Mark a queue job failed and report it in the status bar.
fn fail_job(state: &mut State, index: usize, message: String) {
    state.status = format!("download failed: {message}");
    state.dl_queue[index].failed = Some(message);
    state.dl_queue[index].done = true;
}

/// Poll worker threads; finalize finished jobs (import/convert).
fn poll_downloads(state: &mut State) {
    use std::sync::atomic::Ordering;
    let mut finished = Vec::new();
    for job in &state.dl_queue {
        if !job.done && job.failed.is_none() && job.progress.finished.load(Ordering::SeqCst) {
            finished.push(job.id);
        }
    }
    for id in finished {
        finalize_download(state, id);
    }
    start_next(state);
}

fn finalize_download(state: &mut State, id: usize) {
    use std::sync::atomic::Ordering;
    let position = state.dl_queue.iter().position(|job| job.id == id);
    let Some(index) = position else {
        return;
    };
    if let Some(err) = state.dl_queue[index]
        .progress
        .failed
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
    {
        fail_job(state, index, err);
        return;
    }
    let written = state.dl_queue[index]
        .progress
        .written
        .load(Ordering::SeqCst);
    let total = state.dl_queue[index].progress.total.load(Ordering::SeqCst);
    if total > 0 && written < total {
        fail_job(
            state,
            index,
            format!("incomplete download: {written}/{total} bytes"),
        );
        return;
    }
    let (dest, after, url) = {
        let job = &state.dl_queue[index];
        (job.dest.clone(), job.after, job.url.clone())
    };
    match after {
        AfterDl::ImportBok => {
            let stem = dest
                .file_stem()
                .and_then(|part| part.to_str())
                .unwrap_or("book")
                .to_string();
            let out = books_dir().join(format!("{stem}.db"));
            match shamela_import::bok::open_bok(&dest) {
                Ok(book) => {
                    let title = if book.meta.title.is_empty() {
                        stem.clone()
                    } else {
                        book.meta.title.clone()
                    };
                    let book_id = book.meta.id;
                    match shamela_import::convert::bok_to_book_db(&book, &out) {
                        Ok(()) => {
                            write_sidecar(&dest, &url);
                            open_imported_db(state, &out, title, book_id, Some(url));
                        }
                        Err(err) => {
                            fail_job(state, index, format!("convert failed: {err}"));
                        }
                    }
                }
                Err(err) => {
                    fail_job(state, index, format!("BOK read failed: {err}"));
                }
            }
        }
        AfterDl::ImportArchive => {
            let stem = dest
                .file_stem()
                .and_then(|part| part.to_str())
                .unwrap_or("book");
            let dir = books_dir().join(format!("{stem}_files"));
            match shamela_import::extract::extract_archive(&dest, &dir) {
                Ok(boks) if !boks.is_empty() => {
                    write_sidecar(&dest, &url);
                    import_bok_list(state, &boks, &url);
                }
                Ok(_) => {
                    fail_job(state, index, String::from("no .bok inside"));
                }
                Err(err) => {
                    fail_job(state, index, format!("extract failed: {err}"));
                }
            }
        }
    }
    let failed = state.dl_queue[index].failed.is_some();
    state.dl_queue[index].done = true;
    let _ = failed;
    let _ = Ordering::SeqCst;
}

/// Remember where a downloaded file came from (per-opened-book downloads).
fn write_sidecar(dest: &std::path::Path, url: &str) {
    if let Some(name) = dest.file_name().and_then(|n| n.to_str()) {
        let sidecar = books_dir().join(format!("{name}.url"));
        let _ = std::fs::write(sidecar, url);
    }
}

/// Import every converted book; open the first.
fn import_bok_list(state: &mut State, boks: &[std::path::PathBuf], url: &str) {
    let mut opened = false;
    let mut count = 0;
    for bok in boks {
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
        count += 1;
        if !opened {
            open_imported_db(state, &out, title, book.meta.id, Some(url.to_string()));
            opened = true;
        }
    }
    if opened {
        state.status = format!("archive: {count} books imported");
    } else {
        state.status = String::from("archive: books found, none readable");
    }
}

fn downloads_dir() -> PathBuf {
    let dir = data_dir().join("downloads");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Archive vs single book by extension.
fn after_kind(name: &str) -> AfterDl {
    let lower = name.to_lowercase();
    if lower.ends_with(".rar")
        || lower.ends_with(".zip")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
    {
        AfterDl::ImportArchive
    } else {
        AfterDl::ImportBok
    }
}

/// Queue one file from the configured archive.org item.
fn enqueue_file(state: &mut State, name: String) {
    let item = state.dl_id.trim().to_string();
    if item.is_empty() {
        state.status = String::from("enter an archive.org item id first");
        return;
    }
    let url = shamela_import::archive::download_url(&item, &name);
    let file_name = name.rsplit('/').next().unwrap_or(&name).to_string();
    let dest = downloads_dir().join(&file_name);
    let after = after_kind(&file_name);
    enqueue(state, file_name.clone(), url, dest, after);
    state.status = format!("queued: {file_name}");
}

/// Parse an archive.org item id out of a bare id or a pasted URL.
fn parse_item_id(raw: &str) -> String {
    let text = raw.trim();
    if let Some(rest) = text.split("archive.org/details/").nth(1) {
        return rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or(rest)
            .to_string();
    }
    if let Some(rest) = text.split("archive.org/download/").nth(1) {
        return rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or(rest)
            .to_string();
    }
    text.trim_end_matches('/').to_string()
}

/// One hex digit of a `%XX` escape.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decode `%XX` escapes (recorded source URLs keep them, local file names
/// must not).
fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Loose name key for matching book titles and file names (drops tashkeel,
/// tatweel and everything that is not a letter or digit).
fn match_key(content: &str) -> String {
    content
        .chars()
        .filter(|ch| !matches!(ch, '\u{064B}'..='\u{0652}' | '\u{0640}' | '\u{0670}'))
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

/// "540 KB" / "12.3 MB" / "1.20 GB".
fn fmt_size(bytes: u64) -> String {
    let mib = bytes as f64 / 1_048_576.0;
    if mib >= 1024.0 {
        format!("{:.2} GB", mib / 1024.0)
    } else if mib >= 1.0 {
        format!("{mib:.1} MB")
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// Leaf file name, trimmed for display.
/// Trim text to `max` characters, adding an ellipsis when shortened.
fn trim_text(content: &str, max: usize) -> String {
    let mut out: String = content.chars().take(max).collect();
    if content.chars().count() > max {
        out.push('…');
    }
    out
}

/// Leaf file name, trimmed for display.
fn display_name(name: &str) -> String {
    trim_text(name.rsplit('/').next().unwrap_or(name), 38)
}

/// Short kind badge of a source file.
fn file_kind(name: &str, lang: Lang) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".bok") {
        tr(lang, "kind_book")
    } else if shamela_import::extract::is_importable(name) {
        tr(lang, "kind_pack")
    } else {
        tr(lang, "kind_unsupported")
    }
}

/// Source files filtered by the name filter (list order preserved).
fn visible_files(state: &State) -> Vec<shamela_import::archive::ArchiveFile> {
    let needle = match_key(&state.dl_filter);
    state
        .dl_files
        .iter()
        .filter(|file| needle.is_empty() || match_key(&file.name).contains(&needle))
        .cloned()
        .collect()
}

/// Queue every selected source file (list order).
fn download_selected(state: &mut State) {
    let names: Vec<String> = state
        .dl_files
        .iter()
        .filter(|file| state.dl_selected.contains(&file.name))
        .map(|file| file.name.clone())
        .collect();
    if names.is_empty() {
        state.status = String::from("nothing selected");
        return;
    }
    let count = names.len();
    for name in names {
        enqueue_file(state, name);
    }
    state.status = format!("queued {count} selected file(s)");
}

/// Download the book that is open right now: the recorded source URL of an
/// imported book when known, otherwise the best name match inside the listed
/// source item (real per-book download from a `.bok` collection).
fn download_current(state: &mut State) {
    let Some(index) = state.active_tab else {
        state.status = String::from("open a book first");
        return;
    };
    let Some(tab) = state.tabs.get(index) else {
        return;
    };
    let title = tab.title.clone();
    let db = tab.db.clone();

    if let Some(db) = db {
        let stem = db
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or("book")
            .to_string();
        if let Ok(url) = std::fs::read_to_string(books_dir().join(format!("{stem}.db.url"))) {
            let url = url.trim().to_string();
            if !url.is_empty() {
                let mut file_name = percent_decode(url.rsplit('/').next().unwrap_or(""));
                if file_name.is_empty() {
                    file_name = format!("{stem}.bok");
                }
                let dest = downloads_dir().join(&file_name);
                let after = after_kind(&file_name);
                enqueue(state, file_name.clone(), url, dest, after);
                state.status = format!("re-downloading: {file_name}");
                return;
            }
        }
    }

    let key = match_key(&title);
    if !key.is_empty() {
        let hit = state
            .dl_files
            .iter()
            .find(|file| {
                let stem = std::path::Path::new(&file.name)
                    .file_stem()
                    .and_then(|part| part.to_str())
                    .unwrap_or(&file.name);
                let stem_key = match_key(stem);
                !stem_key.is_empty() && (stem_key.contains(&key) || key.contains(&stem_key))
            })
            .cloned();
        if let Some(file) = hit {
            enqueue_file(state, file.name.clone());
            state.status = format!("queued book: {}", display_name(&file.name));
            return;
        }
    }

    state.status = String::from(tr(state.lang, "no_source_match"));
}

fn cancel_all(state: &mut State) {
    use std::sync::atomic::Ordering;
    let mut stopping = 0;
    let mut pending = Vec::new();
    for (index, job) in state.dl_queue.iter_mut().enumerate() {
        if job.done || job.failed.is_some() {
            continue;
        }
        if job.started {
            job.progress.cancel.store(true, Ordering::SeqCst);
            stopping += 1;
        } else {
            pending.push(index);
        }
    }
    let cancelled = pending.len();
    for index in pending {
        fail_job(state, index, String::from("cancelled"));
    }
    state.status = if stopping == 0 && cancelled == 0 {
        String::from("nothing to stop")
    } else {
        format!("stopping {stopping} download(s), cleared {cancelled} pending")
    };
}

fn remove_job(state: &mut State, id: usize) {
    use std::sync::atomic::Ordering;
    if let Some(index) = state.dl_queue.iter().position(|job| job.id == id) {
        if !state.dl_queue[index].done && state.dl_queue[index].failed.is_none() {
            state.dl_queue[index]
                .progress
                .cancel
                .store(true, Ordering::SeqCst);
        }
        state.dl_queue.remove(index);
        start_next(state);
    }
}

fn retry_job(state: &mut State, id: usize) {
    use std::sync::atomic::Ordering;
    if let Some(job) = state.dl_queue.iter_mut().find(|job| job.id == id) {
        job.progress.written.store(0, Ordering::SeqCst);
        job.progress.total.store(0, Ordering::SeqCst);
        job.progress.finished.store(false, Ordering::SeqCst);
        job.progress.cancel.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = job.progress.failed.lock() {
            *slot = None;
        }
        job.failed = None;
        job.started = false;
        job.done = false;
    }
    start_next(state);
}

fn tafsir_cache_dir() -> PathBuf {
    let dir = data_dir().join("tafsir");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn quran_active(state: &State) -> bool {
    matches!(
        state
            .active_tab
            .and_then(|index| state.tabs.get(index))
            .map(|tab| &tab.kind),
        Some(TabKind::Quran { .. })
    )
}

/// Load the selected tafsir for the active sura (cached on disk).
fn ensure_tafsir(state: &mut State) {
    if !state.show_tafsir {
        return;
    }
    let sura = state
        .active_tab
        .and_then(|index| state.tabs.get(index))
        .and_then(|tab| match &tab.kind {
            TabKind::Quran { sura, .. } => Some(*sura),
            _ => None,
        })
        .unwrap_or(state.quran_sura);
    load_tafsir(state, sura);
}

fn load_tafsir(state: &mut State, sura: i64) {
    if state.tafsir_for == Some(sura) && !state.tafsir_lines.is_empty() {
        return;
    }
    let tafsirs = shamela_import::tafsir_dl::TAFSIRS;
    let (slug, name) = tafsirs[state.tafsir_ix % tafsirs.len()];
    match shamela_import::tafsir_dl::load_cached(&tafsir_cache_dir(), slug, sura) {
        Ok(lines) => {
            state.tafsir_lines = lines;
            state.tafsir_for = Some(sura);
            state.status = format!("{name}: sura {sura}");
        }
        Err(err) => {
            state.status = format!("tafsir failed: {err}");
            state.tafsir_lines.clear();
            state.tafsir_for = Some(sura);
        }
    }
}

/// Commentary linking: show tafsir for Quran, search "sharh" for a book.
fn sharh_link(state: &mut State) {
    if quran_active(state) {
        state.show_tafsir = !state.show_tafsir;
        if state.show_tafsir {
            ensure_tafsir(state);
        }
        state.status = String::from("tafsir (commentary) toggled");
        return;
    }
    let Some((_, title, _)) = active_text(state) else {
        state.status = String::from("open a book first");
        return;
    };
    let stem = title.split_whitespace().next().unwrap_or("").to_string();
    if stem.is_empty() {
        state.status = String::from("open a book first");
        return;
    }
    state.panel = Panel::Library;
    state.query = format!("شرح {stem}");
    run_search(state);
}

/// Open a page in the system browser (help, releases).
fn open_external(url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = url;
    }
}

fn text_colors() -> [Color; 4] {
    [
        Color::BLACK,
        Color::from_rgb(0.25, 0.18, 0.10),
        Color::from_rgb(0.10, 0.18, 0.35),
        Color::from_rgb(0.42, 0.10, 0.12),
    ]
}

fn bg_colors() -> [Color; 4] {
    [
        Color::WHITE,
        Color::from_rgb(0.99, 0.97, 0.90),
        Color::from_rgb(0.95, 0.91, 0.82),
        Color::from_rgb(0.92, 0.94, 0.97),
    ]
}

fn next_color(list: &[Color], current: Color) -> Color {
    let index = list.iter().position(|color| *color == current).unwrap_or(0);
    list[(index + 1) % list.len()]
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
        db: None,
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
    let persist = matches!(
        &message,
        Message::FontBigger
            | Message::FontSmaller
            | Message::ThemeToggle
            | Message::LangCycle
            | Message::ToggleTashkeel
            | Message::ToggleOptHamza
            | Message::ToggleOptTashkeel
            | Message::ToggleOptTatweel
            | Message::AccentNext
            | Message::ToggleCorners
            | Message::LineBigger
            | Message::LineSmaller
            | Message::ReaderFgNext
            | Message::ReaderBgNext
            | Message::ToggleReopenLast
            | Message::ToggleNotifySearch
            | Message::ResetAll
    );
    let task = apply(state, message);
    if persist {
        save_settings(state);
    }
    task
}

fn apply(state: &mut State, message: Message) -> Task<Message> {
    if !matches!(&message, Message::MenuToggle(_)) {
        state.menu_open = None;
    }
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
            state.dl_id = parse_item_id(&id);
        }
        Message::DownloadList => refresh_download_list(state),
        Message::DownloadFile(name) => enqueue_file(state, name),
        Message::DownloadAll => {
            let files: Vec<String> = state.dl_files.iter().map(|f| f.name.clone()).collect();
            let count = files.len();
            for name in files {
                enqueue_file(state, name);
            }
            state.status = if count == 0 {
                String::from("no files listed — fetch a source first")
            } else {
                format!("queued all {count} file(s)")
            };
        }
        Message::DownloadCurrent => download_current(state),
        Message::DownloadOpen => {
            state.show_downloads = true;
        }
        Message::DlToggle => {
            state.show_downloads = !state.show_downloads;
        }
        Message::DlTick => poll_downloads(state),
        Message::DlRemove(id) => remove_job(state, id),
        Message::DlRetry(id) => retry_job(state, id),
        Message::DlCancel => cancel_all(state),
        Message::DlToggleFile(name) => {
            if !state.dl_selected.remove(&name) {
                state.dl_selected.insert(name);
            }
        }
        Message::DlSelectAll => {
            let files = visible_files(state);
            for file in files {
                state.dl_selected.insert(file.name);
            }
            state.status = format!("selected {} file(s)", state.dl_selected.len());
        }
        Message::DlSelectNone => {
            state.dl_selected.clear();
            state.status = String::from("selection cleared");
        }
        Message::DlDownloadSelected => download_selected(state),
        Message::DlFilterInput(text) => state.dl_filter = text,
        Message::DlClearFinished => {
            let before = state.dl_queue.len();
            state.dl_queue.retain(|job| !job.done);
            state.status = format!("cleared {} finished job(s)", before - state.dl_queue.len());
        }
        Message::DlOpenFolder => {
            let dir = downloads_dir();
            open_external(&dir.display().to_string());
            state.status = format!("downloads: {}", dir.display());
        }
        Message::DlPresetSource(id) => {
            state.dl_id = id;
            refresh_download_list(state);
        }
        Message::AccentNext => {
            state.accent = state.accent.next();
        }
        Message::ToggleCorners => {
            state.rounded = !state.rounded;
        }
        Message::LineBigger => {
            state.line_height = (state.line_height + 0.1).min(2.4);
        }
        Message::LineSmaller => {
            state.line_height = (state.line_height - 0.1).max(1.0);
        }
        Message::ReaderFgNext => {
            state.reader_text = next_color(&text_colors(), state.reader_text);
        }
        Message::ReaderBgNext => {
            state.reader_bg = next_color(&bg_colors(), state.reader_bg);
        }
        Message::TafsirCycle => {
            state.tafsir_ix = (state.tafsir_ix + 1) % shamela_import::tafsir_dl::TAFSIRS.len();
            state.tafsir_for = None;
            ensure_tafsir(state);
        }
        Message::TafsirToggle => {
            state.show_tafsir = !state.show_tafsir;
            if state.show_tafsir {
                ensure_tafsir(state);
            }
        }
        Message::OpenReleases => open_external(RELEASES_URL),
        Message::OpenHelp => open_external(HELP_URL),
        Message::SharhLink => sharh_link(state),
        Message::ToggleReopenLast => {
            state.reopen_last = !state.reopen_last;
        }
        Message::ToggleNotifySearch => {
            state.notify_search = !state.notify_search;
        }
        Message::EscapePressed => {
            if state.menu_open.is_some() {
                state.menu_open = None;
            } else if state.show_goto {
                state.show_goto = false;
            } else if state.show_find {
                state.show_find = false;
                state.find_hits.clear();
                state.find_pos = 0;
            } else if state.show_settings {
                state.show_settings = false;
            } else if state.show_downloads {
                state.show_downloads = false;
            } else if let Some(index) = state.active_tab {
                close_tab(state, index);
            }
        }
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
            state.font_size = 24.0;
            state.theme_kind = AppTheme::Light;
            state.show_tashkeel = true;
            state.accent = Accent::Royal;
            state.rounded = true;
            state.line_height = 1.6;
            state.reader_text = Color::BLACK;
            state.reader_bg = Color::WHITE;
            state.search_opts = NormalizeOptions::default();
            rebuild_index(state);
            state.status = String::from("settings reset");
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
            open_imported_db(state, &path, title, book_id, None);
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
        Message::MenuToggle(index) => {
            state.menu_open = if state.menu_open == Some(index) {
                None
            } else {
                Some(index)
            };
        }
        Message::QuitApp => std::process::exit(0),
        Message::GoToToggle => {
            state.show_goto = !state.show_goto;
            if state.show_goto {
                let current = active_reader(state)
                    .map(|reader| (reader.index + 1).to_string())
                    .unwrap_or_default();
                state.goto_input = current;
            }
        }
        Message::GoToClose => state.show_goto = false,
        Message::GoToInput(text) => state.goto_input = text,
        Message::GoToSubmit => {
            let pages = active_reader(state).map_or(0, |reader| reader.pages.len());
            if pages == 0 {
                state.status = String::from("open a book first");
            } else if let Some(page) = parse_number(&state.goto_input) {
                if (1..=pages).contains(&page) {
                    jump_to_page(state, page - 1);
                    state.show_goto = false;
                    state.status = format!("page {page} / {pages}");
                } else {
                    state.status = format!("page must be 1..{pages}");
                }
            } else {
                state.status = String::from("type a page number");
            }
        }
        Message::FindToggle => {
            state.show_find = !state.show_find;
            if state.show_find {
                run_find(state);
            } else {
                state.find_hits.clear();
                state.find_pos = 0;
            }
        }
        Message::FindClose => {
            state.show_find = false;
            state.find_hits.clear();
            state.find_pos = 0;
        }
        Message::FindInput(text) => {
            state.find_input = text;
            run_find(state);
        }
        Message::FindNext => find_step(state, 1),
        Message::FindPrev => find_step(state, -1),
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
    let chrome = state.chrome();
    let theme_icon = match state.theme_kind {
        AppTheme::Light => "moon",
        AppTheme::Dark => "sun",
    };
    let mut row_one_items: Vec<Element<'_, Message>> = vec![
        tool_btn(
            "selbk",
            ("Choose book", "اختيار كتاب"),
            Message::PanelSelected(Panel::Library),
            chrome,
        ),
        tool_btn(
            "display",
            ("Display book", "عرض الكتاب"),
            Message::FocusBook,
            chrome,
        ),
        tool_btn(
            "tree",
            ("Toggle index", "الفهرس"),
            Message::ToggleToc,
            chrome,
        ),
        tool_btn(
            "com",
            ("Toggle notes", "التعليقات"),
            Message::ToggleNotes,
            chrome,
        ),
        tool_btn(
            "searchpage",
            ("Find in page", "بحث في الصفحة"),
            Message::FindPage,
            chrome,
        ),
        tool_btn(
            "searchbok",
            ("Search this book", "بحث في الكتاب"),
            Message::SearchBook,
            chrome,
        ),
        tool_btn(
            "search",
            ("Global search", "بحث عام"),
            Message::PanelSelected(Panel::Library),
            chrome,
        ),
        tool_btn(
            "lastsearch",
            ("Last search", "البحث الأخير"),
            Message::LastSearch,
            chrome,
        ),
        tool_btn(
            "saved",
            ("Saved", "المحفوظ"),
            Message::PanelSelected(Panel::Bookmarks),
            chrome,
        ),
        tool_btn(
            "first",
            ("First page", "أول صفحة"),
            Message::PageFirst,
            chrome,
        ),
        tool_btn("prev", ("Previous", "السابق"), Message::ReaderPrev, chrome),
        tool_btn("next", ("Next", "التالي"), Message::ReaderNext, chrome),
        tool_btn("last", ("Last", "الأخير"), Message::PageLast, chrome),
        tool_btn(
            "dipquran",
            ("Quran", "القرآن"),
            Message::PanelSelected(Panel::Quran),
            chrome,
        ),
        tool_btn(
            "sharh",
            ("Commentary link", "ربط الشرح"),
            Message::SharhLink,
            chrome,
        ),
    ];
    row_one_items.reverse();
    row_one_items.insert(0, Space::with_width(Length::Fill).into());
    let row_one = row(row_one_items).spacing(2).padding(2);
    let mut row_two_items: Vec<Element<'_, Message>> = vec![
        tool_btn(
            "import",
            ("Import BOK", "استيراد"),
            Message::ImportBok,
            chrome,
        ),
        tool_btn(
            "ctr",
            ("Control room", "غرفة التحكم"),
            Message::PanelSelected(Panel::Control),
            chrome,
        ),
        tool_btn(
            "authors",
            ("Authors", "المؤلفون"),
            Message::PanelSelected(Panel::Authors),
            chrome,
        ),
        tool_btn(
            "copy",
            ("Copy page", "نسخ الصفحة"),
            Message::CopyPage,
            chrome,
        ),
        tool_btn(
            "bkmrk",
            ("Bookmark", "إشارة مرجعية"),
            Message::BookmarkAdd,
            chrome,
        ),
        tool_btn("export", ("Export", "تصدير"), Message::ExportBook, chrome),
        tool_btn(
            "betaka",
            ("Book card", "بطاقة الكتاب"),
            Message::OpenCard,
            chrome,
        ),
        tool_btn(
            "upgrade",
            ("Live update", "الترقية الحية"),
            Message::OpenReleases,
            chrome,
        ),
        tool_btn(
            "options",
            ("Settings", "الإعدادات"),
            Message::ToggleSettings,
            chrome,
        ),
        tool_btn("stop", ("Stop", "إيقاف"), Message::DlCancel, chrome),
        tool_btn(
            "globe",
            ("Downloads", "التحميل"),
            Message::DownloadOpen,
            chrome,
        ),
        tool_btn(
            theme_icon,
            ("Theme", "المظهر"),
            Message::ThemeToggle,
            chrome,
        ),
        button(ar(state.lang.code(), 14.0))
            .on_press(Message::LangCycle)
            .into(),
        tool_btn("help", ("Help", "مساعدة"), Message::OpenHelp, chrome),
        tool_btn(
            "plus",
            ("Open database", "فتح قاعدة"),
            Message::OpenDatabase,
            chrome,
        ),
        button(ar("A+", 14.0)).on_press(Message::FontBigger).into(),
        button(ar("A-", 14.0)).on_press(Message::FontSmaller).into(),
    ];
    row_two_items.reverse();
    row_two_items.insert(0, Space::with_width(Length::Fill).into());
    let row_two = row(row_two_items).spacing(2).padding(2);
    column![row_one, row_two].into()
}

/// Short author line for a book row (ids resolved through the library).
fn author_line(state: &State, authors: &str) -> String {
    let mut names: Vec<&str> = Vec::new();
    for id in authors.split(',') {
        let Ok(id) = id.trim().parse::<i64>() else {
            continue;
        };
        let Some(name) = state.author_names.get(&id) else {
            continue;
        };
        names.push(name.as_str());
        if names.len() == 2 {
            break;
        }
    }
    names.join(" • ")
}

/// Book row: title with a smaller author line underneath.
fn book_row<'a>(state: &'a State, book: &Book) -> Element<'a, Message> {
    let size = item_size(state.font_size);
    let authors = author_line(state, &book.authors);
    let mut lines = column![ar(book.name.clone(), size)]
        .spacing(1)
        .width(Length::Fill);
    if !authors.is_empty() {
        lines = lines.push(
            text(authors)
                .font(AMIRI)
                .size(size - 3.0)
                .shaping(Shaping::Advanced),
        );
    }
    button(lines)
        .on_press(Message::BookOpened(book.id))
        .width(Length::Fill)
        .padding(5)
        .style(button::text)
        .into()
}

fn book_buttons<'a>(state: &'a State, books: &'a [Book]) -> Element<'a, Message> {
    if books.is_empty() {
        return ar(tr(state.lang, "no_results"), state.font_size);
    }
    let items: Vec<Element<'static, Message>> = books
        .iter()
        .take(200)
        .map(|book| book_row(state, book))
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
            state.chrome(),
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
            state.chrome(),
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
                    state.chrome(),
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

/// Curated keyless archive.org sources (id, Arabic label).
const DL_PRESETS: &[(&str, &str)] = &[
    ("mutunulhadith", "متون الحديث (١٧٢ كتاب)"),
    ("kotobe-mojahed-din-maktab-shamele", "مكتبة كتب — حزمة"),
    ("ShamelaLibrary348-1", "الشاملة الرسمية — حزمة"),
];

/// How many source-file rows each view draws (further rows need filtering).
const PANEL_FILE_ROWS: usize = 120;
const MODAL_FILE_ROWS: usize = 250;

/// One source-file row: selection box, download button, size, kind, name.
/// One source-file row: selection box, download button, size, kind, name.
fn file_row(
    state: &State,
    file: &shamela_import::archive::ArchiveFile,
) -> Element<'static, Message> {
    let selected = state.dl_selected.contains(&file.name);
    row![
        button(ar_boxed(if selected { "[x]" } else { "[ ]" }, 12.0))
            .on_press(Message::DlToggleFile(file.name.clone()))
            .style(button::text),
        button(ar_boxed(tr(state.lang, "download"), 12.0))
            .on_press(Message::DownloadFile(file.name.clone())),
        ar_boxed(fmt_size(file.size), 12.0),
        ar_boxed(file_kind(&file.name, state.lang), 12.0),
        Space::with_width(Length::Fill),
        ar_boxed(display_name(&file.name), 13.0),
    ]
    .spacing(6)
    .width(Length::Fill)
    .into()
}

/// Narrow variant for the side panel: the name lives in a fill row.
fn file_row_panel(
    state: &State,
    file: &shamela_import::archive::ArchiveFile,
) -> Element<'static, Message> {
    let selected = state.dl_selected.contains(&file.name);
    let label = format!("{} ({})", display_name(&file.name), fmt_size(file.size));
    row![
        button(ar_boxed(if selected { "[x]" } else { "[ ]" }, 12.0))
            .on_press(Message::DlToggleFile(file.name.clone()))
            .style(button::text),
        button(ar_boxed(tr(state.lang, "download"), 12.0))
            .on_press(Message::DownloadFile(file.name.clone())),
        list_row(
            label,
            false,
            Message::DownloadFile(file.name.clone()),
            item_size(state.font_size) - 2.0,
            state.chrome(),
        ),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn downloads_panel(state: &State) -> Element<'_, Message> {
    let lang = state.lang;
    let input = text_input(tr(lang, "source_hint"), state.dl_id.as_str())
        .on_input(Message::DownloadIdInput)
        .on_submit(Message::DownloadList)
        .padding(8);
    let fetch = button(ar_boxed(tr(lang, "fetch_books"), 14.0))
        .on_press(Message::DownloadList)
        .width(Length::Fill);
    let actions = row![
        button(ar_boxed(tr(lang, "download_all"), 14.0))
            .on_press(Message::DownloadAll)
            .width(Length::Fill),
        button(ar_boxed(tr(lang, "download_selected"), 14.0))
            .on_press(Message::DlDownloadSelected)
            .width(Length::Fill),
    ]
    .spacing(4);
    let actions2 = row![
        button(ar_boxed(tr(lang, "dl_queue"), 14.0))
            .on_press(Message::DownloadOpen)
            .width(Length::Fill),
        button(ar_boxed(tr(lang, "open_folder"), 14.0))
            .on_press(Message::DlOpenFolder)
            .width(Length::Fill),
    ]
    .spacing(4);
    let filter = text_input(tr(lang, "filter_files"), state.dl_filter.as_str())
        .on_input(Message::DlFilterInput)
        .padding(6);
    let mut items: Vec<Element<'_, Message>> = vec![
        ar(tr(lang, "downloads"), title_size(state.font_size)),
        input.into(),
        fetch.into(),
        actions.into(),
        actions2.into(),
        ar(tr(lang, "filter_word"), item_size(state.font_size) - 2.0),
        filter.into(),
    ];
    let visible = visible_files(state);
    let total = state.dl_files.len();
    if total == 0 {
        items.push(ar(tr(lang, "no_files"), item_size(state.font_size) - 2.0));
    } else {
        items.push(ar(
            format!("{} {}/{}", tr(lang, "shown_of"), visible.len(), total),
            item_size(state.font_size) - 2.0,
        ));
    }
    for file in visible.iter().take(PANEL_FILE_ROWS) {
        items.push(file_row_panel(state, file));
    }
    scrollable(column(items).spacing(6))
        .height(Length::Fill)
        .into()
}

/// Books of the picked section (middle sub-panel).
fn books_panel(state: &State) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> =
        vec![ar(tr(state.lang, "books"), title_size(state.font_size))];
    if state.books.is_empty() {
        let hint = if state.query.trim().chars().count() >= MIN_QUERY_CHARS {
            tr(state.lang, "no_results")
        } else {
            tr(state.lang, "pick_section")
        };
        items.push(ar(hint, item_size(state.font_size)));
    }
    for book in state.books.iter().take(200) {
        items.push(book_row(state, book));
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

/// Control room: imported book files with sizes and deletion.
fn control_panel(state: &State) -> Element<'_, Message> {
    let total: u64 = state.control_files.iter().map(|(_, size)| size).sum();
    let megabytes = total as f64 / 1_048_576.0;
    let mut items: Vec<Element<'_, Message>> = vec![
        ar(tr(state.lang, "control_room"), title_size(state.font_size)),
        ar(
            format!("{megabytes:.1} MB : {n}", n = state.control_files.len()),
            item_size(state.font_size),
        ),
    ];
    for (name, size) in &state.control_files {
        let megabytes = *size as f64 / 1_048_576.0;
        items.push(
            row![
                list_row(
                    format!("{name} ({megabytes:.1} MB)"),
                    false,
                    Message::OpenImported(name.clone()),
                    item_size(state.font_size),
                    state.chrome(),
                ),
                button(ticon("x", 16.0)).on_press(Message::DeleteBook(name.clone())),
            ]
            .spacing(4)
            .into(),
        );
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fill)
        .into()
}

/// First (sura, aya) of each of the 30 juz of the Madani mushaf.
const JUZ_STARTS: [(i64, i64); 30] = [
    (1, 1),
    (2, 142),
    (2, 253),
    (3, 92),
    (4, 24),
    (4, 148),
    (5, 82),
    (6, 111),
    (7, 88),
    (8, 41),
    (9, 93),
    (11, 6),
    (12, 53),
    (15, 1),
    (17, 1),
    (18, 75),
    (21, 1),
    (23, 1),
    (25, 21),
    (27, 56),
    (29, 46),
    (33, 31),
    (36, 28),
    (39, 32),
    (41, 47),
    (46, 1),
    (51, 31),
    (58, 1),
    (67, 1),
    (78, 1),
];

/// Juz number containing the opening of a sura.
fn juz_of_sura(sura: i64) -> i64 {
    JUZ_STARTS
        .iter()
        .rposition(|&(start, _)| start <= sura)
        .map_or(1, |index| index as i64 + 1)
}

/// Juz number when the given sura opens a new juz.
fn juz_starting_at(sura: i64) -> Option<i64> {
    JUZ_STARTS
        .iter()
        .position(|&(start, _)| start == sura)
        .map(|index| index as i64 + 1)
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
                    state.chrome(),
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
    let mut suras: Vec<Element<'_, Message>> = Vec::new();
    for (index, name) in shamela_content::quran::SURA_NAMES.iter().enumerate() {
        let id = index as i64 + 1;
        if let Some(juz) = juz_starting_at(id) {
            suras.push(
                container(ar_boxed(
                    format!("{} {juz}", tr(state.lang, "juz_word")),
                    12.0,
                ))
                .width(Length::Fill)
                .align_x(iced::Alignment::End)
                .into(),
            );
        }
        suras.push(list_row(
            format!("{id}. {name}"),
            id == state.quran_sura,
            Message::QuranSura(id),
            13.0,
            state.chrome(),
        ));
    }
    column![
        search,
        scrollable(column(suras).spacing(2)).height(Length::Fill)
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

/// Bottom tabs of the side pane.
fn panel_tabs(state: &State) -> Element<'_, Message> {
    let accent = state.accent.color();
    let radius = state.chrome().radius;
    let tabs = [
        ("tree", ("Library", "الأقسام"), Panel::Library),
        ("authors", ("Authors", "المؤلفون"), Panel::Authors),
        ("bkmrk", ("Bookmarks", "الإشارات"), Panel::Bookmarks),
        ("dipquran", ("Quran", "القرآن"), Panel::Quran),
        ("globe", ("Downloads", "التحميل"), Panel::Downloads),
        ("ctr", ("Control room", "غرفة التحكم"), Panel::Control),
    ];
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    for (icon, tip, panel) in tabs {
        let active = state.panel == panel;
        let label = format!("{} / {}", tip.0, tip.1);
        items.push(
            Tooltip::new(
                button(ticon(icon, 18.0))
                    .on_press(Message::PanelSelected(panel))
                    .padding(4)
                    .style(move |theme: &Theme, status: button::Status| {
                        if active {
                            accent_style(accent, radius)(theme, status)
                        } else {
                            button::text(theme, status)
                        }
                    }),
                ar(label, 13.0),
                Position::Bottom,
            )
            .into(),
        );
    }
    items.reverse();
    items.insert(0, Space::with_width(Length::Fill).into());
    row(items).spacing(2).into()
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
        .width(Length::Fill)
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
            reader.pages.get(reader.index)?;
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
        button(ticon("searchbok", 18.0)).on_press(Message::FindToggle),
        button(ticon("searchpage", 18.0)).on_press(Message::GoToToggle),
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
            .line_height(LineHeight::Relative(state.line_height))
            .color(state.reader_text)
            .shaping(Shaping::Advanced)
            .width(Length::Fill),
    )
    .height(if state.show_notes {
        Length::FillPortion(3)
    } else {
        Length::Fill
    })
    .into();
    let mut page_parts: Vec<Element<'_, Message>> = Vec::new();
    if state.show_find {
        let hits = state.find_hits.len();
        let counter = if hits == 0 {
            String::from("0 / 0")
        } else {
            format!("{} / {hits}", state.find_pos + 1)
        };
        let mut bar = column![row![
            ar_boxed(tr(state.lang, "find_word"), 12.0),
            text_input(tr(state.lang, "find_in_book"), state.find_input.as_str())
                .on_input(Message::FindInput)
                .on_submit(Message::FindNext)
                .padding(6),
            button(ticon("prev", 16.0)).on_press(Message::FindPrev),
            button(ticon("next", 16.0)).on_press(Message::FindNext),
            ar_boxed(counter, 13.0),
            button(ticon("x", 14.0)).on_press(Message::FindClose),
        ]
        .spacing(6)]
        .spacing(4);
        let snippet = find_snippet(state);
        if !snippet.is_empty() {
            bar = bar.push(ar(snippet, 12.0));
        }
        page_parts.push(bar.into());
    }
    page_parts.push(nav.into());
    page_parts.push(page_scroll);
    if state.show_notes {
        page_parts.push(
            scrollable(column(note_items).spacing(2))
                .height(Length::FillPortion(1))
                .into(),
        );
    }
    let page_view = container(column(page_parts).spacing(8).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(6)
        .style(reader_bg_style(state.reader_bg));

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
                    state.chrome(),
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
    let chrome = state.chrome();
    let tabs: Vec<Element<'_, Message>> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let label: String = tab.title.chars().take(20).collect();
            let active = state.active_tab == Some(index);
            let tab_button = button(ar_boxed(label, 13.0)).on_press(Message::TabSelected(index));
            let tab_button = if active {
                tab_button.style(accent_style(chrome.accent, chrome.radius))
            } else {
                tab_button.style(button::text)
            };
            row![
                tab_button,
                button(ticon("x", 13.0)).on_press(Message::TabClosed(index)),
            ]
            .spacing(2)
            .into()
        })
        .collect();
    let mut tabs = tabs;
    tabs.reverse();
    let mut tab_children: Vec<Element<'_, Message>> = vec![Space::with_width(Length::Fill).into()];
    tab_children.extend(tabs);
    let tab_bar = row(tab_children).spacing(4);
    let content: Element<'_, Message> =
        match state.active_tab.and_then(|index| state.tabs.get(index)) {
            Some(tab) => match &tab.kind {
                TabKind::Quran { sura, verses } => {
                    let name = shamela_content::quran::SURA_NAMES
                        .get(*sura as usize - 1)
                        .unwrap_or(&"?");
                    let head = format!(
                        "{name} — {} {} ({})",
                        tr(state.lang, "juz_word"),
                        juz_of_sura(*sura),
                        verses.len()
                    );
                    let tafsir_label = shamela_import::tafsir_dl::TAFSIRS
                        [state.tafsir_ix % shamela_import::tafsir_dl::TAFSIRS.len()]
                    .1;
                    let mut body_text = String::new();
                    for (index, verse) in verses.iter().enumerate() {
                        let verse_text = apply_tashkeel(&verse.text, state.show_tashkeel);
                        body_text.push_str(&format!("({}) {verse_text}\n", verse.aya));
                        if state.show_tafsir {
                            if let Some(tafsir) = state.tafsir_lines.get(index) {
                                body_text.push_str(&format!("\n{tafsir}\n"));
                            }
                        }
                        body_text.push('\n');
                    }
                    column![
                        row![
                            button(ticon("prev", 18.0)).on_press(Message::ReaderPrev),
                            button(ticon("next", 18.0)).on_press(Message::ReaderNext),
                            button(ar_boxed(tafsir_label, 14.0)).on_press(Message::TafsirCycle),
                            button(ar_boxed(tr(state.lang, "tafsir"), 14.0))
                                .on_press(Message::TafsirToggle)
                                .style(accent_style(
                                    if state.show_tafsir {
                                        chrome.accent
                                    } else {
                                        Color::from_rgb(0.55, 0.45, 0.30)
                                    },
                                    chrome.radius
                                )),
                            tool_btn(
                                "bkmrk",
                                ("Bookmark", "إشارة مرجعية"),
                                Message::BookmarkAdd,
                                chrome
                            ),
                            tool_btn(
                                "x",
                                ("Close", "إغلاق"),
                                Message::TabClosed(state.active_tab.unwrap_or(0)),
                                chrome
                            ),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center),
                        container(
                            scrollable(
                                text(format!("{head}\n\n{body_text}"))
                                    .font(AMIRI)
                                    .size(state.font_size)
                                    .line_height(LineHeight::Relative(state.line_height))
                                    .color(state.reader_text)
                                    .shaping(Shaping::Advanced)
                                    .width(Length::Fill),
                            )
                            .height(Length::Fill),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(6)
                        .style(reader_bg_style(state.reader_bg)),
                    ]
                    .spacing(8)
                    .into()
                }
                TabKind::Card(body) => column![
                    row![
                        tool_btn(
                            "export",
                            ("Download", "تحميل الكتاب"),
                            Message::DownloadCurrent,
                            chrome
                        ),
                        tool_btn(
                            "bkmrk",
                            ("Bookmark", "إشارة مرجعية"),
                            Message::BookmarkAdd,
                            chrome
                        ),
                        tool_btn(
                            "x",
                            ("Close", "إغلاق"),
                            Message::TabClosed(state.active_tab.unwrap_or(0)),
                            chrome
                        ),
                    ]
                    .spacing(8),
                    container(
                        scrollable(
                            text(apply_tashkeel(body, state.show_tashkeel).into_owned())
                                .font(AMIRI)
                                .size(state.font_size)
                                .line_height(LineHeight::Relative(state.line_height))
                                .color(state.reader_text)
                                .shaping(Shaping::Advanced)
                                .width(Length::Fill),
                        )
                        .height(Length::Fill),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(6)
                    .style(reader_bg_style(state.reader_bg)),
                ]
                .spacing(8)
                .into(),
                TabKind::Reader(reader_tab) => column![
                    row![
                        tool_btn(
                            "export",
                            ("Download", "تحميل الكتاب"),
                            Message::DownloadCurrent,
                            chrome
                        ),
                        tool_btn(
                            "bkmrk",
                            ("Bookmark", "إشارة مرجعية"),
                            Message::BookmarkAdd,
                            chrome
                        ),
                        tool_btn(
                            "x",
                            ("Close", "إغلاق"),
                            Message::TabClosed(state.active_tab.unwrap_or(0)),
                            chrome
                        ),
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

/// One menu: title key plus its action entries.
type MenuDef = (&'static str, Vec<(&'static str, Message)>);

/// Menu bar contents: title key plus the actions of each menu.
fn menu_entries(state: &State) -> Vec<MenuDef> {
    let close_tab = Message::TabClosed(state.active_tab.unwrap_or(0));
    vec![
        (
            "menu_file",
            vec![
                ("open_db", Message::OpenDatabase),
                ("import_bok", Message::ImportBok),
                ("download_from_net", Message::DownloadOpen),
                ("export", Message::ExportBook),
                ("close", close_tab),
                ("quit", Message::QuitApp),
            ],
        ),
        (
            "menu_edit",
            vec![
                ("copy_page", Message::CopyPage),
                ("add_bookmark", Message::BookmarkAdd),
                ("find_book", Message::SearchBook),
                ("find_page", Message::FindPage),
            ],
        ),
        (
            "menu_search",
            vec![
                ("new_search", Message::FocusBook),
                ("find_book", Message::SearchBook),
                ("last_search", Message::LastSearch),
                ("authors", Message::PanelSelected(Panel::Authors)),
            ],
        ),
        (
            "menu_view",
            vec![
                ("sections", Message::PanelSelected(Panel::Library)),
                ("authors", Message::PanelSelected(Panel::Authors)),
                ("bookmarks", Message::PanelSelected(Panel::Bookmarks)),
                ("notes", Message::ToggleNotes),
                ("toc_word", Message::ToggleToc),
                ("theme", Message::ThemeToggle),
                ("settings", Message::ToggleSettings),
            ],
        ),
        (
            "menu_quran",
            vec![
                ("quran", Message::PanelSelected(Panel::Quran)),
                ("download_quran", Message::QuranDownload),
                ("tafsir", Message::TafsirToggle),
                ("tafsir_next", Message::TafsirCycle),
            ],
        ),
        (
            "menu_help",
            vec![
                ("help", Message::OpenHelp),
                ("upgrade", Message::OpenReleases),
                ("about", Message::About),
            ],
        ),
    ]
}

/// Desktop-style menu bar with an expanding item strip (RTL aware).
fn menu_bar(state: &State) -> Element<'_, Message> {
    let lang = state.lang;
    let menus = menu_entries(state);
    let mut titles: Vec<Element<'_, Message>> = Vec::new();
    for (index, (key, _)) in menus.iter().enumerate() {
        let open = state.menu_open == Some(index);
        let entry: Element<'_, Message> = if open {
            button(ar_boxed(tr(lang, key), 13.0))
                .on_press(Message::MenuToggle(index))
                .style(accent_style(state.accent.color(), state.chrome().radius))
                .into()
        } else {
            button(ar_boxed(tr(lang, key), 13.0))
                .on_press(Message::MenuToggle(index))
                .style(button::text)
                .into()
        };
        titles.push(entry);
    }
    titles.reverse();
    titles.insert(0, Space::with_width(Length::Fill).into());
    let bar = row(titles).spacing(2).padding(2);
    let Some(open) = state.menu_open else {
        return bar.into();
    };
    let Some((_, items)) = menus.get(open) else {
        return bar.into();
    };
    let mut strip: Vec<Element<'_, Message>> = Vec::new();
    for (key, action) in items {
        strip.push(
            button(ar_boxed(tr(lang, key), 13.0))
                .on_press(action.clone())
                .style(button::text)
                .into(),
        );
    }
    strip.reverse();
    strip.insert(0, Space::with_width(Length::Fill).into());
    let strip_row = row(strip).spacing(2).padding(2);
    column![bar, strip_row].spacing(1).into()
}

/// Active reader tab, when the front tab is a book.
fn active_reader(state: &State) -> Option<&ReaderTab> {
    let tab = state.active_tab.and_then(|index| state.tabs.get(index))?;
    match &tab.kind {
        TabKind::Reader(reader) => Some(reader),
        _ => None,
    }
}

/// Page/sura position for the status bar.
fn reading_position(state: &State) -> Option<String> {
    let tab = state.active_tab.and_then(|index| state.tabs.get(index))?;
    match &tab.kind {
        TabKind::Reader(reader) if !reader.pages.is_empty() => Some(format!(
            "{} {} / {}",
            tr(state.lang, "page_word"),
            reader.index + 1,
            reader.pages.len()
        )),
        TabKind::Quran { sura, .. } => {
            Some(format!("{} {} / 114", tr(state.lang, "sura_word"), sura))
        }
        _ => None,
    }
}

/// Parse a page number typed in Arabic-Indic or Latin digits.
fn parse_number(text: &str) -> Option<usize> {
    let mut digits = String::new();
    for ch in text.chars() {
        match ch {
            '0'..='9' => digits.push(ch),
            '\u{0660}'..='\u{0669}' => digits.push(char::from(b'0' + (ch as u32 - 0x0660) as u8)),
            '\u{06F0}'..='\u{06F9}' => digits.push(char::from(b'0' + (ch as u32 - 0x06F0) as u8)),
            _ => {}
        }
    }
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Recompute the in-book find hits and show the first matching page.
fn run_find(state: &mut State) {
    let needle = shamela_index::normalize_arabic(&state.find_input, &state.search_opts);
    let mut hits: Vec<usize> = Vec::new();
    if !needle.trim().is_empty() {
        if let Some(tab) = state.active_tab.and_then(|index| state.tabs.get(index)) {
            if let TabKind::Reader(reader) = &tab.kind {
                for (index, page) in reader.pages.iter().enumerate() {
                    let text = content_to_text(&page.content);
                    let hay = shamela_index::normalize_arabic(&text, &state.search_opts);
                    if hay.contains(&needle) {
                        hits.push(index);
                    }
                }
            }
        }
    }
    let total = hits.len();
    let first = hits.first().copied();
    state.find_hits = hits;
    state.find_pos = 0;
    if let Some(page) = first {
        if active_reader(state).map(|reader| reader.index) != Some(page) {
            jump_to_page(state, page);
        }
    }
    state.status = if total == 0 {
        tr(state.lang, "find_none").to_string()
    } else {
        format!("{} {total}", tr(state.lang, "find_hits"))
    };
}

/// Jump between in-book find hits.
fn find_step(state: &mut State, delta: isize) {
    let len = state.find_hits.len();
    if len == 0 {
        state.status = tr(state.lang, "find_none").to_string();
        return;
    }
    let current = state.find_pos.min(len - 1);
    let next = (current as isize + delta).rem_euclid(len as isize) as usize;
    state.find_pos = next;
    let page = state.find_hits[next];
    jump_to_page(state, page);
    state.status = format!("{} {} / {len}", tr(state.lang, "find_hits"), next + 1);
}

/// First characters of the matching page, for the find bar.
fn find_snippet(state: &State) -> String {
    let Some(reader) = active_reader(state) else {
        return String::new();
    };
    let Some(&page) = state.find_hits.get(state.find_pos) else {
        return String::new();
    };
    let Some(page) = reader.pages.get(page) else {
        return String::new();
    };
    let text = content_to_text(&page.content);
    trim_text(text.trim(), 70)
}

/// Go-to-page popup.
fn goto_modal(state: &State) -> Element<'_, Message> {
    let dark = matches!(state.theme_kind, AppTheme::Dark);
    let lang = state.lang;
    let pages = active_reader(state).map_or(0, |reader| reader.pages.len());
    let title = row![
        button(ticon("x", 18.0)).on_press(Message::GoToClose),
        Space::with_width(Length::Fill),
        ar_boxed(tr(lang, "goto_page"), 17.0),
        ticon("searchpage", 20.0),
    ]
    .spacing(8);
    let hint = if pages == 0 {
        String::from("open a book first")
    } else {
        format!("1 .. {pages}")
    };
    let input = text_input(tr(lang, "page_word"), state.goto_input.as_str())
        .on_input(Message::GoToInput)
        .on_submit(Message::GoToSubmit)
        .padding(8);
    let actions = row![
        button(ar_boxed(tr(lang, "goto"), 14.0)).on_press(Message::GoToSubmit),
        button(ar_boxed(tr(lang, "cancel"), 14.0)).on_press(Message::GoToClose),
    ]
    .spacing(6);
    let body = column![title, ar(hint, 13.0), input, actions]
        .spacing(8)
        .padding(4)
        .width(Length::Fixed(320.0));
    let card = container(body)
        .padding(16)
        .width(Length::Fixed(360.0))
        .style(popup_card_style(dark));
    modal_shell(card)
}

fn status_bar(state: &State) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();
    if let Some(position) = reading_position(state) {
        items.push(ar_boxed(position, 13.0));
    }
    items.push(Space::with_width(Length::Fill).into());
    items.push(ar(state.status.as_str(), 13.0));
    row(items).padding(6).into()
}

/// Reading-page background style.
fn reader_bg_style(bg: Color) -> impl Fn(&Theme) -> container::Style {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        ..container::Style::default()
    }
}

/// Dimmed backdrop with a centered card (popup window look).
fn modal_shell<'a>(card: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(column![
        Space::with_height(Length::Fill),
        row![
            Space::with_width(Length::Fill),
            card.into(),
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

/// Card background for popups (follows the light/dark theme).
fn popup_card_style(dark: bool) -> impl Fn(&Theme) -> container::Style {
    let bg = if dark {
        Color::from_rgb(0.16, 0.16, 0.20)
    } else {
        Color::from_rgb(0.99, 0.99, 0.98)
    };
    let fg = if dark {
        Color::from_rgb(0.93, 0.93, 0.96)
    } else {
        Color::from_rgb(0.10, 0.10, 0.12)
    };
    move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(fg),
        border: iced::border::Border {
            color: Color::from_rgb(0.55, 0.55, 0.60),
            width: 1.0,
            radius: iced::border::Radius::new(10.0),
        },
        ..container::Style::default()
    }
}

/// Settings popup: every option works (display, search, data).
fn settings_modal(state: &State) -> Element<'_, Message> {
    let dark = matches!(state.theme_kind, AppTheme::Dark);
    let swatch = |color: Color| -> Element<'static, Message> {
        container(Space::with_width(Length::Fixed(0.0)))
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(color)),
                border: iced::border::Border {
                    color: Color::from_rgb(0.4, 0.4, 0.4),
                    width: 1.0,
                    radius: iced::border::Radius::new(3.0),
                },
                ..container::Style::default()
            })
            .into()
    };
    let title = row![
        button(ticon("x", 18.0)).on_press(Message::ToggleSettings),
        Space::with_width(Length::Fill),
        ar_boxed(tr(state.lang, "settings"), 18.0),
        ticon("options", 22.0),
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
        button(
            row![
                container(Space::with_width(Length::Fixed(0.0)))
                    .width(Length::Fixed(14.0))
                    .height(Length::Fixed(14.0))
                    .style({
                        let accent = state.accent.color();
                        move |_: &Theme| container::Style {
                            background: Some(Background::Color(accent)),
                            border: iced::border::Border {
                                color: Color::from_rgb(0.4, 0.4, 0.4),
                                width: 1.0,
                                radius: iced::border::Radius::new(3.0),
                            },
                            ..container::Style::default()
                        }
                    }),
                ar(state.accent.name(), 14.0),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::AccentNext),
    ]
    .spacing(6);
    let line_row = row![
        ar(tr(state.lang, "line_height"), 14.0),
        button(ar("-", 14.0)).on_press(Message::LineSmaller),
        ar(format!("{:.1}", state.line_height), 14.0),
        button(ar("+", 14.0)).on_press(Message::LineBigger),
    ]
    .spacing(6);
    let color_row = row![
        ar(tr(state.lang, "text_color"), 14.0),
        swatch(state.reader_text),
        button(ar(">", 13.0)).on_press(Message::ReaderFgNext),
        ar(tr(state.lang, "page_color"), 14.0),
        swatch(state.reader_bg),
        button(ar(">", 13.0)).on_press(Message::ReaderBgNext),
    ]
    .spacing(6);
    let body = column![
        title,
        ar(tr(state.lang, "display"), 16.0),
        font_row,
        line_row,
        color_row,
        toggle_row(
            tr(state.lang, "corners").to_string(),
            state.rounded,
            Message::ToggleCorners
        ),
        toggle_row(
            tr(state.lang, "show_tashkeel").to_string(),
            state.show_tashkeel,
            Message::ToggleTashkeel
        ),
        toggle_row(
            tr(state.lang, "reopen_last").to_string(),
            state.reopen_last,
            Message::ToggleReopenLast
        ),
        toggle_row(
            tr(state.lang, "notify_search").to_string(),
            state.notify_search,
            Message::ToggleNotifySearch
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
        row![
            button(ar(tr(state.lang, "clear_cache"), 14.0))
                .on_press(Message::ClearCache)
                .width(Length::Fill),
            button(ar(tr(state.lang, "reset_all"), 14.0))
                .on_press(Message::ResetAll)
                .width(Length::Fill),
        ]
        .spacing(6),
        row![
            button(ar(tr(state.lang, "download_current"), 14.0))
                .on_press(Message::DownloadCurrent)
                .width(Length::Fill),
            button(ar(tr(state.lang, "about"), 14.0))
                .on_press(Message::About)
                .width(Length::Fill),
        ]
        .spacing(6),
    ]
    .spacing(8)
    .padding(4)
    .width(Length::Fixed(470.0));
    let card = container(scrollable(body).height(Length::Fixed(470.0)))
        .padding(18)
        .width(Length::Fixed(510.0))
        .style(popup_card_style(dark));
    modal_shell(card)
}

/// Download manager popup: source, selection, queue and per-file downloads.
/// Download manager popup: source, selection, queue and per-file downloads.
fn downloads_modal(state: &State) -> Element<'_, Message> {
    let dark = matches!(state.theme_kind, AppTheme::Dark);
    let lang = state.lang;
    let title = row![
        button(ticon("x", 18.0)).on_press(Message::DlToggle),
        Space::with_width(Length::Fill),
        ar_boxed(tr(lang, "downloads"), 18.0),
        ticon("globe", 22.0),
    ]
    .spacing(8);

    let input = text_input(tr(lang, "source_hint"), state.dl_id.as_str())
        .on_input(Message::DownloadIdInput)
        .on_submit(Message::DownloadList)
        .padding(8);
    let source_row = row![
        input,
        button(ar_boxed(tr(lang, "fetch_books"), 14.0)).on_press(Message::DownloadList),
    ]
    .spacing(6);

    let mut chips: Vec<Element<'_, Message>> = vec![ar_boxed(tr(lang, "preset_sources"), 13.0)];
    for (id, label) in DL_PRESETS {
        chips.push(
            button(ar_boxed(*label, 12.0))
                .on_press(Message::DlPresetSource(String::from(*id)))
                .into(),
        );
    }
    let presets = row(chips).spacing(6);

    let total = state.dl_files.len();
    let visible = visible_files(state);
    let all_size: u64 = state.dl_files.iter().map(|file| file.size).sum();
    let sel_size: u64 = state
        .dl_files
        .iter()
        .filter(|file| state.dl_selected.contains(&file.name))
        .map(|file| file.size)
        .sum();
    let sel_count = state.dl_selected.len();

    let controls = row![
        button(ar_boxed(tr(lang, "download_all"), 14.0)).on_press(Message::DownloadAll),
        button(ar_boxed(tr(lang, "download_selected"), 14.0)).on_press(Message::DlDownloadSelected),
        button(ar_boxed(tr(lang, "select_all"), 14.0)).on_press(Message::DlSelectAll),
        button(ar_boxed(tr(lang, "select_none"), 14.0)).on_press(Message::DlSelectNone),
    ]
    .spacing(6);
    let controls2 = row![
        button(ar_boxed(tr(lang, "download_current"), 14.0)).on_press(Message::DownloadCurrent),
        button(ar_boxed(tr(lang, "stop_all"), 14.0)).on_press(Message::DlCancel),
    ]
    .spacing(6);

    let summary = ar(
        format!(
            "{}: {total} ({}) — {}: {sel_count} ({})",
            tr(lang, "files_word"),
            fmt_size(all_size),
            tr(lang, "selected_word"),
            fmt_size(sel_size),
        ),
        13.0,
    );

    let done = state.dl_queue.iter().filter(|job| job.done).count();
    let queue_header = row![
        Space::with_width(Length::Fill),
        button(ar_boxed(tr(lang, "open_folder"), 12.0)).on_press(Message::DlOpenFolder),
        button(ar_boxed(tr(lang, "clear_finished"), 12.0)).on_press(Message::DlClearFinished),
        ar_boxed(format!("{done}/{}", state.dl_queue.len()), 13.0),
        ar_boxed(tr(lang, "dl_queue"), 15.0),
    ]
    .spacing(6);

    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    if state.dl_queue.is_empty() {
        rows.push(ar_boxed(tr(lang, "no_downloads"), 15.0));
    }
    for job in &state.dl_queue {
        let status = if let Some(err) = &job.failed {
            format!("failed: {err}")
        } else if job.done {
            String::from("done")
        } else {
            job.progress.status()
        };
        let mut actions: Vec<Element<'_, Message>> = Vec::new();
        if job.failed.is_some() {
            actions.push(
                button(ar_boxed(tr(lang, "retry"), 13.0))
                    .on_press(Message::DlRetry(job.id))
                    .into(),
            );
            actions.push(
                button(ar_boxed(tr(lang, "remove"), 13.0))
                    .on_press(Message::DlRemove(job.id))
                    .into(),
            );
        } else if job.done {
            actions.push(
                button(ar_boxed(tr(lang, "remove"), 13.0))
                    .on_press(Message::DlRemove(job.id))
                    .into(),
            );
        } else {
            actions.push(
                button(ar_boxed(tr(lang, "cancel"), 13.0))
                    .on_press(Message::DlCancel)
                    .into(),
            );
        }
        let kind = if matches!(job.after, AfterDl::ImportArchive) {
            tr(lang, "kind_pack")
        } else {
            tr(lang, "kind_book")
        };
        rows.push(
            column![
                row![
                    ar_boxed(kind, 12.0),
                    Space::with_width(Length::Fill),
                    ar_boxed(trim_text(&status, 26), 13.0),
                    ar_boxed(display_name(&job.label), 15.0),
                ]
                .spacing(6),
                progress_bar(0.0..=1.0, job.progress.fraction()).height(10.0),
                row(actions).spacing(4),
            ]
            .spacing(4)
            .into(),
        );
    }
    let list = scrollable(column(rows).spacing(10)).height(Length::Fixed(170.0));

    let filter = text_input(tr(lang, "filter_files"), state.dl_filter.as_str())
        .on_input(Message::DlFilterInput)
        .padding(6)
        .width(Length::Fixed(220.0));
    let files_header = row![
        filter,
        ar_boxed(tr(lang, "filter_word"), 12.0),
        Space::with_width(Length::Fill),
        ar_boxed(
            format!(
                "{} — {} {}/{}",
                tr(lang, "files_of"),
                tr(lang, "shown_of"),
                visible.len(),
                total
            ),
            13.0,
        ),
    ]
    .spacing(6);
    let mut files: Vec<Element<'_, Message>> = Vec::new();
    for file in visible.iter().take(MODAL_FILE_ROWS) {
        files.push(file_row(state, file));
    }
    if files.is_empty() {
        files.push(ar_boxed(tr(lang, "no_files"), 14.0));
    }
    let file_list = scrollable(column(files).spacing(4)).height(Length::Fixed(170.0));

    let body = column![
        title,
        source_row,
        presets,
        controls,
        controls2,
        summary,
        queue_header,
        list,
        files_header,
        file_list,
    ]
    .spacing(8)
    .padding(4)
    .width(Length::Fixed(640.0));
    let card = container(body)
        .padding(18)
        .width(Length::Fixed(700.0))
        .style(popup_card_style(dark));
    modal_shell(card)
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
fn hotkey(key: keyboard::Key, mods: keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::key::Named;
    if mods.control() {
        let shortcut = match &key {
            keyboard::Key::Character(ch) => ch.as_str(),
            _ => "",
        };
        match shortcut {
            "f" | "F" => return Some(Message::FindToggle),
            "g" | "G" => return Some(Message::GoToToggle),
            _ => {}
        }
    }
    match key {
        keyboard::Key::Named(Named::F5) => Some(Message::ReaderNext),
        keyboard::Key::Named(Named::F6) => Some(Message::ReaderPrev),
        keyboard::Key::Named(Named::F3) => Some(Message::PanelSelected(Panel::Library)),
        keyboard::Key::Named(Named::F1) => Some(Message::OpenHelp),
        keyboard::Key::Named(Named::F10) => Some(Message::ToggleSettings),
        keyboard::Key::Named(Named::Escape) => Some(Message::EscapePressed),
        _ => None,
    }
}

/// Keyboard shortcuts + a 400 ms download poll while jobs are active.
fn subscribe(state: &State) -> iced::Subscription<Message> {
    let keys = keyboard::on_key_press(hotkey);
    let tick = if dl_active(state) {
        iced::time::every(std::time::Duration::from_millis(400)).map(|_| Message::DlTick)
    } else {
        iced::Subscription::none()
    };
    iced::Subscription::batch([keys, tick])
}

fn view(state: &State) -> Element<'_, Message> {
    let top = column![menu_bar(state), tool_bar(state)].spacing(2);
    let body: Element<'_, Message> = if state.lib.is_none() && state.tabs.is_empty() {
        welcome(state)
    } else {
        pane_grid::PaneGrid::new(&state.panes, |_pane, content, _| {
            pane_grid::Content::new(match content {
                PaneKind::Reader => reader(state),
                PaneKind::Books => books_panel(state),
                PaneKind::Side => {
                    let body = if quran_active(state) {
                        quran_panel(state)
                    } else {
                        sidebar(state)
                    };
                    column![body, panel_tabs(state)].spacing(4).into()
                }
            })
        })
        .spacing(6)
        .height(Length::Fill)
        .on_resize(10.0, Message::PaneResized)
        .into()
    };
    let content = column![top, body, status_bar(state)];
    if !state.show_settings && !state.show_downloads && !state.show_goto {
        return content.into();
    }
    let mut stack = Stack::new().push(content);
    if state.show_settings {
        stack = stack.push(settings_modal(state));
    }
    if state.show_downloads {
        stack = stack.push(downloads_modal(state));
    }
    if state.show_goto {
        stack = stack.push(goto_modal(state));
    }
    stack.width(Length::Fill).height(Length::Fill).into()
}

fn theme(state: &State) -> Theme {
    let base = match state.theme_kind {
        AppTheme::Light => Theme::Light,
        AppTheme::Dark => Theme::Dark,
    };
    let mut palette = base.palette();
    palette.primary = state.accent.color();
    Theme::custom(String::from("ShamelaCraft"), palette)
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--self-test" {
        std::process::exit(self_test(&args[2], &args[3]));
    }
    iced::application("ShamelaCraft", update, view)
        .window_size((1400.0, 900.0))
        .subscription(subscribe)
        .theme(theme)
        .font(include_bytes!("../assets/fonts/Amiri-Regular.ttf"))
        .run()
}
