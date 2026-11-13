use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub language: Option<String>,
    pub default_editor: String,
    pub associations: HashMap<String, String>,
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
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let candidate = parent.join("config.json");
                if candidate.exists() {
                    return candidate;
                }
            }
        }
        PathBuf::from("config.json")
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
        let default_cfg = Self::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Błąd serializacji konfiguracji: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Błąd zapisu pliku '{:?}': {}", path, e))?;
        Ok(())
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
