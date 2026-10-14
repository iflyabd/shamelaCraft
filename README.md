# ShamelaCraft — شاملة كرافت

Lightweight, open-source Arabic library reader in pure Rust.

> **Clean-room reimplementation.** ShamelaCraft shares no code, text, or assets
> with any proprietary reader. It only reads the documented shape of the
> Shamela-4 `master.db` SQLite catalog that already lives on the user's own
> machine. No affiliation with shamela.ws.

## Features (v0.1)

- Open a Shamela-4 `master.db` (read-only, never modified)
- Browse the 40+ category tree with catalog-ordered books
- Tolerant Arabic search (tashkeel / hamza / tatweel insensitive) over titles
  and authors, powered by [tantivy](https://github.com/quickwit-oss/tantivy)
- Tabbed catalog cards (title, authors, death dates, print status)
- Legacy `.bok` import (pure-Rust Jet/MDB reader) into local `Page`/`Title`
  databases, with full-text reader tabs, page navigation, and TOC jumping
- Keyless acquisition from public archive.org Shamela collections, with an
  in-app downloads panel (list `.bok` files, one-click download + import)
- Persistent bookmarks (`~/.shamelacraft/bookmarks.db`)
- RTL-first layout, bundled Amiri font (SIL OFL), light/dark themes,
  adjustable font size
- Trilingual interface (Arabic / English / French) with one-key switch
- Page translation (free MyMemory API, chunked + locally cached)
- Native builds for x86-64 **and ARM64** Linux — no emulation needed

## Roadmap to full parity (detail by detail)

- [x] `shamela-content`: per-book `.db` reader (`Page`/`Title` + patch semantics)
- [x] S1/S2 substitution-cipher decoder port (see Knowledge below)
- [x] HTML content pipeline port (titles, hadeeth tags, footnotes, ⦗⦘ markers, ligatures)
- [x] `shamela-import`: legacy `.bok` reader (pure-Rust `jetdb`, cp1256 recovery)
- [x] `shamela-import`: keyless archive.org item listing + `.bok` downloads
- [x] Reader tabs with page navigation + TOC jumping in the GUI
- [ ] S2 roots lookup wiring (morphological search expansion)
- [ ] EPUB importer (same `Page`/`Title` target)
- [ ] Content acquisition guide (official per-book downloads need an API key: mail@shamela.ws)
- [ ] Optional: Lucene stored-fields reader for offline desktop data
- [ ] Advanced search: exact phrase, AND/OR, footnotes scope, per-book scope
- [ ] Sharh↔Matn linked scrolling, side-by-side tabs
- [ ] Notes, highlights, reading history, export
- [ ] Windows / macOS / Android packaging
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
Rust.)

## Getting books

Three keyless paths, all offline-friendly after the first fetch:

1. **In-app downloads (easiest).** Open the تحميل panel, keep or change the
   archive.org item id (e.g. `shamela-16241`, `shamela-13700`, `golden-shamela_202601`),
   list the `.bok` files, and download any book — it converts to the local
   `Page`/`Title` format and opens in a reader tab automatically.
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
crates/shamelacraft    iced GUI binary + bookmarks store
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

### المزايا (الإصدار ٠٫١)

- فتح `master.db` للقراءة فقط دون أي تعديل
- تصفح شجرة الأقسام والكتب بالترتيب الأصلي
- بحث عربي متسامح (يتجاهل التشكيل والهمز والتطويل) في العناوين والمؤلفين
- تبويبات لبطاقات الكتب، مفضلة محفوظة، مظهران فاتح/داكن، تكبير الخط
- واجهة عربية من اليمين لليسار مع خط أميري مضمّن
- نسخ أصلية للينكس x86-64 وARM64 — بلا محاكاة
