use std::path::{Component, Path, PathBuf};

use crate::clipboard::ClipboardOp;
use crate::drives::scan_drives;
use crate::fs_item::FileItem;
use crate::ui::context_menu::ContextMenuAction;
use crate::ui::menu_bars::MenuAction;
use crate::ui::panels::PanelAction;
use crate::worker::WorkerCommand;

use super::{CommanderApp, PanelSide};

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn normalize_cd_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    // Keep root unchanged when already at root
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

fn parse_windows_drive_prefix(raw: &str) -> Option<(char, &str)> {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let drive = (bytes[0] as char).to_ascii_uppercase();
        let rest = &raw[2..];
        if rest.is_empty() || rest.starts_with('\\') || rest.starts_with('/') {
            return Some((drive, rest.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

fn normalize_windows_drive_path(drive: char, rest: &str) -> PathBuf {
    let mut stack: Vec<&str> = Vec::new();
    for part in rest.split(['\\', '/']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }
    if stack.is_empty() {
        PathBuf::from(format!("{}:\\", drive))
    } else {
        PathBuf::from(format!("{}:\\{}", drive, stack.join(r"\")))
    }
}

fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        return user_home_dir().unwrap_or_else(|| PathBuf::from(raw));
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix(r"~\")) {
        if let Some(home) = user_home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

/// Parses an interactive directory-change command (`cd ...` or standalone Windows drive `D:`)
/// into a normalized target path relative to `current_dir`. Returns `None` for other shell commands.
pub fn parse_cd_command(cmd: &str, current_dir: &Path) -> Option<PathBuf> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Standalone Windows drive letter switch like "D:" or "c:"
    let bytes = trimmed.as_bytes();
    if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Some(PathBuf::from(format!("{}:\\", (bytes[0] as char).to_ascii_uppercase())));
    }

    // Standalone UNC server root like "\\TRUENAS" or "\\TRUENAS\"
    if crate::panel::unc_server_root(Path::new(trimmed)).is_some() {
        return crate::panel::normalize_unc_path(trimmed);
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower == "cd" {
        return user_home_dir().or_else(|| Some(current_dir.to_path_buf()));
    }

    let after_cd = if lower.starts_with("cd ") || lower.starts_with("cd\t") {
        trimmed[2..].trim_start()
    } else {
        return None;
    };

    // Strip optional Windows "cd /d " flag
    let after_flag = if after_cd.len() >= 3
        && after_cd[..2].eq_ignore_ascii_case("/d")
        && after_cd.as_bytes()[2].is_ascii_whitespace()
    {
        after_cd[2..].trim_start()
    } else {
        after_cd
    };

    let unquoted = if (after_flag.starts_with('"') && after_flag.ends_with('"') && after_flag.len() >= 2)
        || (after_flag.starts_with('\'') && after_flag.ends_with('\'') && after_flag.len() >= 2)
    {
        &after_flag[1..after_flag.len() - 1]
    } else {
        after_flag
    };

    let arg = unquoted.trim();
    if arg.is_empty() || arg == "~" {
        return user_home_dir().or_else(|| Some(current_dir.to_path_buf()));
    }

    if arg == "/" || arg == "\\" {
        if let Some((server, share_opt, _)) = crate::panel::parse_unc_parts(current_dir) {
            if let Some(share) = share_opt {
                return Some(PathBuf::from(format!(r"\\{}\{}", server, share)));
            }
            return Some(PathBuf::from(format!(r"\\{}", server)));
        }
        if let Some((drive, _)) = parse_windows_drive_prefix(&current_dir.to_string_lossy()) {
            return Some(PathBuf::from(format!("{}:\\", drive)));
        }
        let mut root = PathBuf::new();
        for comp in current_dir.components() {
            match comp {
                Component::Prefix(p) => root.push(p.as_os_str()),
                Component::RootDir => {
                    root.push(comp.as_os_str());
                    break;
                }
                _ => break,
            }
        }
        if root.as_os_str().is_empty() {
            return Some(PathBuf::from("/"));
        }
        return Some(root);
    }

    if let Some(unc) = crate::panel::normalize_unc_path(arg) {
        return Some(unc);
    }
    if crate::panel::is_unc_path(current_dir) && !Path::new(arg).is_absolute() && !arg.starts_with('~') {
        let combined = format!(
            r"{}\{}",
            current_dir.to_string_lossy().trim_end_matches(['\\', '/']),
            arg
        );
        return crate::panel::normalize_unc_path(&combined);
    }

    if let Some((drive, rest)) = parse_windows_drive_prefix(arg) {
        return Some(normalize_windows_drive_path(drive, rest));
    }
    if !arg.starts_with('~') {
        if let Some((drive, cwd_rest)) = parse_windows_drive_prefix(&current_dir.to_string_lossy()) {
            if arg.starts_with('\\') || arg.starts_with('/') {
                return Some(normalize_windows_drive_path(drive, arg));
            }
            let combined = format!("{}\\{}", cwd_rest.trim_end_matches(['\\', '/']), arg);
            return Some(normalize_windows_drive_path(drive, &combined));
        }
    }

    let expanded = expand_tilde(arg);
    let candidate = if expanded.is_absolute() {
        expanded
    } else {
        current_dir.join(expanded)
    };
    Some(normalize_cd_path(&candidate))
}

impl CommanderApp {
    pub(super) fn navigate_panel_to(&mut self, side: PanelSide, path: PathBuf) {
        let repaint_signal = self.make_repaint_signal();
        match side {
            PanelSide::Left => self.left_panel.start_loading_dir(path, None, Some(repaint_signal)),
            PanelSide::Right => self.right_panel.start_loading_dir(path, None, Some(repaint_signal)),
        }
    }

    pub(super) fn navigate_panel_to_with_focus(
        &mut self,
        side: PanelSide,
        path: PathBuf,
        focus_name: Option<String>,
    ) {
        let repaint_signal = self.make_repaint_signal();
        match side {
            PanelSide::Left => self
                .left_panel
                .start_loading_dir(path, focus_name, Some(repaint_signal)),
            PanelSide::Right => self
                .right_panel
                .start_loading_dir(path, focus_name, Some(repaint_signal)),
        }
    }

    pub(super) fn navigate_panel_back(&mut self, side: PanelSide) {
        let repaint_signal = self.make_repaint_signal();
        match side {
            PanelSide::Left => {
                self.left_panel.navigate_back_async(Some(repaint_signal));
            }
            PanelSide::Right => {
                self.right_panel.navigate_back_async(Some(repaint_signal));
            }
        }
    }

    pub(super) fn navigate_panel_forward(&mut self, side: PanelSide) {
        let repaint_signal = self.make_repaint_signal();
        match side {
            PanelSide::Left => {
                self.left_panel.navigate_forward_async(Some(repaint_signal));
            }
            PanelSide::Right => {
                self.right_panel.navigate_forward_async(Some(repaint_signal));
            }
        }
    }

    pub(super) fn open_item(&mut self, item: &FileItem) {
        if item.is_parent {
            let repaint_signal = self.make_repaint_signal();
            self.active_panel_mut().go_up_async(Some(repaint_signal));
        } else if self.active_panel().is_in_zip() {
            if item.is_dir {
                let name = item.name.clone();
                let repaint_signal = self.make_repaint_signal();
                self.active_panel_mut()
                    .enter_zip_dir_async(&name, Some(repaint_signal));
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
                    let ok_msg = self
                        .i18n
                        .t_fmt("status.zip_file_opened", &[("name", &file_name)]);
                    let err_tmpl = self.i18n.t("status.zip_file_open_error").to_string();
                    let tx = self.bg_status_tx.clone();
                    let sig = self.make_repaint_signal();
                    std::thread::spawn(move || {
                        let status = match crate::opener::open_zip_entry(&archive_path, &internal_name) {
                            Ok(_) => ok_msg,
                            Err(e) => err_tmpl.replace("{err}", &e),
                        };
                        let _ = tx.send(status);
                        sig();
                    });
                } else {
                    self.trigger_f3_view();
                }
            }
        } else if item.is_dir {
            let path = item.path.clone();
            let repaint_signal = self.make_repaint_signal();
            self.active_panel_mut()
                .start_loading_dir(path, None, Some(repaint_signal));
        } else if crate::archive::is_supported_archive(&item.path) {
            let repaint_signal = self.make_repaint_signal();
            self.active_panel_mut()
                .enter_zip_async(item.path.clone(), Some(repaint_signal));
        } else {
            let name = item.name.clone();
            match crate::opener::open_file(&item.path) {
                Ok(_) => {
                    let msg = self.i18n.t_fmt("status.launched", &[("name", &name)]);
                    self.set_status(msg);
                }
                Err(err) => {
                    self.set_status(err);
                }
            }
        }
    }

    pub(super) fn trigger_f3_view(&mut self) {
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

        // 1. Podgląd plików multimedialnych oraz nieobsługiwanych natywnie formatów graficznych w programie systemowym
        let is_native_img = crate::opener::is_native_preview_image(Path::new(&item_name));
        if (!is_native_img && crate::opener::is_image_file(&item_name))
            || crate::opener::is_media_file(&item_name)
        {
            let is_img = crate::opener::is_image_file(&item_name);
            let kind_label = if is_img {
                self.i18n.t("status.kind_image")
            } else {
                self.i18n.t("status.kind_media")
            };

            if let Some(loc) = zip_info {
                let internal_name = if loc.internal_dir.is_empty() {
                    item_name.clone()
                } else {
                    format!("{}{}", loc.internal_dir, item_name)
                };
                let ok_msg = self.i18n.t_fmt(
                    "status.preview_zip_system",
                    &[("kind", kind_label), ("name", &item_name)],
                );
                let err_tmpl = self.i18n.t("status.zip_file_open_error").to_string();
                let tx = self.bg_status_tx.clone();
                let sig = self.make_repaint_signal();
                std::thread::spawn(move || {
                    let status = match crate::opener::open_zip_entry(&loc.archive_path, &internal_name) {
                        Ok(_) => ok_msg,
                        Err(e) => err_tmpl.replace("{err}", &e),
                    };
                    let _ = tx.send(status);
                    sig();
                });
            } else {
                match crate::opener::open_file(&item_path) {
                    Ok(_) => {
                        let msg = self.i18n.t_fmt(
                            "status.preview_system",
                            &[("kind", kind_label), ("name", &item_name)],
                        );
                        self.set_status(msg);
                    }
                    Err(e) => {
                        let msg = self.i18n.t_fmt("status.preview_error", &[("err", &e)]);
                        self.set_status(msg);
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
            let (tx, rx) = crossbeam_channel::bounded(1);
            let sig = self.make_repaint_signal();
            let extracting_msg = self
                .i18n
                .t_fmt("status.zip_extracting_preview", &[("name", &item_name)]);
            self.set_status(extracting_msg);
            self.pending_zip_preview = Some((item_name, rx));
            std::thread::spawn(move || {
                let res = crate::archive::extract_single_archive_entry(
                    &loc.archive_path,
                    &internal_name,
                    &target_file,
                )
                .map(|_| target_file);
                let _ = tx.send(res);
                sig();
            });
        } else if let Err(e) = self.viewer.open_file(item_name, item_path) {
            let msg = self.i18n.t_fmt("status.preview_error", &[("err", &e)]);
            self.set_status(msg);
        }
    }

    pub(super) fn trigger_f4_edit(&mut self) {
        let active = self.active_panel();
        if let Some(item) = active.items.get(active.cursor_idx) {
            if !item.is_dir && !item.is_parent {
                let ext = item.path.extension().and_then(|s| s.to_str()).unwrap_or("");
                let editor_cmd = self.config.get_editor_for_ext(ext).to_string();
                let path = item.path.clone();
                match crate::opener::open_with_editor(&path, &editor_cmd) {
                    Ok(_) => {
                        let msg = self
                            .i18n
                            .t_fmt("status.opened_in_editor", &[("editor", &editor_cmd)]);
                        self.set_status(msg);
                    }
                    Err(e) => {
                        // Jeśli dedykowany edytor zawiódł (np. brak w PATH), spróbuj domyślnego
                        if editor_cmd != self.config.default_editor && !self.config.default_editor.is_empty() {
                            let fallback = self.config.default_editor.clone();
                            if crate::opener::open_with_editor(&path, &fallback).is_ok() {
                                let msg = self.i18n.t_fmt(
                                    "status.editor_fallback_used",
                                    &[("editor", &editor_cmd), ("fallback", &fallback)],
                                );
                                self.set_status(msg);
                                return;
                            }
                        }
                        let msg = self
                            .i18n
                            .t_fmt("status.editor_error", &[("editor", &editor_cmd), ("err", &e)]);
                        self.set_status(msg);
                    }
                }
            }
        }
    }

    pub(super) fn trigger_f5_copy(&mut self) {
        let active = self.active_panel();
        let sources = active.selected_or_cursor_items();
        if !sources.is_empty() {
            let passive_dir = self.passive_panel().current_path.to_string_lossy().to_string();
            self.file_ops.copy_sources = sources;
            self.file_ops.copy_dest_path = passive_dir;
            self.file_ops.show_copy = true;
        }
    }

    pub(super) fn trigger_f6_move(&mut self) {
        let sources = self.active_panel().selected_or_cursor_items();
        if !sources.is_empty() {
            let passive_dir = self.passive_panel().current_path.to_string_lossy().to_string();
            self.file_ops.move_sources = sources;
            self.file_ops.move_dest_path = passive_dir;
            self.file_ops.show_move = true;
        }
    }

    pub(super) fn trigger_f7_mkdir(&mut self) {
        self.file_ops.mkdir_input.clear();
        self.file_ops.show_mkdir = true;
    }

    pub(super) fn trigger_f8_delete(&mut self) {
        let targets = self.active_panel().selected_or_cursor_items();
        if targets.is_empty() {
            return;
        }
        if !self.config.behavior.confirm_delete {
            self.worker.enqueue(WorkerCommand::Delete {
                targets,
                use_trash: true,
            });
            self.progress.is_open = true;
            self.egui_ctx.request_repaint();
        } else {
            self.file_ops.delete_targets = targets;
            self.file_ops.delete_use_trash = true;
            self.file_ops.show_delete = true;
        }
    }

    pub(super) fn trigger_rename(&mut self) {
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

    pub(super) fn trigger_pack_zip(&mut self) {
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

    pub(super) fn trigger_properties(&mut self) {
        let target = {
            let active = self.active_panel();
            active
                .items
                .get(active.cursor_idx)
                .filter(|item| !item.is_parent)
                .cloned()
        };

        if let Some(item) = target {
            self.open_properties_for_item(&item);
        }
    }

    pub(super) fn trigger_properties_for_path(&mut self, dir_path: PathBuf) {
        let item = FileItem::from_path(&dir_path).unwrap_or_else(|| {
            let name = dir_path.to_string_lossy().to_string();
            let meta = dir_path.metadata().ok();
            let modified = meta.as_ref().and_then(|m| m.modified().ok());
            let is_readonly = meta
                .as_ref()
                .map(|m| m.permissions().readonly())
                .unwrap_or(false);
            FileItem {
                name,
                ext: String::new(),
                path: dir_path,
                is_dir: true,
                is_parent: false,
                size: 0,
                modified,
                is_hidden: false,
                is_readonly,
                is_symlink: false,
            }
        });
        self.open_properties_for_item(&item);
    }

    fn open_properties_for_item(&mut self, item: &FileItem) {
        let modified = item.modified.map(|time| {
            let dt: chrono::DateTime<chrono::Local> = time.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        });
        let path = item.path.clone();
        let (is_calculating, rx, size) = if item.is_dir {
            let known_size = if self.active_panel().is_calculated_dir(&path) {
                item.size
            } else {
                0
            };
            let (tx, rx) = crossbeam_channel::unbounded();
            let dir_path = path.clone();
            let repaint_signal = self.make_repaint_signal();
            std::thread::spawn(move || {
                let mut total_size: u64 = 0;
                let mut files_cnt = 0;
                let mut dirs_cnt = 0;
                for entry in walkdir::WalkDir::new(&dir_path)
                    .min_depth(1)
                    .follow_links(false)
                    .into_iter()
                    .flatten()
                {
                    if entry.file_type().is_dir() {
                        dirs_cnt += 1;
                    } else if entry.file_type().is_file() {
                        files_cnt += 1;
                        if let Ok(meta) = entry.metadata() {
                            total_size = total_size.saturating_add(meta.len());
                        }
                    }
                }
                let _ = tx.send((total_size, files_cnt, dirs_cnt));
                repaint_signal();
            });
            (true, Some(rx), known_size)
        } else {
            (false, None, item.size)
        };

        self.properties.cancel_checksum();
        self.properties = crate::ui::state::PropertiesDialogState {
            is_open: true,
            name: item.name.clone(),
            path,
            is_dir: item.is_dir,
            size,
            files_count: 0,
            dirs_count: 0,
            modified,
            readonly: item.is_readonly,
            hidden: item.is_hidden,
            is_calculating,
            rx,
            ..Default::default()
        };
    }

    pub(super) fn poll_properties_calculation(&mut self) {
        if self.properties.is_open {
            if self.properties.is_calculating {
                if let Some(ref rx) = self.properties.rx {
                    if let Ok((total_size, files, dirs)) = rx.try_recv() {
                        self.properties.size = total_size;
                        self.properties.files_count = files;
                        self.properties.dirs_count = dirs;
                        self.properties.is_calculating = false;
                        self.properties.rx = None;

                        // Also cache in active panel so Size column reflects it!
                        let dir_path = self.properties.path.clone();
                        let active = self.active_panel_mut();
                        if let Some(item) = active.items.iter_mut().find(|it| it.path == dir_path) {
                            item.size = total_size;
                        }
                        if let Some(all_item) = active.all_items.iter_mut().find(|it| it.path == dir_path) {
                            all_item.size = total_size;
                        }
                        active.calculated_dirs.insert(dir_path);
                        active.recalculate_totals();
                    }
                }
            }
            if self.properties.checksum_calculating {
                self.properties.poll_checksum();
            }
        } else {
            if self.properties.is_calculating {
                self.properties.is_calculating = false;
                self.properties.rx = None;
            }
            if self.properties.checksum_calculating {
                self.properties.cancel_checksum();
            }
        }
    }

    pub(super) fn calculate_current_dir_size(&mut self) {
        let repaint_signal = self.make_repaint_signal();
        let active = self.active_panel();
        if let Some(item) = active.items.get(active.cursor_idx) {
            if item.is_dir && !item.is_parent {
                let path = item.path.clone();
                let name = item.name.clone();
                self.active_panel_mut()
                    .start_calculate_dir_size(path, Some(repaint_signal));
                let msg = self
                    .i18n
                    .t_fmt("status.calc_dir_size_start", &[("name", &name)]);
                self.set_status(msg);
            }
        }
    }

    pub(super) fn poll_dir_size_calculations(&mut self) {
        let left_finished = self.left_panel.poll_all_dir_sizes();
        for (path, size, files) in left_finished {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let size_str = humansize::format_size(size, humansize::DECIMAL);
            let files_str = files.to_string();
            let msg = self.i18n.t_fmt(
                "status.calc_dir_size_done_named",
                &[("name", &name), ("size", &size_str), ("files", &files_str)],
            );
            self.set_status(msg);
        }
        let right_finished = self.right_panel.poll_all_dir_sizes();
        for (path, size, files) in right_finished {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let size_str = humansize::format_size(size, humansize::DECIMAL);
            let files_str = files.to_string();
            let msg = self.i18n.t_fmt(
                "status.calc_dir_size_done_named",
                &[("name", &name), ("size", &size_str), ("files", &files_str)],
            );
            self.set_status(msg);
        }
    }

    pub(super) fn open_bookmarks_dialog(&mut self) {
        let active_dir = self.active_panel().current_path.clone();
        self.bookmarks_dialog.open_for_dir(&active_dir);
    }

    pub(super) fn toggle_quick_view(&mut self) {
        self.quick_view.toggle();
        let key = if self.quick_view.enabled {
            "status.quick_view_on"
        } else {
            "status.quick_view_off"
        };
        let msg = self.i18n.t(key).to_string();
        self.set_status(msg);
    }

    pub(super) fn open_multi_rename_dialog(&mut self) {
        if self.active_panel().is_in_zip() {
            let msg = self.i18n.t("status.unsupported_in_zip").to_string();
            self.set_status(msg);
            return;
        }
        let active = self.active_panel();
        let mut items: Vec<FileItem> = if !active.selected_paths.is_empty() {
            active
                .items
                .iter()
                .filter(|it| !it.is_parent && active.selected_paths.contains(&it.path))
                .cloned()
                .collect()
        } else if let Some(it) = active.items.get(active.cursor_idx).filter(|it| !it.is_parent) {
            vec![it.clone()]
        } else {
            Vec::new()
        };

        if items.is_empty() {
            let msg = self.i18n.t("status.multi_rename_empty").to_string();
            self.set_status(msg);
            return;
        }

        self.multi_rename.items.clear();
        self.multi_rename.items.append(&mut items);
        self.multi_rename.is_open = true;
    }

    pub(super) fn trigger_compare_dirs(&mut self) {
        let (l_cnt, r_cnt) = crate::panel::compare_directories(
            self.left_panel.active_mut(),
            self.right_panel.active_mut(),
        );
        let msg = if l_cnt == 0 && r_cnt == 0 {
            self.i18n.t("status.compare_dirs_identical").to_string()
        } else {
            self.i18n.t_fmt(
                "status.compare_dirs_done",
                &[("left", &l_cnt.to_string()), ("right", &r_cnt.to_string())],
            )
        };
        self.set_status(msg);
    }

    pub(super) fn trigger_compare_files(&mut self) {
        let active = self.active_panel();
        let passive = self.passive_panel();

        let selected_files: Vec<PathBuf> = active
            .items
            .iter()
            .filter(|it| !it.is_parent && !it.is_dir && active.selected_paths.contains(&it.path))
            .map(|it| it.path.clone())
            .collect();

        let pair: Option<(PathBuf, PathBuf)> = if selected_files.len() >= 2 {
            Some((selected_files[0].clone(), selected_files[1].clone()))
        } else if let Some(a_item) = active
            .items
            .get(active.cursor_idx)
            .filter(|it| !it.is_parent && !it.is_dir)
        {
            // Match by filename in passive panel first, else cursor file in passive panel
            let p_match = passive
                .items
                .iter()
                .find(|it| !it.is_parent && !it.is_dir && it.name.eq_ignore_ascii_case(&a_item.name))
                .or_else(|| {
                    passive
                        .items
                        .get(passive.cursor_idx)
                        .filter(|it| !it.is_parent && !it.is_dir)
                });
            p_match.map(|b_item| (a_item.path.clone(), b_item.path.clone()))
        } else {
            None
        };

        let Some((left_path, right_path)) = pair else {
            let msg = self.i18n.t("diff.no_files_selected").to_string();
            self.set_status(msg);
            return;
        };

        self.diff_dialog.left_path = Some(left_path);
        self.diff_dialog.right_path = Some(right_path);
        self.diff_dialog.is_open = true;
        self.start_diff_load();
    }

    pub(super) fn start_diff_load(&mut self) {
        let (Some(left), Some(right)) = (
            self.diff_dialog.left_path.clone(),
            self.diff_dialog.right_path.clone(),
        ) else {
            return;
        };
        let ignore_ws = self.diff_dialog.ignore_whitespace;
        let (tx, rx) = crossbeam_channel::bounded(1);
        let sig = self.make_repaint_signal();
        self.diff_dialog.result = None;
        self.diff_dialog.error = None;
        self.diff_dialog.loading_rx = Some(rx);

        std::thread::spawn(move || {
            let res = crate::diff::load_and_diff_files(&left, &right, ignore_ws);
            let _ = tx.send(res);
            sig();
        });
    }

    pub(super) fn navigate_bookmarks_path(&mut self, side: PanelSide, path: PathBuf) {
        let raw = path.to_string_lossy().to_string();
        if let Some(unc) = crate::panel::normalize_unc_path(&raw) {
            self.navigate_panel_to(side, unc);
        } else if path.is_dir() {
            self.navigate_panel_to(side, path);
        } else {
            let msg = self.i18n.t_fmt("status.path_not_found", &[("path", &raw)]);
            self.set_status(msg);
        }
    }

    pub(super) fn open_mask_select_dialog(&mut self, is_select: bool) {
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

    pub(super) fn open_search_dialog(&mut self) {
        self.search.dir = self.active_panel().current_path.to_string_lossy().to_string();
        if self.search.mask.is_empty() {
            self.search.mask = "*".to_string();
        }
        self.search.is_open = true;
    }

    pub(super) fn start_search(&mut self) {
        self.search.stop();
        self.search.results.clear();
        self.search.selected_idx = None;
        self.search.scanned_count = 0;
        self.search.status_text = self.i18n.t("status.search_searching").to_string();

        let params = crate::search::SearchParams {
            start_dir: PathBuf::from(self.search.dir.trim()),
            mask: self.search.mask.clone(),
            text: self.search.text.clone(),
            case_sensitive: self.search.case_sensitive,
            recursive: self.search.recursive,
        };
        self.search.handle = Some(crate::search::start_search(params));
    }

    pub(super) fn poll_search_messages(&mut self) {
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
                        self.search.status_text = self.i18n.t_fmt(
                            "status.search_finished",
                            &[
                                ("scanned", &self.search.scanned_count.to_string()),
                                ("found", &found.to_string()),
                            ],
                        );
                        self.search.handle = None;
                        break;
                    }
                    crate::search::SearchMessage::Error(err) => {
                        self.search.status_text =
                            self.i18n.t_fmt("status.search_error", &[("err", &err)]);
                        self.search.handle = None;
                        break;
                    }
                }
            }
        }
    }

    pub(super) fn jump_to_search_result(&mut self, res: &crate::search::SearchResult) {
        if let Some(parent) = res.path.parent() {
            let repaint_signal = self.make_repaint_signal();
            let file_name = res.name.clone();
            self.active_panel_mut().start_loading_dir(
                parent.to_path_buf(),
                Some(file_name),
                Some(repaint_signal),
            );
        }
        self.search.is_open = false;
    }

    pub(super) fn execute_cli_cmd(&mut self) {
        let cmd = self.cmd_line.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        if self.cmd_history.last() != Some(&cmd) {
            self.cmd_history.push(cmd.clone());
            if self.cmd_history.len() > 100 {
                self.cmd_history.remove(0);
            }
        }
        self.cmd_history_idx = None;

        let active_dir = self.active_panel().current_path.clone();
        if let Some(target) = parse_cd_command(&cmd, &active_dir) {
            self.cmd_line.clear();
            if crate::panel::is_unc_path(&target) || target.is_dir() {
                self.navigate_panel_to(self.active_side, target);
            } else {
                let target_s = target.to_string_lossy().to_string();
                let msg = self
                    .i18n
                    .t_fmt("status.cd_not_found", &[("path", &target_s)]);
                self.set_status(msg);
            }
            return;
        }

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

        let msg = self.i18n.t_fmt("status.cli_executed", &[("cmd", &cmd)]);
        self.set_status(msg);
        self.cmd_line.clear();
        let repaint_signal = self.make_repaint_signal();
        self.active_panel_mut()
            .refresh_async(None, Some(repaint_signal));
    }

    pub(super) fn open_drive_menu(&mut self, side: PanelSide) {
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

    pub(super) fn swap_panels(&mut self) {
        std::mem::swap(&mut self.left_panel, &mut self.right_panel);
        self.left_panel.scroll_to_cursor = true;
        self.right_panel.scroll_to_cursor = true;
        let msg = self.i18n.t("status.panels_swapped").to_string();
        self.set_status(msg);
    }

    pub(super) fn request_exit(&mut self, ctx: &egui::Context) {
        if self.worker.is_busy() && !self.allow_close_with_active_tasks {
            self.file_ops.show_exit_confirm = true;
        } else {
            self.allow_close_with_active_tasks = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub(super) fn dispatch_menu_action(&mut self, ctx: &egui::Context, action: MenuAction) {
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
            MenuAction::ExitApp => self.request_exit(ctx),
            MenuAction::SelectAll => self.active_panel_mut().select_all(),
            MenuAction::DeselectAll => self.active_panel_mut().deselect_all(),
            MenuAction::InvertSelection => self.active_panel_mut().invert_selection(),
            MenuAction::SelectGroup => self.open_mask_select_dialog(true),
            MenuAction::DeselectGroup => self.open_mask_select_dialog(false),
            MenuAction::OpenSearchDialog => self.open_search_dialog(),
            MenuAction::OpenBookmarks => self.open_bookmarks_dialog(),
            MenuAction::ToggleQuickView => self.toggle_quick_view(),
            MenuAction::OpenMultiRename => self.open_multi_rename_dialog(),
            MenuAction::CompareDirs => self.trigger_compare_dirs(),
            MenuAction::CompareFiles => self.trigger_compare_files(),
            MenuAction::NewTab => {
                self.active_tabs_mut().duplicate_active_tab();
                let msg = self.i18n.t("status.tab_created").to_string();
                self.set_status(msg);
            }
            MenuAction::CloseTab => {
                if self.active_tabs_mut().close_active_tab() {
                    let msg = self.i18n.t("status.tab_closed").to_string();
                    self.set_status(msg);
                } else if self.active_tabs().len() > 1 {
                    let msg = self.i18n.t("status.tab_locked_cannot_close").to_string();
                    self.set_status(msg);
                }
            }
            MenuAction::OpenQueueDialog => self.progress.show_queue_dialog = true,
            MenuAction::ToggleFilter => self.active_panel_mut().is_filter_open = true,
            MenuAction::ToggleHidden => {
                self.left_panel.toggle_hidden_all();
                self.right_panel.toggle_hidden_all();
                let state_str = if self.active_panel().show_hidden {
                    self.i18n.t("status.hidden_visible")
                } else {
                    self.i18n.t("status.hidden_hidden")
                };
                let msg = self
                    .i18n
                    .t_fmt("status.hidden_status", &[("state", state_str)]);
                self.set_status(msg);
            }
            MenuAction::SwapPanels => self.swap_panels(),
            MenuAction::OpenDriveMenu(side) => self.open_drive_menu(side),
            MenuAction::RefreshPanels => {
                self.refresh_both_panels_async();
            }
            MenuAction::OpenConfigDialog => {
                self.config_dialog.open_with_config(&self.config);
            }
            MenuAction::SetLanguage(target_lang) => {
                match target_lang {
                    Some(lang) => {
                        self.i18n.set_language(lang);
                        self.worker.set_language(lang);
                        self.config.language = Some(lang.code().to_string());
                        let msg = self
                            .i18n
                            .t_fmt("status.lang_changed", &[("lang", lang.display_name())]);
                        self.set_status(msg);
                    }
                    None => {
                        let sys_lang = crate::i18n::Language::detect_system_language();
                        self.i18n.set_language(sys_lang);
                        self.worker.set_language(sys_lang);
                        self.config.language = None;
                        let sys_label = self.i18n.t("menu.lang_system").to_string();
                        let msg = self
                            .i18n
                            .t_fmt("status.lang_changed", &[("lang", &sys_label)]);
                        self.set_status(msg);
                    }
                }
                let _ = self.config.save();
            }
            MenuAction::SetTheme(theme) => {
                self.config.appearance.theme = theme;
                crate::ui::theme::apply_theme_and_font_size(ctx, &self.config.appearance);
                let _ = self.config.save();
                let theme_name = self
                    .i18n
                    .t(crate::ui::menu_bars::theme_label_key(theme))
                    .to_string();
                let msg = self
                    .i18n
                    .t_fmt("status.theme_changed", &[("theme", &theme_name)]);
                self.set_status(msg);
            }
            MenuAction::OpenTerminal => {
                let path = self.active_panel().current_path.clone();
                if let Err(e) = crate::opener::open_terminal(&path) {
                    self.set_status(e);
                }
            }
        }
    }

    pub(super) fn open_context_menu(&mut self, side: PanelSide, pos: egui::Pos2, row_idx: Option<usize>) {
        self.active_side = side;
        let can_paste = self.clipboard.has_files();

        let (
            in_zip,
            show_hidden,
            sort_column,
            sort_order,
            current_path,
            has_selection,
            target_item,
        ) = {
            let panel = self.active_panel_mut();
            let target_item = if let Some(idx) = row_idx {
                if let Some(it) = panel.items.get(idx).cloned() {
                    if !it.is_parent {
                        if !panel.selected_paths.contains(&it.path) {
                            panel.deselect_all();
                        }
                        panel.cursor_idx = idx;
                        let sel_cnt = panel.selected_or_cursor_items().len();
                        Some((idx, it, sel_cnt))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            (
                panel.is_in_zip(),
                panel.show_hidden,
                panel.sort_column,
                panel.sort_order,
                panel.current_path.clone(),
                !panel.selected_paths.is_empty(),
                target_item,
            )
        };

        if let Some((idx, item, selected_count)) = target_item {
            self.context_menu.open_for_item(
                side,
                pos,
                idx,
                item,
                selected_count,
                in_zip,
                can_paste,
                show_hidden,
                sort_column,
                sort_order,
            );
        } else {
            self.context_menu.open_for_background(
                side,
                pos,
                current_path,
                in_zip,
                can_paste,
                has_selection,
                show_hidden,
                sort_column,
                sort_order,
            );
        }
    }

    pub(super) fn trigger_clipboard_copy(&mut self) {
        let sources = self.active_panel().selected_or_cursor_items();
        if sources.is_empty() {
            return;
        }
        let count = sources.len();
        match self.clipboard.set_files(sources, ClipboardOp::Copy) {
            Ok(()) => {
                let msg = self
                    .i18n
                    .t_fmt("status.clipboard_copied", &[("count", &count.to_string())]);
                self.set_status(msg);
            }
            Err(e) => self.set_status(e),
        }
    }

    pub(super) fn trigger_clipboard_cut(&mut self) {
        let sources = self.active_panel().selected_or_cursor_items();
        if sources.is_empty() {
            return;
        }
        let count = sources.len();
        match self.clipboard.set_files(sources, ClipboardOp::Cut) {
            Ok(()) => {
                let msg = self
                    .i18n
                    .t_fmt("status.clipboard_cut", &[("count", &count.to_string())]);
                self.set_status(msg);
            }
            Err(e) => self.set_status(e),
        }
    }

    pub(super) fn trigger_clipboard_paste(&mut self, target_dir: PathBuf) {
        if let Some((sources, op)) = self.clipboard.get_files() {
            let existing: Vec<PathBuf> = sources.into_iter().filter(|p| p.exists()).collect();
            if existing.is_empty() {
                let msg = self.i18n.t("status.clipboard_empty_existing").to_string();
                self.set_status(msg);
                return;
            }
            if let Some(first_name) = existing
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
            {
                self.last_copy_info = Some((target_dir.clone(), first_name));
            }
            match op {
                ClipboardOp::Copy => {
                    self.worker.enqueue(WorkerCommand::Copy {
                        sources: existing,
                        target_dir,
                    });
                }
                ClipboardOp::Cut => {
                    self.worker.enqueue(WorkerCommand::Move {
                        sources: existing,
                        target_dir,
                    });
                    self.clipboard.clear();
                }
            }
            self.progress.is_open = true;
            self.egui_ctx.request_repaint();
        }
    }

    fn trigger_new_text_file(&mut self, dir: PathBuf) {
        let base_name = self.i18n.t("ctx_menu.new_text_file_default_name").to_string();
        let new_path = crate::opener::generate_non_colliding_file_name(&dir, &base_name, "txt");
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_path)
        {
            Ok(_) => {
                let file_name = new_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let repaint_signal = self.make_repaint_signal();
                self.active_panel_mut().refresh_async(
                    Some(file_name.clone()),
                    Some(std::sync::Arc::clone(&repaint_signal)),
                );
                self.passive_panel_mut()
                    .refresh_async(None, Some(repaint_signal));
                let msg = self.i18n.t_fmt("status.file_created", &[("name", &file_name)]);
                self.set_status(msg);
                self.file_ops.rename_source = Some(new_path);
                self.file_ops.rename_input = file_name;
                self.file_ops.show_rename = true;
            }
            Err(e) => {
                let msg = self
                    .i18n
                    .t_fmt("status.file_create_error", &[("err", &e.to_string())]);
                self.set_status(msg);
            }
        }
    }

    pub(super) fn dispatch_context_menu_action(&mut self, ctx: &egui::Context, action: ContextMenuAction) {
        match action {
            ContextMenuAction::OpenItem(item) => self.open_item(&item),
            ContextMenuAction::OpenInOtherPanel(dir) => {
                let other_side = match self.active_side {
                    PanelSide::Left => PanelSide::Right,
                    PanelSide::Right => PanelSide::Left,
                };
                self.navigate_panel_to(other_side, dir);
            }
            ContextMenuAction::OpenWith(path) => {
                if let Err(e) = crate::opener::open_with_dialog(&path) {
                    self.set_status(e);
                }
            }
            ContextMenuAction::RunAsAdmin(path) => {
                if let Err(e) = crate::opener::run_as_administrator(&path) {
                    self.set_status(e);
                }
            }
            ContextMenuAction::ViewF3 => self.trigger_f3_view(),
            ContextMenuAction::EditF4 => self.trigger_f4_edit(),
            ContextMenuAction::Cut => self.trigger_clipboard_cut(),
            ContextMenuAction::Copy => self.trigger_clipboard_copy(),
            ContextMenuAction::Paste(target_dir) => self.trigger_clipboard_paste(target_dir),
            ContextMenuAction::CopyAsPath(paths) => {
                let targets = if paths.is_empty() {
                    let active = self.active_panel();
                    let ordered: Vec<PathBuf> = active
                        .items
                        .iter()
                        .filter(|it| !it.is_parent && active.selected_paths.contains(&it.path))
                        .map(|it| it.path.clone())
                        .collect();
                    if ordered.is_empty() {
                        active.selected_or_cursor_items()
                    } else {
                        ordered
                    }
                } else {
                    paths
                };
                let joined = targets
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("\n");
                ctx.copy_text(joined);
                let msg = self.i18n.t("status.path_copied").to_string();
                self.set_status(msg);
            }
            ContextMenuAction::CopyName(name) => {
                ctx.copy_text(name);
                let msg = self.i18n.t("status.name_copied").to_string();
                self.set_status(msg);
            }
            ContextMenuAction::CopyToOtherPanelF5 => self.trigger_f5_copy(),
            ContextMenuAction::MoveToOtherPanelF6 => self.trigger_f6_move(),
            ContextMenuAction::PackZip => self.trigger_pack_zip(),
            ContextMenuAction::ExtractZipHere(archive) => {
                let target_dir = self.active_panel().current_path.clone();
                self.worker.enqueue(WorkerCommand::ExtractZip {
                    archive_path: archive,
                    target_dir,
                    filter_prefix: None,
                    selected_entries: Vec::new(),
                });
                self.progress.is_open = true;
                ctx.request_repaint();
            }
            ContextMenuAction::ExtractZipToFolder(archive) => {
                let stem = crate::archive::ArchiveFormat::strip_archive_stem(&archive);
                let target_dir = self.active_panel().current_path.join(stem);
                self.worker.enqueue(WorkerCommand::ExtractZip {
                    archive_path: archive,
                    target_dir,
                    filter_prefix: None,
                    selected_entries: Vec::new(),
                });
                self.progress.is_open = true;
                ctx.request_repaint();
            }
            ContextMenuAction::CalculateDirSize => self.calculate_current_dir_size(),
            ContextMenuAction::Rename => self.trigger_rename(),
            ContextMenuAction::Delete => self.trigger_f8_delete(),
            ContextMenuAction::NewFolder => self.trigger_f7_mkdir(),
            ContextMenuAction::NewTextFile(dir) => self.trigger_new_text_file(dir),
            ContextMenuAction::NewCustomFile => {
                self.file_ops.new_file_input.clear();
                self.file_ops.show_new_file = true;
            }
            ContextMenuAction::Refresh => {
                let repaint_signal = self.make_repaint_signal();
                self.active_panel_mut()
                    .refresh_async(None, Some(repaint_signal));
            }
            ContextMenuAction::SortBy(col) => {
                let active = self.active_panel_mut();
                if active.sort_column == col {
                    active.sort_order = match active.sort_order {
                        crate::fs_item::SortOrder::Ascending => crate::fs_item::SortOrder::Descending,
                        crate::fs_item::SortOrder::Descending => crate::fs_item::SortOrder::Ascending,
                    };
                } else {
                    active.sort_column = col;
                    active.sort_order = crate::fs_item::SortOrder::Ascending;
                }
                active.sort_items();
            }
            ContextMenuAction::SetSortOrder(ord) => {
                let active = self.active_panel_mut();
                active.sort_order = ord;
                active.sort_items();
            }
            ContextMenuAction::SelectAll => self.active_panel_mut().select_all(),
            ContextMenuAction::DeselectAll => self.active_panel_mut().deselect_all(),
            ContextMenuAction::ToggleHidden => {
                self.left_panel.toggle_hidden_all();
                self.right_panel.toggle_hidden_all();
                let state_str = if self.active_panel().show_hidden {
                    self.i18n.t("status.hidden_visible")
                } else {
                    self.i18n.t("status.hidden_hidden")
                };
                let msg = self
                    .i18n
                    .t_fmt("status.hidden_status", &[("state", state_str)]);
                self.set_status(msg);
            }
            ContextMenuAction::OpenTerminal(dir) => {
                if let Err(e) = crate::opener::open_terminal(&dir) {
                    self.set_status(e);
                }
            }
            ContextMenuAction::ShowInExplorer { path, select_item } => {
                if let Err(e) = crate::opener::show_in_explorer(&path, select_item) {
                    self.set_status(e);
                }
            }
            ContextMenuAction::Properties(None) => self.trigger_properties(),
            ContextMenuAction::Properties(Some(dir)) => self.trigger_properties_for_path(dir),
            ContextMenuAction::SystemProperties(path) => {
                if let Err(e) = crate::opener::open_system_properties(&path) {
                    self.set_status(e);
                }
            }
        }
    }

    pub(super) fn dispatch_panel_action(&mut self, action: PanelAction) {
        match action {
            PanelAction::OpenDriveMenu(side) => self.open_drive_menu(side),
            PanelAction::OpenItem(item) => self.open_item(&item),
            PanelAction::ChangeDir(side, path) => self.navigate_panel_to(side, path),
            PanelAction::CancelLoading(side) => {
                let p = match side {
                    PanelSide::Left => &mut self.left_panel,
                    PanelSide::Right => &mut self.right_panel,
                };
                if p.cancel_loading() {
                    let msg = self.i18n.t("panels.loading_cancelled").to_string();
                    self.set_status(msg);
                }
            }
            PanelAction::GoUp(side) => {
                let repaint_signal = self.make_repaint_signal();
                let p = match side {
                    PanelSide::Left => &mut self.left_panel,
                    PanelSide::Right => &mut self.right_panel,
                };
                p.go_up_async(Some(repaint_signal));
            }
            PanelAction::GoBack(side) => self.navigate_panel_back(side),
            PanelAction::GoForward(side) => self.navigate_panel_forward(side),
            PanelAction::SelectTab(side, idx) => {
                self.active_side = side;
                match side {
                    PanelSide::Left => self.left_panel.switch_to(idx),
                    PanelSide::Right => self.right_panel.switch_to(idx),
                }
            }
            PanelAction::CloseTab(side, idx) => {
                self.active_side = side;
                let (closed, count) = match side {
                    PanelSide::Left => (self.left_panel.close_tab(idx), self.left_panel.len()),
                    PanelSide::Right => (self.right_panel.close_tab(idx), self.right_panel.len()),
                };
                if closed {
                    let msg = self.i18n.t("status.tab_closed").to_string();
                    self.set_status(msg);
                } else if count > 1 {
                    let msg = self.i18n.t("status.tab_locked_cannot_close").to_string();
                    self.set_status(msg);
                }
            }
            PanelAction::NewTab(side) => {
                self.active_side = side;
                match side {
                    PanelSide::Left => {
                        self.left_panel.duplicate_active_tab();
                    }
                    PanelSide::Right => {
                        self.right_panel.duplicate_active_tab();
                    }
                }
                let msg = self.i18n.t("status.tab_created").to_string();
                self.set_status(msg);
            }
            PanelAction::ToggleLockTab(side, idx) => {
                self.active_side = side;
                let mgr = match side {
                    PanelSide::Left => &mut self.left_panel,
                    PanelSide::Right => &mut self.right_panel,
                };
                if let Some(locked) = mgr.toggle_lock(idx) {
                    let name = mgr
                        .tabs
                        .get(idx)
                        .map(crate::tabs::PanelTabManager::tab_title)
                        .unwrap_or_default();
                    let key = if locked {
                        "status.tab_lock_on"
                    } else {
                        "status.tab_lock_off"
                    };
                    let msg = self.i18n.t_fmt(key, &[("name", &name)]);
                    self.set_status(msg);
                }
            }
            PanelAction::OpenBookmarks(side) => {
                self.active_side = side;
                self.open_bookmarks_dialog();
            }
            PanelAction::CloseQuickView => {
                if self.quick_view.enabled {
                    self.toggle_quick_view();
                }
            }
            PanelAction::OpenQuickViewInF3 => {
                self.trigger_f3_view();
            }
            PanelAction::NavigateToInputPath(side, raw) => {
                let trimmed = raw.trim();
                let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
                    || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
                {
                    &trimmed[1..trimmed.len() - 1]
                } else {
                    trimmed
                };
                if let Some(unc_target) = crate::panel::normalize_unc_path(unquoted) {
                    self.navigate_panel_to(side, unc_target);
                } else {
                    let target = expand_tilde(unquoted);
                    if target.is_dir() {
                        self.navigate_panel_to(side, target);
                    } else if crate::archive::is_supported_archive(&target) {
                        let sig = self.make_repaint_signal();
                        match side {
                            PanelSide::Left => self.left_panel.enter_zip_async(target, Some(sig)),
                            PanelSide::Right => self.right_panel.enter_zip_async(target, Some(sig)),
                        }
                    } else if target.is_file() {
                        if let Some(parent) = target.parent() {
                            let focus = target.file_name().map(|n| n.to_string_lossy().to_string());
                            self.navigate_panel_to_with_focus(side, parent.to_path_buf(), focus);
                        }
                    } else {
                        let path_s = target.to_string_lossy().to_string();
                        let msg = self.i18n.t_fmt("status.path_not_found", &[("path", &path_s)]);
                        self.set_status(msg);
                    }
                }
            }
            PanelAction::OpenContextMenu { side, pos, row_idx } => {
                self.open_context_menu(side, pos, row_idx);
            }
            PanelAction::DropFiles {
                sources,
                target_dir,
                is_move,
            } => {
                let existing: Vec<PathBuf> = sources.into_iter().filter(|p| p.exists()).collect();
                if existing.is_empty() {
                    return;
                }
                if let Some(first_name) = existing
                    .first()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                {
                    self.last_copy_info = Some((target_dir.clone(), first_name));
                }
                let count_str = existing.len().to_string();
                let msg = if is_move {
                    self.worker.enqueue(WorkerCommand::Move {
                        sources: existing,
                        target_dir,
                    });
                    self.i18n
                        .t_fmt("status.drag_drop_move", &[("count", &count_str)])
                } else {
                    self.worker.enqueue(WorkerCommand::Copy {
                        sources: existing,
                        target_dir,
                    });
                    self.i18n
                        .t_fmt("status.drag_drop_copy", &[("count", &count_str)])
                };
                self.set_status(msg);
                self.progress.is_open = true;
                self.egui_ctx.request_repaint();
            }
        }
    }
}
