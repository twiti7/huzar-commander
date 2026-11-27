use std::path::PathBuf;

use chrono::{DateTime, Local};
use egui::{Color32, RichText, Stroke};
use egui_extras::{Column, TableBuilder};

use crate::app::PanelSide;
use crate::config::{format_file_size, AppearanceConfig};
use crate::drives::DriveInfo;
use crate::fs_item::{FileItem, SortColumn, SortOrder};
use crate::i18n::I18n;
use crate::tabs::PanelTabManager;
use crate::ui::state::{DragDropState, InternalDragPayload, QuickViewState};
use crate::ui::theme::ThemePalette;
use crate::ui::viewer_window::{render_quick_view_panel, QuickViewAction};

pub enum PanelAction {
    OpenDriveMenu(PanelSide),
    OpenItem(FileItem),
    ChangeDir(PanelSide, std::path::PathBuf),
    CancelLoading(PanelSide),
    GoUp(PanelSide),
    GoBack(PanelSide),
    GoForward(PanelSide),
    NavigateToInputPath(PanelSide, String),
    SelectTab(PanelSide, usize),
    CloseTab(PanelSide, usize),
    NewTab(PanelSide),
    ToggleLockTab(PanelSide, usize),
    OpenBookmarks(PanelSide),
    CloseQuickView,
    OpenQuickViewInF3,
    OpenContextMenu {
        side: PanelSide,
        pos: egui::Pos2,
        row_idx: Option<usize>,
    },
    DropFiles {
        sources: Vec<PathBuf>,
        target_dir: PathBuf,
        is_move: bool,
    },
}

struct PanelDragContext<'a> {
    is_drop_target: bool,
    drag_active: Option<&'a InternalDragPayload>,
    drag_start_out: &'a mut Option<(Vec<PathBuf>, egui::Pos2)>,
    hovered_folder_out: &'a mut Option<PathBuf>,
}

fn panel_can_accept_drop(panel: &crate::panel::PanelState) -> bool {
    !panel.is_in_zip()
        && panel.custom_title.is_none()
        && crate::panel::unc_server_root(&panel.current_path).is_none()
}

/// Splits a filesystem path into clickable `(label, full_path)` breadcrumb segments.
pub fn build_breadcrumb_segments(path: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let raw = path.to_string_lossy();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();

    // 1. Windows UNC path (\\server, \\server\share, \\server\share\dir...)
    if let Some((server, share_opt, rest)) = crate::panel::parse_unc_parts(path) {
        let mut acc = format!(r"\\{}", server);
        segments.push((acc.clone(), std::path::PathBuf::from(&acc)));
        if let Some(share) = share_opt {
            acc.push('\\');
            acc.push_str(&share);
            segments.push((share, std::path::PathBuf::from(&acc)));
            for part in rest {
                acc.push('\\');
                acc.push_str(&part);
                segments.push((part, std::path::PathBuf::from(&acc)));
            }
        }
        return segments;
    }

    // 2. Windows drive path (C:\...)
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let mut acc = format!("{}:\\", (bytes[0] as char).to_ascii_uppercase());
        segments.push((acc.clone(), std::path::PathBuf::from(&acc)));
        let rest = trimmed[2..].trim_start_matches(['\\', '/']);
        for part in rest.split(['\\', '/']).filter(|p| !p.is_empty()) {
            if !acc.ends_with('\\') {
                acc.push('\\');
            }
            acc.push_str(part);
            segments.push((part.to_string(), std::path::PathBuf::from(&acc)));
        }
        return segments;
    }

    // 3. Unix root path (/...)
    if let Some(rest) = trimmed.strip_prefix('/') {
        let mut acc = String::from("/");
        segments.push(("/".to_string(), std::path::PathBuf::from(&acc)));
        for part in rest.split('/').filter(|p| !p.is_empty()) {
            if !acc.ends_with('/') {
                acc.push('/');
            }
            acc.push_str(part);
            segments.push((part.to_string(), std::path::PathBuf::from(&acc)));
        }
        return segments;
    }

    // 4. Relative fallback
    let mut acc = std::path::PathBuf::new();
    for comp in path.components() {
        acc.push(comp.as_os_str());
        segments.push((comp.as_os_str().to_string_lossy().to_string(), acc.clone()));
    }
    segments
}

pub fn sort_indicator(current: SortColumn, target: SortColumn, order: SortOrder) -> &'static str {
    if current == target {
        match order {
            SortOrder::Ascending => "▲",
            SortOrder::Descending => "▼",
        }
    } else {
        ""
    }
}

