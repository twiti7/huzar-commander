use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use crate::app::PanelSide;
use crate::opener::ViewerMode;
use crate::search::{SearchHandle, SearchResult};

#[derive(Default)]
pub struct SearchDialogState {
    pub is_open: bool,
    pub mask: String,
    pub dir: String,
    pub text: String,
    pub case_sensitive: bool,
    pub recursive: bool,
    pub results: Vec<SearchResult>,
    pub selected_idx: Option<usize>,
    pub handle: Option<SearchHandle>,
    pub status_text: String,
    pub scanned_count: usize,
}

impl SearchDialogState {
    pub fn stop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub struct ViewerWindowState {
    pub is_open: bool,
    pub title: String,
    pub content: String,
    pub path: Option<PathBuf>,
    pub total_size: u64,
    pub offset: u64,
    pub mode: ViewerMode,
    pub chunk: Vec<u8>,
    pub search_query: String,
    pub is_search_open: bool,
    pub search_status: Option<String>,
    pub structured_subview: crate::ui::structured_viewer::StructuredSubView,
    pub scroll_ratio: f32,
    pub scroll_to_top: bool,
    pub scroll_to_bottom: bool,
}

impl Default for ViewerWindowState {
    fn default() -> Self {
        Self {
            is_open: false,
            title: String::new(),
            content: String::new(),
            path: None,
            total_size: 0,
            offset: 0,
            mode: ViewerMode::Text,
            chunk: Vec::new(),
            search_query: String::new(),
            is_search_open: false,
            search_status: None,
            structured_subview: crate::ui::structured_viewer::StructuredSubView::Code,
            scroll_ratio: 0.0,
            scroll_to_top: false,
            scroll_to_bottom: false,
        }
    }
}

impl ViewerWindowState {
    pub fn chunk_size(&self) -> usize {
        if self.mode == ViewerMode::Markdown || self.mode == ViewerMode::Json || self.mode == ViewerMode::Yaml {
            1024 * 1024
        } else {
            64 * 1024
        }
    }

