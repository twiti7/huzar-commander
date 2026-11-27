use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use crate::panel::{unc_server_root, PanelState};

#[derive(Debug, Clone)]
pub struct PanelTab {
    pub panel: PanelState,
    pub is_locked: bool,
}

#[derive(Debug, Clone)]
pub struct PanelTabManager {
    pub tabs: Vec<PanelTab>,
    pub active_idx: usize,
}

impl PanelTabManager {
    pub fn new(initial: PanelState) -> Self {
        Self {
            tabs: vec![PanelTab {
                panel: initial,
                is_locked: false,
            }],
            active_idx: 0,
        }
    }

    pub fn from_tabs(mut tabs: Vec<PanelTab>, active_idx: usize) -> Self {
        if tabs.is_empty() {
            let fallback = if cfg!(target_os = "windows") {
                PathBuf::from("C:\\")
            } else {
                PathBuf::from("/")
            };
            tabs.push(PanelTab {
                panel: PanelState::new_deferred(fallback, true),
                is_locked: false,
            });
        }
        let clamped = active_idx.min(tabs.len() - 1);
        Self {
            tabs,
            active_idx: clamped,
        }
    }

    pub fn active(&self) -> &PanelState {
        &self.tabs[self.active_idx].panel
    }

    pub fn active_mut(&mut self) -> &mut PanelState {
        &mut self.tabs[self.active_idx].panel
    }

    pub fn active_tab(&self) -> &PanelTab {
        &self.tabs[self.active_idx]
    }

    pub fn active_tab_mut(&mut self) -> &mut PanelTab {
        &mut self.tabs[self.active_idx]
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn tab_title(tab: &PanelTab) -> String {
        if let Some(ref title) = tab.panel.custom_title {
            return format!("[{}]", title);
        }
        if let Some(ref loc) = tab.panel.zip_location {
            return loc
                .archive_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| loc.archive_path.to_string_lossy().to_string());
        }
        if let Some(srv) = unc_server_root(&tab.panel.current_path) {
            return format!(r"\\{}", srv);
        }
        tab.panel
            .current_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| tab.panel.current_path.to_string_lossy().to_string())
    }

    pub fn duplicate_active_tab(&mut self) -> usize {
        let cloned = self.active().clone_for_new_tab();
        let insert_idx = self.active_idx + 1;
        self.tabs.insert(
            insert_idx,
            PanelTab {
                panel: cloned,
                is_locked: false,
            },
        );
        self.active_idx = insert_idx;
        insert_idx
    }

    pub fn close_tab(&mut self, idx: usize) -> bool {
        if self.tabs.len() <= 1 || idx >= self.tabs.len() || self.tabs[idx].is_locked {
            return false;
        }
        self.tabs[idx].panel.cancel_loading();
        self.tabs.remove(idx);
        if idx < self.active_idx || self.active_idx >= self.tabs.len() {
            self.active_idx = self.active_idx.saturating_sub(1);
        }
        true
    }

    pub fn close_active_tab(&mut self) -> bool {
        self.close_tab(self.active_idx)
    }

    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_idx = idx;
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_idx = (self.active_idx + 1) % self.tabs.len();
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_idx = (self.active_idx + self.tabs.len() - 1) % self.tabs.len();
        }
    }

    pub fn toggle_lock(&mut self, idx: usize) -> Option<bool> {
        let tab = self.tabs.get_mut(idx)?;
        tab.is_locked = !tab.is_locked;
        Some(tab.is_locked)
    }

    pub fn set_show_hidden_all(&mut self, show_hidden: bool) {
        for tab in &mut self.tabs {
            tab.panel.show_hidden = show_hidden;
            tab.panel.apply_filter();
        }
    }

    pub fn toggle_hidden_all(&mut self) {
        let next = !self.active().show_hidden;
        self.set_show_hidden_all(next);
    }

    pub fn poll_all_loaders(&mut self) -> Vec<Result<PathBuf, String>> {
        let mut results = Vec::new();
        for tab in &mut self.tabs {
            if let Some(res) = tab.panel.poll_loading() {
                results.push(res);
            }
        }
        results
    }

    pub fn poll_all_dir_sizes(&mut self) -> Vec<(PathBuf, u64, usize)> {
        let mut finished = Vec::new();
        for tab in &mut self.tabs {
            finished.extend(tab.panel.poll_dir_sizes());
        }
        finished
    }

    pub fn any_loading(&self) -> bool {
        self.tabs.iter().any(|t| t.panel.is_loading())
    }

    pub fn any_calculating_dirs(&self) -> bool {
        self.tabs.iter().any(|t| t.panel.has_calculating_dirs())
    }
}

impl Deref for PanelTabManager {
    type Target = PanelState;

    fn deref(&self) -> &Self::Target {
        self.active()
    }
}

impl DerefMut for PanelTabManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.active_mut()
    }
}
