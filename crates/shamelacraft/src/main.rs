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

/// Which index the Quran panel lists: the suras or the thirty juz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum QuranIndex {
    #[default]
    Sura,
    Juz,
}

/// How the side pane shows its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SideMode {
    /// Tabbed panels: sections, authors, bookmarks, Quran, downloads, control.
    #[default]
    Tabs,
    /// One tree: sections with the books inside them, expandable.
    Tree,
    /// Two resizable trees: the catalog and the index of the open book.
    TwoTrees,
}

/// The two parts of the reader pane: the page and its index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderKind {
    Page,
    Index,
}

/// Sections of the side pane in the two-tree layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideKind {
    Catalog,
    Index,
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
    /// Local converted database of an imported book (kept for later
    /// re-export/re-open features; title search no longer reads it).
    #[allow(dead_code)]
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
    DownloadTab(usize),
    DlToggleFile(String),
    DlSelectAll,
    DlSelectNone,
    DlDownloadSelected,
    DlFilterInput(String),
    DlClearFinished,
    DlOpenFolder,
    DlPresetSource(usize),
    DlBrowseArchive(String),
    DlBackArchive,
    DlSourcesToggle,
    DlQueryInput(String),
    DlSearch,
    DlGlobalSearch,
    DlDownloadIndexed(usize),
    DlFetchNames,
    DlFetchNamesIndexed(usize),
    DlClearResults,
    DlOpenItem(String),
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
    QuranJuz(i64),
    QuranIndexPick(QuranIndex),
    QuranSearch(String),
    QuranDownload,
    NoteInput(String),
    NoteSave,
    NoteDelete(i64),
    CopyPage,
    LastSearch,
    PaneResized(pane_grid::ResizeEvent),
    SideResized(pane_grid::ResizeEvent),
    ReaderPaneResized(pane_grid::ResizeEvent),
    TogglePane(PaneKind),
    ClosePane(PaneKind),
    SideModePick(SideMode),
    SideModeCycle,
    ResetLayout,
    ToggleSection(i64),
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

/// Background name-index result: pack key plus the id → title map.
type NameFetch = (
    String,
    Result<std::collections::HashMap<i64, String>, String>,
);

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
    /// Which index the Quran panel shows: suras or juz.
    quran_index: QuranIndex,
    quran_query: String,
    quran_results: Vec<shamela_content::quran::Verse>,
    tafsir_ix: usize,
    show_tafsir: bool,
    tafsir_for: Option<i64>,
    tafsir_lines: Vec<String>,
    note_input: String,
    notes: Vec<shamela_content::notes::Note>,
    panes: pane_grid::State<PaneKind>,
    /// Inner split of the side pane (catalog tree / book index tree).
    side_panes: pane_grid::State<SideKind>,
    /// Inner split of the reader pane (page / book index).
    reader_panes: pane_grid::State<ReaderKind>,
    /// Layout of the side pane.
    side_mode: SideMode,
    /// Which side panes are visible.
    show_books_pane: bool,
    show_side_pane: bool,
    /// Section opened in the catalog tree.
    expanded_category: Option<i64>,
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
    dl_archive: String,
    dl_sources: bool,
    dl_files: Vec<shamela_import::archive::ArchiveFile>,
    dl_selected: std::collections::BTreeSet<String>,
    dl_filter: String,
    dl_query: String,
    dl_results: Vec<shamela_import::archive::ArchiveItem>,
    /// Indices into `source_index()` for the cross-source title search.
    dl_global: Option<Vec<usize>>,
    /// Master database names by book id, used to label numbered files.
    book_titles: std::collections::HashMap<i64, String>,
    /// Names fetched from a pack's own `main.mdb`, keyed by source+container.
    pack_names: std::collections::HashMap<String, std::collections::HashMap<i64, String>>,
    /// Pending background name-index fetch.
    name_rx: Option<std::sync::mpsc::Receiver<NameFetch>>,
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
            quran_index: QuranIndex::Sura,
            quran_query: String::new(),
            quran_results: Vec::new(),
            tafsir_ix: 0,
            show_tafsir: false,
            tafsir_for: None,
            tafsir_lines: Vec::new(),
            note_input: String::new(),
            notes: Vec::new(),
            panes: init_panes(true, true),
            side_panes: init_side_panes(),
            reader_panes: init_reader_panes(),
            side_mode: SideMode::Tabs,
            show_books_pane: true,
            show_side_pane: true,
            expanded_category: None,
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
            dl_archive: String::new(),
            dl_sources: false,
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
            dl_query: String::new(),
            dl_results: Vec::new(),
            dl_global: None,
            book_titles: std::collections::HashMap::new(),
            pack_names: std::collections::HashMap::new(),
            name_rx: None,
            font_size: 24.0,
            theme_kind: AppTheme::Light,
            status: String::from("افتح ملف master.db لبدء التصفح"),
        };
        load_settings(&mut state);
        let (reader_text, reader_bg) = reader_colors(state.theme_kind);
        state.reader_text = reader_text;
        state.reader_bg = reader_bg;
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
    let content = button(container(ar(label, size)).width(Length::Fill))
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

/// Hover labels: black box with white text, in every theme.
fn tooltip_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.04, 0.04, 0.05))),
        text_color: Some(Color::WHITE),
        border: iced::border::Border {
            color: Color::from_rgb(0.35, 0.35, 0.40),
            width: 1.0,
            radius: iced::border::Radius::new(6.0),
        },
        ..container::Style::default()
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
    .style(tooltip_style)
    .gap(8.0)
    .into()
}

/// Reader | books | side navigation, user-resizable via drag handles.
fn init_panes(show_books: bool, show_side: bool) -> pane_grid::State<PaneKind> {
    let (mut panes, reader) = pane_grid::State::new(PaneKind::Reader);
    match (show_books, show_side) {
        (true, true) => {
            if let Some((books, first)) = panes.split(Axis::Vertical, reader, PaneKind::Books) {
                if let Some((_, second)) = panes.split(Axis::Vertical, books, PaneKind::Side) {
                    panes.resize(first, 0.58);
                    panes.resize(second, 0.42);
                }
            }
        }
        (true, false) => {
            if let Some((_, first)) = panes.split(Axis::Vertical, reader, PaneKind::Books) {
                panes.resize(first, 0.62);
            }
        }
        (false, true) => {
            if let Some((_, first)) = panes.split(Axis::Vertical, reader, PaneKind::Side) {
                panes.resize(first, 0.62);
            }
        }
        (false, false) => {}
    }
    panes
}

/// Page over book index inside the reader pane, split by a drag handle.
fn init_reader_panes() -> pane_grid::State<ReaderKind> {
    let (mut panes, page) = pane_grid::State::new(ReaderKind::Page);
    if let Some((_, split)) = panes.split(Axis::Vertical, page, ReaderKind::Index) {
        panes.resize(split, 0.72);
    }
    panes
}

/// Show or hide the index column of the reader.
fn set_reader_index_visible(state: &mut State, visible: bool) {
    let existing = state
        .reader_panes
        .iter()
        .find(|(_, kind)| **kind == ReaderKind::Index)
        .map(|(pane, _)| *pane);
    match (existing, visible) {
        (Some(pane), false) => {
            let _ = state.reader_panes.close(pane);
        }
        (None, true) => {
            let page = state
                .reader_panes
                .iter()
                .find(|(_, kind)| **kind == ReaderKind::Page)
                .map(|(pane, _)| *pane);
            if let Some(page) = page {
                if let Some((_, split)) =
                    state
                        .reader_panes
                        .split(Axis::Vertical, page, ReaderKind::Index)
                {
                    state.reader_panes.resize(split, 0.72);
                }
            }
        }
        _ => {}
    }
}

/// Catalog tree over book index tree inside the side pane.
fn init_side_panes() -> pane_grid::State<SideKind> {
    let (mut panes, catalog) = pane_grid::State::new(SideKind::Catalog);
    if let Some((_, split)) = panes.split(Axis::Horizontal, catalog, SideKind::Index) {
        panes.resize(split, 0.6);
    }
    panes
}

/// Show or hide one pane, keeping the rest of the layout intact.
fn set_pane_visible(state: &mut State, kind: PaneKind, visible: bool) {
    let existing = state
        .panes
        .iter()
        .find(|(_, pane_kind)| **pane_kind == kind)
        .map(|(pane, _)| *pane);
    match (existing, visible) {
        (Some(pane), false) => {
            let _ = state.panes.close(pane);
        }
        (None, true) => {
            let target = state
                .panes
                .iter()
                .find(|(_, pane_kind)| **pane_kind == PaneKind::Reader)
                .map(|(pane, _)| *pane)
                .or_else(|| state.panes.iter().next().map(|(pane, _)| *pane));
            if let Some(target) = target {
                if let Some((_, split)) = state.panes.split(Axis::Vertical, target, kind) {
                    state.panes.resize(split, 0.5);
                }
            }
        }
        _ => {}
    }
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
    let side_mode = match state.side_mode {
        SideMode::Tabs => "tabs",
        SideMode::Tree => "tree",
        SideMode::TwoTrees => "twotrees",
    };
    let body = format!(
        "font={}\ntheme={theme}\nlang={lang}\naccent={accent}\nrounded={}\nline={:.1}\n\ntashkeel={}\nhamza={}\nsearch_tashkeel={}\ntatweel={}\nreopen={}\nnotify={}\n\nside_mode={side_mode}\nbooks_pane={}\nside_pane={}\n",
        state.font_size,
        u8::from(state.rounded),
        state.line_height,
        u8::from(state.show_tashkeel),
        u8::from(state.search_opts.unify_hamza),
        u8::from(state.search_opts.strip_tashkeel),
        u8::from(state.search_opts.strip_tatweel),
        u8::from(state.reopen_last),
        u8::from(state.notify_search),
        u8::from(state.show_books_pane),
        u8::from(state.show_side_pane),
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
            "side_mode" => {
                state.side_mode = match value {
                    "tree" => SideMode::Tree,
                    "twotrees" => SideMode::TwoTrees,
                    _ => SideMode::Tabs,
                }
            }
            "books_pane" => state.show_books_pane = value == "1",
            "side_pane" => state.show_side_pane = value == "1",
            _ => {}
        }
    }
    state.panes = init_panes(state.show_books_pane, state.show_side_pane);
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
                    let book_titles = lib.book_names().unwrap_or_default();
                    state.lib = Some(lib);
                    state.author_names = author_names;
                    state.book_titles = book_titles;
                    preload_name_caches(state);
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
        .add_filter(
            "Shamela book (bok/mdb/mbok)",
            &["bok", "mdb", "mbok", "BOK", "MDB", "MBOK"],
        )
        .set_title("Shamela book");
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
            state.status = format!("book read failed: {err}");
            return;
        }
    };
    let title = if book.meta.title.is_empty() {
        stem
    } else {
        book.meta.title.clone()
    };
    match shamela_import::convert::bok_to_book_db(&book, &out) {
        Ok(()) => open_imported_db(state, &out, title, None),
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

/// Stable positive id for an imported book, reserved above the master
/// library's numeric range so notes/bookmarks never collide with it.
fn imported_book_id(path: &std::path::Path) -> i64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in path.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    1_000_000_000 + (hash % 1_000_000_000) as i64
}

