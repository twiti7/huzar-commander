mod actions;
mod dialogs;
mod shortcuts;

use std::path::PathBuf;
use std::time::Instant;

pub use actions::parse_cd_command;

use crate::clipboard::FileClipboard;
use crate::drives::{scan_drives, DriveInfo};
use crate::panel::PanelState;
use crate::tabs::{PanelTab, PanelTabManager};
use crate::ui::dialog_queue::render_background_tasks_bar;
use crate::ui::menu_bars::{render_bottom_f_keys, render_cli_status_bar, render_top_menu_bar};
use crate::ui::panels::render_dual_panels;
use crate::ui::state::{
    AboutDialogState, BookmarksDialogState, ConflictDialogState, ContextMenuState, DiffDialogState,
    DragDropState, DriveMenuState, FileOpsState, MultiRenameDialogState, ProgressUiState,
    PropertiesDialogState, QuickViewState, SearchDialogState, ViewerWindowState,
};
use crate::watcher::DirWatcher;
use crate::worker::{IoWorker, QueueStateSnapshot, WorkerEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide {
    Left,
    Right,
}

pub struct CommanderApp {
    pub left_panel: PanelTabManager,
    pub right_panel: PanelTabManager,
    pub active_side: PanelSide,
    pub drives: Vec<DriveInfo>,

    // Sub-states
    pub file_ops: FileOpsState,
    pub drive_menu: DriveMenuState,
    pub progress: ProgressUiState,
    pub search: SearchDialogState,
    pub viewer: ViewerWindowState,
    pub quick_view: QuickViewState,
    pub properties: PropertiesDialogState,
    pub config_dialog: crate::ui::state::ConfigDialogState,
    pub about_dialog: AboutDialogState,
    pub bookmarks_dialog: BookmarksDialogState,
    pub multi_rename: MultiRenameDialogState,
    pub diff_dialog: DiffDialogState,
    pub context_menu: ContextMenuState,
    pub conflict: ConflictDialogState,
    pub drag_drop: DragDropState,
    pub clipboard: FileClipboard,
    pub i18n: crate::i18n::I18n,
    pub config: crate::config::AppConfig,

    // Worker & Queue
    pub worker: IoWorker,
    pub queue_snapshot: QueueStateSnapshot,
    pub dir_watcher: DirWatcher,

    // Async ZIP preview & background status
    pub pending_zip_preview: Option<(String, crossbeam_channel::Receiver<Result<PathBuf, String>>)>,
    pub bg_status_tx: crossbeam_channel::Sender<String>,
    pub bg_status_rx: crossbeam_channel::Receiver<String>,

    // Layout & session
    pub split_ratio: f32,
    pub last_window_size: Option<[f32; 2]>,
    pub last_window_maximized: bool,

    // Command line & status
    pub cmd_line: String,
    pub cmd_history: Vec<String>,
    pub cmd_history_idx: Option<usize>,
    pub focus_cli_requested: bool,
    pub status_bar_msg: Option<(String, Instant)>,
    pub last_copy_info: Option<(PathBuf, String)>,
    pub allow_close_with_active_tasks: bool,

    pub egui_ctx: egui::Context,
}

impl CommanderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::ui::fonts::configure_app_fonts(&cc.egui_ctx);

        let drives = scan_drives();
        let fallback_path = if cfg!(target_os = "windows") {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        let current_dir = std::env::current_dir().unwrap_or(fallback_path);

        let config = crate::config::AppConfig::load();
        crate::ui::theme::apply_theme_and_font_size(&cc.egui_ctx, &config.appearance);

        let valid_path_args: Vec<PathBuf> = std::env::args()
            .skip(1)
            .filter(|arg| !arg.starts_with('-'))
            .map(PathBuf::from)
            .collect();

        let mut focus_item = None;
        let left_path = if let Some(p) = valid_path_args.first() {
            if let Some(unc) = crate::panel::normalize_unc_path(&p.to_string_lossy()) {
                unc
            } else if p.is_dir() {
                p.clone()
            } else if p.is_file() {
                focus_item = p.file_name().map(|n| n.to_string_lossy().to_string());
                p.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| current_dir.clone())
            } else {
                current_dir.clone()
            }
        } else if let Some(ref saved) = config.session.left_panel.path {
            let p = PathBuf::from(saved);
            if p.is_dir() {
                p
            } else {
                current_dir.clone()
            }
        } else {
            current_dir.clone()
        };

        let right_path = if let Some(p) = valid_path_args.get(1) {
            if let Some(unc) = crate::panel::normalize_unc_path(&p.to_string_lossy()) {
                unc
            } else if p.is_dir() {
                p.clone()
            } else if drives.len() > 1 {
                drives[1].path.clone()
            } else {
                left_path.clone()
            }
        } else if let Some(ref saved) = config.session.right_panel.path {
            let p = PathBuf::from(saved);
            if p.is_dir() {
                p
            } else if drives.len() > 1 {
                drives[1].path.clone()
            } else {
                left_path.clone()
            }
        } else if drives.len() > 1 {
            drives[1].path.clone()
        } else {
            left_path.clone()
        };

        let initial_lang = if let Some(ref l) = config.language {
            crate::i18n::Language::from_locale_str(l)
                .unwrap_or_else(crate::i18n::Language::detect_system_language)
        } else {
            crate::i18n::Language::detect_system_language()
        };
        let i18n = crate::i18n::I18n::new(initial_lang);

        let worker = IoWorker::new();
        worker.set_language(initial_lang);
        let repaint_ctx = cc.egui_ctx.clone();
        worker.set_repaint_signal(move || {
            repaint_ctx.request_repaint();
        });
        let (bg_status_tx, bg_status_rx) = crossbeam_channel::unbounded();

        let watcher_ctx = cc.egui_ctx.clone();
        let watcher_signal: std::sync::Arc<dyn Fn() + Send + Sync> =
            std::sync::Arc::new(move || {
                watcher_ctx.request_repaint();
            });
        let dir_watcher = DirWatcher::new(
            std::time::Duration::from_millis(250),
            Some(std::sync::Arc::clone(&watcher_signal)),
        );

        let build_tabs = |has_cli_override: bool,
                          fallback_path: PathBuf,
                          session_panel: &crate::config::PanelSessionConfig,
                          focus_item: Option<String>|
         -> PanelTabManager {
            if !has_cli_override && !session_panel.tabs.is_empty() {
                let mut restored = Vec::new();
                let saved_active = session_panel.active_tab;
                for tab_cfg in &session_panel.tabs {
                    let p = PathBuf::from(&tab_cfg.path);
                    if !crate::panel::is_unc_path(&p) && p.is_dir() {
                        let mut panel = if restored.len() == saved_active {
                            PanelState::new(p)
                        } else {
                            let mut def = PanelState::new_deferred(p.clone(), config.session.show_hidden);
                            def.start_loading_dir(
                                p,
                                None,
                                Some(std::sync::Arc::clone(&watcher_signal)),
                            );
                            def
                        };
                        panel.show_hidden = config.session.show_hidden;
                        if let Some(col) = tab_cfg.sort_column {
                            panel.sort_column = col;
                        }
                        if let Some(ord) = tab_cfg.sort_order {
                            panel.sort_order = ord;
                        }
                        panel.sort_items();
                        panel.apply_filter();
                        restored.push(PanelTab {
                            panel,
                            is_locked: tab_cfg.is_locked,
                        });
                    }
                }
                if !restored.is_empty() {
                    return PanelTabManager::from_tabs(restored, saved_active);
                }
            }

            let mut panel = PanelState::new(fallback_path);
            panel.show_hidden = config.session.show_hidden;
            if let Some(col) = session_panel.sort_column {
                panel.sort_column = col;
            }
            if let Some(ord) = session_panel.sort_order {
                panel.sort_order = ord;
            }
            panel.sort_items();
            panel.apply_filter();
            if let Some(target_name) = focus_item {
                if let Some(pos) = panel.items.iter().position(|it| it.name == target_name) {
                    panel.cursor_idx = pos;
                    panel.scroll_to_cursor = true;
                }
            }
            PanelTabManager::new(panel)
        };

        let left_panel = build_tabs(
            !valid_path_args.is_empty(),
            left_path,
            &config.session.left_panel,
            focus_item,
        );
        let right_panel = build_tabs(
            valid_path_args.len() > 1,
            right_path,
            &config.session.right_panel,
            None,
        );

        let split_ratio = config.session.split_ratio.clamp(0.2, 0.8);
        let last_window_maximized = config.session.window_maximized;
        let last_window_size = match (config.session.window_width, config.session.window_height) {
            (Some(w), Some(h)) => Some([w, h]),
            _ => None,
        };

        let mut app = Self {
            left_panel,
            right_panel,
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
            quick_view: QuickViewState::default(),
            properties: PropertiesDialogState::default(),
            config_dialog: crate::ui::state::ConfigDialogState::default(),
            about_dialog: AboutDialogState::default(),
            bookmarks_dialog: BookmarksDialogState::default(),
            multi_rename: MultiRenameDialogState::default(),
            diff_dialog: DiffDialogState::default(),
            context_menu: ContextMenuState::default(),
            conflict: ConflictDialogState::default(),
            drag_drop: DragDropState::default(),
            clipboard: FileClipboard::default(),
            i18n,
            config,

            worker,
            queue_snapshot: QueueStateSnapshot::default(),
            dir_watcher,

            pending_zip_preview: None,
            bg_status_tx,
            bg_status_rx,

            split_ratio,
            last_window_size,
            last_window_maximized,

            cmd_line: String::new(),
            cmd_history: Vec::new(),
            cmd_history_idx: None,
            focus_cli_requested: false,
            status_bar_msg: None,
            last_copy_info: None,
            allow_close_with_active_tasks: false,
            egui_ctx: cc.egui_ctx.clone(),
        };

        let welcome = app.i18n.t("status.welcome").to_string();
        app.set_status(welcome);
        app
    }

    pub(super) fn active_tabs(&self) -> &PanelTabManager {
        match self.active_side {
            PanelSide::Left => &self.left_panel,
            PanelSide::Right => &self.right_panel,
        }
    }

    pub(super) fn active_tabs_mut(&mut self) -> &mut PanelTabManager {
        match self.active_side {
            PanelSide::Left => &mut self.left_panel,
            PanelSide::Right => &mut self.right_panel,
        }
    }

    pub(super) fn active_panel(&self) -> &PanelState {
        self.active_tabs().active()
    }

    pub(super) fn active_panel_mut(&mut self) -> &mut PanelState {
        self.active_tabs_mut().active_mut()
    }

    pub(super) fn passive_panel(&self) -> &PanelState {
        match self.active_side {
            PanelSide::Left => self.right_panel.active(),
            PanelSide::Right => self.left_panel.active(),
        }
    }

    pub(super) fn passive_panel_mut(&mut self) -> &mut PanelState {
        match self.active_side {
            PanelSide::Left => self.right_panel.active_mut(),
            PanelSide::Right => self.left_panel.active_mut(),
        }
    }

    pub(super) fn set_status(&mut self, msg: impl Into<String>) {
        self.status_bar_msg = Some((msg.into(), Instant::now()));
    }

    pub(super) fn is_any_modal_open(&self) -> bool {
        self.file_ops.show_mkdir
            || self.file_ops.show_new_file
            || self.file_ops.show_delete
            || self.file_ops.show_copy
            || self.file_ops.show_move
            || self.file_ops.show_rename
            || self.file_ops.show_pack_zip
            || self.file_ops.show_mask_select
            || self.file_ops.show_exit_confirm
            || self.conflict.is_open
            || self.properties.is_open
            || self.viewer.is_open
            || self.about_dialog.is_open
            || self.bookmarks_dialog.is_open
            || self.multi_rename.is_open
            || self.diff_dialog.is_open
            || self.search.is_open
            || self.config_dialog.is_open
            || self.progress.show_queue_dialog
    }

    pub(super) fn make_repaint_signal(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        let ctx = self.egui_ctx.clone();
        std::sync::Arc::new(move || {
            ctx.request_repaint();
        })
    }

    fn save_panel_session(mgr: &PanelTabManager, target: &mut crate::config::PanelSessionConfig) {
        let active = mgr.active();
        if !active.is_in_zip()
            && !crate::panel::is_unc_path(&active.current_path)
            && active.current_path.is_dir()
        {
            target.path = Some(active.current_path.to_string_lossy().to_string());
        }
        target.sort_column = Some(active.sort_column);
        target.sort_order = Some(active.sort_order);

        let mut saved_tabs = Vec::new();
        let mut mapped_active = 0usize;
        for (idx, tab) in mgr.tabs.iter().enumerate() {
            if !tab.panel.is_in_zip()
                && !crate::panel::is_unc_path(&tab.panel.current_path)
                && tab.panel.current_path.is_dir()
            {
                if idx == mgr.active_idx {
                    mapped_active = saved_tabs.len();
                }
                saved_tabs.push(crate::config::TabSessionConfig {
                    path: tab.panel.current_path.to_string_lossy().to_string(),
                    is_locked: tab.is_locked,
                    sort_column: Some(tab.panel.sort_column),
                    sort_order: Some(tab.panel.sort_order),
                });
            }
        }
        target.tabs = saved_tabs;
        target.active_tab = mapped_active;
    }

    pub(super) fn save_session(&mut self) {
        Self::save_panel_session(&self.left_panel, &mut self.config.session.left_panel);
        Self::save_panel_session(&self.right_panel, &mut self.config.session.right_panel);

        self.config.session.show_hidden = self.left_panel.active().show_hidden;
        self.config.session.split_ratio = self.split_ratio.clamp(0.2, 0.8);
        self.config.session.window_maximized = self.last_window_maximized;
        if !self.last_window_maximized {
            if let Some([w, h]) = self.last_window_size {
                self.config.session.window_width = Some(w);
                self.config.session.window_height = Some(h);
            }
        }
        let _ = self.config.save();
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
                    self.progress.current_item = self.i18n.t("queue.initializing").to_string();
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
                    let msg = self.i18n.t_fmt("status.error_prefix", &[("err", &message)]);
                    self.set_status(msg);
                    needs_refresh = true;
                }
                WorkerEvent::ConflictDetected {
                    source,
                    destination,
                    reply_tx,
                    ..
                } => {
                    let suggested = crate::worker::generate_non_colliding_copy_name_for_lang(
                        &destination.path,
                        self.i18n.language(),
                    );
                    let suggested_name = suggested
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| destination.name.clone());
                    self.conflict = ConflictDialogState {
                        is_open: true,
                        source: Some(source),
                        destination: Some(destination),
                        reply_tx: Some(reply_tx),
                        rename_input: suggested_name,
                        show_rename_input: false,
                    };
                    ctx.request_repaint();
                }
            }
        }

        if needs_refresh {
            self.refresh_both_panels_after_worker();
            ctx.request_repaint();
        }
    }

    pub(super) fn refresh_both_panels_after_worker(&mut self) {
        let repaint_signal = self.make_repaint_signal();
        let copy_info = self.last_copy_info.take();
        let active_side = self.active_side;

        let (left_focus, right_focus) = if let Some((target_dir, first_name)) = copy_info {
            match active_side {
                PanelSide::Left => {
                    let rf = if self.right_panel.current_path == target_dir {
                        Some(first_name)
                    } else {
                        None
                    };
                    (None, rf)
                }
                PanelSide::Right => {
                    let lf = if self.left_panel.current_path == target_dir {
                        Some(first_name)
                    } else {
                        None
                    };
                    (lf, None)
                }
            }
        } else {
            (None, None)
        };

        self.left_panel
            .refresh_async(left_focus, Some(std::sync::Arc::clone(&repaint_signal)));
        self.right_panel
            .refresh_async(right_focus, Some(repaint_signal));
    }

    pub(super) fn refresh_both_panels_async(&mut self) {
        let repaint_signal = self.make_repaint_signal();
        self.left_panel
            .refresh_async(None, Some(std::sync::Arc::clone(&repaint_signal)));
        self.right_panel.refresh_async(None, Some(repaint_signal));
    }

    fn poll_panel_loaders(&mut self) {
        for res in self.left_panel.poll_all_loaders() {
            if let Err(err) = res {
                let msg = self.i18n.t_fmt("status.dir_load_error", &[("err", &err)]);
                self.set_status(msg);
            }
        }
        for res in self.right_panel.poll_all_loaders() {
            if let Err(err) = res {
                let msg = self.i18n.t_fmt("status.dir_load_error", &[("err", &err)]);
                self.set_status(msg);
            }
        }
    }

    fn sync_and_poll_dir_watcher(&mut self) {
        let mut desired = Vec::new();
        let left_active = self.left_panel.active();
        if !left_active.is_in_zip() && left_active.custom_title.is_none() {
            desired.push(left_active.current_path.clone());
        }
        let right_active = self.right_panel.active();
        if !right_active.is_in_zip() && right_active.custom_title.is_none() {
            desired.push(right_active.current_path.clone());
        }
        self.dir_watcher.sync_watched_dirs(&desired);

        if self.worker.is_busy() {
            return;
        }

        let ready = self.dir_watcher.poll_ready_dirs();
        if ready.is_empty() {
            return;
        }

        let sig = self.make_repaint_signal();
        let mut refreshed_left = false;
        let mut refreshed_right = false;
        for dir in ready {
            if !refreshed_left {
                let lp = self.left_panel.active_mut();
                if !lp.is_in_zip()
                    && lp.custom_title.is_none()
                    && !lp.is_loading()
                    && crate::watcher::paths_match(&lp.current_path, &dir)
                {
                    lp.refresh_async(None, Some(std::sync::Arc::clone(&sig)));
                    refreshed_left = true;
                }
            }
            if !refreshed_right {
                let rp = self.right_panel.active_mut();
                if !rp.is_in_zip()
                    && rp.custom_title.is_none()
                    && !rp.is_loading()
                    && crate::watcher::paths_match(&rp.current_path, &dir)
                {
                    rp.refresh_async(None, Some(std::sync::Arc::clone(&sig)));
                    refreshed_right = true;
                }
            }
        }
    }

    fn poll_background_tasks(&mut self) {
        while let Ok(msg) = self.bg_status_rx.try_recv() {
            self.set_status(msg);
        }
        if let Some((ref item_name, ref rx)) = self.pending_zip_preview {
            match rx.try_recv() {
                Ok(Ok(target_file)) => {
                    let name = item_name.clone();
                    self.pending_zip_preview = None;
                    if let Err(e) = self.viewer.open_file(name, target_file) {
                        let msg = self.i18n.t_fmt("status.preview_error", &[("err", &e)]);
                        self.set_status(msg);
                    }
                }
                Ok(Err(e)) => {
                    self.pending_zip_preview = None;
                    let msg = self
                        .i18n
                        .t_fmt("status.zip_extract_preview_error", &[("err", &e)]);
                    self.set_status(msg);
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.pending_zip_preview = None;
                }
            }
        }
    }
}

