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
- Persistent bookmarks (`~/.shamelacraft/bookmarks.db`)
- RTL-first layout, bundled Amiri font (SIL OFL), light/dark themes,
  adjustable font size
- Native builds for x86-64 **and ARM64** Linux — no emulation needed

## Roadmap to full parity (detail by detail)

- [ ] `ContentProvider` trait + Lucene stored-fields reader (full book text)
- [ ] BOK importer (legacy libraries)
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

## Layout

```text
crates/shamela-db      read-only master.db access (rusqlite)
crates/shamela-index   Arabic normalization + tantivy index
crates/shamelacraft    iced GUI binary + bookmarks store
```

## Fonts

Amiri Regular/Bold are bundled from
[google/fonts](https://github.com/google/fonts/tree/main/ofl/amiri) under the
SIL Open Font License 1.1 (`crates/shamelacraft/assets/fonts/OFL.txt`).

## Credits

- Idea: **صقر قريش (Arab Falcon)**

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).

---

## شاملة كرافت

قارئ مكتبة عربي خفيف ومفتوح المصدر مكتوب بلغة Rust.

> **إعادة تنفيذ نظيفة.** لا يشارك المشروع أي شيفرة أو نصوص أو أصول مع أي
> قارئ مملوك، ويقرأ فقط بنية ملف `master.db` الموجود على جهاز المستخدم.
> لا علاقة له بموقع shamela.ws.

### الشكر والتقدير

- الفكرة: **صقر قريش (Arab Falcon)**

### المزايا (الإصدار ٠٫١)

- فتح `master.db` للقراءة فقط دون أي تعديل
- تصفح شجرة الأقسام والكتب بالترتيب الأصلي
- بحث عربي متسامح (يتجاهل التشكيل والهمز والتطويل) في العناوين والمؤلفين
- تبويبات لبطاقات الكتب، مفضلة محفوظة، مظهران فاتح/داكن، تكبير الخط
- واجهة عربية من اليمين لليسار مع خط أميري مضمّن
- نسخ أصلية للينكس x86-64 وARM64 — بلا محاكاة