/// Open a converted `Page`/`Title` database file in a reader tab.
fn open_imported_db(
    state: &mut State,
    out: &std::path::Path,
    title: String,
    origin: Option<String>,
) {
    let book_id = imported_book_id(out);
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
    state.dl_files.clear();
    state.dl_selected.clear();
    state.dl_filter.clear();
    let archive = state.dl_archive.trim().to_string();
    if archive.is_empty() {
        match shamela_import::archive::item_files(&id) {
            Ok(files) => {
                let items = shamela_import::archive::downloadable_files(&files);
                let count = items.len();
                state.dl_files = items.into_iter().cloned().collect();
                state.status = format!("item {id}: {count} importable files");
            }
            Err(err) => {
                state.status = format!("fetch failed: {err}");
            }
        }
    } else {
        match shamela_import::archive::browse_archive(&id, &archive) {
            Ok(files) => {
                let count = files.len();
                state.dl_files = files;
                state.status = if count == 0 {
                    format!("pack {archive}: no books inside")
                } else {
                    format!("pack {archive}: {count} books inside")
                };
            }
            Err(err) => {
                state.status = format!("pack browse failed: {err}");
            }
        }
    }
}

/// Load the configured source once, so download buttons and the popup have
/// the complete book list ready.
fn ensure_source_files(state: &mut State) {
    if state.dl_files.is_empty() && !state.dl_id.trim().is_empty() {
        refresh_download_list(state);
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
                    match shamela_import::convert::bok_to_book_db(&book, &out) {
                        Ok(()) => {
                            write_sidecar(&dest, &url);
                            open_imported_db(state, &out, title, Some(url));
                        }
                        Err(err) => {
                            fail_job(state, index, format!("convert failed: {err}"));
                        }
                    }
                }
                Err(err) => {
                    fail_job(state, index, format!("book read failed: {err}"));
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
                    fail_job(state, index, String::from("no books inside"));
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

/// Best origin URL for one book extracted from a downloaded pack: the exact
/// inner file URL when the current listing or the embedded catalog knows it,
/// else the pack URL.
fn extracted_origin(state: &State, local: &std::path::Path, fallback: &str) -> String {
    let leaf = local
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if leaf.is_empty() {
        return fallback.to_string();
    }
    let item = state.dl_id.trim();
    if !item.is_empty() {
        if let Some(file) = state
            .dl_files
            .iter()
            .find(|file| file.name.rsplit('/').next() == Some(leaf))
        {
            return shamela_import::archive::file_url(item, file);
        }
        let pack = safe_leaf(fallback);
        if let Some(entry) = source_index().iter().find(|file| {
            let source = &DL_PRESETS[usize::from(file.source)];
            source.id == item && file.archive == pack && file.name.rsplit('/').next() == Some(leaf)
        }) {
            let source = &DL_PRESETS[usize::from(entry.source)];
            return shamela_import::archive::download_url(
                source.id,
                &format!("{}/{}", entry.archive, entry.name),
            );
        }
    }
    fallback.to_string()
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
        let out = books_dir().join(format!(
            "{}_{inner_stem}.db",
            short_hash(&bok.display().to_string())
        ));
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
        let origin = extracted_origin(state, bok, url);
        if let Some(stem) = out.file_stem().and_then(|part| part.to_str()) {
            let _ = std::fs::write(books_dir().join(format!("{stem}.db.url")), &origin);
        }
        if !opened {
            open_imported_db(state, &out, title, Some(origin));
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
    if shamela_import::extract::is_archive(name) {
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
    let file = state
        .dl_files
        .iter()
        .find(|file| file.name == name)
        .cloned();
    let url = file
        .as_ref()
        .map(|file| shamela_import::archive::file_url(&item, file))
        .unwrap_or_else(|| shamela_import::archive::download_url(&item, &name));
    let leaf = safe_leaf(&name);
    let file_name = if state.dl_archive.is_empty() {
        leaf.clone()
    } else {
        format!(
            "{}_{leaf}",
            short_hash(&format!("{item}/{}/{name}", state.dl_archive))
        )
    };
    let dest = downloads_dir().join(&file_name);
    let after = after_kind(&leaf);
    enqueue(state, leaf.clone(), url, dest, after);
    state.status = format!("queued: {leaf}");
}

/// Queue one book of the embedded cross-source catalog.
fn download_indexed(state: &mut State, index: usize) {
    let Some(file) = source_index().get(index) else {
        return;
    };
    let source = &DL_PRESETS[usize::from(file.source)];
    let leaf = safe_leaf(&file.name);
    let url = if file.archive.is_empty() {
        shamela_import::archive::download_url(source.id, &file.name)
    } else {
        shamela_import::archive::download_url(source.id, &format!("{}/{}", file.archive, file.name))
    };
    let file_name = if file.archive.is_empty() {
        leaf.clone()
    } else {
        format!(
            "{}_{leaf}",
            short_hash(&format!("{}/{}/{}", source.id, file.archive, file.name))
        )
    };
    let dest = downloads_dir().join(&file_name);
    let after = after_kind(&leaf);
    enqueue(state, leaf.clone(), url, dest, after);
    state.status = format!("queued: {leaf} ({})", source.label);
}

/// Search every curated source by book name, file name or source label.
fn search_all_sources(state: &mut State) {
    let key = title_key(&state.dl_query);
    if key.chars().count() < 2 {
        state.status = String::from(tr(state.lang, "search_all_hint"));
        return;
    }
    // Score every match, best first: the same title, then a title that only
    // adds or drops words, then the same words in another order, then file
    // names and finally the source name. This keeps the book the user opened
    // at the top even when hundreds of rows share a word.
    let mut scored: Vec<(u8, usize)> = Vec::new();
    for (index, file) in source_index().iter().enumerate() {
        let source = &DL_PRESETS[usize::from(file.source)];
        let label = file_label(state, source.id, &file.archive, &file.name);
        let source_hit = key.chars().count() >= 4
            && source
                .label
                .split_whitespace()
                .any(|word| title_key(word).contains(&key));
        let score = if let Some(score) = title_score(&state.dl_query, &label) {
            score
        } else if title_key(&file.name).contains(&key) {
            5
        } else if source_hit {
            6
        } else {
            continue;
        };
        scored.push((score, index));
    }
    scored.sort_unstable();
    scored.truncate(MAX_GLOBAL_RESULTS);
    let count = scored.len();
    let hits: Vec<usize> = scored.into_iter().map(|(_, index)| index).collect();
    state.dl_global = Some(hits);
    state.dl_results.clear();
    state.dl_sources = false;
    state.status = format!(
        "{}: {count} — \"{}\"",
        tr(state.lang, "search_all_sources"),
        state.dl_query.trim()
    );
}

/// Stable short tag used to keep deep-pack file names unique.
fn short_hash(text: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:08x}")
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

/// Normalized key for title matching: no tashkeel, no tatweel, unified
/// hamza, lower case and alphanumerics only. Two spellings of one title
/// ("أحاديث" / "احاديث") therefore share the same key.
fn title_key(content: &str) -> String {
    content
        .chars()
        .filter(|ch| !matches!(ch, '\u{064B}'..='\u{0652}' | '\u{0640}' | '\u{0670}'))
        .map(|ch| match ch {
            '\u{0623}' | '\u{0625}' | '\u{0622}' | '\u{0671}' => '\u{0627}',
            '\u{0624}' => '\u{0648}',
            '\u{0626}' => '\u{064A}',
            other => other,
        })
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

/// Words that say nothing about the book: editions, volumes and print data.
const TITLE_NOISE: &[&str] = &[
    "الطبعة",
    "طبعة",
    "تحقيق",
    "تعليق",
    "الجزء",
    "جزء",
    "المجلد",
    "مجلد",
    "ط",
    "مجموعة",
];

/// Meaningful words of a title, normalized, without edition noise.
fn title_tokens(content: &str) -> Vec<String> {
    content
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(title_key)
        .filter(|word| !word.is_empty())
        .filter(|word| !word.chars().all(|ch| ch.is_ascii_digit()))
        .filter(|word| !TITLE_NOISE.contains(&word.as_str()))
        .collect()
}

/// Title without edition noise, used to compare two spellings of one book.
fn title_core(content: &str) -> String {
    title_tokens(content).join("")
}

/// Shortest label that may stand in for a longer title.
const TITLE_MIN_MATCH: usize = 8;

/// How well a wanted title matches a label. Lower is better, `None` means no
/// match at all. The score also orders the result list, so the edition the
/// user opened comes first.
fn title_score(wanted: &str, label: &str) -> Option<u8> {
    let key = title_key(wanted);
    let other = title_key(label);
    if key.is_empty() || other.is_empty() {
        return None;
    }
    if key == other {
        return Some(0);
    }
    // The label carries the searched words: a keyword or a whole title.
    if other.contains(&key) {
        return Some(if other.starts_with(&key) { 1 } else { 2 });
    }
    // The label is the shorter spelling of the wanted title. Only a real book
    // name may stand in for a longer one, or every short word would match.
    if key.contains(&other) {
        return if other.chars().count() >= TITLE_MIN_MATCH {
            Some(if key.starts_with(&other) { 1 } else { 2 })
        } else {
            None
        };
    }
    // Same title, different edition marks.
    let core = title_core(wanted);
    let other_core = title_core(label);
    if !core.is_empty() && !other_core.is_empty() {
        if core == other_core {
            return Some(2);
        }
        if (core.contains(&other_core) || other_core.contains(&core))
            && other_core.chars().count() >= TITLE_MIN_MATCH
        {
            return Some(3);
        }
    }
    // Same words in a different order or with a few extra ones.
    let words = title_tokens(wanted);
    let other_words = title_tokens(label);
    if words.len() >= 2 && other_words.len() >= 2 && words.first() == other_words.first() {
        let hits = words
            .iter()
            .filter(|word| other_words.contains(word))
            .count();
        if hits >= 2 && hits * 4 >= words.len() * 3 {
            return Some(4);
        }
    }
    None
}

/// "540 KB" / "12.3 MB" / "1.20 GB" / "—" when the listing has no size.
fn fmt_size(bytes: u64) -> String {
    if bytes == 0 {
        return String::from("—");
    }
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

/// Numeric stem of a listed book file (`10746.mbok`, `Books/4/34.mdb`).
fn file_id(name: &str) -> Option<i64> {
    let leaf = name.rsplit('/').next().unwrap_or(name);
    let stem = leaf.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(leaf);
    stem.trim().parse::<i64>().ok()
}

/// Last path segment of a name or URL, decoded and safe to use as a file
/// name. Deep archive.org URLs can encode the inner folder in one segment
/// (`shamela%2F10006.mbok`), which must never leak into a local path.
fn safe_leaf(name: &str) -> String {
    let decoded = percent_decode(name);
    let leaf = decoded
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if leaf.is_empty() || leaf == "." || leaf == ".." {
        String::from("book.bok")
    } else {
        leaf
    }
}

/// Cache key of a pack's fetched name index.
fn pack_key(source_id: &str, archive: &str) -> String {
    format!("{source_id}\t{archive}")
}

/// Human label for one source book row: a name fetched from the pack's own
/// catalog, the open master database when the source uses official book ids,
/// or a plain `كتاب <n>` fallback when nothing can know it yet.
fn file_label(state: &State, source_id: &str, archive: &str, name: &str) -> String {
    if let Some(id) = file_id(name) {
        if let Some(map) = state.pack_names.get(&pack_key(source_id, archive)) {
            if let Some(title) = map.get(&id) {
                return title.clone();
            }
        }
        if OFFICIAL_ID_SOURCES.contains(&source_id) {
            if let Some(title) = state.book_titles.get(&id) {
                return title.clone();
            }
        }
        return format!("{} {id}", tr(state.lang, "book_number"));
    }
    display_name(name)
}

/// Load every name index that was fetched in an earlier session, so numbered
/// files show their real names without another download.
fn preload_name_caches(state: &mut State) {
    for entry in NAME_INDEXES {
        let key = pack_key(entry.id, entry.archive);
        if let std::collections::hash_map::Entry::Vacant(slot) = state.pack_names.entry(key) {
            if let Some(map) = load_name_cache(entry) {
                slot.insert(map);
            }
        }
    }
}

fn names_dir() -> PathBuf {
    let dir = data_dir().join("names");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn name_cache_path(entry: &NameIndexSource) -> PathBuf {
    names_dir().join(format!(
        "names-{}.tsv",
        short_hash(&format!("{}/{}/{}", entry.id, entry.archive, entry.inner))
    ))
}

fn load_name_cache(entry: &NameIndexSource) -> Option<std::collections::HashMap<i64, String>> {
    let text = std::fs::read_to_string(name_cache_path(entry)).ok()?;
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let mut parts = line.splitn(2, '\t');
        let Some(id) = parts.next().and_then(|raw| raw.trim().parse::<i64>().ok()) else {
            continue;
        };
        let title = parts.next().unwrap_or("").trim();
        if !title.is_empty() {
            map.insert(id, title.to_string());
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Load (or download) a pack's own name catalog, in the background so large
/// indexes never freeze the window.
fn start_name_fetch(state: &mut State) {
    if state.name_rx.is_some() {
        state.status = format!("{}…", tr(state.lang, "names_loading"));
        return;
    }
    let Some(entry) = name_index_source(&state.dl_id, &state.dl_archive) else {
        state.status = String::from(tr(state.lang, "names_no_index"));
        return;
    };
    let key = pack_key(entry.id, entry.archive);
    if state.pack_names.contains_key(&key) {
        state.status = String::from(tr(state.lang, "names_loaded"));
        return;
    }
    if let Some(map) = load_name_cache(entry) {
        let count = map.len();
        state.pack_names.insert(key, map);
        state.status = format!("{}: {count}", tr(state.lang, "names_loaded"));
        return;
    }
    let url = shamela_import::archive::download_url(
        entry.id,
        &format!("{}/{}", entry.archive, entry.inner),
    );
    let cache = name_cache_path(entry);
    let download = cache.with_extension("mdb");
    let (tx, rx) = std::sync::mpsc::channel();
    state.name_rx = Some(rx);
    state.status = format!("{}…", tr(state.lang, "names_loading"));
    std::thread::spawn(move || {
        let result = shamela_import::archive::download_to(&url, &download)
            .map_err(|error| error.to_string())
            .and_then(|_| {
                let index = shamela_import::bok::name_index(&download)
                    .map_err(|error| error.to_string())?;
                let mut text = String::new();
                for (id, title) in &index {
                    text.push_str(&format!("{id}\t{title}\n"));
                }
                let _ = std::fs::write(&cache, text);
                let _ = std::fs::remove_file(&download);
                Ok(index)
            });
        let _ = tx.send((key, result));
    });
}

/// Pick up a finished name-index fetch (called from the periodic tick).
fn poll_name_fetch(state: &mut State) {
    let received = state.name_rx.as_ref().map(|rx| rx.try_recv());
    let Some(received) = received else {
        return;
    };
    match received {
        Ok((key, result)) => {
            state.name_rx = None;
            match result {
                Ok(map) => {
                    let count = map.len();
                    state.pack_names.insert(key, map);
                    state.status = format!("{}: {count}", tr(state.lang, "names_loaded"));
                }
                Err(error) => {
                    state.status = format!("names failed: {error}");
                }
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            state.name_rx = None;
        }
    }
}

/// archive.org can list and stream entries from ZIP/RAR containers, but its
/// 7z view often fails; 7z files are downloaded whole and unpacked locally.
fn can_browse(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".rar")
}

/// Short kind badge of a source file.
fn file_kind(name: &str, lang: Lang) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".bok") || lower.ends_with(".mdb") || lower.ends_with(".mbok") {
        tr(lang, "kind_book")
    } else if shamela_import::extract::is_importable(name) {
        tr(lang, "kind_pack")
    } else {
        tr(lang, "kind_unsupported")
    }
}

/// Source files filtered by the name filter (list order preserved).
/// Numbered files match on their resolved title as well as their file name.
fn visible_files(state: &State) -> Vec<shamela_import::archive::ArchiveFile> {
    let needle = match_key(&state.dl_filter);
    state
        .dl_files
        .iter()
        .filter(|file| {
            needle.is_empty()
                || match_key(&file.name).contains(&needle)
                || match_key(&file_label(
                    state,
                    &state.dl_id,
                    &state.dl_archive,
                    &file.name,
                ))
                .contains(&needle)
        })
        .cloned()
        .collect()
}

/// Queue every selected source file (list order).
fn download_selected(state: &mut State) {
    if state.dl_sources {
        state.status = String::from("open a source first");
        return;
    }
    let total = state
        .dl_files
        .iter()
        .filter(|file| state.dl_selected.contains(&file.name))
        .count();
    let names: Vec<String> = state
        .dl_files
        .iter()
        .filter(|file| state.dl_selected.contains(&file.name))
        .take(MAX_QUEUE_BATCH)
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
    state.status = if total > MAX_QUEUE_BATCH {
        format!("queued first {count} of {total} selected file(s) — narrow with the filter")
    } else {
        format!("queued {count} selected file(s)")
    };
}

/// Search every curated source for the title of one open tab (active or not)
/// and open the download manager on the results. Reading, not downloading:
/// the user picks the exact edition from the list.
fn download_tab(state: &mut State, index: usize) {
    let Some(tab) = state.tabs.get(index) else {
        state.status = String::from("open a book first");
        return;
    };
    let title = tab.title.clone();
    state.dl_query = title;
    state.dl_results.clear();
    state.dl_sources = false;
    state.show_downloads = true;
    search_all_sources(state);
    // The curated sources do not hold every book of the master catalog, so
    // fall back to archive.org when the local catalog has no match.
    let empty = state
        .dl_global
        .as_ref()
        .map(|hits| hits.is_empty())
        .unwrap_or(true);
    if empty {
        search_archive(state);
    }
}

/// Search archive.org for items that carry the given title. Used on its own
/// and as the fallback when the curated sources have no match.
fn search_archive(state: &mut State) {
    let query = state.dl_query.trim().to_string();
    if query.is_empty() {
        state.status = String::from("type a book title to search archive.org");
        return;
    }
    state.dl_sources = false;
    state.dl_archive.clear();
    match shamela_import::archive::search_items(&query, 30) {
        Ok(items) => {
            let count = items.len();
            state.dl_results = items;
            state.status = format!("archive.org: {count} book item(s) for \"{query}\"");
        }
        Err(error) => {
            state.dl_results.clear();
            state.status = format!("archive search failed: {error}");
        }
    }
}

/// Download the book that is open right now (toolbar button).
fn download_current(state: &mut State) {
    let Some(index) = state.active_tab else {
        state.status = String::from("open a book first");
        return;
    };
    download_tab(state, index);
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

/// Page text and background that follow the active light/dark theme.
fn reader_colors(kind: AppTheme) -> (Color, Color) {
    match kind {
        AppTheme::Light => (Color::BLACK, Color::WHITE),
        AppTheme::Dark => (
            Color::from_rgb(0.92, 0.92, 0.95),
            Color::from_rgb(0.11, 0.11, 0.14),
        ),
    }
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
            | Message::TogglePane(_)
            | Message::ClosePane(_)
            | Message::SideModePick(_)
            | Message::SideModeCycle
            | Message::ResetLayout
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
            if panel == Panel::Downloads {
                ensure_source_files(state);
            }
        }
        Message::BookmarkAdd => add_bookmark(state),
        Message::BookmarkRemove(id) => remove_bookmark(state, id),
        Message::ReaderPrev => move_page(state, -1),
        Message::ReaderNext => move_page(state, 1),
        Message::TocJump(target) => jump_to_page(state, target),
        Message::DownloadIdInput(id) => {
            state.dl_id = parse_item_id(&id);
            // A typed id always means "show this item", not an inner pack
            // or file list left over from the previous source.
            state.dl_archive.clear();
            state.dl_files.clear();
            state.dl_selected.clear();
            state.dl_global = None;
        }
        Message::DownloadList => {
            state.dl_results.clear();
            state.dl_global = None;
            state.dl_archive.clear();
            state.dl_sources = false;
            refresh_download_list(state);
        }
        Message::DownloadFile(name) => enqueue_file(state, name),
        Message::DownloadAll => {
            if state.dl_sources {
                state.status = String::from("open a source first");
            } else {
                let total = state.dl_files.len();
                let files: Vec<String> = state
                    .dl_files
                    .iter()
                    .take(MAX_QUEUE_BATCH)
                    .map(|file| file.name.clone())
                    .collect();
                let queued = files.len();
                for name in files {
                    enqueue_file(state, name);
                }
                state.status = if total == 0 {
                    String::from("no files listed — fetch a source first")
                } else if total > MAX_QUEUE_BATCH {
                    format!("queued first {queued} of {total} file(s) — narrow with the filter")
                } else {
                    format!("queued all {total} file(s)")
                };
            }
        }
        Message::DownloadCurrent => download_current(state),
        Message::DownloadTab(index) => download_tab(state, index),
        Message::DownloadOpen => {
            state.show_downloads = true;
            preload_name_caches(state);
            ensure_source_files(state);
        }
        Message::DlToggle => {
            state.show_downloads = !state.show_downloads;
            if state.show_downloads {
                preload_name_caches(state);
                ensure_source_files(state);
            }
        }
        Message::DlTick => {
            poll_name_fetch(state);
            poll_downloads(state);
        }
        Message::DlRemove(id) => remove_job(state, id),
        Message::DlRetry(id) => retry_job(state, id),
        Message::DlCancel => cancel_all(state),
        Message::DlToggleFile(name) => {
            if !state.dl_selected.remove(&name) {
                state.dl_selected.insert(name);
            }
        }
        Message::DlSelectAll => {
            if state.dl_sources {
                state.status = String::from("open a source first");
            } else {
                let files = visible_files(state);
                for file in files {
                    state.dl_selected.insert(file.name);
                }
                state.status = format!("selected {} file(s)", state.dl_selected.len());
            }
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
        Message::DlPresetSource(index) => {
            let Some(source) = DL_PRESETS.get(index) else {
                return Task::none();
            };
            state.dl_id = String::from(source.id);
            state.dl_archive = source.archive.unwrap_or("").to_string();
            state.dl_results.clear();
            state.dl_global = None;
            state.dl_filter.clear();
            state.dl_sources = false;
            refresh_download_list(state);
        }
        Message::DlBrowseArchive(archive) => {
            state.dl_archive = archive;
            state.dl_results.clear();
            state.dl_global = None;
            state.dl_filter.clear();
            state.dl_sources = false;
            refresh_download_list(state);
        }
        Message::DlBackArchive => {
            if !state.dl_archive.is_empty() {
                state.dl_archive.clear();
                state.dl_filter.clear();
                refresh_download_list(state);
            }
        }
        Message::DlSourcesToggle => {
            state.dl_sources = !state.dl_sources;
            if state.dl_sources {
                state.dl_results.clear();
                state.dl_global = None;
            }
        }
        Message::DlQueryInput(text) => state.dl_query = text,
        Message::DlSearch => {
            state.dl_global = None;
            search_archive(state);
        }
        Message::DlGlobalSearch => search_all_sources(state),
        Message::DlDownloadIndexed(index) => download_indexed(state, index),
        Message::DlFetchNames => start_name_fetch(state),
        Message::DlFetchNamesIndexed(index) => {
            if let Some(file) = source_index().get(index) {
                let source = &DL_PRESETS[usize::from(file.source)];
                state.dl_id = source.id.to_string();
                state.dl_archive = file.archive.clone();
            }
            start_name_fetch(state);
        }
        Message::DlClearResults => {
            state.dl_results.clear();
            state.dl_global = None;
        }
        Message::DlOpenItem(id) => {
            state.dl_id = id;
            state.dl_archive.clear();
            state.dl_sources = false;
            state.dl_results.clear();
            state.dl_global = None;
            state.dl_filter.clear();
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
            let (reader_text, reader_bg) = reader_colors(state.theme_kind);
            state.reader_text = reader_text;
            state.reader_bg = reader_bg;
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
        Message::SideResized(event) => {
            state.side_panes.resize(event.split, event.ratio);
        }
        Message::ReaderPaneResized(event) => {
            state.reader_panes.resize(event.split, event.ratio);
        }
        Message::TogglePane(kind) => {
            let visible = match kind {
                PaneKind::Books => !state.show_books_pane,
                PaneKind::Side => !state.show_side_pane,
                PaneKind::Reader => true,
            };
            match kind {
                PaneKind::Books => state.show_books_pane = visible,
                PaneKind::Side => state.show_side_pane = visible,
                PaneKind::Reader => {}
            }
            set_pane_visible(state, kind, visible);
        }
        Message::ClosePane(kind) => {
            match kind {
                PaneKind::Books => state.show_books_pane = false,
                PaneKind::Side => state.show_side_pane = false,
                PaneKind::Reader => {}
            }
            set_pane_visible(state, kind, false);
        }
        Message::SideModePick(mode) => {
            state.side_mode = mode;
        }
        Message::SideModeCycle => {
            state.side_mode = match state.side_mode {
                SideMode::Tabs => SideMode::Tree,
                SideMode::Tree => SideMode::TwoTrees,
                SideMode::TwoTrees => SideMode::Tabs,
            };
        }
        Message::ResetLayout => {
            state.show_books_pane = true;
            state.show_side_pane = true;
            state.side_mode = SideMode::Tabs;
            state.expanded_category = None;
            state.panes = init_panes(true, true);
            state.side_panes = init_side_panes();
        }
        Message::ToggleSection(id) => {
            if state.expanded_category == Some(id) {
                state.expanded_category = None;
            } else {
                state.expanded_category = Some(id);
                state.active_category = Some(id);
                state.active_author = None;
                state.query.clear();
                state.results.clear();
                state.author_results.clear();
                load_category(state);
            }
        }
        Message::ToggleToc => {
            state.show_toc = !state.show_toc;
            set_reader_index_visible(state, state.show_toc);
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
            let (reader_text, reader_bg) = reader_colors(state.theme_kind);
            state.reader_text = reader_text;
            state.reader_bg = reader_bg;
            state.search_opts = NormalizeOptions::default();
            rebuild_index(state);
            state.status = String::from("settings reset");
        }
        Message::OpenImported(name) => {
            let path = books_dir().join(&name);
            let title = path
                .file_stem()
                .and_then(|part| part.to_str())
                .unwrap_or("book")
                .to_string();
            open_imported_db(state, &path, title, None);
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
        Message::QuranJuz(juz) => {
            let index = (juz.clamp(1, 30) - 1) as usize;
            if let Some(&(sura, _)) = JUZ_STARTS.get(index) {
                state.quran_sura = sura;
                open_quran_tab(state, sura);
            }
        }
        Message::QuranIndexPick(index) => {
            state.quran_index = index;
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
    let items: Vec<Element<'a, Message>> = books
        .iter()
        .take(200)
        .map(|book| book_row(state, book))
        .collect();
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
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
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
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
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
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
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// One curated keyless source. `archive` switches the popup to a container
/// listing so a single book can be fetched without downloading the whole pack.
struct DlSource {
    id: &'static str,
    label: &'static str,
    archive: Option<&'static str>,
    count: &'static str,
    format: &'static str,
    note: &'static str,
    group: &'static str,
}

const DL_PRESETS: &[DlSource] = &[
    DlSource {
        id: "mutunulhadith",
        label: "متون الحديث",
        archive: None,
        count: "١٧٢",
        format: "BOK",
        note: "RAR ٩٩MB",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "shamelah-books",
        label: "المكتبة الشاملة ٢.٠",
        archive: None,
        count: "٨٠ + ٧z",
        format: "BOK",
        note: "BOK + ٧z",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "ghazali3shamela",
        label: "كتب الغزالي",
        archive: None,
        count: "٣٢",
        format: "BOK",
        note: "",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "AlbanyBok",
        label: "كتب الألباني",
        archive: None,
        count: "٩٤",
        format: "BOK",
        note: "ZIP ٣٥MB",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "ahl_sunna_books_shamela",
        label: "كتب أهل السنة",
        archive: None,
        count: "١٥١",
        format: "BOK",
        note: "ZIP ٣٤MB",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "kotobe-mojahed-din-maktab-shamele",
        label: "مكتبة المجاهد",
        archive: None,
        count: "١٦",
        format: "BOK",
        note: "RAR ١.٨MB",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "SHAMELA_201405",
        label: "نماذج الشاملة ٢٠١٤",
        archive: None,
        count: "٩",
        format: "BOK",
        note: "RAR ٢٥MB",
        group: "كتب BOK مباشرة",
    },
    DlSource {
        id: "bin-wahaf-shamela",
        label: "مكتبة ابن وهف",
        archive: Some("BinWahaf-Shamela.zip"),
        count: "١٣٣",
        format: "MDB",
        note: "ZIP ٧٦MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "ABDOFAIZE2010_GMAIL_201702",
        label: "المكتبة السلفية الشاملة",
        archive: Some("المكتبة السلفية الشاملة.rar"),
        count: "٤٧٠٠",
        format: "MDB",
        note: "RAR ١.٦GB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ١/٧",
        archive: Some("shamela-hesham.part01.rar"),
        count: "٣١٠٧",
        format: "MDB",
        note: "RAR ٩٥٤MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ٢/٧",
        archive: Some("shamela-hesham.part02.rar"),
        count: "٢٩٣٣",
        format: "MDB",
        note: "RAR ٩٥٤MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ٣/٧",
        archive: Some("shamela-hesham.part03.rar"),
        count: "٣٦٢٨",
        format: "MDB",
        note: "RAR ٩٥٤MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ٤/٧",
        archive: Some("shamela-hesham.part04.rar"),
        count: "٢٤٢٨",
        format: "MDB",
        note: "RAR ٩٥٤MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ٥/٧",
        archive: Some("shamela-hesham.part05.rar"),
        count: "١٣٣٩",
        format: "MDB",
        note: "RAR ٩٥٤MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ٦/٧",
        archive: Some("shamela-hesham.part06.rar"),
        count: "—",
        format: "MDB",
        note: "RAR ٩٥٤MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "shamela-hesham",
        label: "شاملة هشام ٧/٧",
        archive: Some("shamela-hesham.part07.rar"),
        count: "—",
        format: "MDB",
        note: "RAR ٥٥MB",
        group: "مكتبات MDB للجوال",
    },
    DlSource {
        id: "walid_201501",
        label: "كتب الألباني — MBOK",
        archive: Some("كتب الالباني.rar"),
        count: "٦٥",
        format: "MBOK",
        note: "RAR ٧٥MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "walid_201501",
        label: "الألباني المرقم آليا",
        archive: Some("كتب الالباني-مرقم آليا.rar"),
        count: "٩",
        format: "MBOK",
        note: "RAR ١٣MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "z_427",
        label: "كتب التاريخ — MBOK",
        archive: Some("كتب التاريخ.rar"),
        count: "١٦٠",
        format: "MBOK",
        note: "RAR ٤٠٠MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "z_427",
        label: "متون الحديث — MBOK",
        archive: Some("متون الحديث.rar"),
        count: "١٧٧",
        format: "MBOK",
        note: "RAR ٤٦٩MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "shamela.ws.005",
        label: "شاملة ws ٠٠٥",
        archive: Some("shamela.ws.005.rar"),
        count: "٥٢",
        format: "MBOK",
        note: "RAR ١٥٦MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "Shamela_android",
        label: "شاملة أندرويد ١/٥",
        archive: Some("shamela.part01.rar"),
        count: "٥٤٦",
        format: "MBOK",
        note: "RAR ١٠٠MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "Shamela_android",
        label: "شاملة أندرويد ٢/٥",
        archive: Some("shamela.part02.rar"),
        count: "٥٢٢",
        format: "MBOK",
        note: "RAR ١٠٠MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "Shamela_android",
        label: "شاملة أندرويد ٣/٥",
        archive: Some("shamela.part03.rar"),
        count: "١٦٣",
        format: "MBOK",
        note: "RAR ١٠٠MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "Shamela_android",
        label: "شاملة أندرويد ٤/٥",
        archive: Some("shamela.part04.rar"),
        count: "١٠٣",
        format: "MBOK",
        note: "RAR ١٠٠MB",
        group: "مكتبات MBOK (SQLite)",
    },
    DlSource {
        id: "Shamela_android",
        label: "شاملة أندرويد ٥/٥",
        archive: Some("shamela.part05.rar"),
        count: "٨٢",
        format: "MBOK",
        note: "RAR ٣٢MB",
        group: "مكتبات MBOK (SQLite)",
    },
];

/// Safety cap for bulk actions and for the rendered file rows. Huge packs
/// contain thousands of books; the filter narrows them before selecting.
const MAX_QUEUE_BATCH: usize = 500;
const MAX_RENDER_FILES: usize = 400;

/// Cross-source title search: cap on returned/rendered hits.
const MAX_GLOBAL_RESULTS: usize = 300;

/// Every book across every curated source, generated from public archive.org
/// listings (metadata only — no book content). One TSV row per file:
/// source index, container (empty = direct item file), file name, size.
const SOURCE_INDEX_TSV: &str = include_str!("../assets/source_index.tsv");

/// One book entry of the cross-source catalog.
#[derive(Debug, Clone)]
struct IndexedFile {
    source: u16,
    archive: String,
    name: String,
    size: u64,
}

/// Parse the embedded cross-source catalog once.
fn source_index() -> &'static [IndexedFile] {
    static INDEX: std::sync::OnceLock<Vec<IndexedFile>> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        SOURCE_INDEX_TSV
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(4, '\t');
                let source = parts.next()?.parse::<u16>().ok()?;
                let archive = parts.next()?.to_string();
                let name = parts.next()?.to_string();
                let size = parts.next()?.trim().parse::<u64>().unwrap_or(0);
                if name.is_empty() || usize::from(source) >= DL_PRESETS.len() {
                    return None;
                }
                Some(IndexedFile {
                    source,
                    archive,
                    name,
                    size,
                })
            })
            .collect()
    })
}

/// A pack that ships its own Shamela catalog (`main.mdb`): fetching that one
/// small file gives every numbered book its real name before any book is
/// downloaded. `inner` is the exact archive.org listing name.
struct NameIndexSource {
    id: &'static str,
    archive: &'static str,
    inner: &'static str,
    size: &'static str,
}

const NAME_INDEXES: &[NameIndexSource] = &[
    NameIndexSource {
        id: "bin-wahaf-shamela",
        archive: "BinWahaf-Shamela.zip",
        inner: "BinWahaf-Shamela/Files/main.mdb",
        size: "٣٢٦KB",
    },
    NameIndexSource {
        id: "ABDOFAIZE2010_GMAIL_201702",
        archive: "المكتبة السلفية الشاملة.rar",
        inner: "الشاملة (F)/Files/main.mdb",
        size: "١٤MB",
    },
];

fn name_index_source(id: &str, archive: &str) -> Option<&'static NameIndexSource> {
    NAME_INDEXES
        .iter()
        .find(|entry| entry.id == id && entry.archive == archive)
}

/// Numbered `.mbok` rips of the official Shamela app: file names are the
/// master database book ids, so the opened database can name them exactly.
/// Other packs number files differently and must not borrow those names.
const OFFICIAL_ID_SOURCES: &[&str] =
    &["Shamela_android", "walid_201501", "z_427", "shamela.ws.005"];

/// One curated source row in the catalog. The whole row is the open
/// control, so long titles can never squeeze a separate button out of
/// shape; the tooltip repeats the full, untrimmed label.
fn source_row(state: &State, index: usize) -> Element<'static, Message> {
    let source = &DL_PRESETS[index];
    let mut label = format!("{} — {} · {}", source.label, source.format, source.count);
    if !source.note.is_empty() {
        label.push_str(" · ");
        label.push_str(source.note);
    }
    let content = list_row(
        trim_text(&label, 68),
        false,
        Message::DlPresetSource(index),
        item_size(state.font_size) - 1.0,
        state.chrome(),
    );
    Tooltip::new(content, ar_boxed(source.label, 12.0), Position::Left)
        .gap(4)
        .style(tooltip_style)
        .into()
}

/// Narrow variant for the side panel: one compact clickable line plus a
/// full-label tooltip on the left (where the panel has room).
fn source_row_panel(state: &State, index: usize) -> Element<'static, Message> {
    let source = &DL_PRESETS[index];
    let label = format!("{} · {}", source.label, source.count);
    let full = format!("{} — {} · {}", source.label, source.format, source.count);
    let content = list_row(
        trim_text(&label, 34),
        false,
        Message::DlPresetSource(index),
        item_size(state.font_size) - 3.0,
        state.chrome(),
    );
    Tooltip::new(content, ar_boxed(full, 12.0), Position::Left)
        .gap(4)
        .style(tooltip_style)
        .into()
}

/// Scrollable source catalog with format group headers.
fn source_list(state: &State) -> Element<'static, Message> {
    let mut items: Vec<Element<'static, Message>> = Vec::new();
    let mut group = "";
    for (index, source) in DL_PRESETS.iter().enumerate() {
        if source.group != group {
            group = source.group;
            items.push(ar_boxed(source.group, 11.0));
        }
        items.push(source_row(state, index));
    }
    scrollable(column(items).spacing(4))
        .height(Length::Fixed(185.0))
        .into()
}

/// One source-file row: selection box, download button, size, kind, name.
fn file_row(
    state: &State,
    file: &shamela_import::archive::ArchiveFile,
) -> Element<'static, Message> {
    let selected = state.dl_selected.contains(&file.name);
    let mut cells: Vec<Element<'static, Message>> = vec![
        button(ar_boxed(if selected { "[x]" } else { "[ ]" }, 12.0))
            .on_press(Message::DlToggleFile(file.name.clone()))
            .style(button::text)
            .into(),
        button(ar_boxed(tr(state.lang, "download"), 12.0))
            .on_press(Message::DownloadFile(file.name.clone()))
            .into(),
    ];
    if state.dl_archive.is_empty() && can_browse(&file.name) {
        cells.push(
            button(ar_boxed(tr(state.lang, "browse_pack"), 10.0))
                .on_press(Message::DlBrowseArchive(file.name.clone()))
                .into(),
        );
    }
    cells.push(ar_boxed(fmt_size(file.size), 12.0));
    cells.push(ar_boxed(file_kind(&file.name, state.lang), 12.0));
    cells.push(Space::with_width(Length::Fill).into());
    let label = file_label(state, &state.dl_id, &state.dl_archive, &file.name);
    let full = format!("{label} — {}", file.name);
    cells.push(
        Tooltip::new(
            ar_boxed(trim_text(&label, 38), 13.0),
            ar_boxed(trim_text(&full, 96), 11.0),
            Position::Left,
        )
        .gap(4)
        .style(tooltip_style)
        .into(),
    );
    row(cells).spacing(6).width(Length::Fill).into()
}

/// Narrow variant for the side panel: the name lives in a fill row.
fn file_row_panel(
    state: &State,
    file: &shamela_import::archive::ArchiveFile,
) -> Element<'static, Message> {
    let selected = state.dl_selected.contains(&file.name);
    let label = format!(
        "{} ({})",
        file_label(state, &state.dl_id, &state.dl_archive, &file.name),
        fmt_size(file.size)
    );
    let mut cells: Vec<Element<'static, Message>> = vec![
        button(ar_boxed(if selected { "[x]" } else { "[ ]" }, 12.0))
            .on_press(Message::DlToggleFile(file.name.clone()))
            .style(button::text)
            .into(),
        button(ar_boxed(tr(state.lang, "download"), 12.0))
            .on_press(Message::DownloadFile(file.name.clone()))
            .into(),
    ];
    if state.dl_archive.is_empty() && can_browse(&file.name) {
        cells.push(
            button(ar_boxed(tr(state.lang, "browse_pack"), 10.0))
                .on_press(Message::DlBrowseArchive(file.name.clone()))
                .into(),
        );
    }
    cells.push(list_row(
        label,
        false,
        Message::DownloadFile(file.name.clone()),
        item_size(state.font_size) - 2.0,
        state.chrome(),
    ));
    row(cells).spacing(4).width(Length::Fill).into()
}

