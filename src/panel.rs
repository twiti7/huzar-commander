use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};

use crate::fs_item::{FileItem, SortColumn, SortOrder};
use crate::zip_support::{is_zip_file, list_zip_entries, ZipLocation};

#[derive(Clone)]
pub struct DirLoadHandle {
    pub target_path: PathBuf,
    pub old_dir_name: Option<String>,
    pub rx: Receiver<Result<Vec<FileItem>, String>>,
    pub cancel: Arc<AtomicBool>,
    pub started_at: Instant,
}

impl std::fmt::Debug for DirLoadHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirLoadHandle")
            .field("target_path", &self.target_path)
            .field("old_dir_name", &self.old_dir_name)
            .field("started_at", &self.started_at)
            .finish()
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PanelState {
    pub current_path: PathBuf,
    pub all_items: Vec<FileItem>,
    pub items: Vec<FileItem>,
    pub selected_paths: HashSet<PathBuf>,
    pub cursor_idx: usize,
    pub scroll_to_cursor: bool,
    pub sort_column: SortColumn,
    pub sort_order: SortOrder,
    pub filter: String,
    pub is_filter_open: bool,
    pub show_hidden: bool,
    pub custom_title: Option<String>,
    pub zip_location: Option<ZipLocation>,
    pub total_bytes: u64,
    pub total_files: usize,
    pub total_dirs: usize,
    pub selected_bytes: u64,
    pub selected_count: usize,
    pub error_message: Option<String>,
    pub loading_handle: Option<DirLoadHandle>,
    pub calculating_dir_sizes: HashSet<PathBuf>,
    pub calculated_dirs: HashSet<PathBuf>,
    pub dir_size_tx: Sender<(PathBuf, u64, usize)>,
    pub dir_size_rx: Receiver<(PathBuf, u64, usize)>,
}

impl PanelState {
    pub fn new(initial_path: PathBuf) -> Self {
        let (dir_size_tx, dir_size_rx) = crossbeam_channel::unbounded();
        let mut panel = Self {
            current_path: initial_path,
            all_items: Vec::new(),
            items: Vec::new(),
            selected_paths: HashSet::new(),
            cursor_idx: 0,
            scroll_to_cursor: true,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            filter: String::new(),
            is_filter_open: false,
            show_hidden: true,
            custom_title: None,
            zip_location: None,
            total_bytes: 0,
            total_files: 0,
            total_dirs: 0,
            selected_bytes: 0,
            selected_count: 0,
            error_message: None,
            loading_handle: None,
            calculating_dir_sizes: HashSet::new(),
            calculated_dirs: HashSet::new(),
            dir_size_tx,
            dir_size_rx,
        };
        panel.refresh();
        panel
    }

    pub fn display_path(&self) -> String {
        if let Some(ref title) = self.custom_title {
            format!("[{}]", title)
        } else if let Some(ref loc) = self.zip_location {
            loc.display_path()
        } else {
            self.current_path.to_string_lossy().to_string()
        }
    }

    pub fn is_in_zip(&self) -> bool {
        self.zip_location.is_some()
    }

    pub fn enter_zip(&mut self, zip_path: PathBuf) {
        if is_zip_file(&zip_path) {
            self.zip_location = Some(ZipLocation::new(zip_path.clone()));
            self.current_path = zip_path;
            self.selected_paths.clear();
            self.cursor_idx = 0;
            self.scroll_to_cursor = true;
            self.refresh();
        }
    }

    pub fn enter_zip_dir(&mut self, dir_name: &str) {
        if let Some(ref mut loc) = self.zip_location {
            loc.enter_subdir(dir_name);
            self.selected_paths.clear();
            self.cursor_idx = 0;
            self.scroll_to_cursor = true;
            self.refresh();
        }
    }