impl eframe::App for CommanderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested())
            && self.worker.is_busy()
            && !self.allow_close_with_active_tasks
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.file_ops.show_exit_confirm = true;
        }

        let (inner_rect, is_max) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().maximized));
        if let Some(max) = is_max {
            self.last_window_maximized = max;
        }
        if !self.last_window_maximized {
            if let Some(r) = inner_rect {
                if r.width() >= 800.0 && r.height() >= 500.0 {
                    self.last_window_size = Some([r.width(), r.height()]);
                }
            }
        }

        self.poll_panel_loaders();
        self.sync_and_poll_dir_watcher();
        self.poll_dir_size_calculations();
        self.poll_properties_calculation();
        self.poll_background_tasks();
        self.handle_worker_events(ctx);
        self.handle_keyboard_shortcuts(ctx);
        self.poll_search_messages();
        if self.diff_dialog.poll() {
            ctx.request_repaint();
        }

        if self.quick_view.enabled {
            let target = {
                let active = self.active_panel();
                if active.is_in_zip() || active.custom_title.is_some() {
                    None
                } else {
                    active
                        .items
                        .get(active.cursor_idx)
                        .map(|it| (it.path.clone(), it.name.clone(), it.is_dir, it.is_parent))
                }
            };
            self.quick_view.request_preview(target);
            let sig = self.make_repaint_signal();
            self.quick_view.poll(ctx, &self.i18n, Some(sig));
        }

        // Keep repainting smoothly while transfer, search, directory loading or size calculation is active
        if self.progress.is_open
            || self.search.handle.is_some()
            || self.left_panel.any_loading()
            || self.right_panel.any_loading()
            || self.left_panel.any_calculating_dirs()
            || self.right_panel.any_calculating_dirs()
            || (self.properties.is_open
                && (self.properties.is_calculating || self.properties.checksum_calculating))
            || self.pending_zip_preview.is_some()
            || self.worker.is_busy()
            || !self.worker.event_rx.is_empty()
            || (self.viewer.is_open && (self.viewer.search_handle.is_some() || self.viewer.image_loader.is_some()))
            || self.diff_dialog.loading_rx.is_some()
            || self.drag_drop.active_drag.is_some()
            || (self.quick_view.enabled && (self.quick_view.dir_calc_rx.is_some() || self.quick_view.viewer.search_handle.is_some() || self.quick_view.viewer.image_loader.is_some()))
        {
            ctx.request_repaint();
        } else if self.dir_watcher.has_pending()
            || (self.viewer.is_open && self.viewer.tail_mode)
            || (self.quick_view.enabled && (self.quick_view.pending_since.is_some() || self.quick_view.viewer.tail_mode))
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let palette = crate::ui::theme::palette_for(self.config.appearance.theme);

        // Top menu bar
        if let Some(act) = render_top_menu_bar(
            ctx,
            &self.i18n,
            self.config.language.as_deref(),
            self.config.appearance.theme,
            &palette,
        ) {
            self.dispatch_menu_action(ctx, act);
        }

        // Bottom function bar (F1 - F10)
        if let Some(act) = render_bottom_f_keys(ctx, &self.i18n, &palette) {
            self.dispatch_menu_action(ctx, act);
        }

        // Background task bar (shown when transfer is running in background)
        render_background_tasks_bar(
            ctx,
            &mut self.progress,
            &self.worker,
            &self.queue_snapshot,
            &self.i18n,
        );

        // Command line & status bar panel
        let active_path = self.active_panel().current_path.to_string_lossy().to_string();
        if render_cli_status_bar(
            ctx,
            &active_path,
            &mut self.cmd_line,
            &self.cmd_history,
            &mut self.cmd_history_idx,
            &mut self.focus_cli_requested,
            self.status_bar_msg.as_ref(),
            &palette,
        ) {
            self.execute_cli_cmd();
        }

        // Central dual panels
        let ctx_menu_was_open = self.context_menu.is_open;
        let pointer_pos = ctx.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
        let pointer_over_ctx_menu =
            ctx_menu_was_open && pointer_pos.is_some_and(|p| self.context_menu.contains_pos(p));
        let primary_dismiss_ctx_menu = ctx_menu_was_open
            && !pointer_over_ctx_menu
            && ctx.input(|i| {
                i.pointer.primary_pressed() || i.pointer.button_pressed(egui::PointerButton::Middle)
            });
        if primary_dismiss_ctx_menu {
            self.context_menu.close();
        }
        let suppress_panel_clicks = pointer_over_ctx_menu || primary_dismiss_ctx_menu;
        if self.context_menu.is_open || self.is_any_modal_open() {
            self.drag_drop.active_drag = None;
        }

        let cut_paths_owned = self.clipboard.cut_paths().map(|s| s.to_vec());
        let panel_actions = render_dual_panels(
            ctx,
            &mut self.left_panel,
            &mut self.right_panel,
            &mut self.active_side,
            &self.drives,
            &self.i18n,
            suppress_panel_clicks,
            cut_paths_owned.as_deref(),
            &mut self.split_ratio,
            &mut self.quick_view,
            &mut self.drag_drop,
            &palette,
            &self.config.appearance,
        );
        for act in panel_actions {
            self.dispatch_panel_action(act);
        }

        self.render_dialogs(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.config.behavior.save_session_on_exit {
            self.save_session();
        }
        self.search.stop();
        if self.worker.is_busy() {
            self.worker.cancel_all();
        }
        crate::opener::cleanup_preview_temp_dir();
    }
}