fn downloads_panel(state: &State) -> Element<'_, Message> {
    let lang = state.lang;
    let input = text_input(tr(lang, "source_hint"), state.dl_id.as_str())
        .font(AMIRI)
        .on_input(Message::DownloadIdInput)
        .on_submit(Message::DownloadList)
        .padding(8);
    let fetch = button(ar_boxed(tr(lang, "fetch_books"), 14.0))
        .on_press(Message::DownloadList)
        .width(Length::Fill);
    let source_toggle = button(ar_boxed(
        if state.dl_sources {
            tr(lang, "files")
        } else {
            tr(lang, "sources")
        },
        13.0,
    ))
    .on_press(Message::DlSourcesToggle)
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
        .font(AMIRI)
        .on_input(Message::DlFilterInput)
        .padding(6);
    let mut items: Vec<Element<'_, Message>> = vec![
        ar(tr(lang, "downloads"), title_size(state.font_size)),
        input.into(),
        fetch.into(),
        source_toggle.into(),
        actions.into(),
        actions2.into(),
    ];

    if state.dl_sources {
        items.push(ar(
            format!("{} — {}", tr(lang, "sources"), DL_PRESETS.len()),
            item_size(state.font_size) - 2.0,
        ));
        let mut group = "";
        for (index, source) in DL_PRESETS.iter().enumerate() {
            if source.group != group {
                group = source.group;
                items.push(ar(source.group, item_size(state.font_size) - 5.0));
            }
            items.push(source_row_panel(state, index));
        }
        return scrollable(column(items).spacing(5))
            .id(iced::widget::scrollable::Id::new("dl_sources_panel"))
            .height(Length::Fill)
            .into();
    }

    if !state.dl_results.is_empty() {
        items.push(ar(
            format!(
                "{} — {}",
                tr(lang, "search_results"),
                state.dl_results.len()
            ),
            item_size(state.font_size) - 2.0,
        ));
        for item in &state.dl_results {
            items.push(
                row![
                    button(ar_boxed(tr(lang, "open"), 12.0))
                        .on_press(Message::DlOpenItem(item.identifier.clone())),
                    list_row(
                        trim_text(&item.title, 44),
                        false,
                        Message::DlOpenItem(item.identifier.clone()),
                        item_size(state.font_size) - 2.0,
                        state.chrome(),
                    ),
                ]
                .spacing(4)
                .width(Length::Fill)
                .into(),
            );
        }
        items.push(
            button(ar_boxed(tr(lang, "back"), 12.0))
                .on_press(Message::DlClearResults)
                .width(Length::Fill)
                .into(),
        );
        return scrollable(column(items).spacing(5))
            .id(iced::widget::scrollable::Id::new("dl_results_panel"))
            .height(Length::Fill)
            .into();
    }

    if !state.dl_archive.is_empty() {
        items.push(
            row![
                button(ar_boxed(tr(lang, "back_to_pack"), 11.0))
                    .on_press(Message::DlBackArchive)
                    .width(Length::Fixed(76.0)),
                Space::with_width(Length::Fill),
                ar_boxed(
                    trim_text(&state.dl_archive, 20),
                    item_size(state.font_size) - 5.0
                ),
            ]
            .spacing(4)
            .width(Length::Fill)
            .into(),
        );
    }
    items.push(ar(
        tr(lang, "filter_word"),
        item_size(state.font_size) - 2.0,
    ));
    items.push(filter.into());
    let visible = visible_files(state);
    let total = state.dl_files.len();
    if total == 0 {
        items.push(ar(tr(lang, "no_files"), item_size(state.font_size) - 2.0));
    } else {
        items.push(ar(
            format!(
                "{} {}/{}",
                tr(lang, "shown_of"),
                visible.len().min(MAX_RENDER_FILES),
                total
            ),
            item_size(state.font_size) - 2.0,
        ));
    }
    for file in visible.iter().take(MAX_RENDER_FILES) {
        items.push(file_row_panel(state, file));
    }
    if visible.len() > MAX_RENDER_FILES {
        items.push(ar(
            format!(
                "{} {MAX_RENDER_FILES} / {}",
                tr(lang, "shown_of"),
                visible.len()
            ),
            item_size(state.font_size) - 4.0,
        ));
    }
    let scroll_id = if state.dl_archive.is_empty() {
        "dl_files_panel"
    } else {
        "dl_open_pack_panel"
    };
    scrollable(column(items).spacing(6))
        .id(iced::widget::scrollable::Id::new(scroll_id))
        .height(Length::Fill)
        .into()
}