#[allow(clippy::too_many_arguments)]
fn render_panel(
    ui: &mut egui::Ui,
    tab_mgr: &mut PanelTabManager,
    side: PanelSide,
    is_active: bool,
    drives: &[DriveInfo],
    i18n: &I18n,
    suppress_pointer: bool,
    cut_paths: Option<&[std::path::PathBuf]>,
    drag_ctx: PanelDragContext<'_>,
    palette: &ThemePalette,
    appearance: &AppearanceConfig,
) -> (bool, Option<PanelAction>) {
    let mut became_active = false;
    let mut action = None;
    let mut open_drive_menu_requested = false;

    let border_stroke = if drag_ctx.is_drop_target {
        Stroke::new(2.5_f32, palette.drop_target_border)
    } else if is_active {
        Stroke::new(2.0_f32, palette.panel_border_active)
    } else {
        Stroke::new(1.0_f32, palette.panel_border_inactive)
    };

    let panel_bg = if is_active {
        palette.panel_bg_active
    } else {
        palette.panel_bg_inactive
    };

    ui.push_id(match side { PanelSide::Left => "panel_L", PanelSide::Right => "panel_R" }, |ui| {
        egui::Frame::NONE
            .fill(panel_bg)
            .stroke(border_stroke)
            .inner_margin(4.0_f32)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());

                // Kliknięcie w dowolne miejsce panelu aktywuje go i zdejmuje focus z menu
                if !suppress_pointer && ui.input(|i| i.pointer.any_pressed()) {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if ui.max_rect().contains(pos) {
                            became_active = true;
                            ui.ctx().memory_mut(|mem| {
                                mem.stop_text_input();
                                mem.close_popup();
                            });
                        }
                    }
                }

                // Drive buttons bar
                ui.horizontal(|ui| {
                    let alt_tag = match side {
                        PanelSide::Left => "Alt+F1",
                        PanelSide::Right => "Alt+F2",
                    };
                    let menu_btn = egui::Button::new(
                        RichText::new(format!("🖴 {}", alt_tag))
                            .strong()
                            .color(palette.item_parent),
                    )
                    .fill(palette.drive_btn_bg)
                    .stroke(Stroke::new(1.0_f32, palette.drive_btn_border));

                    let open_drives_tip = i18n.t_fmt("panels.open_drives", &[("key", alt_tag)]);
                    let resp = ui
                        .add(menu_btn)
                        .on_hover_text(open_drives_tip);
                    if resp.clicked() {
                        open_drive_menu_requested = true;
                    }
                    resp.surrender_focus();

                    for drive in drives {
                        let is_current = tab_mgr.active().current_path.starts_with(&drive.path);
                        let (bg_color, text_color) = if is_current {
                            (palette.drive_btn_active_bg, palette.drive_btn_active_fg)
                        } else {
                            (palette.drive_btn_bg, palette.drive_btn_fg)
                        };

                        let btn = egui::Button::new(RichText::new(&drive.label).strong().color(text_color))
                            .fill(bg_color)
                            .stroke(Stroke::new(1.0_f32, if is_current { palette.panel_border_active } else { palette.drive_btn_border }));

                        let resp = ui.add(btn);
                        if resp.clicked() {
                            action = Some(PanelAction::ChangeDir(side, drive.path.clone()));
                        }
                        resp.surrender_focus();
                    }
                });

                ui.add_space(2.0);

                // Tab bar
                {
                    let tab_count = tab_mgr.tabs.len();
                    let active_tab_idx = tab_mgr.active_idx;
                    ui.horizontal(|ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt(match side {
                                PanelSide::Left => "tabs_scroll_L",
                                PanelSide::Right => "tabs_scroll_R",
                            })
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                for (idx, tab) in tab_mgr.tabs.iter().enumerate() {
                                    let is_cur = idx == active_tab_idx;
                                    let raw_title = PanelTabManager::tab_title(tab);
                                    let short_title = if raw_title.chars().count() > 22 {
                                        let prefix: String = raw_title.chars().take(21).collect();
                                        format!("{}…", prefix)
                                    } else {
                                        raw_title
                                    };
                                    let display = if tab.is_locked {
                                        format!("🔒 {}", short_title)
                                    } else {
                                        short_title
                                    };

                                    let (fill, border, fg) = if is_cur {
                                        (
                                            palette.tab_bg_active,
                                            palette.tab_border_active,
                                            palette.tab_fg_active,
                                        )
                                    } else {
                                        (
                                            palette.tab_bg_inactive,
                                            palette.tab_border_inactive,
                                            palette.tab_fg_inactive,
                                        )
                                    };

                                    let tab_tip = format!(
                                        "{}\n{}",
                                        tab.panel.display_path(),
                                        i18n.t("panels.tab_tooltip_hint")
                                    );

                                    egui::Frame::NONE
                                        .fill(fill)
                                        .stroke(Stroke::new(1.0_f32, border))
                                        .corner_radius(3.0_f32)
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                let lbl_btn = egui::Button::new(
                                                    RichText::new(display).size(11.5).color(fg),
                                                )
                                                .frame(false);
                                                let resp = ui.add(lbl_btn).on_hover_text(tab_tip);
                                                if resp.clicked() {
                                                    became_active = true;
                                                    action = Some(PanelAction::SelectTab(side, idx));
                                                }
                                                if resp.middle_clicked()
                                                    && tab_count > 1
                                                    && !tab.is_locked
                                                {
                                                    became_active = true;
                                                    action = Some(PanelAction::CloseTab(side, idx));
                                                }
                                                if resp.secondary_clicked() {
                                                    became_active = true;
                                                    action =
                                                        Some(PanelAction::ToggleLockTab(side, idx));
                                                }
                                                resp.surrender_focus();

                                                if tab_count > 1 && !tab.is_locked {
                                                    let close_resp = ui
                                                        .add(
                                                            egui::Button::new(
                                                                RichText::new("×")
                                                                    .size(11.5)
                                                                    .color(Color32::from_rgb(
                                                                        190, 130, 130,
                                                                    )),
                                                            )
                                                            .frame(false),
                                                        )
                                                        .on_hover_text(
                                                            i18n.t("panels.close_tab_tip"),
                                                        );
                                                    if close_resp.clicked() {
                                                        became_active = true;
                                                        action =
                                                            Some(PanelAction::CloseTab(side, idx));
                                                    }
                                                    close_resp.surrender_focus();
                                                }
                                            });
                                        });
                                }

                                let new_tab_resp = ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("+")
                                                .strong()
                                                .size(12.0)
                                                .color(palette.accent),
                                        )
                                        .fill(palette.drive_btn_bg),
                                    )
                                    .on_hover_text(i18n.t("panels.new_tab_tip"));
                                if new_tab_resp.clicked() {
                                    became_active = true;
                                    action = Some(PanelAction::NewTab(side));
                                }
                                new_tab_resp.surrender_focus();
                            });
                    });
                }

                ui.add_space(2.0);

                let panel = tab_mgr.active_mut();

                // Path bar with [..] button
                let path_bg = if is_active {
                    palette.path_bar_bg_active
                } else {
                    palette.path_bar_bg_inactive
                };

                egui::Frame::NONE
                    .fill(path_bg)
                    .stroke(Stroke::new(1.0_f32, palette.path_bar_border))
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let back_btn = egui::Button::new(RichText::new("◀").strong())
                                .fill(palette.drive_btn_bg);
                            let back_resp = ui
                                .add_enabled(panel.can_go_back(), back_btn)
                                .on_hover_text(i18n.t("panels.nav_back"));
                            if back_resp.clicked() {
                                action = Some(PanelAction::GoBack(side));
                            }
                            back_resp.surrender_focus();

                            let fwd_btn = egui::Button::new(RichText::new("▶").strong())
                                .fill(palette.drive_btn_bg);
                            let fwd_resp = ui
                                .add_enabled(panel.can_go_forward(), fwd_btn)
                                .on_hover_text(i18n.t("panels.nav_forward"));
                            if fwd_resp.clicked() {
                                action = Some(PanelAction::GoForward(side));
                            }
                            fwd_resp.surrender_focus();

                            let up_btn = egui::Button::new(RichText::new("[..]").strong().color(palette.item_parent))
                                .fill(palette.drive_btn_bg);
                            let up_resp = ui.add(up_btn);
                            if up_resp.clicked() {
                                action = Some(PanelAction::GoUp(side));
                            }
                            up_resp.surrender_focus();

                            if panel.is_editing_path {
                                let edit_w = (ui.available_width() - 30.0).max(80.0);
                                let path_edit = ui.add(
                                    egui::TextEdit::singleline(&mut panel.path_edit_text)
                                        .desired_width(edit_w),
                                );
                                if panel.focus_path_edit {
                                    path_edit.request_focus();
                                    panel.focus_path_edit = false;
                                }
                                let enter_hit = (path_edit.lost_focus() || path_edit.has_focus())
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if enter_hit {
                                    let target_str = panel.path_edit_text.trim().to_string();
                                    panel.cancel_path_edit();
                                    if !target_str.is_empty() {
                                        action = Some(PanelAction::NavigateToInputPath(side, target_str));
                                    }
                                    ui.ctx().memory_mut(|m| m.stop_text_input());
                                }
                                if ui
                                    .button(RichText::new("✖").color(Color32::from_rgb(255, 120, 120)))
                                    .clicked()
                                {
                                    panel.cancel_path_edit();
                                }
                            } else {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let bm_resp = ui
                                        .button(
                                            RichText::new("★")
                                                .size(12.0)
                                                .color(Color32::from_rgb(255, 215, 90)),
                                        )
                                        .on_hover_text(i18n.t("panels.bookmarks_tip"));
                                    if bm_resp.clicked() {
                                        became_active = true;
                                        action = Some(PanelAction::OpenBookmarks(side));
                                    }
                                    bm_resp.surrender_focus();

                                    if !panel.is_in_zip() && panel.custom_title.is_none() {
                                        let edit_resp = ui
                                            .button(RichText::new("✏").size(12.0))
                                            .on_hover_text(i18n.t("panels.edit_path_tip"));
                                        if edit_resp.clicked() {
                                            became_active = true;
                                            panel.start_path_edit();
                                        }
                                        edit_resp.surrender_focus();
                                    }

                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        let path_color = if is_active {
                                            palette.path_text_active
                                        } else {
                                            palette.path_text_inactive
                                        };
                                        if panel.is_in_zip() || panel.custom_title.is_some() {
                                            let path_display = format!("{}/*.*", panel.display_path());
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(path_display).strong().color(path_color),
                                                )
                                                .truncate(),
                                            );
                                        } else {
                                            let segments = build_breadcrumb_segments(&panel.current_path);
                                            let seg_count = segments.len();
                                            egui::ScrollArea::horizontal()
                                                .id_salt(match side {
                                                    PanelSide::Left => "path_bc_L",
                                                    PanelSide::Right => "path_bc_R",
                                                })
                                                .scroll_bar_visibility(
                                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                                )
                                                .show(ui, |ui| {
                                                    for (idx, (label, seg_path)) in
                                                        segments.into_iter().enumerate()
                                                    {
                                                        let is_last = idx + 1 == seg_count;
                                                        let text_col = if is_last {
                                                            path_color
                                                        } else if is_active {
                                                            palette.breadcrumb_parent_active
                                                        } else {
                                                            palette.breadcrumb_parent_inactive
                                                        };
                                                        let seg_btn = egui::Button::new(
                                                            RichText::new(&label).strong().color(text_col),
                                                        )
                                                        .frame(false);
                                                        let seg_resp = ui.add(seg_btn);
                                                        if seg_resp.clicked() && !is_last {
                                                            action = Some(PanelAction::ChangeDir(
                                                                side, seg_path,
                                                            ));
                                                        }
                                                        seg_resp.surrender_focus();
                                                        if !is_last {
                                                            ui.label(
                                                                RichText::new("▸")
                                                                    .size(11.0)
                                                                    .color(palette.muted_text),
                                                            );
                                                        }
                                                    }
                                                });
                                        }
                                    });
                                });
                            }
                        });
                    });

                // Filter bar and Summary Status Line
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔍").size(12.0).color(Color32::from_rgb(0, 190, 255)));
                    let filter_edit = ui.add(
                        egui::TextEdit::singleline(&mut panel.filter)
                            .hint_text(i18n.t("panels.filter_hint"))
                            .desired_width(130.0),
                    );
                    if filter_edit.changed() {
                        panel.apply_filter();
                    }
                    if panel.is_filter_open {
                        filter_edit.request_focus();
                        panel.is_filter_open = false;
                    }

                    if !panel.filter.is_empty() {
                        let filtered_s = panel.items.len().saturating_sub(1).to_string();
                        let total_s = panel.all_items.len().saturating_sub(1).to_string();
                        let count_text = i18n.t_fmt("panels.items_count", &[("filtered", &filtered_s), ("total", &total_s)]);
                        ui.label(RichText::new(count_text).size(11.0).color(Color32::from_rgb(120, 240, 150)));
                        if ui.button(RichText::new("✖").color(Color32::from_rgb(255, 120, 120))).on_hover_text(i18n.t("panels.clear_filter")).clicked() {
                            panel.clear_filter();
                        }
                    }

                    if let Some(qs) = panel.quick_search_active() {
                        let qs_text = i18n.t_fmt("panels.quick_search_label", &[("query", qs)]);
                        ui.label(
                            RichText::new(qs_text)
                                .size(11.0)
                                .strong()
                                .color(Color32::from_rgb(0, 230, 255)),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let sel_cnt = panel.selected_count.to_string();
                        let sel_sz = format_file_size(panel.selected_bytes, appearance.size_format);
                        let tot_cnt = (panel.total_files + panel.total_dirs).to_string();
                        let tot_sz = format_file_size(panel.total_bytes, appearance.size_format);
                        let status_text = i18n.t_fmt(
                            "panels.status_summary",
                            &[
                                ("sel_count", &sel_cnt),
                                ("sel_bytes", &sel_sz),
                                ("total_count", &tot_cnt),
                                ("total_bytes", &tot_sz),
                            ],
                        );
                        ui.label(RichText::new(status_text).size(11.0).color(palette.muted_text));
                    });
                });

                ui.separator();

                if panel.is_loading() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.add(egui::Spinner::new().size(36.0));
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(i18n.t("panels.loading_title"))
                                .strong()
                                .size(15.0)
                                .color(Color32::from_rgb(0, 200, 255)),
                        );
                        ui.add_space(8.0);
                        if let Some(target) = panel.loading_target() {
                            ui.label(
                                RichText::new(target.to_string_lossy().to_string())
                                    .size(13.0)
                                    .color(Color32::from_rgb(255, 230, 120))
                                    .monospace(),
                            );
                        }
                        ui.add_space(6.0);
                        let elapsed = panel.loading_elapsed();
                        ui.label(
                            RichText::new(format!("{:.1} s", elapsed.as_secs_f32()))
                                .size(12.0)
                                .color(Color32::from_rgb(160, 175, 200)),
                        );
                        ui.add_space(18.0);
                        let cancel_btn = egui::Button::new(
                            RichText::new(i18n.t("panels.loading_cancel"))
                                .strong()
                                .size(13.0)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(180, 45, 45))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 90, 90)));

                        if ui.add_sized([160.0, 32.0], cancel_btn).clicked() {
                            action = Some(PanelAction::CancelLoading(side));
                        }
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(i18n.t("panels.loading_tip"))
                                .size(11.0)
                                .color(Color32::from_rgb(120, 140, 170)),
                        );
                    });
                } else {
                    // File Table
                    let total_rows = panel.items.len();
                    let in_zip = panel.is_in_zip();
                    let can_accept_drop = panel_can_accept_drop(panel);
                    let mut clicked_row = None;
                    let mut double_clicked_row = None;
                    let mut right_clicked_row = None;
                    let mut right_click_pos = None;
                    let mut sort_clicked = None;
                    let mut drag_start_row: Option<(usize, egui::Pos2)> = None;
                    let mut hovered_drop_folder: Option<(PathBuf, egui::Rect)> = None;
                    let drag_trigger_pressed = ui.input(|i| {
                        i.pointer.primary_pressed() && !i.modifiers.ctrl && !i.modifiers.shift
                    });
                    let table_rect = ui.available_rect_before_wrap();
                    let fallback_pointer_pos = ui.input(|i| i.pointer.interact_pos());

                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .auto_shrink([false, false])
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::remainder().at_least(90.0)) // Nazwa takes remainder!
                    .column(Column::initial(50.0).at_least(40.0).resizable(true)) // Rozsz
                    .column(Column::initial(80.0).at_least(60.0).resizable(true)) // Rozmiar
                    .column(Column::initial(130.0).at_least(100.0).resizable(true)); // Data

                if panel.scroll_to_cursor {
                    table = table.scroll_to_row(panel.cursor_idx, Some(egui::Align::Center));
                    panel.scroll_to_cursor = false;
                }

                table
                    .header(22.0, |mut header| {
                        header.col(|ui| {
                            let label = format!("{} {}", i18n.t("panels.col_name"), sort_indicator(panel.sort_column, SortColumn::Name, panel.sort_order));
                            if ui.button(RichText::new(label).strong()).clicked() {
                                sort_clicked = Some(SortColumn::Name);
                            }
                        });
                        header.col(|ui| {
                            let label = format!("{} {}", i18n.t("panels.col_ext"), sort_indicator(panel.sort_column, SortColumn::Ext, panel.sort_order));
                            if ui.button(RichText::new(label).strong()).clicked() {
                                sort_clicked = Some(SortColumn::Ext);
                            }
                        });
                        header.col(|ui| {
                            let label = format!("{} {}", i18n.t("panels.col_size"), sort_indicator(panel.sort_column, SortColumn::Size, panel.sort_order));
                            if ui.button(RichText::new(label).strong()).clicked() {
                                sort_clicked = Some(SortColumn::Size);
                            }
                        });
                        header.col(|ui| {
                            let label = format!("{} {}", i18n.t("panels.col_date"), sort_indicator(panel.sort_column, SortColumn::Date, panel.sort_order));
                            if ui.button(RichText::new(label).strong()).clicked() {
                                sort_clicked = Some(SortColumn::Date);
                            }
                        });
                    })
                    .body(|body| {
                        let row_h = appearance.row_height.clamp(16.0, 30.0);
                        body.rows(row_h, total_rows, |mut row| {
                            let idx = row.index();
                            let item = &panel.items[idx];
                            let is_cursor = panel.cursor_idx == idx;
                            let is_selected = panel.selected_paths.contains(&item.path);

                            // Cursor highlight across the entire row
                            row.set_selected(is_cursor);

                            // Theme colors for file types
                            let base_text_color = if is_selected {
                                palette.item_selected
                            } else if is_cursor && is_active {
                                palette.item_cursor_active
                            } else if item.is_parent {
                                palette.item_parent
                            } else if item.is_dir {
                                palette.item_dir
                            } else {
                                match item.ext.as_str() {
                                    "exe" | "bat" | "cmd" | "msi" => palette.item_exec,
                                    "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "xz" => {
                                        palette.item_archive
                                    }
                                    "rs" | "toml" | "json" | "c" | "cpp" | "py" | "md" | "txt" => {
                                        palette.item_code
                                    }
                                    _ => palette.item_normal,
                                }
                            };
                            let is_cut = !item.is_parent
                                && cut_paths.is_some_and(|paths| paths.contains(&item.path));
                            let text_color = if is_cut {
                                base_text_color.linear_multiply(0.55)
                            } else {
                                base_text_color
                            };

                            // Name column
                            row.col(|ui| {
                                let label_prefix = if item.is_parent {
                                    "[..] "
                                } else if item.is_symlink {
                                    "🔗 "
                                } else if item.is_dir {
                                    "📁 "
                                } else {
                                    "   "
                                };
                                let text = format!("{}{}", label_prefix, item.name);
                                let rich = if is_cursor && is_active {
                                    RichText::new(text).color(text_color).strong()
                                } else {
                                    RichText::new(text).color(text_color)
                                };
                                ui.add(egui::Label::new(rich).truncate());
                            });

                            // Ext column
                            row.col(|ui| {
                                ui.add(egui::Label::new(RichText::new(&item.ext).color(text_color)).truncate());
                            });

                            // Size column (Right-aligned)
                            row.col(|ui| {
                                let (size_str, size_color) = if item.is_dir {
                                    if panel.is_calculating_dir(&item.path) {
                                        ("<...>".to_string(), palette.accent)
                                    } else if panel.is_calculated_dir(&item.path) || item.size > 0 {
                                        (format_file_size(item.size, appearance.size_format), text_color)
                                    } else if item.is_symlink {
                                        ("<LNK>".to_string(), text_color)
                                    } else {
                                        ("<DIR>".to_string(), text_color)
                                    }
                                } else {
                                    (format_file_size(item.size, appearance.size_format), text_color)
                                };
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(size_str).color(size_color));
                                });
                            });

                            // Date column (Right-aligned)
                            row.col(|ui| {
                                let date_str = item
                                    .modified
                                    .map(|time| {
                                        let dt: DateTime<Local> = time.into();
                                        dt.format("%Y-%m-%d %H:%M").to_string()
                                    })
                                    .unwrap_or_default();
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(date_str).color(text_color));
                                });
                            });

                            // Capture click / selection / drag across the entire row
                            if !suppress_pointer {
                                let row_resp = row.response();
                                if row_resp.clicked() {
                                    clicked_row = Some(idx);
                                }
                                if row_resp.double_clicked() {
                                    double_clicked_row = Some(idx);
                                }
                                if row_resp.secondary_clicked() {
                                    right_clicked_row = Some(idx);
                                    right_click_pos =
                                        row_resp.interact_pointer_pos().or(fallback_pointer_pos);
                                }
                                if !in_zip
                                    && !item.is_parent
                                    && row_resp.hovered()
                                    && drag_trigger_pressed
                                {
                                    if let Some(pos) =
                                        row_resp.interact_pointer_pos().or(fallback_pointer_pos)
                                    {
                                        drag_start_row = Some((idx, pos));
                                    }
                                }
                                if let Some(active_drag) = drag_ctx.drag_active {
                                    if active_drag.is_dragging
                                        && can_accept_drop
                                        && item.is_dir
                                        && !item.is_parent
                                        && row_resp.hovered()
                                        && !active_drag.sources.contains(&item.path)
                                    {
                                        hovered_drop_folder =
                                            Some((item.path.clone(), row_resp.rect));
                                    }
                                }
                            }
                        });
                    });

                if let Some((folder, rect)) = hovered_drop_folder {
                    ui.painter().rect_stroke(
                        rect,
                        2.0_f32,
                        Stroke::new(1.5_f32, palette.drop_target_border),
                        egui::StrokeKind::Inside,
                    );
                    *drag_ctx.hovered_folder_out = Some(folder);
                }

                if let Some((idx, start_pos)) = drag_start_row {
                    if let Some(item) = panel.items.get(idx) {
                        let sources = if panel.selected_paths.contains(&item.path) {
                            panel
                                .items
                                .iter()
                                .filter(|it| !it.is_parent && panel.selected_paths.contains(&it.path))
                                .map(|it| it.path.clone())
                                .collect()
                        } else {
                            vec![item.path.clone()]
                        };
                        if !sources.is_empty() {
                            *drag_ctx.drag_start_out = Some((sources, start_pos));
                        }
                    }
                }

                // Apply sort
                if let Some(col) = sort_clicked {
                    if panel.sort_column == col {
                        panel.sort_order = match panel.sort_order {
                            SortOrder::Ascending => SortOrder::Descending,
                            SortOrder::Descending => SortOrder::Ascending,
                        };
                    } else {
                        panel.sort_column = col;
                        panel.sort_order = SortOrder::Ascending;
                    }
                    panel.sort_items();
                }

                // Handle clicks anywhere on the row
                if clicked_row.is_some() || right_clicked_row.is_some() || double_clicked_row.is_some() {
                    ui.ctx().memory_mut(|mem| {
                        mem.stop_text_input();
                        mem.close_popup();
                    });
                }

                if let Some(idx) = clicked_row {
                    became_active = true;
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.ctrl {
                        panel.toggle_select(idx);
                        panel.cursor_idx = idx;
                    } else if modifiers.shift {
                        let anchor = panel.cursor_idx;
                        panel.select_range(anchor, idx);
                        panel.cursor_idx = idx;
                    } else {
                        panel.cursor_idx = idx;
                    }
                }

                if let Some(idx) = right_clicked_row {
                    became_active = true;
                    let pos = right_click_pos.unwrap_or_else(|| table_rect.center());
                    action = Some(PanelAction::OpenContextMenu {
                        side,
                        pos,
                        row_idx: Some(idx),
                    });
                } else if !suppress_pointer && ui.input(|i| i.pointer.secondary_clicked()) {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if table_rect.contains(pos) && pos.y > table_rect.min.y + 24.0 {
                            became_active = true;
                            ui.ctx().memory_mut(|mem| {
                                mem.stop_text_input();
                                mem.close_popup();
                            });
                            action = Some(PanelAction::OpenContextMenu {
                                side,
                                pos,
                                row_idx: None,
                            });
                        }
                    }
                }

                if let Some(idx) = double_clicked_row {
                    became_active = true;
                    panel.cursor_idx = idx;
                    if let Some(it) = panel.items.get(idx).cloned() {
                        action = Some(PanelAction::OpenItem(it));
                    }
                }
            }
            });
    });

    if open_drive_menu_requested {
        action = Some(PanelAction::OpenDriveMenu(side));
    }

    (became_active, action)
}

