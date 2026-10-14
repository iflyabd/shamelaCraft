//! Minimal trilingual UI tables (Arabic / English / French).
//!
//! Only interface chrome goes through [`tr`]; book content is never
//! translated by the UI language (see the page-translation provider for
//! that). English and French exist so the interface stays fully usable
//! anywhere; Arabic is the default.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Ar,
    En,
    Fr,
}

impl Lang {
    /// Short Latin code shown on the language button (always renders).
    pub fn code(self) -> &'static str {
        match self {
            Self::Ar => "AR",
            Self::En => "EN",
            Self::Fr => "FR",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Ar => Self::En,
            Self::En => Self::Fr,
            Self::Fr => Self::Ar,
        }
    }
}

/// Interface string by key. Every arm is a plain literal.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    match (lang, key) {
        (_, "app_name") => "ShamelaCraft",
        (Lang::Ar, "tagline") => "قارئ مكتبة عربي خفيف",
        (Lang::En, "tagline") => "lightweight Arabic library reader",
        (Lang::Fr, "tagline") => "lecteur arabe léger de bibliothèque",
        (Lang::Ar, "welcome") => {
            "افتح master.db من نسختك، أو استورد كتاب BOK، أو حمّل من archive.org."
        }
        (Lang::En, "welcome") => {
            "Open a master.db, import a BOK book, or download from archive.org."
        }
        (Lang::Fr, "welcome") => {
            "Ouvrez un master.db, importez un livre BOK ou téléchargez depuis archive.org."
        }
        (Lang::Ar, "open_db") => "فتح قاعدة البيانات",
        (Lang::En, "open_db") => "Open database",
        (Lang::Fr, "open_db") => "Ouvrir la base",
        (Lang::Ar, "import_bok") => "استيراد BOK",
        (Lang::En, "import_bok") => "Import BOK",
        (Lang::Fr, "import_bok") => "Importer BOK",
        (Lang::Ar, "download_from_net") => "تحميل من archive.org",
        (Lang::En, "download_from_net") => "Download from archive.org",
        (Lang::Fr, "download_from_net") => "Télécharger depuis archive.org",
        (Lang::Ar, "library") => "المكتبة",
        (Lang::En, "library") => "Library",
        (Lang::Fr, "library") => "Bibliothèque",
        (Lang::Ar, "authors") => "المؤلفون",
        (Lang::En, "authors") => "Authors",
        (Lang::Fr, "authors") => "Auteurs",
        (Lang::Ar, "bookmarks") => "المفضلة",
        (Lang::En, "bookmarks") => "Bookmarks",
        (Lang::Fr, "bookmarks") => "Favoris",
        (Lang::Ar, "downloads") => "التحميل",
        (Lang::En, "downloads") => "Downloads",
        (Lang::Fr, "downloads") => "Téléchargements",
        (Lang::Ar, "sections") => "الأقسام",
        (Lang::En, "sections") => "Categories",
        (Lang::Fr, "sections") => "Catégories",
        (Lang::Ar, "books") => "الكتب",
        (Lang::En, "books") => "Books",
        (Lang::Fr, "books") => "Livres",
        (Lang::Ar, "no_results") => "لا نتائج",
        (Lang::En, "no_results") => "No results",
        (Lang::Fr, "no_results") => "Aucun résultat",
        (Lang::Ar, "pick_section") => "اختر قسما أو مؤلفا",
        (Lang::En, "pick_section") => "Pick a category or author",
        (Lang::Fr, "pick_section") => "Choisissez une catégorie ou un auteur",
        (Lang::Ar, "type_author") => "ابحث باسم مؤلف",
        (Lang::En, "type_author") => "Search an author name",
        (Lang::Fr, "type_author") => "Rechercher un auteur",
        (Lang::Ar, "no_bookmarks") => "لا عناصر بعد",
        (Lang::En, "no_bookmarks") => "Empty for now",
        (Lang::Fr, "no_bookmarks") => "Rien pour l'instant",
        (Lang::Ar, "search_hint") => "بحث ...",
        (Lang::En, "search_hint") => "search ...",
        (Lang::Fr, "search_hint") => "rechercher ...",
        (Lang::Ar, "fetch_books") => "جلب الكتب",
        (Lang::En, "fetch_books") => "List books",
        (Lang::Fr, "fetch_books") => "Lister les livres",
        (Lang::Ar, "choose_book") => "اختر كتابا من القائمة",
        (Lang::En, "choose_book") => "Pick a book from the list",
        (Lang::Fr, "choose_book") => "Choisissez un livre dans la liste",
        (Lang::Ar, "delete") => "حذف",
        (Lang::En, "delete") => "Delete",
        (Lang::Fr, "delete") => "Supprimer",
        (Lang::Ar, "quran") => "القرآن الكريم",
        (Lang::En, "quran") => "Quran",
        (Lang::Fr, "quran") => "Coran",
        (Lang::Ar, "download_quran") => "تحميل المصحف",
        (Lang::En, "download_quran") => "Download mushaf",
        (Lang::Fr, "download_quran") => "Télécharger le Coran",
        (Lang::Ar, "quran_search") => "بحث في الآيات ...",
        (Lang::En, "quran_search") => "search verses ...",
        (Lang::Fr, "quran_search") => "rechercher un verset ...",
        (Lang::Ar, "notes") => "تعليقاتي",
        (Lang::En, "notes") => "My notes",
        (Lang::Fr, "notes") => "Mes notes",
        (Lang::Ar, "note_hint") => "اكتب تعليقا ...",
        (Lang::En, "note_hint") => "write a note ...",
        (Lang::Fr, "note_hint") => "écrire une note ...",
        (Lang::Ar, "control_room") => "غرفة التحكم",
        (Lang::En, "control_room") => "Control room",
        (Lang::Fr, "control_room") => "Salle de contrôle",
        (Lang::Ar, "settings") => "الإعدادات",
        (Lang::En, "settings") => "Settings",
        (Lang::Fr, "settings") => "Paramètres",
        (Lang::Ar, "display") => "العرض",
        (Lang::En, "display") => "Display",
        (Lang::Fr, "display") => "Affichage",
        (Lang::Ar, "search_options") => "خيارات البحث",
        (Lang::En, "search_options") => "Search options",
        (Lang::Fr, "search_options") => "Options de recherche",
        (Lang::Ar, "data_section") => "البيانات",
        (Lang::En, "data_section") => "Data",
        (Lang::Fr, "data_section") => "Données",
        (Lang::Ar, "show_tashkeel") => "عرض التشكيل",
        (Lang::En, "show_tashkeel") => "Show diacritics",
        (Lang::Fr, "show_tashkeel") => "Afficher les signes",
        (Lang::Ar, "unify_hamza") => "توحيد الهمزة",
        (Lang::En, "unify_hamza") => "Unify hamza",
        (Lang::Fr, "unify_hamza") => "Unifier le hamza",
        (Lang::Ar, "ignore_tashkeel") => "تجاهل التشكيل في البحث",
        (Lang::En, "ignore_tashkeel") => "Ignore diacritics in search",
        (Lang::Fr, "ignore_tashkeel") => "Ignorer les signes (recherche)",
        (Lang::Ar, "strip_tatweel") => "إزالة التطويل",
        (Lang::En, "strip_tatweel") => "Strip tatweel",
        (Lang::Fr, "strip_tatweel") => "Supprimer le tatwil",
        (Lang::Ar, "clear_cache") => "مسح ذاكرة الترجمة",
        (Lang::En, "clear_cache") => "Clear translation cache",
        (Lang::Fr, "clear_cache") => "Vider le cache",
        (Lang::Ar, "reset_all") => "إعادة الضبط",
        (Lang::En, "reset_all") => "Reset all",
        (Lang::Fr, "reset_all") => "Tout réinitialiser",
        (Lang::Ar, "control_room") => "غرفة التحكم",
        (Lang::En, "control_room") => "Control room",
        (Lang::Fr, "control_room") => "Salle de contrôle",
        _ => "?",
    }
}