/// Books of the picked section (middle sub-panel).
/// Books pane: title with a close button, then the list of books.
fn books_pane(state: &State) -> Element<'_, Message> {
    let close: Element<'_, Message> = Tooltip::new(
        button(ticon("x", 14.0)).on_press(Message::ClosePane(PaneKind::Books)),
        ar_boxed(tr(state.lang, "close_pane"), 12.0),
        Position::Left,
    )
    .gap(4)
    .style(tooltip_style)
    .into();
    let header = row![
        ar(tr(state.lang, "books"), title_size(state.font_size)),
        Space::with_width(Length::Fill),
        close,
    ]
    .spacing(6);
    column![header, books_panel(state)]
        .spacing(4)
        .height(Length::Fill)
        .into()
}

/// Side pane: close button, the chosen layout and the panel tabs.
fn side_pane(state: &State) -> Element<'_, Message> {
    let close: Element<'_, Message> = Tooltip::new(
        button(ticon("x", 14.0)).on_press(Message::ClosePane(PaneKind::Side)),
        ar_boxed(tr(state.lang, "close_pane"), 12.0),
        Position::Left,
    )
    .gap(4)
    .style(tooltip_style)
    .into();
    let header = row![Space::with_width(Length::Fill), close].spacing(6);
    // The tree layouts replace the sections panel; the other panels (authors,
    // bookmarks, Quran, downloads, control) stay reachable from the tabs.
    let body: Element<'_, Message> = match state.side_mode {
        SideMode::Tabs => sidebar(state),
        SideMode::Tree if matches!(state.panel, Panel::Library) => catalog_tree(state),
        SideMode::TwoTrees if matches!(state.panel, Panel::Library) => two_trees(state),
        _ => sidebar(state),
    };
    let tabs: Element<'_, Message> = panel_tabs(state);
    column![header, body, tabs]
        .spacing(4)
        .height(Length::Fill)
        .into()
}