#[allow(clippy::too_many_arguments)]
pub fn render_dual_panels(
    ctx: &egui::Context,
    left_panel: &mut PanelTabManager,
    right_panel: &mut PanelTabManager,
    active_side: &mut PanelSide,
    drives: &[DriveInfo],
    i18n: &I18n,
    suppress_pointer: bool,
    cut_paths: Option<&[std::path::PathBuf]>,
    split_ratio: &mut f32,
    quick_view: &mut QuickViewState,
    drag_drop: &mut DragDropState,
    palette: &ThemePalette,
    appearance: &AppearanceConfig,
) -> Vec<PanelAction> {
    let mut actions = Vec::new();
    *split_ratio = split_ratio.clamp(0.2, 0.8);

    let pointer_pos = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
    let hovered_os_count = ctx.input(|i| i.raw.hovered_files.len());
    let dropped_os_paths: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(4, 2)))
        .show(ctx, |ui| {
            let avail_rect = ui.available_rect_before_wrap();
            let spacing = 8.0_f32;
            let usable_w = (avail_rect.width() - spacing).max(240.0);
            let min_col_w = 120.0_f32.min(usable_w * 0.45);
            let col_w = (usable_w * *split_ratio).round().clamp(min_col_w, usable_w - min_col_w);
            let col_h = avail_rect.height();

            let left_rect = egui::Rect::from_min_size(
                avail_rect.min,
                egui::vec2(col_w, col_h),
            );
            let split_rect = egui::Rect::from_min_size(
                egui::pos2(avail_rect.min.x + col_w, avail_rect.min.y),
                egui::vec2(spacing, col_h),
            );
            let right_w = (avail_rect.max.x - (avail_rect.min.x + col_w + spacing)).max(100.0);
            let right_rect = egui::Rect::from_min_size(
                egui::pos2(avail_rect.min.x + col_w + spacing, avail_rect.min.y),
                egui::vec2(right_w, col_h),
            );

            let left_visible = !(quick_view.enabled && *active_side == PanelSide::Right);
            let right_visible = !(quick_view.enabled && *active_side == PanelSide::Left);
            let left_accepts = left_visible && panel_can_accept_drop(left_panel.active());
            let right_accepts = right_visible && panel_can_accept_drop(right_panel.active());

            let hovered_side = pointer_pos.and_then(|p| {
                if left_rect.contains(p) {
                    Some(PanelSide::Left)
                } else if right_rect.contains(p) {
                    Some(PanelSide::Right)
                } else {
                    None
                }
            });

            let is_internal_dragging = drag_drop.active_drag.as_ref().is_some_and(|d| d.is_dragging);
            let drag_source_side = drag_drop.active_drag.as_ref().map(|d| d.source_side);
            let os_target_side = if hovered_os_count > 0 {
                Some(hovered_side.unwrap_or(*active_side))
            } else {
                None
            };

            let left_is_drop_target = (is_internal_dragging
                && left_accepts
                && hovered_side == Some(PanelSide::Left)
                && drag_source_side == Some(PanelSide::Right))
                || (os_target_side == Some(PanelSide::Left) && left_accepts);
            let right_is_drop_target = (is_internal_dragging
                && right_accepts
                && hovered_side == Some(PanelSide::Right)
                && drag_source_side == Some(PanelSide::Left))
                || (os_target_side == Some(PanelSide::Right) && right_accepts);

            let active_drag_snapshot = drag_drop.active_drag.clone();
            let mut left_drag_start: Option<(Vec<PathBuf>, egui::Pos2)> = None;
            let mut right_drag_start: Option<(Vec<PathBuf>, egui::Pos2)> = None;
            let mut left_hovered_folder: Option<PathBuf> = None;
            let mut right_hovered_folder: Option<PathBuf> = None;

            // Left panel UI
            let mut left_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(left_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            left_ui.set_clip_rect(left_rect);
            left_ui.push_id("left_panel_col", |ui| {
                if !left_visible {
                    if let Some(qv_act) = render_quick_view_panel(ui, quick_view, i18n) {
                        match qv_act {
                            QuickViewAction::Close => actions.push(PanelAction::CloseQuickView),
                            QuickViewAction::OpenInF3 => actions.push(PanelAction::OpenQuickViewInF3),
                        }
                    }
                } else {
                    let is_active = *active_side == PanelSide::Left;
                    let (became_active, act) = render_panel(
                        ui,
                        left_panel,
                        PanelSide::Left,
                        is_active,
                        drives,
                        i18n,
                        suppress_pointer,
                        cut_paths,
                        PanelDragContext {
                            is_drop_target: left_is_drop_target,
                            drag_active: active_drag_snapshot.as_ref(),
                            drag_start_out: &mut left_drag_start,
                            hovered_folder_out: &mut left_hovered_folder,
                        },
                        palette,
                        appearance,
                    );
                    if became_active {
                        *active_side = PanelSide::Left;
                    }
                    if let Some(a) = act {
                        actions.push(a);
                    }
                }
            });

            // Interactive draggable splitter between left and right panels
            let split_resp = ui.interact(
                split_rect,
                ui.id().with("dual_panel_splitter"),
                egui::Sense::click_and_drag(),
            );
            if split_resp.hovered() || split_resp.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if split_resp.dragged() && usable_w > 0.0 {
                let new_left_w = col_w + split_resp.drag_delta().x;
                *split_ratio = (new_left_w / usable_w).clamp(0.2, 0.8);
            }
            if split_resp.double_clicked() {
                *split_ratio = 0.5;
            }
            let divider_color = if split_resp.hovered() || split_resp.dragged() {
                palette.splitter_hot
            } else {
                palette.splitter_idle
            };
            ui.painter().line_segment(
                [
                    egui::pos2(split_rect.center().x, split_rect.top() + 4.0),
                    egui::pos2(split_rect.center().x, split_rect.bottom() - 4.0),
                ],
                Stroke::new(if split_resp.hovered() || split_resp.dragged() { 2.0_f32 } else { 1.0_f32 }, divider_color),
            );

            // Right panel UI
            let mut right_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(right_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            right_ui.set_clip_rect(right_rect);
            right_ui.push_id("right_panel_col", |ui| {
                if !right_visible {
                    if let Some(qv_act) = render_quick_view_panel(ui, quick_view, i18n) {
                        match qv_act {
                            QuickViewAction::Close => actions.push(PanelAction::CloseQuickView),
                            QuickViewAction::OpenInF3 => actions.push(PanelAction::OpenQuickViewInF3),
                        }
                    }
                } else {
                    let is_active = *active_side == PanelSide::Right;
                    let (became_active, act) = render_panel(
                        ui,
                        right_panel,
                        PanelSide::Right,
                        is_active,
                        drives,
                        i18n,
                        suppress_pointer,
                        cut_paths,
                        PanelDragContext {
                            is_drop_target: right_is_drop_target,
                            drag_active: active_drag_snapshot.as_ref(),
                            drag_start_out: &mut right_drag_start,
                            hovered_folder_out: &mut right_hovered_folder,
                        },
                        palette,
                        appearance,
                    );
                    if became_active {
                        *active_side = PanelSide::Right;
                    }
                    if let Some(a) = act {
                        actions.push(a);
                    }
                }
            });

            // Internal Drag & Drop state machine
            if drag_drop.active_drag.is_none() && !suppress_pointer {
                if let Some((sources, origin_pos)) = left_drag_start {
                    drag_drop.active_drag = Some(InternalDragPayload {
                        source_side: PanelSide::Left,
                        sources,
                        origin_pos,
                        is_dragging: false,
                    });
                } else if let Some((sources, origin_pos)) = right_drag_start {
                    drag_drop.active_drag = Some(InternalDragPayload {
                        source_side: PanelSide::Right,
                        sources,
                        origin_pos,
                        is_dragging: false,
                    });
                }
            }

            if drag_drop.active_drag.is_some() {
                let source_side = drag_drop
                    .active_drag
                    .as_ref()
                    .map(|d| d.source_side)
                    .unwrap_or(PanelSide::Left);
                let opposite_target = match source_side {
                    PanelSide::Left if hovered_side == Some(PanelSide::Right) && right_accepts => {
                        Some(right_panel.active().current_path.clone())
                    }
                    PanelSide::Right if hovered_side == Some(PanelSide::Left) && left_accepts => {
                        Some(left_panel.active().current_path.clone())
                    }
                    _ => None,
                };
                let resolved_target = left_hovered_folder
                    .clone()
                    .or_else(|| right_hovered_folder.clone())
                    .or(opposite_target);

                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    drag_drop.active_drag = None;
                } else if ctx.input(|i| i.pointer.primary_down()) {
                    if let Some(ref mut payload) = drag_drop.active_drag {
                        if !payload.is_dragging
                            && pointer_pos.is_some_and(|p| p.distance(payload.origin_pos) >= 6.0)
                        {
                            payload.is_dragging = true;
                        }
                        if payload.is_dragging {
                            let is_move = ctx.input(|i| i.modifiers.shift);
                            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                            if let Some(pos) = pointer_pos {
                                let count_str = payload.sources.len().to_string();
                                let base_badge = i18n.t_fmt(
                                    if is_move {
                                        "panels.drag_move_badge"
                                    } else {
                                        "panels.drag_copy_badge"
                                    },
                                    &[("count", &count_str)],
                                );
                                let badge_text = if let Some(ref target) = resolved_target {
                                    let dir_label = target
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or_else(|| target.to_string_lossy().to_string());
                                    format!("{} → {}", base_badge, dir_label)
                                } else {
                                    base_badge
                                };

                                egui::Area::new(egui::Id::new("internal_drag_badge"))
                                    .order(egui::Order::Tooltip)
                                    .interactable(false)
                                    .fixed_pos(pos + egui::vec2(14.0, 14.0))
                                    .show(ctx, |ui| {
                                        let bg = if resolved_target.is_some() {
                                            Color32::from_rgb(18, 48, 38)
                                        } else {
                                            Color32::from_rgb(20, 30, 48)
                                        };
                                        let stroke_col = if resolved_target.is_some() {
                                            Color32::from_rgb(0, 220, 140)
                                        } else {
                                            Color32::from_rgb(80, 170, 255)
                                        };
                                        egui::Frame::NONE
                                            .fill(bg)
                                            .stroke(Stroke::new(1.5_f32, stroke_col))
                                            .corner_radius(4.0_f32)
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(badge_text)
                                                        .strong()
                                                        .size(12.0)
                                                        .color(Color32::WHITE),
                                                );
                                            });
                                    });
                            }
                        }
                    }
                } else if let Some(payload) = drag_drop.active_drag.take() {
                    if payload.is_dragging {
                        if let Some(target_dir) = resolved_target {
                            let is_move = ctx.input(|i| i.modifiers.shift);
                            actions.push(PanelAction::DropFiles {
                                sources: payload.sources,
                                target_dir,
                                is_move,
                            });
                        }
                    }
                }
            }

            // External OS Drag & Drop visual overlay and dispatch
            if hovered_os_count > 0 {
                let target_rect_opt = match os_target_side {
                    Some(PanelSide::Left) if left_accepts => Some(left_rect),
                    Some(PanelSide::Right) if right_accepts => Some(right_rect),
                    _ => None,
                };
                if let Some(target_rect) = target_rect_opt {
                    let count_str = hovered_os_count.to_string();
                    let hint = i18n.t_fmt("panels.drop_os_hint", &[("count", &count_str)]);
                    let banner_rect = egui::Rect::from_center_size(
                        target_rect.center(),
                        egui::vec2((target_rect.width() * 0.72).clamp(180.0, 360.0), 46.0),
                    );
                    let painter = ui.painter();
                    painter.rect_filled(
                        banner_rect,
                        egui::CornerRadius::same(6),
                        Color32::from_rgba_unmultiplied(12, 34, 42, 235),
                    );
                    painter.rect_stroke(
                        banner_rect,
                        egui::CornerRadius::same(6),
                        Stroke::new(2.0_f32, Color32::from_rgb(0, 220, 140)),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        banner_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        hint,
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(160, 255, 210),
                    );
                }
            }

            if !dropped_os_paths.is_empty() {
                let target_side = hovered_side.unwrap_or(*active_side);
                let (accepts, target_dir) = match target_side {
                    PanelSide::Left => (left_accepts, left_panel.active().current_path.clone()),
                    PanelSide::Right => (right_accepts, right_panel.active().current_path.clone()),
                };
                if accepts {
                    *active_side = target_side;
                    actions.push(PanelAction::DropFiles {
                        sources: dropped_os_paths,
                        target_dir,
                        is_move: false,
                    });
                }
            }

            // If user clicked anywhere inside a panel area (including empty space), activate it
            if !suppress_pointer && ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if left_rect.contains(pos) && left_visible {
                        *active_side = PanelSide::Left;
                    } else if right_rect.contains(pos) && right_visible {
                        *active_side = PanelSide::Right;
                    }
                }
            }
        });

    actions
}