    /// Oblicza aktualny procent przeglądania całego pliku (uwzględniając offset chunka oraz przewinięcie w oknie).
    pub fn calculate_view_percentage(&self) -> f64 {
        if self.total_size == 0 {
            return 100.0;
        }

        let chunk_step = self.chunk_size() as u64;
        let chunk_len = self.chunk.len() as u64;

        if self.total_size <= chunk_step {
            // Cały plik mieści się w jednym widoku
            (self.scroll_ratio as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            // Plik wielostronicowy (paginowany / streaming z dysku)
            let chunk_start = self.offset as f64;
            let chunk_span = chunk_len as f64;
            let current_pos = chunk_start + chunk_span * self.scroll_ratio as f64;
            ((current_pos / self.total_size as f64) * 100.0).clamp(0.0, 100.0)
        }
    }

    /// Oblicza bieżący offset bajtowy widoczny na samej górze ekranu
    pub fn current_visible_offset(&self) -> u64 {
        if self.total_size == 0 {
            return 0;
        }
        let chunk_len = self.chunk.len() as u64;
        if chunk_len == 0 {
            return self.offset;
        }

        let delta = (chunk_len as f64 * self.scroll_ratio as f64).round() as u64;
        let visible = self.offset + delta;
        visible.min(self.total_size)
    }

    pub fn open_file(&mut self, title: String, path: PathBuf) -> Result<(), String> {
        self.title = title;
        self.path = Some(path.clone());
        self.offset = 0;
        self.scroll_ratio = 0.0;

        let is_md = crate::opener::is_markdown_file(&path);
        let is_json = crate::opener::is_json_file(&path);
        let is_yaml = crate::opener::is_yaml_file(&path);
        let initial_chunk_size = if is_md || is_json || is_yaml { 1024 * 1024 } else { 64 * 1024 };

        let (buf, total_size) = crate::opener::read_file_chunk(&path, 0, initial_chunk_size)?;
        self.total_size = total_size;
        let is_binary = crate::opener::detect_is_binary(&buf);
        self.mode = if is_binary {
            ViewerMode::Hex
        } else if is_md {
            ViewerMode::Markdown
        } else if is_json {
            ViewerMode::Json
        } else if is_yaml {
            ViewerMode::Yaml
        } else {
            ViewerMode::Text
        };
        self.structured_subview = crate::ui::structured_viewer::StructuredSubView::Code;
        self.chunk = buf;
        self.refresh_content();
        self.is_open = true;
        Ok(())
    }

    pub fn reload_chunk(&mut self) {
        if let Some(path) = self.path.clone() {
            let chunk_sz = self.chunk_size();
            let max_offset = self.total_size.saturating_sub(chunk_sz as u64);
            if self.offset > max_offset && self.total_size > chunk_sz as u64 {
                self.offset = max_offset;
            }
            match crate::opener::read_file_chunk(&path, self.offset, chunk_sz) {
                Ok((buf, total_size)) => {
                    self.total_size = total_size;
                    self.chunk = buf;
                    self.refresh_content();
                }
                Err(e) => {
                    self.content = format!("Błąd odczytu pliku: {}", e);
                }
            }
        }
    }

    pub fn refresh_content(&mut self) {
        self.content = match self.mode {
            ViewerMode::Hex => {
                crate::opener::format_hex_chunk(&self.chunk, self.offset, self.total_size)
            }
            ViewerMode::Text => {
                crate::opener::format_text_chunk(&self.chunk, self.offset, self.total_size)
            }
            ViewerMode::Markdown | ViewerMode::Json | ViewerMode::Yaml => {
                String::from_utf8_lossy(&self.chunk).to_string()
            }
        };
    }

    pub fn seek_next(&mut self) {
        let chunk_sz = self.chunk_size() as u64;
        let max_offset = self.total_size.saturating_sub(chunk_sz);
        if self.offset < max_offset {
            self.offset = (self.offset + chunk_sz).min(max_offset);
            self.reload_chunk();
        }
    }

    pub fn seek_prev(&mut self) {
        let chunk_sz = self.chunk_size() as u64;
        if self.offset > 0 {
            self.offset = self.offset.saturating_sub(chunk_sz);
            self.reload_chunk();
        }
    }

    pub fn find_next(&mut self, forward: bool) {
        let needle = self.search_query.trim().to_lowercase();
        if needle.is_empty() {
            return;
        }
        let needle_bytes = needle.as_bytes();
        let path = match self.path {
            Some(ref p) => p.clone(),
            None => return,
        };

        let start_pos = if forward {
            self.offset + 1
        } else {
            self.offset.saturating_sub(64 * 1024)
        };

        use std::fs::File;
        use std::io::{Read, Seek, SeekFrom};

        let Ok(mut file) = File::open(&path) else {
            return;
        };
        let Ok(file_len) = file.metadata().map(|m| m.len()) else {
            return;
        };

        let mut pos = if forward { start_pos.min(file_len) } else { 0 };
        if file.seek(SeekFrom::Start(pos)).is_err() {
            return;
        }

        let mut buf = vec![0u8; 128 * 1024];
        let mut overlap = Vec::new();

        while pos < file_len {
            let to_read = (buf.len() as u64).min(file_len - pos) as usize;
            if file.read_exact(&mut buf[..to_read]).is_err() {
                break;
            }

            let mut scan_buf = overlap.clone();
            scan_buf.extend_from_slice(&buf[..to_read]);
            let lower_scan = scan_buf.to_ascii_lowercase();

            if let Some(idx) = lower_scan
                .windows(needle_bytes.len())
                .position(|w| w == needle_bytes)
            {
                let found_offset = (pos + idx as u64).saturating_sub(overlap.len() as u64);
                self.offset = found_offset;
                self.reload_chunk();
                self.search_status = Some(format!("Trafienie na pozycji 0x{:08X}", found_offset));
                return;
            }

            let overlap_len = needle_bytes.len().min(to_read);
            overlap = buf[to_read - overlap_len..to_read].to_vec();
            pos += to_read as u64;
        }

        self.search_status = Some("Koniec pliku - brak kolejnych wystąpień".to_string());
    }
}

#[derive(Default)]
pub struct FileOpsState {
    pub show_mkdir: bool,
    pub mkdir_input: String,