/// Catalog tree: every section, expandable to the books inside it.
fn catalog_tree(state: &State) -> Element<'_, Message> {
    let size = item_size(state.font_size);
    let mut items: Vec<Element<'_, Message>> =
        vec![ar(tr(state.lang, "sections"), title_size(state.font_size))];
    for category in &state.categories {
        let expanded = state.expanded_category == Some(category.id);
        let marker = if expanded { "▾" } else { "▸" };
        items.push(list_row(
            format!("{marker} {}", category.name),
            Some(category.id) == state.active_category,
            Message::ToggleSection(category.id),
            size,
            state.chrome(),
        ));
        if expanded && state.active_category == Some(category.id) {
            if state.books.is_empty() {
                items.push(ar(tr(state.lang, "no_results"), size));
            }
            for book in state.books.iter().take(200) {
                items.push(book_row(state, book));
            }
        }
    }
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Index tree: the table of contents of the open book.
fn index_tree(state: &State) -> Element<'_, Message> {
    let size = (state.font_size - 4.0).max(12.0);
    let mut items: Vec<Element<'_, Message>> = vec![ar(
        tr(state.lang, "book_index"),
        title_size(state.font_size),
    )];
    let Some(tab) = active_reader(state) else {
        items.push(ar(tr(state.lang, "choose_book"), size));
        return scrollable(column(items).spacing(4).width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    };
    let mut flat: Vec<(usize, &TocNode)> = Vec::new();
    flatten_toc(&tab.toc, 0, &mut flat);
    let here = tab
        .pages
        .get(tab.index)
        .map(|page| page.id.to_string())
        .unwrap_or_default();
    for (depth, node) in flat {
        let label = format!("{}{}", "  ".repeat(depth), node.content);
        let selected = !here.is_empty() && node.page.as_deref() == Some(here.as_str());
        match node
            .page
            .as_deref()
            .and_then(|raw| tab.pages.iter().position(|page| page.id.to_string() == raw))
        {
            Some(index) => items.push(list_row(
                label,
                selected,
                Message::TocJump(index),
                size,
                state.chrome(),
            )),
            None => items.push(ar(label.as_str(), size)),
        }
    }
    scrollable(column(items).spacing(2).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Flatten a table of contents into depth/entry pairs.
fn flatten_toc<'a>(nodes: &'a [TocNode], depth: usize, out: &mut Vec<(usize, &'a TocNode)>) {
    for node in nodes {
        out.push((depth, node));
        flatten_toc(&node.children, depth + 1, out);
    }
}

/// Two resizable trees inside the side pane: catalog and book index.
fn two_trees(state: &State) -> Element<'_, Message> {
    pane_grid::PaneGrid::new(&state.side_panes, |_pane, content, _| {
        pane_grid::Content::new(match content {
            SideKind::Catalog => catalog_tree(state),
            SideKind::Index => index_tree(state),
        })
    })
    .spacing(4)
    .height(Length::Fill)
    .on_resize(8.0, Message::SideResized)
    .into()
}

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
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
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
            .width(Length::Fill)
            .into(),
        );
    }
    scrollable(column(items).spacing(4).width(Length::Fill))
        .width(Length::Fill)
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

