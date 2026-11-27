use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

fn default_show_hidden() -> bool {
    true
}

fn default_split_ratio() -> f32 {
    0.5
}

fn default_maximized() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ThemeKind {
    #[default]
    MidnightDark,
    ClassicNortonBlue,
    Light,
    HighContrast,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 4] = [
        ThemeKind::MidnightDark,
        ThemeKind::ClassicNortonBlue,
        ThemeKind::Light,
        ThemeKind::HighContrast,
    ];
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SizeFormat {
    #[default]
    Decimal,
    Binary,
    Bytes,
}

fn default_font_size() -> f32 {
    13.5
}

fn default_row_height() -> f32 {
    20.0
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub theme: ThemeKind,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_row_height")]
    pub row_height: f32,
    #[serde(default)]
    pub size_format: SizeFormat,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: ThemeKind::MidnightDark,
            font_size: default_font_size(),
            row_height: default_row_height(),
            size_format: SizeFormat::Decimal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BehaviorConfig {
    #[serde(default = "default_true")]
    pub confirm_delete: bool,
    #[serde(default = "default_true")]
    pub save_session_on_exit: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            confirm_delete: true,
            save_session_on_exit: true,
        }
    }
}

/// Formats a byte count according to the configured `SizeFormat` (Decimal SI, Binary IEC, or exact grouped bytes).
pub fn format_file_size(bytes: u64, fmt: SizeFormat) -> String {
    match fmt {
        SizeFormat::Decimal => humansize::format_size(bytes, humansize::DECIMAL),
        SizeFormat::Binary => humansize::format_size(bytes, humansize::BINARY),
        SizeFormat::Bytes => {
            let digits = bytes.to_string();
            let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 2);
            let len = digits.len();
            for (idx, ch) in digits.chars().enumerate() {
                if idx > 0 && (len - idx).is_multiple_of(3) {
                    out.push(' ');
                }
                out.push(ch);
            }
            out.push_str(" B");
            out
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TabSessionConfig {
    pub path: String,
    #[serde(default)]
    pub is_locked: bool,
    #[serde(default)]
    pub sort_column: Option<crate::fs_item::SortColumn>,
    #[serde(default)]
    pub sort_order: Option<crate::fs_item::SortOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PanelSessionConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sort_column: Option<crate::fs_item::SortColumn>,
    #[serde(default)]
    pub sort_order: Option<crate::fs_item::SortOrder>,
    #[serde(default)]
    pub tabs: Vec<TabSessionConfig>,
    #[serde(default)]
    pub active_tab: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    #[serde(default)]
    pub left_panel: PanelSessionConfig,
    #[serde(default)]
    pub right_panel: PanelSessionConfig,
    #[serde(default = "default_show_hidden")]
    pub show_hidden: bool,
    #[serde(default = "default_split_ratio")]
    pub split_ratio: f32,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default = "default_maximized")]
    pub window_maximized: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            left_panel: PanelSessionConfig::default(),
            right_panel: PanelSessionConfig::default(),
            show_hidden: default_show_hidden(),
            split_ratio: default_split_ratio(),
            window_width: None,
            window_height: None,
            window_maximized: default_maximized(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub language: Option<String>,
    pub default_editor: String,
    pub associations: HashMap<String, String>,
    #[serde(default)]
    pub session: SessionState,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut associations = HashMap::new();

        #[cfg(target_os = "windows")]
        {
            associations.insert("rs".to_string(), "code".to_string());
            associations.insert("toml".to_string(), "code".to_string());
            associations.insert("json".to_string(), "code".to_string());
            associations.insert("yaml".to_string(), "code".to_string());
            associations.insert("yml".to_string(), "code".to_string());
            associations.insert("md".to_string(), "code".to_string());
            associations.insert("txt".to_string(), "notepad.exe".to_string());
            associations.insert("log".to_string(), "notepad.exe".to_string());
        }

        #[cfg(not(target_os = "windows"))]
        {
            associations.insert("rs".to_string(), "code".to_string());
            associations.insert("toml".to_string(), "code".to_string());
            associations.insert("json".to_string(), "code".to_string());
            associations.insert("yaml".to_string(), "code".to_string());
            associations.insert("yml".to_string(), "code".to_string());
            associations.insert("md".to_string(), "code".to_string());
            associations.insert("txt".to_string(), "gedit".to_string());
            associations.insert("log".to_string(), "gedit".to_string());
        }

        let default_editor = if cfg!(target_os = "windows") {
            "notepad.exe".to_string()
        } else if let Ok(ed) = std::env::var("EDITOR") {
            ed
        } else {
            "gedit".to_string()
        };

        Self {
            language: None,
            default_editor,
            associations,
            session: SessionState::default(),
            bookmarks: Vec::new(),
            appearance: AppearanceConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

impl AppConfig {
    /// Resolves the configuration file path deterministically using portable mode (near exe)
    /// or standard per-user configuration directories (`%APPDATA%` on Windows, XDG/HOME on Unix).
    pub fn resolve_config_path_with(
        exe_dir: Option<&Path>,
        app_data_dir: Option<&Path>,
        xdg_config_home: Option<&Path>,
        home_dir: Option<&Path>,
    ) -> PathBuf {
        if let Some(dir) = exe_dir {
            if !dir.as_os_str().is_empty()
                && (dir.join("portable.flag").exists() || dir.join("config.json").exists())
            {
                return dir.join("config.json");
            }
        }

        #[cfg(target_os = "windows")]
        {
            let _ = (xdg_config_home, home_dir);
            if let Some(ad) = app_data_dir {
                if !ad.as_os_str().is_empty() {
                    return ad.join("HuzarCommander").join("config.json");
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = app_data_dir;
            if let Some(xdg) = xdg_config_home {
                if !xdg.as_os_str().is_empty() {
                    return xdg.join("huzar-commander").join("config.json");
                }
            }
            if let Some(home) = home_dir {
                if !home.as_os_str().is_empty() {
                    return home.join(".config").join("huzar-commander").join("config.json");
                }
            }
        }

        PathBuf::from("config.json")
    }

    pub fn config_path() -> PathBuf {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()));
        let app_data = std::env::var_os("APPDATA").map(PathBuf::from);
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let home = std::env::var_os("HOME").map(PathBuf::from);

        Self::resolve_config_path_with(
            exe_dir.as_deref(),
            app_data.as_deref(),
            xdg.as_deref(),
            home.as_deref(),
        )
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }

        let legacy_path = PathBuf::from("config.json");
        if legacy_path != path && legacy_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&legacy_path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    let _ = cfg.save_to(&path);
                    return cfg;
                }
            }
        }

        let default_cfg = Self::default();
        let _ = default_cfg.save_to(&path);
        default_cfg
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Błąd tworzenia katalogu konfiguracji '{:?}': {}", parent, e))?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Błąd serializacji konfiguracji: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Błąd zapisu pliku '{:?}': {}", path, e))?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::config_path())
    }

    pub fn get_editor_for_ext(&self, ext: &str) -> String {
        let clean_ext = ext.trim_start_matches('.').trim().to_lowercase();
        if let Some(ed) = self.associations.get(&clean_ext) {
            let ed_trimmed = ed.trim();
            if !ed_trimmed.is_empty() {
                return ed_trimmed.to_string();
            }
        }
        self.default_editor.clone()
    }

    pub fn set_association(&mut self, ext: &str, editor: &str) {
        let clean_ext = ext.trim_start_matches('.').trim().to_lowercase();
        let clean_ed = editor.trim().to_string();
        if !clean_ext.is_empty() && !clean_ed.is_empty() {
            self.associations.insert(clean_ext, clean_ed);
        }
    }

    pub fn remove_association(&mut self, ext: &str) {
        let clean_ext = ext.trim_start_matches('.').trim().to_lowercase();
        self.associations.remove(&clean_ext);
    }
}
