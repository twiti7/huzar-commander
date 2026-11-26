use crate::ui::context_menu::render_context_menu;
use crate::ui::dialog_bookmarks::{render_bookmarks_dialog, BookmarksDialogAction};
use crate::ui::dialog_diff::{render_diff_dialog, DiffDialogAction};
use crate::ui::dialog_multi_rename::{render_multi_rename_dialog, MultiRenameAction};
use crate::ui::dialog_queue::{render_progress_dialog, render_queue_dialog};
use crate::ui::dialog_search::{render_search_dialog, SearchDialogAction};
use crate::ui::file_dialogs::{
    render_about_dialog, render_conflict_dialog, render_copy_dialog, render_delete_dialog,
    render_drive_menu, render_exit_confirm_dialog, render_mask_select_dialog, render_mkdir_dialog,
    render_move_dialog, render_new_file_dialog, render_pack_zip_dialog, render_properties_dialog,
    render_rename_dialog, FileOpsAction, PropertiesDialogAction,
};
use crate::ui::viewer_window::render_viewer_window;
use crate::worker::WorkerCommand;

use super::CommanderApp;

impl CommanderApp {
    pub(super) fn render_dialogs(&mut self, ctx: &egui::Context) {
        // Mkdir dialog
        let active_dir = self.active_panel().current_path.clone();
        if let Some(act) = render_mkdir_dialog(ctx, &mut self.file_ops, &active_dir, &self.i18n) {
            match act {
                FileOpsAction::CreateDir { path, name } => match std::fs::create_dir_all(&path) {
                    Ok(()) => {
                        let repaint_signal = self.make_repaint_signal();
                        self.active_panel_mut().refresh_async(
                            Some(name),
                            Some(std::sync::Arc::clone(&repaint_signal)),
                        );
                        self.passive_panel_mut()
                            .refresh_async(None, Some(repaint_signal));
                        let msg = self.i18n.t("status.dir_created").to_string();
                        self.set_status(msg);
                        ctx.request_repaint();
                    }
                    Err(e) => {
                        let msg = self
                            .i18n
                            .t_fmt("status.dir_create_error", &[("err", &e.to_string())]);
                        self.set_status(msg);
                    }
                },
                FileOpsAction::EnqueueCommand(cmd) => {
                    self.worker.enqueue(cmd);
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                FileOpsAction::CreateFile { .. } => {}
            }
        }

        // New file dialog
        if let Some(FileOpsAction::CreateFile { path, name }) =
            render_new_file_dialog(ctx, &mut self.file_ops, &active_dir, &self.i18n)
        {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => {
                    let repaint_signal = self.make_repaint_signal();
                    self.active_panel_mut().refresh_async(
                        Some(name.clone()),
                        Some(std::sync::Arc::clone(&repaint_signal)),
                    );
                    self.passive_panel_mut()
                        .refresh_async(None, Some(repaint_signal));
                    let msg = self.i18n.t_fmt("status.file_created", &[("name", &name)]);
                    self.set_status(msg);
                    ctx.request_repaint();
                }
                Err(e) => {
                    let msg = self
                        .i18n
                        .t_fmt("status.file_create_error", &[("err", &e.to_string())]);
                    self.set_status(msg);
                }
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
        let zip_loc = self
            .active_panel()
            .zip_location
            .as_ref()
            .map(|loc| (loc.archive_path.clone(), loc.internal_dir.clone()));
        let is_busy = self.progress.is_open;
        let zip_loc_ref = zip_loc.as_ref().map(|(p, s)| (p.as_path(), s.as_str()));
        if let Some(act) = render_copy_dialog(
            ctx,
            &mut self.file_ops,
            in_zip,
            zip_loc_ref,
            is_busy,
            &self.i18n,
        ) {
            match act {
                FileOpsAction::EnqueueCommand(cmd) => {
                    if let WorkerCommand::Copy {
                        ref sources,
                        ref target_dir,
                    } = cmd
                    {
                        if let Some(name) = sources
                            .first()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                        {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.is_open = true;
                    ctx.request_repaint();
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    if let WorkerCommand::Copy {
                        ref sources,
                        ref target_dir,
                    } = cmd
                    {
                        if let Some(name) = sources
                            .first()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                        {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                    let msg = self.i18n.t("status.bg_task_started").to_string();
                    self.set_status(msg);
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
                    if let WorkerCommand::Move {
                        ref sources,
                        ref target_dir,
                    } = cmd
                    {
                        if let Some(name) = sources
                            .first()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                        {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.is_open = true;
                    ctx.request_repaint();
                }
                FileOpsAction::EnqueueCommandWithBackground(cmd) => {
                    if let WorkerCommand::Move {
                        ref sources,
                        ref target_dir,
                    } = cmd
                    {
                        if let Some(name) = sources
                            .first()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                        {
                            self.last_copy_info = Some((target_dir.clone(), name));
                        }
                    }
                    self.worker.enqueue(cmd);
                    self.progress.in_background = true;
                    let msg = self.i18n.t("status.bg_move_started").to_string();
                    self.set_status(msg);
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
                    let msg = self.i18n.t("status.bg_zip_started").to_string();
                    self.set_status(msg);
                }
                FileOpsAction::StatusMessage(msg) => self.set_status(msg),
                _ => {}
            }
        }

        // Mask select / deselect dialog (+ / -)
        if let Some((is_select, pattern)) =
            render_mask_select_dialog(ctx, &mut self.file_ops, &self.i18n)
        {
            if is_select {
                self.active_panel_mut().select_by_pattern(&pattern);
                let msg = self
                    .i18n
                    .t_fmt("status.selected_by_pattern", &[("pattern", &pattern)]);
                self.set_status(msg);
            } else {
                self.active_panel_mut().deselect_by_pattern(&pattern);
                let msg = self
                    .i18n
                    .t_fmt("status.deselected_by_pattern", &[("pattern", &pattern)]);
                self.set_status(msg);
            }
        }

        // Properties dialog (Alt+Enter)
        if let Some(PropertiesDialogAction::CalculateChecksums) =
            render_properties_dialog(ctx, &mut self.properties, &self.i18n)
        {
            let sig = self.make_repaint_signal();
            self.properties.start_checksum_calculation(Some(sig));
            ctx.request_repaint();
        }

        // Drive selection dialog (Alt+F1 / Alt+F2)
        if let Some((side, path)) = render_drive_menu(
            ctx,
            &mut self.drive_menu,
            &self.drives,
            &self.left_panel.current_path,
            &self.right_panel.current_path,
            &self.i18n,
        ) {
            self.navigate_panel_to(side, path);
            self.active_side = side;
        }

        // About dialog
        render_about_dialog(ctx, &mut self.about_dialog, &self.i18n);

        // Progress modal
        render_progress_dialog(
            ctx,
            &mut self.progress,
            &self.worker,
            &self.queue_snapshot,
            &self.i18n,
        );

        // Queue dialog
        render_queue_dialog(
            ctx,
            &mut self.progress,
            &self.worker,
            &self.queue_snapshot,
            &self.i18n,
        );

        // Search dialog (Alt+F7)
        if let Some(search_act) = render_search_dialog(ctx, &mut self.search, &self.i18n) {
            match search_act {
                SearchDialogAction::StartSearch => self.start_search(),
                SearchDialogAction::StopSearch => self.search.stop(),
                SearchDialogAction::JumpTo(res) => self.jump_to_search_result(&res),
                SearchDialogAction::PreviewFile(res) => {
                    if let Err(e) = self.viewer.open_file(res.name, res.path) {
                        let msg = self.i18n.t_fmt("status.preview_error", &[("err", &e)]);
                        self.set_status(msg);
                    }
                }
                SearchDialogAction::FeedToListbox(results) => {
                    let count = results.len();
                    self.active_panel_mut().load_search_results(&results);
                    self.search.is_open = false;
                    let msg = self.i18n.t_fmt(
                        "status.search_results_applied",
                        &[("count", &count.to_string())],
                    );
                    self.set_status(msg);
                }
                SearchDialogAction::Close => self.search.is_open = false,
            }
        }

        // Viewer window (F3)
        render_viewer_window(ctx, &mut self.viewer, &self.i18n);

        // Bookmarks Hotlist dialog (Ctrl+D)
        if let Some(bm_act) = render_bookmarks_dialog(
            ctx,
            &mut self.bookmarks_dialog,
            &self.config.bookmarks,
            &active_dir,
            &self.i18n,
        ) {
            match bm_act {
                BookmarksDialogAction::Navigate(path) => {
                    let side = self.active_side;
                    self.navigate_bookmarks_path(side, path);
                }
                BookmarksDialogAction::NavigateOtherPanel(path) => {
                    let other = match self.active_side {
                        super::PanelSide::Left => super::PanelSide::Right,
                        super::PanelSide::Right => super::PanelSide::Left,
                    };
                    self.navigate_bookmarks_path(other, path);
                }
                BookmarksDialogAction::Add { name, path } => {
                    self.config
                        .bookmarks
                        .push(crate::config::Bookmark { name: name.clone(), path });
                    self.bookmarks_dialog.cursor_idx = self.config.bookmarks.len().saturating_sub(1);
                    let _ = self.config.save();
                    let msg = self
                        .i18n
                        .t_fmt("status.bookmark_added", &[("name", &name)]);
                    self.set_status(msg);
                }
                BookmarksDialogAction::Update { idx, name, path } => {
                    if let Some(bm) = self.config.bookmarks.get_mut(idx) {
                        bm.name = name.clone();
                        bm.path = path;
                        let _ = self.config.save();
                        let msg = self
                            .i18n
                            .t_fmt("status.bookmark_updated", &[("name", &name)]);
                        self.set_status(msg);
                    }
                }
                BookmarksDialogAction::Remove(idx) => {
                    if idx < self.config.bookmarks.len() {
                        let removed = self.config.bookmarks.remove(idx);
                        if self.bookmarks_dialog.editing_idx == Some(idx) {
                            self.bookmarks_dialog.editing_idx = None;
                        }
                        let _ = self.config.save();
                        let msg = self
                            .i18n
                            .t_fmt("status.bookmark_removed", &[("name", &removed.name)]);
                        self.set_status(msg);
                    }
                }
                BookmarksDialogAction::MoveUp(idx) => {
                    if idx > 0 && idx < self.config.bookmarks.len() {
                        self.config.bookmarks.swap(idx, idx - 1);
                        self.bookmarks_dialog.cursor_idx = idx - 1;
                        let _ = self.config.save();
                    }
                }
                BookmarksDialogAction::MoveDown(idx) => {
                    if idx + 1 < self.config.bookmarks.len() {
                        self.config.bookmarks.swap(idx, idx + 1);
                        self.bookmarks_dialog.cursor_idx = idx + 1;
                        let _ = self.config.save();
                    }
                }
                BookmarksDialogAction::Close => {}
            }
        }

        // Config dialog (Skojarzenia edytorów)
        if let Some(act) =
            crate::ui::dialog_config::render_config_dialog(ctx, &mut self.config_dialog, &self.i18n)
        {
            match act {
                crate::ui::dialog_config::ConfigDialogAction::Save(new_cfg) => {
                    self.config.language = new_cfg.language;
                    self.config.default_editor = new_cfg.default_editor;
                    self.config.associations = new_cfg.associations;
                    self.config.appearance = new_cfg.appearance;
                    self.config.behavior = new_cfg.behavior;
                    crate::ui::theme::apply_theme_and_font_size(ctx, &self.config.appearance);
                    if let Err(e) = self.config.save() {
                        let msg = self
                            .i18n
                            .t_fmt("status.config_save_error", &[("err", &e)]);
                        self.set_status(msg);
                    } else {
                        let msg = self.i18n.t("config.saved").to_string();
                        self.set_status(msg);
                    }
                }
                crate::ui::dialog_config::ConfigDialogAction::PreviewAppearance(app_cfg) => {
                    crate::ui::theme::apply_theme_and_font_size(ctx, &app_cfg);
                    ctx.request_repaint();
                }
                crate::ui::dialog_config::ConfigDialogAction::RevertAppearance(orig_cfg) => {
                    crate::ui::theme::apply_theme_and_font_size(ctx, &orig_cfg);
                    ctx.request_repaint();
                }
                crate::ui::dialog_config::ConfigDialogAction::Close => {}
            }
        }

        // Multi-Rename dialog (Ctrl+M)
        if let Some(mr_act) =
            render_multi_rename_dialog(ctx, &mut self.multi_rename, &self.i18n)
        {
            match mr_act {
                MultiRenameAction::Execute(pairs) => {
                    let cnt = pairs.len();
                    self.multi_rename.last_batch = pairs.clone();
                    self.worker.enqueue(WorkerCommand::BatchRename { pairs });
                    self.progress.is_open = true;
                    let msg = self
                        .i18n
                        .t_fmt("status.multi_rename_started", &[("count", &cnt.to_string())]);
                    self.set_status(msg);
                    ctx.request_repaint();
                }
                MultiRenameAction::Undo(pairs) => {
                    let reversed: Vec<(std::path::PathBuf, std::path::PathBuf)> =
                        pairs.into_iter().rev().map(|(old, new)| (new, old)).collect();
                    let cnt = reversed.len();
                    self.multi_rename.last_batch.clear();
                    self.worker
                        .enqueue(WorkerCommand::BatchRename { pairs: reversed });
                    self.progress.is_open = true;
                    let msg = self.i18n.t_fmt(
                        "status.multi_rename_undo_started",
                        &[("count", &cnt.to_string())],
                    );
                    self.set_status(msg);
                    ctx.request_repaint();
                }
                MultiRenameAction::Close => {}
            }
        }

        // Side-by-side File Diff dialog (Ctrl+Shift+D)
        if let Some(DiffDialogAction::Reload) =
            render_diff_dialog(ctx, &mut self.diff_dialog, &self.i18n)
        {
            self.start_diff_load();
        }

        // Context menu
        if let Some(act) = render_context_menu(ctx, &mut self.context_menu, &self.i18n) {
            self.dispatch_context_menu_action(ctx, act);
        }

        // File conflict dialog (Copy / Move overwrite prompt)
        render_conflict_dialog(ctx, &mut self.conflict, &self.i18n);

        // Exit confirmation dialog when background tasks are active
        if render_exit_confirm_dialog(ctx, &mut self.file_ops, &self.i18n) {
            self.worker.cancel_all();
            self.allow_close_with_active_tasks = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