/// Juz number containing the opening verse of a sura.
fn juz_of_sura(sura: i64) -> i64 {
    JUZ_STARTS
        .iter()
        .rposition(|&(start, aya)| start < sura || (start == sura && aya <= 1))
        .map_or(1, |index| index as i64 + 1)
}

fn quran_panel(state: &State) -> Element<'_, Message> {
    let lang = state.lang;
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
        .font(AMIRI)
        .on_input(Message::QuranSearch)
        .padding(8)
        .width(Length::Fill);
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
    // One index at a time: either the suras or the thirty juz, never mixed.
    let chrome = state.chrome();
    let accent = state.accent.color();
    let sura_button = button(ar_boxed(tr(lang, "quran_sura_index"), 12.0))
        .on_press(Message::QuranIndexPick(QuranIndex::Sura))
        .width(Length::Fill);
    let sura_button = if matches!(state.quran_index, QuranIndex::Sura) {
        sura_button.style(accent_style(accent, chrome.radius))
    } else {
        sura_button.style(button::text)
    };
    let juz_button = button(ar_boxed(tr(lang, "quran_juz_index"), 12.0))
        .on_press(Message::QuranIndexPick(QuranIndex::Juz))
        .width(Length::Fill);
    let juz_button = if matches!(state.quran_index, QuranIndex::Juz) {
        juz_button.style(accent_style(accent, chrome.radius))
    } else {
        juz_button.style(button::text)
    };
    let mode_row = row![sura_button, juz_button].spacing(4).width(Length::Fill);
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    match state.quran_index {
        QuranIndex::Sura => {
            for (index, name) in shamela_content::quran::SURA_NAMES.iter().enumerate() {
                let id = index as i64 + 1;
                rows.push(list_row(
                    format!("{id}. {name}"),
                    id == state.quran_sura,
                    Message::QuranSura(id),
                    13.0,
                    state.chrome(),
                ));
            }
        }
        QuranIndex::Juz => {
            let current = juz_of_sura(state.quran_sura);
            for (index, &(sura, aya)) in JUZ_STARTS.iter().enumerate() {
                let juz = index as i64 + 1;
                let name = shamela_content::quran::SURA_NAMES
                    .get((sura - 1) as usize)
                    .copied()
                    .unwrap_or("?");
                let label = if aya <= 1 {
                    format!("{} {juz} — {name}", tr(lang, "juz_word"))
                } else {
                    format!("{} {juz} — {name} {aya}", tr(lang, "juz_word"))
                };
                rows.push(list_row(
                    label,
                    juz == current,
                    Message::QuranJuz(juz),
                    13.0,
                    state.chrome(),
                ));
            }
        }
    }
    column![
        search,
        mode_row,
        scrollable(column(rows).spacing(2).width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .spacing(8)
    .width(Length::Fill)
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
            .style(tooltip_style)
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
            .font(AMIRI)
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

fn reader_page_view<'a>(state: &'a State, tab: &'a ReaderTab) -> Element<'a, Message> {
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
                .font(AMIRI)
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
        let head = row![
            button(ticon("x", 14.0)).on_press(Message::FindClose),
            Space::with_width(Length::Fill),
            ar_boxed(tr(state.lang, "find_book"), 13.0),
        ]
        .width(Length::Fill);
        let line = row![
            ar_boxed(counter, 13.0),
            button(ticon("next", 16.0)).on_press(Message::FindNext),
            button(ticon("prev", 16.0)).on_press(Message::FindPrev),
            text_input(tr(state.lang, "find_in_book"), state.find_input.as_str())
                .font(AMIRI)
                .on_input(Message::FindInput)
                .on_submit(Message::FindNext)
                .padding(6),
        ]
        .spacing(6)
        .width(Length::Fill);
        let mut bar = column![head, line].spacing(4);
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
    page_view.into()
}

fn reader_index_view<'a>(state: &'a State, tab: &'a ReaderTab) -> Element<'a, Message> {
    let mut flat: Vec<(usize, &TocNode)> = Vec::new();
    flatten_toc(&tab.toc, 0, &mut flat);
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
                    (state.font_size - 4.0).max(12.0),
                    state.chrome(),
                ),
                None => ar(label.as_str(), (state.font_size - 4.0).max(12.0)),
            }
        })
        .collect();
    scrollable(column(entries).spacing(2))
        .height(Length::Fill)
        .into()
}

