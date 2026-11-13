use std::path::PathBuf;
use std::time::Instant;

use egui::{Color32, Key};

use crate::drives::{scan_drives, DriveInfo};
use crate::fs_item::FileItem;
use crate::panel::PanelState;
use crate::ui::dialog_queue::{
    render_background_tasks_bar, render_progress_dialog, render_queue_dialog,
};
use crate::ui::dialog_search::{render_search_dialog, SearchDialogAction};
use crate::ui::file_dialogs::{
    render_about_dialog, render_copy_dialog, render_delete_dialog, render_drive_menu,
    render_mask_select_dialog, render_mkdir_dialog, render_move_dialog, render_pack_zip_dialog,
    render_properties_dialog, render_rename_dialog, FileOpsAction,
};
use crate::ui::menu_bars::{
    render_bottom_f_keys, render_cli_status_bar, render_top_menu_bar, MenuAction,
};
use crate::ui::panels::{render_dual_panels, PanelAction};
use crate::ui::state::{
    AboutDialogState, DriveMenuState, FileOpsState, ProgressUiState, PropertiesDialogState,
    SearchDialogState, ViewerWindowState,
};
use crate::ui::viewer_window::render_viewer_window;
use crate::worker::{IoWorker, QueueStateSnapshot, WorkerCommand, WorkerEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide {
    Left,
    Right,
}

pub struct CommanderApp {
    pub left_panel: PanelState,
    pub right_panel: PanelState,
    pub active_side: PanelSide,
    pub drives: Vec<DriveInfo>,

    // Sub-states
    pub file_ops: FileOpsState,
    pub drive_menu: DriveMenuState,
    pub progress: ProgressUiState,
    pub search: SearchDialogState,
    pub viewer: ViewerWindowState,
    pub properties: PropertiesDialogState,
    pub config_dialog: crate::ui::state::ConfigDialogState,
    pub about_dialog: AboutDialogState,
    pub i18n: crate::i18n::I18n,
    pub config: crate::config::AppConfig,

    // Worker & Queue
    pub worker: IoWorker,
    pub queue_snapshot: QueueStateSnapshot,

    // Command line & status
    pub cmd_line: String,
    pub status_bar_msg: Option<(String, Instant)>,
    pub last_copy_info: Option<(PathBuf, String)>,
}

impl CommanderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Dark midnight theme
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(13, 17, 23);
        visuals.window_fill = Color32::from_rgb(18, 24, 38);
        visuals.override_text_color = Some(Color32::from_rgb(220, 230, 245));
        cc.egui_ctx.set_visuals(visuals);

        let drives = scan_drives();
        let fallback_path = if cfg!(target_os = "windows") {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        let current_dir = std::env::current_dir().unwrap_or(fallback_path);

        let left_path = current_dir.clone();
        let right_path = if drives.len() > 1 {
            drives[1].path.clone()
        } else {
            current_dir
        };

        let config = crate::config::AppConfig::load();
        let initial_lang = if let Some(ref l) = config.language {
            crate::i18n::Language::from_locale_str(l).unwrap_or_else(crate::i18n::Language::detect_system_language)
        } else {
            crate::i18n::Language::detect_system_language()
        };
        let i18n = crate::i18n::I18n::new(initial_lang);

        let worker = IoWorker::new();
        let repaint_ctx = cc.egui_ctx.clone();
        worker.set_repaint_signal(move || {
            repaint_ctx.request_repaint();
        });

        let mut app = Self {
            left_panel: PanelState::new(left_path),
            right_panel: PanelState::new(right_path),
            active_side: PanelSide::Left,
            drives,

            file_ops: FileOpsState::default(),
            drive_menu: DriveMenuState::default(),
            progress: ProgressUiState::default(),
            search: SearchDialogState {
                mask: "*".to_string(),
                recursive: true,
                ..Default::default()
            },
            viewer: ViewerWindowState::default(),
            properties: PropertiesDialogState::default(),
            config_dialog: crate::ui::state::ConfigDialogState::default(),
            about_dialog: AboutDialogState::default(),
            i18n,
            config,

            worker,
            queue_snapshot: QueueStateSnapshot::default(),

            cmd_line: String::new(),
            status_bar_msg: None,
            last_copy_info: None,
        };

        let welcome = app.i18n.t("status.welcome").to_string();
        app.set_status(welcome);
        app
    }

    fn active_panel(&self) -> &PanelState {
        match self.active_side {
            PanelSide::Left => &self.left_panel,
            PanelSide::Right => &self.right_panel,
        }
    }

    fn active_panel_mut(&mut self) -> &mut PanelState {
        match self.active_side {
            PanelSide::Left => &mut self.left_panel,
            PanelSide::Right => &mut self.right_panel,
        }
    }

    fn passive_panel(&self) -> &PanelState {
        match self.active_side {
            PanelSide::Left => &self.right_panel,
            PanelSide::Right => &self.left_panel,
        }
    }

    fn passive_panel_mut(&mut self) -> &mut PanelState {
        match self.active_side {
            PanelSide::Left => &mut self.right_panel,
            PanelSide::Right => &mut self.left_panel,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_bar_msg = Some((msg.into(), Instant::now()));
    }

    fn handle_worker_events(&mut self, ctx: &egui::Context) {
        let mut needs_refresh = false;
        while let Ok(event) = self.worker.event_rx.try_recv() {
            match event {
                WorkerEvent::QueueUpdated(snapshot) => {
                    self.progress.queue_bytes_done = snapshot.queue_bytes_done;
                    self.progress.queue_total_bytes = snapshot.queue_total_bytes;
                    self.queue_snapshot = snapshot;
                    ctx.request_repaint();
                }
                WorkerEvent::TaskStarted {
                    title,
                    total_items,
                    total_bytes,
                    ..
                } => {
                    self.progress.is_open = true;
                    self.progress.title = title;
                    self.progress.current_item = "Inicjalizacja...".to_string();
                    self.progress.items_done = 0;
                    self.progress.items_total = total_items;
                    self.progress.bytes_done = 0;
                    self.progress.bytes_total = total_bytes;
                    self.progress.file_bytes_done = 0;
                    self.progress.file_bytes_total = 0;
                    self.progress.speed_bps = 0.0;
                    self.progress.eta_secs = None;
                    self.progress.speed_history.clear();
                    self.progress.start_time = Some(Instant::now());
                    ctx.request_repaint();
                }
                WorkerEvent::Progress {
                    current_name,
                    task_items_done,
                    task_total_items,
                    task_bytes_done,
                    task_total_bytes,
                    current_file_bytes,
                    current_file_size,
                    queue_bytes_done,
                    queue_total_bytes,
                    speed_bps,
                    eta_secs,
                    ..
                } => {
                    self.progress.current_item = current_name;
                    self.progress.items_done = task_items_done;
                    self.progress.items_total = task_total_items;
                    self.progress.bytes_done = task_bytes_done;
                    self.progress.bytes_total = task_total_bytes;
                    self.progress.file_bytes_done = current_file_bytes;
                    self.progress.file_bytes_total = current_file_size;
                    self.progress.speed_bps = speed_bps;
                    self.progress.eta_secs = eta_secs;

                    self.progress.queue_bytes_done = queue_bytes_done;
                    self.progress.queue_total_bytes = queue_total_bytes;

                    self.progress.speed_history.push_back(speed_bps);
                    if self.progress.speed_history.len() > 70 {
                        self.progress.speed_history.pop_front();
                    }
                    ctx.request_repaint();
                }
                WorkerEvent::TaskFinished { message, .. } => {
                    self.set_status(&message);
                    needs_refresh = true;
                }
                WorkerEvent::QueueFinished { message } => {
                    self.progress.is_open = false;
                    self.progress.in_background = false;
                    self.set_status(&message);
                    needs_refresh = true;
                }
                WorkerEvent::Error { message, .. } => {
                    self.set_status(format!("BŁĄD: {}", message));
                    needs_refresh = true;
                }
            }
        }

        if needs_refresh {
            self.left_panel.refresh();
            self.right_panel.refresh();

            // Ustawienie kursora w panelu pasywnym na skopiowanym elemencie
            if let Some((target_dir, first_name)) = self.last_copy_info.take() {
                let passive = self.passive_panel_mut();
                if passive.current_path == target_dir {
                    if let Some(pos) = passive.items.iter().position(|it| it.name == first_name) {
                        passive.cursor_idx = pos;
                        passive.scroll_to_cursor = true;
                    }
                }
            }

            ctx.request_repaint();
        }
    }

    fn open_item(&mut self, item: &FileItem) {
        if item.is_parent {
            self.active_panel_mut().go_up();
        } else if self.active_panel().is_in_zip() {
            if item.is_dir {
                let name = item.name.clone();
                self.active_panel_mut().enter_zip_dir(&name);
            } else {
                let active = self.active_panel();
                if let Some(loc) = &active.zip_location {
                    let internal_name = if loc.internal_dir.is_empty() {
                        item.name.clone()
                    } else {
                        format!("{}{}", loc.internal_dir, item.name)
                    };
                    let archive_path = loc.archive_path.clone();
                    let file_name = item.name.clone();
                    match crate::opener::open_zip_entry(&archive_path, &internal_name) {
                        Ok(_) => {
                            self.set_status(format!("Otwarto plik z archiwum: {}", file_name));
                        }
                        Err(e) => {
                            self.set_status(format!("Błąd otwierania pliku z ZIP: {}", e));
                        }
                    }
                } else {
                    self.trigger_f3_view();
                }
            }
        } else if item.is_dir {
            let path = item.path.clone();
            self.active_panel_mut().change_dir(path);
        } else if crate::zip_support::is_zip_file(&item.path) {
            self.active_panel_mut().enter_zip(item.path.clone());
        } else {
            let name = item.name.clone();
            match crate::opener::open_file(&item.path) {
                Ok(_) => {
                    self.set_status(format!("Uruchomiono: {}", name));
                }
                Err(err) => {
                    self.set_status(err);
                }
            }
        }
    }

    fn trigger_f3_view(&mut self) {
        let (item_name, item_path, is_file, zip_info) = {
            let active = self.active_panel();
            let item_opt = active.items.get(active.cursor_idx).cloned();
            if let Some(item) = item_opt {
                if !item.is_dir && !item.is_parent {
                    (
                        item.name.clone(),
                        item.path.clone(),
                        true,
                        active.zip_location.clone(),
                    )
                } else {
                    (String::new(), PathBuf::new(), false, None)
                }
            } else {
                (String::new(), PathBuf::new(), false, None)
            }
        };

        if !is_file {
            return;
        }

        // 1. Podgląd plików graficznych oraz multimedialnych w dedykowanym programie systemowym
        if crate::opener::is_image_file(&item_name) || crate::opener::is_media_file(&item_name) {
            let is_img = crate::opener::is_image_file(&item_name);
            let kind_label = if is_img { "grafiki" } else { "multimediów" };

            if let Some(loc) = zip_info {
                let internal_name = if loc.internal_dir.is_empty() {
                    item_name.clone()
                } else {
                    format!("{}{}", loc.internal_dir, item_name)
                };
                match crate::opener::open_zip_entry(&loc.archive_path, &internal_name) {
                    Ok(_) => {
                        self.set_status(format!(
                            "Podgląd {} z archiwum ZIP w programie systemowym: {}",
                            kind_label, item_name
                        ));
                    }
                    Err(e) => {
                        self.set_status(format!("Błąd podglądu pliku z ZIP: {}", e));
                    }
                }
            } else {
                match crate::opener::open_file(&item_path) {
                    Ok(_) => {
                        self.set_status(format!(
                            "Podgląd {} w programie systemowym: {}",
                            kind_label, item_name
                        ));
                    }
                    Err(e) => {
                        self.set_status(format!("Błąd podglądu pliku: {}", e));
                    }
                }
            }
            return;
        }

        // 2. Podgląd w oknie F3 z pełnym przeglądaniem (streaming, Text / Hex, suwak i paginacja)
        if let Some(loc) = zip_info {
            let internal_name = if loc.internal_dir.is_empty() {
                item_name.clone()
            } else {
                format!("{}{}", loc.internal_dir, item_name)
            };
            let temp_dir = std::env::temp_dir().join("cmdr_preview").join("zip_view");
            let target_file = temp_dir.join(&item_name);
            match crate::zip_support::extract_single_zip_entry(
                &loc.archive_path,
                &internal_name,
                &target_file,
            ) {
                Ok(_) => {
                    if let Err(e) = self.viewer.open_file(item_name, target_file) {
                        self.set_status(format!("Błąd podglądu: {}", e));
                    }
                }
                Err(e) => {
                    self.set_status(format!("Błąd wypakowania do podglądu ZIP: {}", e));
                }
            }
        } else {
            if let Err(e) = self.viewer.open_file(item_name, item_path) {
                self.set_status(format!("Błąd podglądu: {}", e));
            }
        }
    }

    fn trigger_f4_edit(&mut self) {
        let active = self.active_panel();
        if let Some(item) = active.items.get(active.cursor_idx) {
            if !item.is_dir && !item.is_parent {
                let ext = item.path.extension().and_then(|s| s.to_str()).unwrap_or("");
                let editor_cmd = self.config.get_editor_for_ext(ext).to_string();
                let path = item.path.clone();
                match crate::opener::open_with_editor(&path, &editor_cmd) {
                    Ok(_) => {
                        self.set_status(format!("Otwarto w edytorze: {}", editor_cmd));
                    }
                    Err(e) => {
                        // Jeśli dedykowany edytor zawiódł (np. brak w PATH), spróbuj domyślnego
                        if editor_cmd != self.config.default_editor && !self.config.default_editor.is_empty() {
                            let fallback = self.config.default_editor.clone();
                            if crate::opener::open_with_editor(&path, &fallback).is_ok() {
                                self.set_status(format!("Błąd '{}', użyto edytora domyślnego: {}", editor_cmd, fallback));
                                return;
                            }
                        }
                        self.set_status(format!("Błąd uruchomienia edytora '{}': {}", editor_cmd, e));
                    }
                }
            }
        }
    }

    fn trigger_f5_copy(&mut self) {
        let active = self.active_panel();
        let sources = active.selected_or_cursor_items();
        if !sources.is_empty() {
            let passive_dir = self.passive_panel().current_path.to_string_lossy().to_string();
            self.file_ops.copy_sources = sources;
            self.file_ops.copy_dest_path = passive_dir;
            self.file_ops.show_copy = true;
        }
    }

    fn trigger_f6_move(&mut self) {
        let sources = self.active_panel().selected_or_cursor_items();
        if !sources.is_empty() {
            let passive_dir = self.passive_panel().current_path.to_string_lossy().to_string();
            self.file_ops.move_sources = sources;
            self.file_ops.move_dest_path = passive_dir;
            self.file_ops.show_move = true;
        }
    }

    fn trigger_f7_mkdir(&mut self) {
        self.file_ops.mkdir_input.clear();
        self.file_ops.show_mkdir = true;
    }

    fn trigger_f8_delete(&mut self) {
        let targets = self.active_panel().selected_or_cursor_items();
        if !targets.is_empty() {
            self.file_ops.delete_targets = targets;
            self.file_ops.delete_use_trash = true;
            self.file_ops.show_delete = true;
        }
    }

    fn trigger_rename(&mut self) {
        let target = {
            let active = self.active_panel();
            active.items.get(active.cursor_idx).and_then(|item| {
                if !item.is_parent {
                    Some((item.path.clone(), item.name.clone()))
                } else {
                    None
                }
            })
        };

        if let Some((path, name)) = target {
            self.file_ops.rename_source = Some(path);
            self.file_ops.rename_input = name;
            self.file_ops.show_rename = true;
        }
    }

    fn trigger_pack_zip(&mut self) {
        let (sources, passive_dir) = {
            let active = self.active_panel();
            let sources = active.selected_or_cursor_items();
            let passive_dir = self.passive_panel().current_path.clone();
            (sources, passive_dir)
        };
        if !sources.is_empty() {
            let default_name = if sources.len() == 1 {
                let stem = sources[0]
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "archive".to_string());
                format!("{}.zip", stem)
            } else {
                "archive.zip".to_string()
            };
            self.file_ops.pack_zip_sources = sources;
            self.file_ops.pack_zip_dest =
                passive_dir.join(default_name).to_string_lossy().to_string();
            self.file_ops.show_pack_zip = true;
        }
    }

    fn trigger_properties(&mut self) {
        let target = {
            let active = self.active_panel();
            active.items.get(active.cursor_idx).and_then(|item| {
                if item.is_parent {
                    None
                } else {
                    let modified = item.modified.map(|time| {
                        let dt: chrono::DateTime<chrono::Local> = time.into();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    });
                    Some((
                        item.is_dir,
                        item.path.clone(),
                        item.name.clone(),
                        item.size,
                        modified,
                        item.is_readonly,
                        item.is_hidden,
                    ))
                }
            })
        };

        if let Some((is_dir, path, name, size, modified, readonly, hidden)) = target {
            let mut files_cnt = 0;
            let mut dirs_cnt = 0;
            if is_dir {
                for entry in walkdir::WalkDir::new(&path).min_depth(1).into_iter().flatten() {
                    if entry.file_type().is_dir() {
                        dirs_cnt += 1;
                    } else if entry.file_type().is_file() {
                        files_cnt += 1;
                    }
                }
            }

            self.properties = PropertiesDialogState {
                is_open: true,
                name,
                path,
                is_dir,
                size,
                files_count: files_cnt,
                dirs_count: dirs_cnt,
                modified,
                readonly,
                hidden,
            };
        }
    }

    fn calculate_current_dir_size(&mut self) {
        let res = self.active_panel_mut().calculate_cursor_dir_size();
        if let Some(size) = res {
            self.set_status(format!(
                "Rozmiar katalogu: {}",
                humansize::format_size(size, humansize::DECIMAL)
            ));
        }
    }

    fn open_mask_select_dialog(&mut self, is_select: bool) {
        self.file_ops.mask_is_select = is_select;
        let active = self.active_panel();
        if let Some(item) = active.items.get(active.cursor_idx) {
            if !item.is_parent && !item.ext.is_empty() {
                self.file_ops.mask_input = format!("*.{}", item.ext);
            } else {
                self.file_ops.mask_input = "*.*".to_string();
            }
        } else {
            self.file_ops.mask_input = "*.*".to_string();
        }
        self.file_ops.show_mask_select = true;
    }

    fn open_search_dialog(&mut self) {
        self.search.dir = self.active_panel().current_path.to_string_lossy().to_string();
        if self.search.mask.is_empty() {
            self.search.mask = "*".to_string();
        }
        self.search.is_open = true;
    }

    fn start_search(&mut self) {
        self.search.stop();
        self.search.results.clear();
        self.search.selected_idx = None;
        self.search.scanned_count = 0;
        self.search.status_text = "Przeszukiwanie plików...".to_string();

        let params = crate::search::SearchParams {
            start_dir: PathBuf::from(self.search.dir.trim()),
            mask: self.search.mask.clone(),
            text: self.search.text.clone(),
            case_sensitive: self.search.case_sensitive,
            recursive: self.search.recursive,
        };
        self.search.handle = Some(crate::search::start_search(params));
    }

    fn poll_search_messages(&mut self) {
        if let Some(ref handle) = self.search.handle {
            while let Ok(msg) = handle.rx.try_recv() {
                match msg {
                    crate::search::SearchMessage::Found(result) => {
                        self.search.results.push(result);
                    }
                    crate::search::SearchMessage::ScannedCount(cnt) => {
                        self.search.scanned_count = cnt;
                    }
                    crate::search::SearchMessage::Finished(found) => {
                        self.search.status_text = format!(
                            "Zakończono. Przeszukano {} plików, znaleziono: {}.",
                            self.search.scanned_count, found
                        );
                        self.search.handle = None;
                        break;
                    }
                    crate::search::SearchMessage::Error(err) => {
                        self.search.status_text = format!("Błąd: {}", err);
                        self.search.handle = None;
                        break;
                    }
                }
            }
        }
    }

    fn jump_to_search_result(&mut self, res: &crate::search::SearchResult) {
        if let Some(parent) = res.path.parent() {
            self.active_panel_mut().change_dir(parent.to_path_buf());
            if let Some(idx) = self.active_panel().items.iter().position(|it| it.name == res.name) {
                self.active_panel_mut().cursor_idx = idx;
                self.active_panel_mut().scroll_to_cursor = true;
            }
        }
        self.search.is_open = false;
    }

    fn execute_cli_cmd(&mut self) {
        let cmd = self.cmd_line.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        let active_dir = self.active_panel().current_path.clone();

        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd.exe")
                .arg("/c")
                .arg(&cmd)
                .current_dir(&active_dir)
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .current_dir(&active_dir)
                .spawn();
        }

        self.set_status(format!("Wykonano polecenie: {}", cmd));
        self.cmd_line.clear();
        self.active_panel_mut().refresh();
    }

    fn open_drive_menu(&mut self, side: PanelSide) {
        self.drives = scan_drives();
        let cur_path = match side {
            PanelSide::Left => self.left_panel.current_path.clone(),
            PanelSide::Right => self.right_panel.current_path.clone(),
        };
        self.drive_menu.cursor_idx = self
            .drives
            .iter()
            .position(|d| cur_path.starts_with(&d.path))
            .unwrap_or(0);
        self.drive_menu.side = Some(side);
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let has_modal = self.file_ops.show_mkdir
            || self.file_ops.show_delete
            || self.file_ops.show_copy
            || self.file_ops.show_move
            || self.file_ops.show_rename
            || self.file_ops.show_pack_zip
            || self.file_ops.show_mask_select
            || self.properties.is_open
            || self.viewer.is_open
            || self.about_dialog.is_open
            || self.search.is_open
            || self.config_dialog.is_open
            || self.progress.show_queue_dialog;

        if has_modal {
            return;
        }

        let mut toggle_drive_side = None;
        ctx.input(|i| {
            if i.modifiers.alt && i.key_pressed(Key::F1) {
                toggle_drive_side = Some(PanelSide::Left);
            } else if i.modifiers.alt && i.key_pressed(Key::F2) {
                toggle_drive_side = Some(PanelSide::Right);
            }
        });

        if let Some(side) = toggle_drive_side {
            if self.drive_menu.side == Some(side) {
                self.drive_menu.side = None;
            } else {
                self.open_drive_menu(side);
            }
            return;
        }

        if self.drive_menu.side.is_some() {
            return;
        }

        let mut is_panel_nav = false;
        let mut enter_pressed = false;

        ctx.input(|i| {
            if i.modifiers.alt && i.key_pressed(Key::F7) {
                self.open_search_dialog();
            } else if i.modifiers.alt && i.key_pressed(Key::F5) {
                self.trigger_pack_zip();
            } else if i.modifiers.alt && i.key_pressed(Key::Enter) {
                self.trigger_properties();
            } else if (i.modifiers.shift && i.key_pressed(Key::F6))
                || (!i.modifiers.alt && !i.modifiers.ctrl && i.key_pressed(Key::F2))
            {
                self.trigger_rename();
            } else if i.modifiers.ctrl && i.key_pressed(Key::R) {
                self.left_panel.refresh();
                self.right_panel.refresh();
                self.set_status("Odświeżono zawartość paneli");
            } else if i.modifiers.ctrl && i.key_pressed(Key::U) {
                std::mem::swap(
                    &mut self.left_panel.current_path,
                    &mut self.right_panel.current_path,
                );
                self.left_panel.refresh();
                self.right_panel.refresh();
                self.set_status("Zamieniono ścieżki paneli");
            } else if i.modifiers.ctrl && i.key_pressed(Key::ArrowLeft) {
                let right_path = self.right_panel.current_path.clone();
                self.left_panel.change_dir(right_path);
            } else if i.modifiers.ctrl && i.key_pressed(Key::ArrowRight) {
                let left_path = self.left_panel.current_path.clone();
                self.right_panel.change_dir(left_path);
            } else if i.modifiers.ctrl && i.key_pressed(Key::H) {
                self.left_panel.toggle_hidden();
                self.right_panel.toggle_hidden();
                let state_str = if self.active_panel().show_hidden {
                    "widoczne"
                } else {
                    "ukryte"
                };
                self.set_status(format!("Pliki ukryte: {}", state_str));
            } else if !i.modifiers.ctrl
                && !i.modifiers.alt
                && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
            {
                self.open_mask_select_dialog(true);
            } else if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::Minus) {
                self.open_mask_select_dialog(false);
            } else if !i.modifiers.any() && i.key_pressed(Key::Space) {
                self.calculate_current_dir_size();
            } else if i.modifiers.ctrl && i.key_pressed(Key::F) {
                self.active_panel_mut().is_filter_open = true;
            } else if i.key_pressed(Key::Escape) {
                if !self.active_panel().filter.is_empty() {
                    self.active_panel_mut().clear_filter();
                }
            } else if !i.modifiers.alt && i.key_pressed(Key::F1) {
                self.about_dialog.open_overview();
            } else if i.key_pressed(Key::Tab) {
                is_panel_nav = true;
                self.active_side = match self.active_side {
                    PanelSide::Left => PanelSide::Right,
                    PanelSide::Right => PanelSide::Left,
                };
            } else if i.key_pressed(Key::F3) {
                self.trigger_f3_view();
            } else if i.key_pressed(Key::F4) {
                self.trigger_f4_edit();
            } else if i.key_pressed(Key::F5) {
                self.trigger_f5_copy();
            } else if i.key_pressed(Key::F6) {
                self.trigger_f6_move();
            } else if i.key_pressed(Key::F7) {
                self.trigger_f7_mkdir();
            } else if i.key_pressed(Key::F8) || i.key_pressed(Key::Delete) {
                self.trigger_f8_delete();
            } else if i.key_pressed(Key::Backspace) {
                is_panel_nav = true;
                self.active_panel_mut().go_up();
            } else if i.key_pressed(Key::ArrowUp) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if p.cursor_idx > 0 {
                    p.cursor_idx -= 1;
                    p.scroll_to_cursor = true;
                }
            } else if i.key_pressed(Key::ArrowDown) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if !p.items.is_empty() && p.cursor_idx + 1 < p.items.len() {
                    p.cursor_idx += 1;
                    p.scroll_to_cursor = true;
                }
            } else if i.key_pressed(Key::PageUp) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                p.cursor_idx = p.cursor_idx.saturating_sub(15);
                p.scroll_to_cursor = true;
            } else if i.key_pressed(Key::PageDown) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if !p.items.is_empty() {
                    p.cursor_idx = (p.cursor_idx + 15).min(p.items.len() - 1);
                    p.scroll_to_cursor = true;
                }
            } else if i.key_pressed(Key::Home) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                p.cursor_idx = 0;
                p.scroll_to_cursor = true;
            } else if i.key_pressed(Key::End) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if !p.items.is_empty() {
                    p.cursor_idx = p.items.len() - 1;
                    p.scroll_to_cursor = true;
                }
            } else if i.key_pressed(Key::Insert) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                let idx = p.cursor_idx;
                p.toggle_select(idx);
                if idx + 1 < p.items.len() {
                    p.cursor_idx += 1;
                    p.scroll_to_cursor = true;
                }
            } else if i.key_pressed(Key::Enter) {
                enter_pressed = true;
            }
        });

        if is_panel_nav {
            // Zdejmij focus z menu i zamknij ewentualne otwarte menu rozwijane
            ctx.memory_mut(|mem| {
                mem.stop_text_input();
                mem.close_popup();
            });
        }

        if enter_pressed {
            if !self.cmd_line.trim().is_empty() && ctx.wants_keyboard_input() {
                return;
            }
            // Zdejmij focus z menu oraz skonsumuj klawisz Enter, aby nie aktywował przycisków menu
            ctx.memory_mut(|mem| {
                mem.stop_text_input();
                mem.close_popup();
            });
            let _ = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

            let active = self.active_panel();
            if let Some(item) = active.items.get(active.cursor_idx).cloned() {
                self.open_item(&item);
            }
        }
    }

    fn dispatch_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::TriggerF1About => self.about_dialog.open_overview(),
            MenuAction::OpenLicensesDialog => self.about_dialog.open_license(),
            MenuAction::TriggerF3View => self.trigger_f3_view(),
            MenuAction::TriggerF4Edit => self.trigger_f4_edit(),
            MenuAction::TriggerF5Copy => self.trigger_f5_copy(),
            MenuAction::TriggerF6Move => self.trigger_f6_move(),
            MenuAction::TriggerRename => self.trigger_rename(),
            MenuAction::TriggerPackZip => self.trigger_pack_zip(),
            MenuAction::TriggerProperties => self.trigger_properties(),
            MenuAction::TriggerCalculateDirSize => self.calculate_current_dir_size(),
            MenuAction::TriggerF7Mkdir => self.trigger_f7_mkdir(),
            MenuAction::TriggerF8Delete => self.trigger_f8_delete(),
            MenuAction::ExitApp => std::process::exit(0),
            MenuAction::SelectAll => self.active_panel_mut().select_all(),
            MenuAction::DeselectAll => self.active_panel_mut().deselect_all(),
            MenuAction::InvertSelection => self.active_panel_mut().invert_selection(),
            MenuAction::SelectGroup => self.open_mask_select_dialog(true),
            MenuAction::DeselectGroup => self.open_mask_select_dialog(false),
            MenuAction::OpenSearchDialog => self.open_search_dialog(),
            MenuAction::OpenQueueDialog => self.progress.show_queue_dialog = true,
            MenuAction::ToggleFilter => self.active_panel_mut().is_filter_open = true,
            MenuAction::ToggleHidden => {
                self.left_panel.toggle_hidden();
                self.right_panel.toggle_hidden();
                let state_str = if self.active_panel().show_hidden {
                    "widoczne"
                } else {
                    "ukryte"
                };
                self.set_status(format!("Pliki ukryte: {}", state_str));
            }
            MenuAction::SwapPanels => {
                std::mem::swap(
                    &mut self.left_panel.current_path,
                    &mut self.right_panel.current_path,
                );
                self.left_panel.refresh();
                self.right_panel.refresh();
                self.set_status("Zamieniono ścieżki paneli");
            }
            MenuAction::OpenDriveMenu(side) => self.open_drive_menu(side),
            MenuAction::RefreshPanels => {
                self.left_panel.refresh();
                self.right_panel.refresh();
            }
            MenuAction::OpenConfigDialog => {
                self.config_dialog.open_with_config(&self.config);
            }
            MenuAction::SetLanguage(target_lang) => {
                match target_lang {
                    Some(lang) => {
                        self.i18n.set_language(lang);
                        self.config.language = Some(lang.code().to_string());
                        let msg = self.i18n.t_fmt("status.lang_changed", &[("lang", lang.display_name())]);
                        self.set_status(msg);
                    }
                    None => {
                        let sys_lang = crate::i18n::Language::detect_system_language();
                        self.i18n.set_language(sys_lang);
                        self.config.language = None;
                        let sys_label = self.i18n.t("menu.lang_system").to_string();
                        let msg = self.i18n.t_fmt("status.lang_changed", &[("lang", &sys_label)]);
                        self.set_status(msg);
                    }
                }
                let _ = self.config.save();
            }
            MenuAction::OpenTerminal => {
                let path = self.active_panel().current_path.clone();
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd.exe")
                        .current_dir(&path)
                        .spawn();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let term = std::env::var("TERMINAL").unwrap_or_else(|_| "x-terminal-emulator".to_string());
                    if std::process::Command::new(&term).current_dir(&path).spawn().is_err()
                        && std::process::Command::new("gnome-terminal").current_dir(&path).spawn().is_err()
                        && std::process::Command::new("konsole").current_dir(&path).spawn().is_err()
                    {
                        let _ = std::process::Command::new("xterm").current_dir(&path).spawn();
                    }
                }
            }
        }
    }

    fn dispatch_panel_action(&mut self, action: PanelAction) {
        match action {
            PanelAction::OpenDriveMenu(side) => self.open_drive_menu(side),
            PanelAction::OpenItem(item) => self.open_item(&item),
        }
    }

    fn render_dialogs(&mut self, ctx: &egui::Context) {
        // Mkdir dialog
        let active_dir = self.active_panel().current_path.clone();
        if let Some(act) = render_mkdir_dialog(ctx, &mut self.file_ops, &active_dir, &self.i18n) {
            match act {
                FileOpsAction::CreateDir { path, name } => {
                    match std::fs::create_dir_all(&path) {
                        Ok(()) => {
                            self.active_panel_mut().refresh();
                            if let Some(pos) = self.active_panel().items.iter().position(|it| it.name == name || it.path == path) {
                                self.active_panel_mut().cursor_idx = pos;
                                self.active_panel_mut().scroll_to_cursor = true;
                            }
                            self.passive_panel_mut().refresh();
                            self.set_status(format!("Utworzono katalog: {}", name));
                            ctx.request_repaint();
                        }
                        Err(e) => self.set_status(format!("Błąd tworzenia katalogu: {}", e)),
                    }
                }
                FileOpsAction::EnqueueCommand(cmd) => {
                    self.worker.enqueue(cmd);
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
            }
        }

        // Delete dialog
        if let Some(act) = render_delete_dialog(ctx, &mut self.file_ops, &self.i18n) {
            match act {
                FileOpsAction::EnqueueCommand(cmd) => {
                    self.worker.enqueue(cmd);
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                _ => {}
            }
        }

        // Copy dialog
        let in_zip = self.active_panel().is_in_zip();
        let zip_loc = self.active_panel().zip_location.as_ref().map(|loc| (loc.archive_path.clone(), loc.internal_dir.clone()));
        let is_busy = self.progress.is_open;
        let zip_loc_ref = zip_loc.as_ref().map(|(p, s)| (p.as_path(), s.as_str()));
        if let Some(act) = render_copy_dialog(ctx, &mut self.file_ops, in_zip, zip_loc_ref, is_busy, &self.i18n) {
            match act {
                FileOpsAction::EnqueueCommand(cmd) => {
                    if let WorkerCommand::Copy { ref sources, ref target_dir } = cmd {
                        if let Some(name) = sources.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()) {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.is_open = true;
                    ctx.request_repaint();
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    if let WorkerCommand::Copy { ref sources, ref target_dir } = cmd {
                        if let Some(name) = sources.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()) {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                    self.set_status("Zadanie uruchomione w tle");
                    ctx.request_repaint();
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                _ => {}
            }
        }

        // Move dialog
        if let Some(act) = render_move_dialog(ctx, &mut self.file_ops, is_busy, &self.i18n) {
            match act {
                FileOpsAction::EnqueueCommand(cmd) => {
                    if let WorkerCommand::Move { ref sources, ref target_dir } = cmd {
                        if let Some(name) = sources.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()) {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.is_open = true;
                    ctx.request_repaint();
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    if let WorkerCommand::Move { ref sources, ref target_dir } = cmd {
                        if let Some(name) = sources.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()) {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                    self.set_status("Zadanie przenoszenia w tle");
                    ctx.request_repaint();
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                _ => {}
            }
        }

        // Rename dialog (Shift+F6 / F2)
        if let Some(act) = render_rename_dialog(ctx, &mut self.file_ops, &self.i18n) {
            match act {
                FileOpsAction::EnqueueCommand(cmd) => {
                    self.worker.enqueue(cmd);
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                _ => {}
            }
        }

        // Pack to ZIP dialog (Alt+F5)
        if let Some(act) = render_pack_zip_dialog(ctx, &mut self.file_ops, is_busy, &self.i18n) {
            match act {
                FileOpsAction::EnqueueCommand(cmd) => {
                    self.worker.enqueue(cmd);
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                    self.set_status("Pakowanie archiwum ZIP w tle");
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                _ => {}
            }
        }

        // Mask select / deselect dialog (+ / -)
        if let Some((is_select, pattern)) = render_mask_select_dialog(ctx, &mut self.file_ops, &self.i18n) {
            if is_select {
                self.active_panel_mut().select_by_pattern(&pattern);
                self.set_status(format!("Zaznaczono według wzorca: {}", pattern));
            } else {
                self.active_panel_mut().deselect_by_pattern(&pattern);
                self.set_status(format!("Odznaczono według wzorca: {}", pattern));
            }
        }

        // Properties dialog (Alt+Enter)
        render_properties_dialog(ctx, &mut self.properties, &self.i18n);

        // Drive selection dialog (Alt+F1 / Alt+F2)
        if let Some((side, path)) = render_drive_menu(
            ctx,
            &mut self.drive_menu,
            &self.drives,
            &self.left_panel.current_path,
            &self.right_panel.current_path,
            &self.i18n,
        ) {
            match side {
                PanelSide::Left => self.left_panel.change_dir(path),
                PanelSide::Right => self.right_panel.change_dir(path),
            }
            self.active_side = side;
        }

        // About dialog
        render_about_dialog(ctx, &mut self.about_dialog, &self.i18n);

        // Progress modal
        render_progress_dialog(ctx, &mut self.progress, &self.worker, &self.queue_snapshot, &self.i18n);

        // Queue dialog
        render_queue_dialog(ctx, &mut self.progress, &self.worker, &self.queue_snapshot, &self.i18n);

        // Search dialog (Alt+F7)
        if let Some(search_act) = render_search_dialog(ctx, &mut self.search, &self.i18n) {
            match search_act {
                SearchDialogAction::StartSearch => self.start_search(),
                SearchDialogAction::StopSearch => self.search.stop(),
                SearchDialogAction::JumpTo(res) => self.jump_to_search_result(&res),
                SearchDialogAction::PreviewFile(res) => {
                    if let Err(e) = self.viewer.open_file(res.name, res.path) {
                        self.set_status(format!("Błąd podglądu pliku: {}", e));
                    }
                }
                SearchDialogAction::FeedToListbox(results) => {
                    let count = results.len();
                    self.active_panel_mut().load_search_results(&results);
                    self.search.is_open = false;
                    self.set_status(format!("Zastosowano {} wyników do panelu", count));
                }
                SearchDialogAction::Close => self.search.is_open = false,
            }
        }

        // Viewer window (F3)
        render_viewer_window(ctx, &mut self.viewer, &self.i18n);

        // Config dialog (Skojarzenia edytorów)
        if let Some(act) = crate::ui::dialog_config::render_config_dialog(ctx, &mut self.config_dialog, &self.i18n) {
            match act {
                crate::ui::dialog_config::ConfigDialogAction::Save(new_cfg) => {
                    self.config = new_cfg;
                    if let Err(e) = self.config.save() {
                        self.set_status(format!("Błąd zapisu konfiguracji: {}", e));
                    } else {
                        let msg = self.i18n.t("config.saved").to_string();
                    self.set_status(msg);
                    }
                }
                crate::ui::dialog_config::ConfigDialogAction::Close => {}
            }
        }
    }
}

impl eframe::App for CommanderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_worker_events(ctx);
        self.handle_keyboard_shortcuts(ctx);
        self.poll_search_messages();

        // Keep repainting smoothly while transfer or search is active
        if self.progress.is_open
            || self.search.handle.is_some()
            || self.worker.is_busy()
            || !self.worker.event_rx.is_empty()
        {
            ctx.request_repaint();
        }

        // Top menu bar
        if let Some(act) = render_top_menu_bar(ctx, &self.i18n, self.config.language.as_deref()) {
            self.dispatch_menu_action(act);
        }

        // Bottom function bar (F1 - F10)
        if let Some(act) = render_bottom_f_keys(ctx, &self.i18n) {
            self.dispatch_menu_action(act);
        }

        // Background task bar (shown when transfer is running in background)
        render_background_tasks_bar(ctx, &mut self.progress, &self.worker, &self.queue_snapshot, &self.i18n);

        // Command line & status bar panel
        let active_path = self.active_panel().current_path.to_string_lossy().to_string();
        if render_cli_status_bar(ctx, &active_path, &mut self.cmd_line, self.status_bar_msg.as_ref()) {
            self.execute_cli_cmd();
        }

        // Central dual panels
        let panel_actions = render_dual_panels(
            ctx,
            &mut self.left_panel,
            &mut self.right_panel,
            &mut self.active_side,
            &self.drives,
            &self.i18n,
        );
        for act in panel_actions {
            self.dispatch_panel_action(act);
        }

        self.render_dialogs(ctx);
    }
}
