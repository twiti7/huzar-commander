use chrono::{DateTime, Local};
use egui::{Color32, RichText, Stroke};
use egui_extras::{Column, TableBuilder};
use humansize::{format_size, DECIMAL};

use crate::app::PanelSide;
use crate::drives::DriveInfo;
use crate::fs_item::{FileItem, SortColumn, SortOrder};
use crate::i18n::I18n;
use crate::panel::PanelState;

pub enum PanelAction {
    OpenDriveMenu(PanelSide),
    OpenItem(FileItem),
    ChangeDir(PanelSide, std::path::PathBuf),
    CancelLoading(PanelSide),
    GoUp(PanelSide),
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

pub fn render_panel(
    ui: &mut egui::Ui,
    panel: &mut PanelState,
    side: PanelSide,
    is_active: bool,
    drives: &[DriveInfo],
    i18n: &I18n,
) -> (bool, Option<PanelAction>) {
    let mut became_active = false;
    let mut action = None;
    let mut open_drive_menu_requested = false;

    let border_stroke = if is_active {
        Stroke::new(2.0_f32, Color32::from_rgb(0, 170, 255))
    } else {
        Stroke::new(1.0_f32, Color32::from_rgb(45, 55, 75))
    };

    let panel_bg = if is_active {
        Color32::from_rgb(16, 24, 42)
    } else {
        Color32::from_rgb(13, 18, 32)
    };

    ui.push_id(match side { PanelSide::Left => "panel_L", PanelSide::Right => "panel_R" }, |ui| {
        egui::Frame::NONE
            .fill(panel_bg)
            .stroke(border_stroke)
            .inner_margin(4.0_f32)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());

                // Kliknięcie w dowolne miejsce panelu aktywuje go i zdejmuje focus z menu
                if ui.input(|i| i.pointer.any_pressed()) {
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
                            .color(Color32::from_rgb(255, 220, 100)),
                    )
                    .fill(Color32::from_rgb(28, 42, 68))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(50, 75, 115)));

                    let open_drives_tip = i18n.t_fmt("panels.open_drives", &[("key", alt_tag)]);
                    let resp = ui
                        .add(menu_btn)
                        .on_hover_text(open_drives_tip);
                    if resp.clicked() {
                        open_drive_menu_requested = true;
                    }
                    resp.surrender_focus();