    pub fn exit_zip(&mut self) {
        if let Some(loc) = self.zip_location.take() {
            let parent = loc
                .archive_path
                .parent()
                .unwrap_or(&loc.archive_path)
                .to_path_buf();
            let archive_name = loc
                .archive_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
            self.current_path = parent;
            self.selected_paths.clear();
            self.refresh();
            if let Some(name) = archive_name {
                if let Some(idx) = self.items.iter().position(|it| it.name == name) {
                    self.cursor_idx = idx;
                    self.scroll_to_cursor = true;
                }
            }
        }
    }

    pub fn refresh(&mut self) {
        self.error_message = None;
        self.calculating_dir_sizes.clear();
        self.calculated_dirs.clear();
        let mut new_items = Vec::new();

        if let Some(ref loc) = self.zip_location {
            // ZIP archive browsing
            let parent_path = if loc.internal_dir.is_empty() {
                loc.archive_path
                    .parent()
                    .unwrap_or(&loc.archive_path)
                    .to_path_buf()
            } else {
                loc.archive_path.clone()
            };
            new_items.push(FileItem::new_parent(parent_path));

            match list_zip_entries(&loc.archive_path, &loc.internal_dir) {
                Ok(entries) => {
                    new_items.extend(entries);
                }
                Err(e) => {
                    self.error_message = Some(e);
                }
            }
        } else {
            // Normal file system directory browsing
            if let Some(parent) = self.current_path.parent() {
                new_items.push(FileItem::new_parent(parent.to_path_buf()));
            }

            match fs::read_dir(&self.current_path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if let Some(item) = FileItem::from_path(&entry.path()) {
                            new_items.push(item);
                        }
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Błąd odczytu: {}", e));
                }
            }
        }