/// Reader pane: the page and its index, split by a draggable divider.
fn reader_tab_view<'a>(state: &'a State, tab: &'a ReaderTab) -> Element<'a, Message> {
    if !state.show_toc {
        return reader_page_view(state, tab);
    }
    pane_grid::PaneGrid::new(&state.reader_panes, |_pane, content, _| {
        pane_grid::Content::new(match content {
            ReaderKind::Page => reader_page_view(state, tab),
            ReaderKind::Index => reader_index_view(state, tab),
        })
    })
    .spacing(8)
    .height(Length::Fill)
    .on_resize(10.0, Message::ReaderPaneResized)
    .into()
}

fn reader(state: &State) -> Element<'_, Message> {
    let chrome = state.chrome();
    let tabs: Vec<Element<'_, Message>> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let label: String = tab.title.chars().take(18).collect();
            let active = state.active_tab == Some(index);
            let tab_button = button(ar_boxed(label, 13.0)).on_press(Message::TabSelected(index));
            let tab_button = if active {
                tab_button.style(accent_style(chrome.accent, chrome.radius))
            } else {
                tab_button.style(button::text)
            };
            let mut children: Vec<Element<'_, Message>> = vec![tab_button.into()];
            // Every open book gets its own download button in its tab.
            if !matches!(&tab.kind, TabKind::Quran { .. }) {
                children.push(
                    Tooltip::new(
                        button(ticon("export", 12.0))
                            .on_press(Message::DownloadTab(index))
                            .padding(2)
                            .style(button::text),
                        ar(tr(state.lang, "download"), 12.0),
                        Position::Bottom,
                    )
                    .style(tooltip_style)
                    .gap(6.0)
                    .into(),
                );
            }
            children.push(
                button(ticon("x", 13.0))
                    .on_press(Message::TabClosed(index))
                    .into(),
            );
            iced::widget::Row::with_children(children).spacing(2).into()
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
                (
                    if state.show_books_pane {
                        "pane_books_on"
                    } else {
                        "pane_books_off"
                    },
                    Message::TogglePane(PaneKind::Books),
                ),
                (
                    if state.show_side_pane {
                        "pane_side_on"
                    } else {
                        "pane_side_off"
                    },
                    Message::TogglePane(PaneKind::Side),
                ),
                ("side_mode_tabs", Message::SideModePick(SideMode::Tabs)),
                ("side_mode_tree", Message::SideModePick(SideMode::Tree)),
                ("side_mode_two", Message::SideModePick(SideMode::TwoTrees)),
                ("layout_reset", Message::ResetLayout),
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

/// Short label of the current side pane layout (settings button).
fn side_mode_short_label(lang: Lang, mode: SideMode) -> &'static str {
    match mode {
        SideMode::Tabs => tr(lang, "side_mode_tabs_short"),
        SideMode::Tree => tr(lang, "side_mode_tree_short"),
        SideMode::TwoTrees => tr(lang, "side_mode_two_short"),
    }
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
        .font(AMIRI)
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
        ar(tr(state.lang, "layout_section"), 16.0),
        toggle_row(
            tr(state.lang, "pane_books").to_string(),
            state.show_books_pane,
            Message::TogglePane(PaneKind::Books)
        ),
        toggle_row(
            tr(state.lang, "pane_side").to_string(),
            state.show_side_pane,
            Message::TogglePane(PaneKind::Side)
        ),
        row![
            ar(tr(state.lang, "side_mode"), 15.0),
            Space::with_width(Length::Fill),
            button(ar_boxed(
                side_mode_short_label(state.lang, state.side_mode),
                14.0
            ))
            .on_press(Message::SideModeCycle)
            .width(Length::Fixed(120.0)),
        ]
        .spacing(6),
        row![button(ar(tr(state.lang, "layout_reset"), 14.0))
            .on_press(Message::ResetLayout)
            .width(Length::Fill),]
        .spacing(6),
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

/// archive.org results: whole collections that carry the searched title.
fn archive_results_view(state: &State) -> (Element<'_, Message>, Element<'_, Message>) {
    let lang = state.lang;
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    for item in &state.dl_results {
        rows.push(
            row![
                button(ar_boxed(tr(lang, "open"), 12.0))
                    .on_press(Message::DlOpenItem(item.identifier.clone())),
                ar_boxed(trim_text(&item.identifier, 22), 11.0),
                Space::with_width(Length::Fill),
                ar_boxed(trim_text(&item.title, 58), 13.0),
            ]
            .spacing(6)
            .width(Length::Fill)
            .into(),
        );
    }
    let header = row![
        button(ar_boxed(tr(lang, "back"), 12.0)).on_press(Message::DlClearResults),
        Space::with_width(Length::Fill),
        ar_boxed(
            format!(
                "{} — {}",
                tr(lang, "search_results"),
                state.dl_results.len()
            ),
            13.0,
        ),
    ]
    .spacing(6);
    let items = scrollable(column(rows).spacing(4)).height(Length::Fixed(145.0));
    (header.into(), items.into())
}

