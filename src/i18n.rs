use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    En,
    Pl,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::Pl => "pl",
            Language::En => "en",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Pl => "Polski",
            Language::En => "English",
        }
    }

    /// Rozpoznaje język na podstawie łańcucha locale (np. "pl_PL", "pl-PL", "en_US", "en").
    pub fn from_locale_str(locale: &str) -> Option<Self> {
        let clean = locale.trim().to_lowercase();
        if clean.starts_with("pl") || clean.starts_with("pol") {
            Some(Language::Pl)
        } else if clean.starts_with("en") || clean.starts_with("eng") {
            Some(Language::En)
        } else {
            None
        }
    }

    /// Wykrywa język systemowy użytkownika.
    /// Jeśli język systemu nie jest obsługiwany (lub brak wsparcia), domyślnie wybiera angielski.
    pub fn detect_system_language() -> Self {
        #[cfg(target_os = "windows")]
        {
            if let Some(lang) = detect_windows_language() {
                return lang;
            }
        }

        if let Some(lang) = detect_env_language() {
            return lang;
        }

        // Domyślny fallback: angielski
        Language::En
    }
}

#[cfg(target_os = "windows")]
extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
    fn GetUserDefaultLocaleName(lpLocaleName: *mut u16, cchLocaleName: i32) -> i32;
}

#[cfg(target_os = "windows")]
fn detect_windows_language() -> Option<Language> {
    // 1. Sprawdź język interfejsu Windows (UI language)
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary = lang_id & 0x03FF;
    match primary {
        0x15 => return Some(Language::Pl), // LANG_POLISH
        0x09 => return Some(Language::En), // LANG_ENGLISH
        _ => {}
    }

    // 2. Sprawdź nazwę locale (np. "pl-PL", "en-US")
    let mut buf = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if len > 1 {
        let slice = &buf[..(len as usize).saturating_sub(1)];
        if let Ok(loc) = String::from_utf16(slice) {
            if let Some(l) = Language::from_locale_str(&loc) {
                return Some(l);
            }
        }
    }

    None
}

fn detect_env_language() -> Option<Language> {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if let Some(l) = Language::from_locale_str(&val) {
                return Some(l);
            }
        }
    }
    None
}

pub struct I18n {
    current_lang: Language,
    translations: HashMap<Language, HashMap<String, String>>,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new_system()
    }
}

impl I18n {
    /// Inicjalizuje i18n z automatycznie wykrytym językiem systemowym (fallback do EN).
    pub fn new_system() -> Self {
        Self::new(Language::detect_system_language())
    }

    pub fn new(default_lang: Language) -> Self {
        let mut translations = HashMap::new();

        // 1. Embedded fallback translations (always available, zero crash)
        let pl_str = include_str!("../locales/pl.json");
        let en_str = include_str!("../locales/en.json");

        let mut pl_map = HashMap::new();
        let mut en_map = HashMap::new();

        if let Ok(v) = serde_json::from_str::<serde_json::Value>(pl_str) {
            flatten_json("", &v, &mut pl_map);
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(en_str) {
            flatten_json("", &v, &mut en_map);
        }

        // 2. Try loading external overrides from disk if present (e.g. ./locales/pl.json)
        Self::try_load_external_override("pl", &mut pl_map);
        Self::try_load_external_override("en", &mut en_map);

        translations.insert(Language::Pl, pl_map);
        translations.insert(Language::En, en_map);

        Self {
            current_lang: default_lang,
            translations,
        }
    }

    fn try_load_external_override(code: &str, map: &mut HashMap<String, String>) {
        let candidates = [
            PathBuf::from(format!("locales/{}.json", code)),
            if let Ok(exe) = std::env::current_exe() {
                exe.parent().unwrap_or(Path::new("")).join(format!("locales/{}.json", code))
            } else {
                PathBuf::new()
            },
        ];

        for path in candidates {
            if path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                        flatten_json("", &v, map);
                        break;
                    }
                }
            }
        }
    }

    pub fn language(&self) -> Language {
        self.current_lang
    }

    pub fn set_language(&mut self, lang: Language) {
        self.current_lang = lang;
    }

    /// Retrieve a translated string by key. Falls back to English or key itself if missing.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(map) = self.translations.get(&self.current_lang) {
            if let Some(val) = map.get(key) {
                return val.as_str();
            }
        }

        // Fallback to English
        if self.current_lang != Language::En {
            if let Some(map) = self.translations.get(&Language::En) {
                if let Some(val) = map.get(key) {
                    return val.as_str();
                }
            }
        }

        // Fallback to Polish
        if self.current_lang != Language::Pl {
            if let Some(map) = self.translations.get(&Language::Pl) {
                if let Some(val) = map.get(key) {
                    return val.as_str();
                }
            }
        }

        // Ultimate fallback: the key itself
        key
    }

    /// Retrieve a translated string and replace placeholder parameters e.g. "{count}" with values.
    pub fn t_fmt(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.t(key);
        let mut result = template.to_string();
        for (k, v) in args {
            let placeholder = format!("{{{}}}", k);
            result = result.replace(&placeholder, v);
        }
        result
    }

    pub fn get_all_keys(&self, lang: Language) -> Vec<String> {
        if let Some(map) = self.translations.get(&lang) {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            keys
        } else {
            Vec::new()
        }
    }
}

fn flatten_json(prefix: &str, value: &serde_json::Value, map: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let new_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(&new_key, v, map);
            }
        }
        serde_json::Value::String(s) => {
            map.insert(prefix.to_string(), s.clone());
        }
        serde_json::Value::Number(n) => {
            map.insert(prefix.to_string(), n.to_string());
        }
        serde_json::Value::Bool(b) => {
            map.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }
}