        self.all_items = new_items;
        self.sort_items();
        self.apply_filter();
    }

    pub fn change_dir(&mut self, new_path: PathBuf) {
        if self.zip_location.is_some() {
            self.zip_location = None;
        }

        if new_path.is_dir() {
            self.custom_title = None;
            self.current_path = new_path;
            self.selected_paths.clear();
            self.cursor_idx = 0;
            self.scroll_to_cursor = true;
            self.refresh();
        }
    }

    pub fn go_up(&mut self) {
        if let Some(ref mut loc) = self.zip_location {
            if loc.go_up() {
                self.selected_paths.clear();
                self.cursor_idx = 0;
                self.scroll_to_cursor = true;
                self.refresh();
            } else {
                self.exit_zip();
            }
            return;
        }

        if let Some(parent) = self.current_path.parent() {
            let old_dir_name = self
                .current_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
            let parent_buf = parent.to_path_buf();
            self.change_dir(parent_buf);

            // Put cursor on the directory we just came out from
            if let Some(name) = old_dir_name {
                if let Some(idx) = self.items.iter().position(|it| it.name == name) {
                    self.cursor_idx = idx;
                    self.scroll_to_cursor = true;
                }
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        self.loading_handle.is_some()
    }

    pub fn loading_target(&self) -> Option<&PathBuf> {
        self.loading_handle.as_ref().map(|h| &h.target_path)
    }

    pub fn loading_elapsed(&self) -> std::time::Duration {
        self.loading_handle
            .as_ref()
            .map(|h| h.started_at.elapsed())
            .unwrap_or_default()
    }

    pub fn cancel_loading(&mut self) -> bool {
        if let Some(handle) = self.loading_handle.take() {
            handle.cancel.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn start_loading_dir(
        &mut self,
        new_path: PathBuf,
        old_dir_name: Option<String>,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        // Przerwij ewentualne poprzednie wczytywanie w toku
        self.cancel_loading();

        if self.zip_location.is_some() {
            self.zip_location = None;
        }

        self.custom_title = None;
        self.error_message = None;
        self.calculating_dir_sizes.clear();
        self.calculated_dirs.clear();

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_thread = Arc::clone(&cancel);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let path_to_scan = new_path.clone();

        std::thread::spawn(move || {
            if cancel_thread.load(Ordering::Relaxed) {
                return;
            }

            let mut new_items = Vec::new();
            if let Some(parent) = path_to_scan.parent() {
                new_items.push(FileItem::new_parent(parent.to_path_buf()));
            }

            match std::fs::read_dir(&path_to_scan) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if cancel_thread.load(Ordering::Relaxed) {
                            return;
                        }
                        if let Some(item) = FileItem::from_path(&entry.path()) {
                            new_items.push(item);
                        }
                    }
                    if !cancel_thread.load(Ordering::Relaxed) {
                        let _ = tx.send(Ok(new_items));
                        if let Some(sig) = repaint_signal {
                            sig();
                        }
                    }
                }
                Err(e) => {
                    if !cancel_thread.load(Ordering::Relaxed) {
                        let _ = tx.send(Err(e.to_string()));
                        if let Some(sig) = repaint_signal {
                            sig();
                        }
                    }
                }
            }
        });

        self.loading_handle = Some(DirLoadHandle {
            target_path: new_path,
            old_dir_name,
            rx,
            cancel,
            started_at: Instant::now(),
        });
    }

    pub fn poll_loading(&mut self) -> Option<Result<PathBuf, String>> {
        let handle = self.loading_handle.as_ref()?;
        match handle.rx.try_recv() {
            Ok(Ok(new_items)) => {
                let target_path = handle.target_path.clone();
                let old_dir_name = handle.old_dir_name.clone();
                self.loading_handle = None;

                self.current_path = target_path.clone();
                self.all_items = new_items;
                self.selected_paths.clear();
                self.cursor_idx = 0;
                self.scroll_to_cursor = true;
                self.sort_items();
                self.apply_filter();

                if let Some(name) = old_dir_name {
                    if let Some(idx) = self.items.iter().position(|it| it.name == name) {
                        self.cursor_idx = idx;
                        self.scroll_to_cursor = true;
                    }
                }

                Some(Ok(target_path))
            }
            Ok(Err(err)) => {
                let _target_path = handle.target_path.clone();
                self.loading_handle = None;
                self.error_message = Some(format!("Błąd odczytu: {}", err));
                Some(Err(err))
            }
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                self.loading_handle = None;
                Some(Err("Wątek przerwany".to_string()))
            }
        }
    }

    pub fn go_up_async(&mut self, repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>) {
        if let Some(ref mut loc) = self.zip_location {
            if loc.go_up() {
                self.selected_paths.clear();
                self.cursor_idx = 0;
                self.scroll_to_cursor = true;
                self.refresh();
            } else {
                self.exit_zip();
            }
            return;
        }

        if let Some(parent) = self.current_path.parent() {
            let old_dir_name = self
                .current_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
            let parent_buf = parent.to_path_buf();
            self.start_loading_dir(parent_buf, old_dir_name, repaint_signal);
        }
    }

    pub fn apply_filter(&mut self) {
        let active_path = self.items.get(self.cursor_idx).map(|it| it.path.clone());

        let show_hidden = self.show_hidden;
        let is_empty_filter = self.filter.trim().is_empty();

        self.items = self
            .all_items
            .iter()
            .filter(|it| {
                if it.is_parent {
                    return true;
                }
                if !show_hidden && it.is_hidden {
                    return false;
                }
                if is_empty_filter {
                    true
                } else {
                    matches_filter(&it.name, &self.filter)
                }
            })
            .cloned()
            .collect();

        self.recalculate_totals();

        if let Some(path) = active_path {
            if let Some(idx) = self.items.iter().position(|it| it.path == path) {
                self.cursor_idx = idx;
            } else if !self.items.is_empty() {
                self.cursor_idx = 0;
            }
        } else if self.cursor_idx >= self.items.len() && !self.items.is_empty() {
            self.cursor_idx = self.items.len() - 1;
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.apply_filter();
    }

    #[allow(dead_code)]
    pub fn set_filter(&mut self, new_filter: String) {
        self.filter = new_filter;
        self.apply_filter();
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.apply_filter();
    }

    pub fn selected_or_cursor_items(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            self.selected_paths.iter().cloned().collect()
        } else if let Some(item) = self.items.get(self.cursor_idx) {
            if !item.is_parent {
                vec![item.path.clone()]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    pub fn toggle_select(&mut self, idx: usize) {
        if let Some(item) = self.items.get(idx) {
            if item.is_parent {
                return;
            }
            if self.selected_paths.contains(&item.path) {
                self.selected_paths.remove(&item.path);
            } else {
                self.selected_paths.insert(item.path.clone());
            }
            self.recalculate_totals();
        }
    }

    pub fn select_range(&mut self, from: usize, to: usize) {
        let start = from.min(to);
        let end = from.max(to);
        for idx in start..=end {
            if let Some(item) = self.items.get(idx) {
                if !item.is_parent {
                    self.selected_paths.insert(item.path.clone());
                }
            }
        }
        self.recalculate_totals();
    }

    pub fn select_all(&mut self) {
        for item in &self.items {
            if !item.is_parent {
                self.selected_paths.insert(item.path.clone());
            }
        }
        self.recalculate_totals();
    }

    pub fn deselect_all(&mut self) {
        self.selected_paths.clear();
        self.recalculate_totals();
    }

    pub fn invert_selection(&mut self) {
        for item in &self.items {
            if !item.is_parent {
                if self.selected_paths.contains(&item.path) {
                    self.selected_paths.remove(&item.path);
                } else {
                    self.selected_paths.insert(item.path.clone());
                }
            }
        }
        self.recalculate_totals();
    }

    pub fn select_by_pattern(&mut self, pattern: &str) {
        for item in &self.items {
            if !item.is_parent && matches_filter(&item.name, pattern) {
                self.selected_paths.insert(item.path.clone());
            }
        }
        self.recalculate_totals();
    }

    pub fn deselect_by_pattern(&mut self, pattern: &str) {
        for item in &self.items {
            if !item.is_parent && matches_filter(&item.name, pattern) {
                self.selected_paths.remove(&item.path);
            }
        }
        self.recalculate_totals();
    }

    pub fn is_calculating_dir(&self, path: &Path) -> bool {
        self.calculating_dir_sizes.contains(path)
    }

    pub fn is_calculated_dir(&self, path: &Path) -> bool {
        self.calculated_dirs.contains(path)
    }

    pub fn has_calculating_dirs(&self) -> bool {
        !self.calculating_dir_sizes.is_empty()
    }

    pub fn start_calculate_dir_size(
        &mut self,
        dir_path: PathBuf,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        if self.calculating_dir_sizes.contains(&dir_path) {
            return;
        }

        self.calculating_dir_sizes.insert(dir_path.clone());
        let tx = self.dir_size_tx.clone();
        let target_path = dir_path.clone();

        std::thread::spawn(move || {
            let mut total_size: u64 = 0;
            let mut file_count: usize = 0;

            for entry in walkdir::WalkDir::new(&target_path)
                .min_depth(1)
                .into_iter()
                .flatten()
            {
                if entry.file_type().is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total_size = total_size.saturating_add(meta.len());
                        file_count += 1;
                    }
                }
            }

            let _ = tx.send((target_path, total_size, file_count));
            if let Some(sig) = repaint_signal {
                sig();
            }
        });
    }

    pub fn poll_dir_sizes(&mut self) -> Vec<(PathBuf, u64, usize)> {
        let mut finished = Vec::new();
        while let Ok((path, size, file_count)) = self.dir_size_rx.try_recv() {
            self.calculating_dir_sizes.remove(&path);
            self.calculated_dirs.insert(path.clone());

            let mut matched = false;
            if let Some(item) = self.items.iter_mut().find(|it| it.path == path) {
                item.size = size;
                matched = true;
            }
            if let Some(item) = self.all_items.iter_mut().find(|it| it.path == path) {
                item.size = size;
                matched = true;
            }

            if matched {
                self.recalculate_totals();
                finished.push((path, size, file_count));
            }
        }
        finished
    }

    pub fn space_select_and_calculate(
        &mut self,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Option<PathBuf> {
        let idx = self.cursor_idx;
        let item = self.items.get(idx)?.clone();
        if item.is_parent {
            return None;
        }

        // 1. Zaznacz / odznacz bieżący element
        self.toggle_select(idx);

        // 2. Jeśli to katalog, uruchom asynchroniczne zliczanie rozmiaru w tle
        let calc_path = if item.is_dir {
            self.start_calculate_dir_size(item.path.clone(), repaint_signal);
            Some(item.path)
        } else {
            None
        };

        // 3. Przesuń kursor w dół o 1 wiersz (jak klawisz Insert), aby natychmiast móc zaznaczać kolejne elementy
        if idx + 1 < self.items.len() {
            self.cursor_idx += 1;
            self.scroll_to_cursor = true;
        }

        calc_path
    }

    pub fn load_search_results(&mut self, results: &[crate::search::SearchResult]) {
        self.zip_location = None;
        self.custom_title = Some("Wyniki wyszukiwania".to_string());
        let parent_item = FileItem::new_parent(self.current_path.clone());
        let mut new_items = vec![parent_item];

        for res in results {
            let item = FileItem {
                name: res.name.clone(),
                ext: res
                    .path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default(),
                path: res.path.clone(),
                is_dir: false,
                is_parent: false,
                size: res.size,
                modified: res.modified,
                is_hidden: false,
                is_readonly: false,
            };
            new_items.push(item);
        }

        self.all_items = new_items;
        self.selected_paths.clear();
        self.cursor_idx = 0;
        self.scroll_to_cursor = true;
        self.sort_items();
        self.apply_filter();
    }

    pub fn sort_items(&mut self) {
        let col = self.sort_column;
        let ord = self.sort_order;

        self.all_items.sort_by(|a, b| {
            // ".." is always at the very top
            if a.is_parent {
                return std::cmp::Ordering::Less;
            }
            if b.is_parent {
                return std::cmp::Ordering::Greater;
            }

            // Directories always come before files
            if a.is_dir != b.is_dir {
                return if a.is_dir {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }

            let cmp = match col {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Ext => a.ext.cmp(&b.ext).then_with(|| a.name.cmp(&b.name)),
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::Date => a.modified.cmp(&b.modified),
            };

            match ord {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });

        self.apply_filter();
    }

    pub fn recalculate_totals(&mut self) {
        let mut total_bytes = 0;
        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut sel_bytes = 0;
        let mut sel_count = 0;

        for item in &self.items {
            if item.is_parent {
                continue;
            }
            if item.is_dir {
                total_dirs += 1;
            } else {
                total_files += 1;
            }
            total_bytes += item.size;

            if self.selected_paths.contains(&item.path) {
                sel_count += 1;
                sel_bytes += item.size;
            }
        }

        self.total_bytes = total_bytes;
        self.total_files = total_files;
        self.total_dirs = total_dirs;
        self.selected_bytes = sel_bytes;
        self.selected_count = sel_count;
    }
}

pub fn matches_filter(name: &str, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() || filter == "*" || filter == "*.*" {
        return true;
    }

    let name_lower = name.to_lowercase();
    let filter_lower = filter.to_lowercase();

    if filter_lower.contains(';') || filter_lower.contains(',') {
        return filter_lower
            .split([';', ','])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .any(|pat| matches_single_pattern(&name_lower, pat));
    }

    matches_single_pattern(&name_lower, &filter_lower)
}

fn matches_single_pattern(name: &str, pattern: &str) -> bool {
    if pattern.contains('*') || pattern.contains('?') {
        glob_match(pattern, name)
    } else {
        name.contains(pattern)
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p_bytes = pattern.as_bytes();
    let t_bytes = text.as_bytes();
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (None, 0);

    while ti < t_bytes.len() {
        if pi < p_bytes.len() && (p_bytes[pi] == b'?' || p_bytes[pi] == t_bytes[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p_bytes.len() && p_bytes[pi] == b'*' {
            star_pi = Some(pi);
            pi += 1;
            star_ti = ti;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < p_bytes.len() && p_bytes[pi] == b'*' {
        pi += 1;
    }

    pi == p_bytes.len()
}