/// Download manager popup: source catalog, selection, queue and per-file
/// downloads. The catalog opens either an item or one of its containers.
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
        .font(AMIRI)
        .on_input(Message::DownloadIdInput)
        .on_submit(Message::DownloadList)
        .padding(8)
        .width(Length::Fill);
    let sources_label = if state.dl_sources {
        tr(lang, "files")
    } else {
        tr(lang, "sources")
    };
    let source_row = row![
        input,
        button(ar_boxed(tr(lang, "fetch_books"), 14.0)).on_press(Message::DownloadList),
        button(ar_boxed(sources_label, 13.0)).on_press(Message::DlSourcesToggle),
    ]
    .spacing(6);

    let archive_search = text_input(tr(lang, "search_all_hint"), state.dl_query.as_str())
        .font(AMIRI)
        .on_input(Message::DlQueryInput)
        .on_submit(Message::DlGlobalSearch)
        .padding(7)
        .width(Length::Fill);
    let search_row = row![
        archive_search,
        button(ar_boxed(tr(lang, "search_all_sources"), 13.0)).on_press(Message::DlGlobalSearch),
        button(ar_boxed(tr(lang, "search_archive"), 13.0)).on_press(Message::DlSearch),
    ]
    .spacing(6);

    let total = state.dl_files.len();
    let visible = visible_files(state);
    let shown = visible.len().min(MAX_RENDER_FILES);
    let all_size: u64 = state.dl_files.iter().map(|file| file.size).sum();
    let sel_size: u64 = state
        .dl_files
        .iter()
        .filter(|file| state.dl_selected.contains(&file.name))
        .map(|file| file.size)
        .sum();
    let sel_count = state.dl_selected.len();
    let numeric_files = state
        .dl_files
        .iter()
        .filter(|file| file_id(&file.name).is_some())
        .count();

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
    let list = scrollable(column(rows).spacing(10)).height(Length::Fixed(145.0));

    let (files_header, file_list): (Element<'_, Message>, Element<'_, Message>) = if state
        .dl_sources
    {
        let header = row![
            button(ar_boxed(tr(lang, "back"), 12.0)).on_press(Message::DlSourcesToggle),
            Space::with_width(Length::Fill),
            ar_boxed(
                format!("{} — {}", tr(lang, "sources"), DL_PRESETS.len()),
                13.0
            ),
        ]
        .spacing(6);
        (header.into(), source_list(state))
    } else if state
        .dl_global
        .as_ref()
        .map(|hits| hits.is_empty())
        .unwrap_or(false)
        && !state.dl_results.is_empty()
    {
        archive_results_view(state)
    } else if let Some(hits) = state.dl_global.as_ref() {
        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        let mut names_offered: Vec<String> = Vec::new();
        for index in hits.iter().take(MAX_RENDER_FILES) {
            let Some(file) = source_index().get(*index) else {
                continue;
            };
            let source = &DL_PRESETS[usize::from(file.source)];
            let label = file_label(state, source.id, &file.archive, &file.name);
            let full = format!("{label} — {}", file.name);
            let tip: Element<'_, Message> = Tooltip::new(
                ar_boxed(trim_text(&label, 46), 13.0),
                ar_boxed(trim_text(&full, 96), 11.0),
                Position::Left,
            )
            .gap(4)
            .style(tooltip_style)
            .into();
            let mut cells: Vec<Element<'_, Message>> = vec![
                button(ar_boxed(tr(lang, "download"), 12.0))
                    .on_press(Message::DlDownloadIndexed(*index))
                    .into(),
                ar_boxed(fmt_size(file.size), 12.0),
                ar_boxed(trim_text(source.label, 22), 11.0),
            ];
            let names_key = pack_key(source.id, &file.archive);
            if name_index_source(source.id, &file.archive).is_some()
                && !state.pack_names.contains_key(&names_key)
                && !names_offered.contains(&names_key)
            {
                names_offered.push(names_key);
                cells.push(
                    button(ar_boxed(tr(lang, "fetch_names"), 10.0))
                        .on_press(Message::DlFetchNamesIndexed(*index))
                        .into(),
                );
            }
            cells.push(Space::with_width(Length::Fill).into());
            cells.push(tip);
            rows.push(row(cells).spacing(6).width(Length::Fill).into());
        }
        if rows.is_empty() {
            rows.push(ar_boxed(tr(lang, "no_results"), 13.0));
            // Nothing in the curated sources: offer the archive.org search.
            rows.push(
                button(ar_boxed(tr(lang, "search_archive"), 12.0))
                    .on_press(Message::DlSearch)
                    .into(),
            );
        }
        let header = row![
            button(ar_boxed(tr(lang, "back"), 12.0)).on_press(Message::DlClearResults),
            Space::with_width(Length::Fill),
            ar_boxed(
                format!("{} — {}", tr(lang, "search_all_sources"), hits.len()),
                13.0,
            ),
        ]
        .spacing(6);
        let items = scrollable(column(rows).spacing(4)).height(Length::Fixed(145.0));
        (header.into(), items.into())
    } else if state.dl_results.is_empty() {
        let filter = text_input(tr(lang, "filter_files"), state.dl_filter.as_str())
            .font(AMIRI)
            .on_input(Message::DlFilterInput)
            .padding(6)
            .width(Length::Fixed(200.0));
        let header: Element<'_, Message> = if state.dl_archive.is_empty() {
            row![
                filter,
                button(ar_boxed(tr(lang, "sources"), 11.0)).on_press(Message::DlSourcesToggle),
                Space::with_width(Length::Fill),
                ar_boxed(
                    format!(
                        "{} — {} {}/{}",
                        tr(lang, "files_of"),
                        tr(lang, "shown_of"),
                        shown,
                        total
                    ),
                    13.0,
                ),
            ]
            .spacing(6)
            .into()
        } else {
            let names_key = pack_key(&state.dl_id, &state.dl_archive);
            let names_count = state.pack_names.get(&names_key).map(|map| map.len());
            let index_info = name_index_source(&state.dl_id, &state.dl_archive);
            let names_row: Element<'_, Message> = if let Some(count) = names_count {
                ar_boxed(format!("{}: {count}", tr(lang, "names_loaded")), 10.0)
            } else if state.name_rx.is_some() {
                ar_boxed(format!("{}…", tr(lang, "names_loading")), 10.0)
            } else if let Some(info) = index_info {
                Tooltip::new(
                    button(ar_boxed(tr(lang, "fetch_names"), 10.0)).on_press(Message::DlFetchNames),
                    ar_boxed(
                        format!("{} ({})", tr(lang, "fetch_names_hint"), info.size),
                        11.0,
                    ),
                    Position::Left,
                )
                .gap(4)
                .style(tooltip_style)
                .into()
            } else if numeric_files > 0 {
                ar_boxed(tr(lang, "names_unknown_hint"), 10.0)
            } else {
                Space::with_height(Length::Fixed(0.0)).into()
            };
            column![
                row![
                    button(ar_boxed(tr(lang, "back_to_pack"), 11.0))
                        .on_press(Message::DlBackArchive),
                    filter,
                    Space::with_width(Length::Fill),
                    ar_boxed(
                        format!(
                            "{}: {} — {}/{}",
                            tr(lang, "inside_pack"),
                            trim_text(&state.dl_archive, 24),
                            shown,
                            total
                        ),
                        12.0,
                    ),
                ]
                .spacing(6),
                names_row,
                ar_boxed(tr(lang, "pack_hint"), 10.0),
            ]
            .spacing(2)
            .into()
        };
        let mut files: Vec<Element<'_, Message>> = Vec::new();
        for file in visible.iter().take(MAX_RENDER_FILES) {
            files.push(file_row(state, file));
        }
        if files.is_empty() {
            files.push(ar_boxed(tr(lang, "no_files"), 14.0));
        }
        if visible.len() > MAX_RENDER_FILES {
            files.push(ar_boxed(
                format!(
                    "{} {MAX_RENDER_FILES} / {}",
                    tr(lang, "shown_of"),
                    visible.len()
                ),
                11.0,
            ));
        }
        let items = scrollable(column(files).spacing(4)).height(Length::Fixed(145.0));
        (header, items.into())
    } else {
        archive_results_view(state)
    };

    let body = column![
        title,
        source_row,
        search_row,
        controls,
        controls2,
        summary,
        queue_header,
        list,
        files_header,
        file_list,
    ]
    .spacing(6)
    .padding(4)
    .width(Length::Fixed(700.0));
    let card = container(body)
        .padding(18)
        .width(Length::Fixed(760.0))
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
    let tick = if dl_active(state) || state.name_rx.is_some() {
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
                PaneKind::Books => books_pane(state),
                PaneKind::Side => {
                    if quran_active(state) {
                        column![quran_panel(state), panel_tabs(state)]
                            .spacing(4)
                            .into()
                    } else {
                        side_pane(state)
                    }
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
    if args.len() >= 3 && args[1] == "--index-names" {
        match shamela_import::bok::name_index(std::path::Path::new(&args[2])) {
            Ok(map) => {
                println!("NAMES OK count={}", map.len());
                let mut ids: Vec<i64> = map.keys().copied().collect();
                ids.sort_unstable();
                for id in ids.iter().take(5) {
                    println!("{id}\t{}", map.get(id).map(String::as_str).unwrap_or(""));
                }
                return iced::Result::Ok(());
            }
            Err(error) => {
                eprintln!("NAMES FAIL: {error}");
                std::process::exit(1);
            }
        }
    }
    iced::application("ShamelaCraft", update, view)
        .window_size((1400.0, 900.0))
        .subscription(subscribe)
        .theme(theme)
        .font(include_bytes!("../assets/fonts/Amiri-Regular.ttf"))
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_leaf_handles_encoded_inner_paths() {
        assert_eq!(
            safe_leaf("https://archive.org/download/x/shamela.part01.rar/shamela%2F10006.mbok"),
            "10006.mbok"
        );
        assert_eq!(
            safe_leaf("https://archive.org/download/x/a%2Fb%2Fc.mdb"),
            "c.mdb"
        );
        assert_eq!(safe_leaf(".../%2E%2E"), "book.bok");
        assert_eq!(safe_leaf(""), "book.bok");
    }

    #[test]
    fn title_scores_ignore_spelling_differences() {
        assert_eq!(title_score("صحيح البخاري", "صحيح البخاري"), Some(0));
        // hamza and tashkeel must not hide a book
        assert_eq!(
            title_score("أحاديث إسماعيل بن جعفر", "احاديث اسماعيل بن جعفر"),
            Some(0)
        );
        // a different edition mark still names the same book
        assert_eq!(
            title_score("أخبار مكة - الفاكهي - ط 4", "أخبار مكة - الفاكهي - ط 2"),
            Some(2)
        );
        // the file may carry more words than the catalog title
        assert_eq!(
            title_score("صحيح البخاري", "صحيح البخاري في صحيح البخاري"),
            Some(1)
        );
        // the catalog title may be the longer one
        assert_eq!(
            title_score("صحيح البخاري", "صحيح البخاري في صحيح البخاري"),
            Some(1)
        );
        // a short word may not stand in for a whole title
        assert_eq!(title_score("صحيح البخاري", "البخاري"), None);
        // a different book must not match
        assert!(title_score("كتاب الفقه", "كتاب الحديث").is_none());
    }

    #[test]
    fn file_id_reads_numeric_stems() {
        assert_eq!(file_id("shamela/10006.mbok"), Some(10006));
        assert_eq!(file_id("Books/4/34.mdb"), Some(34));
        assert_eq!(file_id("book.mdb"), None);
        assert_eq!(file_id(""), None);
    }

    #[test]
    fn embedded_source_index_parses() {
        let index = source_index();
        assert!(index.len() > 20_000);
        assert!(index
            .iter()
            .all(|file| usize::from(file.source) < DL_PRESETS.len()));
        assert!(index.iter().all(|file| !file.name.is_empty()));
    }
}