    pub show_delete: bool,
    pub delete_targets: Vec<PathBuf>,
    pub delete_use_trash: bool,

    pub show_copy: bool,
    pub copy_sources: Vec<PathBuf>,
    pub copy_dest_path: String,

    pub show_move: bool,
    pub move_sources: Vec<PathBuf>,
    pub move_dest_path: String,

    pub show_rename: bool,
    pub rename_source: Option<PathBuf>,
    pub rename_input: String,

    pub show_pack_zip: bool,
    pub pack_zip_sources: Vec<PathBuf>,
    pub pack_zip_dest: String,

    pub show_mask_select: bool,
    pub mask_is_select: bool,
    pub mask_input: String,
}

#[derive(Default, Clone)]
pub struct PropertiesDialogState {
    pub is_open: bool,
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub files_count: usize,
    pub dirs_count: usize,
    pub modified: Option<String>,
    pub readonly: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigDialogState {
    pub is_open: bool,
    pub language: Option<String>,
    pub default_editor: String,
    pub associations: Vec<(String, String)>,
    pub new_ext: String,
    pub new_editor: String,
}

impl ConfigDialogState {
    pub fn open_with_config(&mut self, config: &crate::config::AppConfig) {
        self.is_open = true;
        self.language = config.language.clone();
        self.default_editor = config.default_editor.clone();
        let mut list: Vec<(String, String)> = config
            .associations
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        self.associations = list;
        self.new_ext.clear();
        self.new_editor.clear();
    }
}

pub struct ProgressUiState {
    pub is_open: bool,
    pub in_background: bool,
    pub title: String,
    pub current_item: String,
    pub items_done: usize,
    pub items_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub file_bytes_done: u64,
    pub file_bytes_total: u64,
    pub speed_bps: f32,
    pub eta_secs: Option<u64>,
    pub speed_history: VecDeque<f32>,
    pub start_time: Option<Instant>,

    // Queue UI
    pub show_queue_dialog: bool,
    pub queue_bytes_done: u64,
    pub queue_total_bytes: u64,
}

impl Default for ProgressUiState {
    fn default() -> Self {
        Self {
            is_open: false,
            in_background: false,
            title: String::new(),
            current_item: String::new(),
            items_done: 0,
            items_total: 0,
            bytes_done: 0,
            bytes_total: 0,
            file_bytes_done: 0,
            file_bytes_total: 0,
            speed_bps: 0.0,
            eta_secs: None,
            speed_history: VecDeque::with_capacity(30),
            start_time: None,
            show_queue_dialog: false,
            queue_bytes_done: 0,
            queue_total_bytes: 0,
        }
    }
}

#[derive(Default)]
pub struct DriveMenuState {
    pub side: Option<PanelSide>,
    pub cursor_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AboutTab {
    #[default]
    Overview,
    License,
    Libraries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LicenseViewKind {
    #[default]
    Mit,
    Apache2,
}

pub struct AboutDialogState {
    pub is_open: bool,
    pub current_tab: AboutTab,
    pub selected_license: LicenseViewKind,
    pub copy_notice_timer: Option<Instant>,
}

impl Default for AboutDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            current_tab: AboutTab::Overview,
            selected_license: LicenseViewKind::Mit,
            copy_notice_timer: None,
        }
    }
}

impl AboutDialogState {
    pub fn open_overview(&mut self) {
        self.is_open = true;
        self.current_tab = AboutTab::Overview;
    }

    pub fn open_license(&mut self) {
        self.is_open = true;
        self.current_tab = AboutTab::License;
    }

    pub fn open_libraries(&mut self) {
        self.is_open = true;
        self.current_tab = AboutTab::Libraries;
    }
}
