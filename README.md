# ShamelaCraft — شاملة كرافت

Lightweight, open-source Arabic library reader in pure Rust.

> **Clean-room reimplementation.** ShamelaCraft shares no code, text, or assets
> with any proprietary reader. It only reads the documented shape of the
> Shamela-4 `master.db` SQLite catalog that already lives on the user's own
> machine. No affiliation with shamela.ws.

## Features (v0.2)

- Open a Shamela-4 `master.db` (read-only, never modified)
- Browse the 40+ category tree with catalog-ordered books
- Tolerant Arabic search (tashkeel / hamza / tatweel insensitive) over titles
  and authors, powered by [tantivy](https://github.com/quickwit-oss/tantivy)
- Tabbed catalog cards (title, authors, death dates, print status)
- Legacy book import into local `Page`/`Title` databases: `.bok`,
  Android `.mdb` (`book` + `title` tables) and `.mbok` (SQLite), with
  full-text reader tabs, page navigation, and TOC jumping
- **Download manager**: a categorized keyless source catalog for archive.org
  (Shamela PC/Android/MBOK libraries, 26 curated entries and 20,700+ catalogued
  books), a pasted item URL, **cross-source title search** (search any book and
  find it in every curated source), **deep pack browsing** (list and fetch one
  book from a ZIP/RAR without downloading the whole collection), per-file /
  multi-select / whole-item downloads, per-book download by title, open-book
  re-download, **real names for numbered `.mdb`/`.mbok` files** (from the open
  master database and from each pack's own `main.mdb` index), name filter,
  size/kind badges, live progress, queue summary, cancel, retry and remove.
  ZIP / TAR / TAR.GZ / RAR / **7z** are extracted locally. Split RAR sets can
  be browsed, but archive.org only extracts inner books from the first volume;
  later volumes return 503 for individual files.
- **One unified reading pane** for imported books and the Uthmani Quran
- **Quran tab** with the sura index replacing the library tree, plus nine
  keyless Arabic tafsirs cached locally
- **Settings popup** with every option persisted: font size, line spacing,
  text/background colors, accent color, rounded corners, light/dark theme,
  diacritics, search normalization, startup behavior, **layout** (pane
  switches, side pane shape, reset)
- **Customizable side panes**: the books pane and the side pane are
  resizable by dragging, each has a close button, and the **عرض / View**
  menu lists them with their state so a closed pane can be brought back.
  The side pane can be shown as tabs, as one expandable tree (sections with
  the books inside them) or as two resizable trees (catalog + the index of
  the open book)
- Persistent bookmarks (`~/.shamelacraft/bookmarks.db`) and page notes
- Toolbar tooltips on every button, bilingual labels, keyboard shortcuts
- RTL-first layout, vertical scrollbars on the **left**, bundled Amiri font,
  resizable reader page/index split, book-list and side panels (every split
  is draggable, every pane can be closed and brought back)
- Trilingual interface (Arabic / English / French) with one-key switch
- Page translation (free MyMemory API, chunked + locally cached)
- Native builds for x86-64 **and ARM64** Linux — no emulation needed

## Roadmap to full parity (detail by detail)

- [x] `shamela-content`: per-book `.db` reader (`Page`/`Title` + patch semantics)
- [x] S1/S2 substitution-cipher decoder port (see Knowledge below)
- [x] HTML content pipeline port (titles, hadeeth tags, footnotes, ⦗⦘ markers, ligatures)
- [x] `shamela-import`: `.bok` / `.mdb` / `.mbok` readers (pure Rust)
- [x] `shamela-import`: keyless archive.org item listing, deep pack browsing,
  and `.bok` / `.mdb` / `.mbok` downloads
- [x] 7z extraction for curated `.bok` collections
- [x] Reader tabs with page navigation + TOC jumping in the GUI
- [ ] S2 roots lookup wiring (morphological search expansion)
- [ ] EPUB importer (same `Page`/`Title` target)
- [ ] Content acquisition guide (official per-book downloads need an API key: mail@shamela.ws)
- [ ] Optional: Lucene stored-fields reader for offline desktop data
- [ ] Advanced search: exact phrase, AND/OR, footnotes scope, per-book scope
- [x] Notes, bookmarks, markdown export
- [x] Quran in the unified reading pane with keyless tafsirs
- [ ] Sharh↔Matn linked scrolling, side-by-side tabs
- [ ] Android packaging (Windows/macOS release builds are wired in CI)
- [ ] Signed releases with checksums

## Build

Prerequisites: stable Rust (1.85+), plus on Debian/Ubuntu:

```sh
sudo apt install pkg-config libgtk-3-dev
```

```sh
cargo build --release -p shamelacraft
./target/release/shamelacraft
```

Run the test-suite (uses an in-memory fixture, no proprietary data needed):

```sh
cargo test --workspace
```

## Installation

Download a ready build from
[Releases](https://github.com/iflyabd/shamelaCraft/releases) (published from
tags like `v0.1.0` — Linux x86-64/ARM64, Windows x86-64, macOS ARM64), or build
from source with stable Rust (1.85+). Debian/Ubuntu needs one system package
for the file picker:

```sh
sudo apt install pkg-config libgtk-3-dev
cargo build --release -p shamelacraft
./target/release/shamelacraft
```

Pure Rust throughout: no C database drivers (SQLite via `rusqlite/bundled`,
Access/Jet via `jetdb`), TLS via rustls. The same code builds on every OS
and CPU architecture without emulation. (Dependency honesty: the only
compiled C in the tree is the vendored SQLite amalgamation and tantivy's
zstd — everything authored for this project, and every format reader, is
Rust. `iced_widget` 0.13.4 is vendored under `vendor/iced_widget` with a
three-line MIT-licensed patch that puts vertical scrollbars on the left for
the RTL interface; it is pure Rust.)

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| F5 / F6 | next / previous page (books and Quran suras) |
| F3 | library and search panel |
| F1 | online help |
| F10 | settings popup |
| Esc | close popup, else close the active tab |

## Getting books

Three keyless paths, all offline-friendly after the first fetch:

1. **In-app downloads (easiest).** Press the globe (التحميل) button. The
   manager takes an archive.org item id or a pasted URL, offers ready sources
   (e.g. `mutunulhadith` — 172 single-book `.bok` files), and lists every
   importable file with its size and kind. Type a title and press **كل المصادر**
   to search the whole embedded catalog (20,700+ books) at once, or **بحث
   archive.org** for items. Search the current list by name (فلترة), tick
   several books (تحديد الكل / تحديد) and download just those — real per-book
   downloads from `.bok` collections; or download the whole pack. Numbered
   `.mbok` files are named from the open master database; packs that ship their
   own catalog (مكتبة ابن وهف / المكتبة السلفية الشاملة) get a **جلب الأسماء**
   button that downloads only their small `main.mdb` index and shows every
   book's real name before any book is downloaded. The green download button
   on any book tab (and تحميل الكتاب in the reader header) opens the download
   manager with that book's title already in the search box and searches every
   curated source at once, so you choose the edition before anything is
   downloaded. Everything converts to the local `Page`/`Title` format and
   opens in a reader tab automatically.
2. **Local `.bok` files.** Use استيراد BOK to convert any legacy `.bok`
   (Jet3/MDB, Windows-1256) with the pure-Rust importer.
3. **Official per-book databases** (needs a free API key from mail@shamela.ws)
   open directly: they already use the `Page`/`Title` schema this reader
   implements.

All imports land in `~/.shamelacraft/books/*.db` and are indexed for
tolerant Arabic search.

## Platform support

| OS | x86-64 | ARM64 |
|----|--------|-------|
| Linux | ✅ release + CI | ✅ release + CI (native, no box64) |
| Windows | ✅ release | ✅ builds from source |
| macOS | ✅ builds from source | ✅ release |

## Layout

```text
crates/shamela-db      read-only master.db access (rusqlite)
crates/shamela-index   Arabic normalization + tantivy index
crates/shamela-content book .db reader, S1/S2 decoder, HTML pipeline, roots lookup
crates/shamela-import  BOK/MDB/MBOK + archive import, keyless downloads, Quran + tafsir fetchers
crates/shamelacraft    iced GUI binary, settings, bookmarks and notes stores
vendor/iced_widget     iced 0.13.4 with the left-scrollbar patch (MIT)
```

## Knowledge & attribution

The content layer ports documented, MIT-licensed research — no proprietary
code is copied, and no book texts are bundled:

- [ragaeeb/shamela](https://github.com/ragaeeb/shamela) — S1/S2 decoder tables
  and algorithm, `Page`/`Title` schema and patch protocol, HTML pipeline
  semantics (title spans, hadeeth tags, footnote/page markers, ligatures)
- [shaybix/mdb2json](https://github.com/shaybix/mdb2json) — legacy `.bok`
  (MS Access) extraction reference for the future BOK importer
- [yshalsager/shamela-epub-exporter](https://github.com/yshalsager/shamela-epub-exporter) —
  EPUB export approach reference

## Fonts

Amiri Regular/Bold are bundled from
[google/fonts](https://github.com/google/fonts/tree/main/ofl/amiri) under the
SIL Open Font License 1.1 (`crates/shamelacraft/assets/fonts/OFL.txt`).

## Credits

- Idea: **عبد القادر الجزائري (Abdelkader El Djazairi)** — known as **صقر قريش (Arab Falcon)**

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).

---

## شاملة كرافت

قارئ مكتبة عربي خفيف ومفتوح المصدر مكتوب بلغة Rust.

> **إعادة تنفيذ نظيفة.** لا يشارك المشروع أي شيفرة أو نصوص أو أصول مع أي
> قارئ مملوك، ويقرأ فقط بنية ملف `master.db` الموجود على جهاز المستخدم.
> لا علاقة له بموقع shamela.ws.

### الشكر والتقدير

- الفكرة: **عبد القادر الجزائري** — المعروف بـ **صقر قريش (Arab Falcon)**

### المزايا (الإصدار ٠٫٢)

- فتح `master.db` للقراءة فقط دون أي تعديل
- تصفح شجرة الأقسام والكتب بالترتيب الأصلي
- بحث عربي متسامح (يتجاهل التشكيل والهمز والتطويل) في العناوين والمؤلفين
- مدير تحميلات: مصادر archive.org جاهزة أو رابط ملصق، تحميل ملف أو عدة
  ملفات محددة أو الحزمة كاملة، تحميل الكتاب المفتوح من رأس القارئ، فلترة
  الأسماء وأحجام الملفات، تقدم حي، ملخص قائمة الانتظار، فتح مجلد التنزيلات،
  مسح المكتمل، إلغاء وإعادة وإزالة
- نافذة قراءة موحدة للكتب والقرآن الكريم، وفهرس السور محل شجرة المكتبة
- تبويب للقرآن مع تسعة تفاسير عربية تُحفظ محليا
- إعدادات محفوظة: حجم الخط، تباعد الأسطر، ألوان النص والخلفية، لون
  الواجهة، استدارة الأزرار، المظهر، التشكيل، خيارات البحث
- تلميحات على أزرار الشريط، اختصارات لوحة مفاتيح، واجهة ثلاثية اللغات
- أشرطة تمرير على اليسار، ولوحات قابلة للتحجيم
- خط أميري مضمّن، نسخ أصلية للينكس x86-64 وARM64 — بلا محاكاة