                    for drive in drives {
                        let is_current = panel.current_path.starts_with(&drive.path);
                        let (bg_color, text_color) = if is_current {
                            (Color32::from_rgb(0, 110, 200), Color32::WHITE)
                        } else {
                            (Color32::from_rgb(25, 35, 55), Color32::from_rgb(180, 200, 230))
                        };

                        let btn = egui::Button::new(RichText::new(&drive.label).strong().color(text_color))
                            .fill(bg_color)
                            .stroke(Stroke::new(1.0_f32, if is_current { Color32::from_rgb(80, 190, 255) } else { Color32::from_rgb(50, 65, 95) }));

                        let resp = ui.add(btn);
                        if resp.clicked() {
                            action = Some(PanelAction::ChangeDir(side, drive.path.clone()));
                        }
                        resp.surrender_focus();
                    }
                });

                ui.add_space(2.0);

                // Path bar with [..] button
                let path_bg = if is_active {
                    Color32::from_rgb(22, 38, 68)
                } else {
                    Color32::from_rgb(18, 26, 46)
                };

                egui::Frame::NONE
                    .fill(path_bg)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 95)))
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let btn = egui::Button::new(RichText::new("[..]").strong().color(Color32::YELLOW))
                                .fill(Color32::from_rgb(30, 45, 75));
                            if ui.add(btn).clicked() {
                                action = Some(PanelAction::GoUp(side));
                            }

                            let path_display = format!("{}/*.*", panel.display_path());
                            let path_color = if is_active {
                                Color32::from_rgb(255, 240, 100)
                            } else {
                                Color32::from_rgb(180, 190, 210)
                            };
                            ui.label(RichText::new(path_display).strong().color(path_color));
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

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let sel_cnt = panel.selected_count.to_string();
                        let sel_sz = format_size(panel.selected_bytes, DECIMAL);
                        let tot_cnt = (panel.total_files + panel.total_dirs).to_string();
                        let tot_sz = format_size(panel.total_bytes, DECIMAL);
                        let status_text = i18n.t_fmt(
                            "panels.status_summary",
                            &[
                                ("sel_count", &sel_cnt),
                                ("sel_bytes", &sel_sz),
                                ("total_count", &tot_cnt),
                                ("total_bytes", &tot_sz),
                            ],
                        );
                        ui.label(RichText::new(status_text).size(11.0).color(Color32::from_rgb(140, 160, 190)));
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
                    let mut clicked_row = None;
                    let mut double_clicked_row = None;
                    let mut right_clicked_row = None;
                    let mut sort_clicked = None;

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
                        body.rows(20.0, total_rows, |mut row| {
                            let idx = row.index();
                            let item = &panel.items[idx];
                            let is_cursor = panel.cursor_idx == idx;
                            let is_selected = panel.selected_paths.contains(&item.path);

                            // Cursor highlight across the entire row
                            row.set_selected(is_cursor);

                            // Theme colors for file types
                            let text_color = if is_selected {
                                Color32::from_rgb(255, 80, 80) // Red tagged
                            } else if is_cursor && is_active {
                                Color32::WHITE // High contrast on active cursor
                            } else if item.is_parent {
                                Color32::from_rgb(255, 220, 100) // Gold
                            } else if item.is_dir {
                                Color32::from_rgb(255, 215, 0) // Yellow
                            } else {
                                match item.ext.as_str() {
                                    "exe" | "bat" | "cmd" | "msi" => Color32::from_rgb(100, 255, 140),
                                    "zip" | "7z" | "rar" | "tar" | "gz" => Color32::from_rgb(255, 170, 90),
                                    "rs" | "toml" | "json" | "c" | "cpp" | "py" | "md" | "txt" => Color32::from_rgb(130, 205, 255),
                                    _ => Color32::from_rgb(230, 235, 245),
                                }
                            };

                            // Name column
                            row.col(|ui| {
                                let label_prefix = if item.is_parent {
                                    "[..] "
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
                                        ("<...>".to_string(), Color32::from_rgb(0, 220, 255))
                                    } else if panel.is_calculated_dir(&item.path) || item.size > 0 {
                                        (format_size(item.size, DECIMAL), text_color)
                                    } else {
                                        ("<DIR>".to_string(), text_color)
                                    }
                                } else {
                                    (format_size(item.size, DECIMAL), text_color)
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

                            // Capture click / selection across the entire row
                            let row_resp = row.response();
                            if row_resp.clicked() {
                                clicked_row = Some(idx);
                            }
                            if row_resp.double_clicked() {
                                double_clicked_row = Some(idx);
                            }
                            if row_resp.secondary_clicked() {
                                right_clicked_row = Some(idx);
                            }
                        });
                    });

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
                    panel.cursor_idx = idx;
                }

                if let Some(idx) = right_clicked_row {
                    became_active = true;
                    panel.cursor_idx = idx;
                    panel.toggle_select(idx);
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

pub fn render_dual_panels(
    ctx: &egui::Context,
    left_panel: &mut PanelState,
    right_panel: &mut PanelState,
    active_side: &mut PanelSide,
    drives: &[DriveInfo],
    i18n: &I18n,
) -> Vec<PanelAction> {
    let mut actions = Vec::new();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(4, 2)))
        .show(ctx, |ui| {
            let avail_rect = ui.available_rect_before_wrap();
            let spacing = 6.0;
            let col_w = ((avail_rect.width() - spacing) / 2.0).floor().max(100.0);
            let col_h = avail_rect.height();

            // Left panel rect & UI
            let left_rect = egui::Rect::from_min_size(
                avail_rect.min,
                egui::vec2(col_w, col_h),
            );
            let mut left_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(left_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            left_ui.set_clip_rect(left_rect);
            left_ui.push_id("left_panel_col", |ui| {
                let is_active = *active_side == PanelSide::Left;
                let (became_active, act) = render_panel(ui, left_panel, PanelSide::Left, is_active, drives, i18n);
                if became_active {
                    *active_side = PanelSide::Left;
                }
                if let Some(a) = act {
                    actions.push(a);
                }
            });

            // Right panel rect & UI
            let right_w = (avail_rect.max.x - (avail_rect.min.x + col_w + spacing)).max(100.0);
            let right_rect = egui::Rect::from_min_size(
                egui::pos2(avail_rect.min.x + col_w + spacing, avail_rect.min.y),
                egui::vec2(right_w, col_h),
            );
            let mut right_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(right_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            right_ui.set_clip_rect(right_rect);
            right_ui.push_id("right_panel_col", |ui| {
                let is_active = *active_side == PanelSide::Right;
                let (became_active, act) = render_panel(ui, right_panel, PanelSide::Right, is_active, drives, i18n);
                if became_active {
                    *active_side = PanelSide::Right;
                }
                if let Some(a) = act {
                    actions.push(a);
                }
            });

            // If user clicked anywhere inside a panel area (including empty space), activate it
            if ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if left_rect.contains(pos) {
                        *active_side = PanelSide::Left;
                    } else if right_rect.contains(pos) {
                        *active_side = PanelSide::Right;
                    }
                }
            }
        });

    actions
}


