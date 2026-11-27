use egui::{Color32, Key, RichText, Stroke};

use crate::diff::DiffLineKind;
use crate::i18n::I18n;
use crate::ui::state::DiffDialogState;

pub enum DiffDialogAction {
    Reload,
    Close,
}

pub fn render_diff_dialog(
    ctx: &egui::Context,
    state: &mut DiffDialogState,
    i18n: &I18n,
) -> Option<DiffDialogAction> {
    if !state.is_open {
        return None;
    }

    let mut action = None;
    let mut is_open = true;

    let screen = ctx.screen_rect();
    let max_w = (screen.width() - 24.0).max(400.0).min(screen.width());
    let max_h = (screen.height() - 24.0).max(300.0).min(screen.height());
    let default_w = (screen.width() * 0.88).clamp(480.0, max_w);
    let default_h = (screen.height() * 0.85).clamp(320.0, max_h);

    egui::Window::new(i18n.t("diff.title"))
        .id(egui::Id::new("side_by_side_diff_dialog"))
        .open(&mut is_open)
        .resizable(true)
        .collapsible(false)
        .constrain(true)
        .default_pos(screen.center())
        .pivot(egui::Align2::CENTER_CENTER)
        .default_size([default_w, default_h])
        .max_size([max_w, max_h])
        .show(ctx, |ui| {
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                action = Some(DiffDialogAction::Close);
                return;
            }

            // Toolbar
            ui.horizontal_wrapped(|ui| {
                let prev_ws = state.ignore_whitespace;
                ui.checkbox(&mut state.ignore_whitespace, i18n.t("diff.ignore_whitespace"));
                if state.ignore_whitespace != prev_ws {
                    action = Some(DiffDialogAction::Reload);
                }

                if let Some(ref res) = state.result {
                    ui.separator();
                    let hunks = res.diff_row_indices.len();
                    if ui
                        .add_enabled(hunks > 0, egui::Button::new(i18n.t("diff.prev_diff")))
                        .clicked()
                        && hunks > 0
                    {
                        state.current_diff_idx = if state.current_diff_idx == 0 {
                            hunks - 1
                        } else {
                            state.current_diff_idx - 1
                        };
                        state.scroll_to_row = res.diff_row_indices.get(state.current_diff_idx).copied();
                    }
                    if ui
                        .add_enabled(hunks > 0, egui::Button::new(i18n.t("diff.next_diff")))
                        .clicked()
                        && hunks > 0
                    {
                        state.current_diff_idx = (state.current_diff_idx + 1) % hunks;
                        state.scroll_to_row = res.diff_row_indices.get(state.current_diff_idx).copied();
                    }

                    ui.separator();
                    if hunks == 0 {
                        ui.label(
                            RichText::new(i18n.t("diff.identical"))
                                .color(Color32::from_rgb(120, 235, 160))
                                .strong(),
                        );
                    } else {
                        let stats = i18n.t_fmt(
                            "diff.stats",
                            &[
                                ("added", &res.added_count.to_string()),
                                ("removed", &res.removed_count.to_string()),
                                ("changed", &res.changed_count.to_string()),
                            ],
                        );
                        ui.label(RichText::new(stats).monospace().color(Color32::LIGHT_BLUE));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_close")).clicked() {
                        action = Some(DiffDialogAction::Close);
                    }
                });
            });

            // File path headers
            ui.horizontal(|ui| {
                let half_w = ((ui.available_width() - 8.0) / 2.0).max(120.0);
                let left_lbl = state
                    .left_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let right_lbl = state
                    .right_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                ui.add_sized(
                    [half_w, 18.0],
                    egui::Label::new(
                        RichText::new(left_lbl)
                            .strong()
                            .monospace()
                            .color(Color32::from_rgb(255, 225, 120)),
                    )
                    .truncate(),
                );
                ui.add_sized(
                    [half_w, 18.0],
                    egui::Label::new(
                        RichText::new(right_lbl)
                            .strong()
                            .monospace()
                            .color(Color32::from_rgb(120, 220, 255)),
                    )
                    .truncate(),
                );
            });

            ui.separator();

            if state.loading_rx.is_some() {
                ui.centered_and_justified(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(24.0));
                        ui.label(i18n.t("diff.loading"));
                    });
                });
                return;
            }

            if let Some(ref err) = state.error {
                ui.label(
                    RichText::new(err)
                        .color(Color32::from_rgb(255, 120, 120))
                        .strong(),
                );
                return;
            }

            if let Some(ref res) = state.result {
                let focused_row = res.diff_row_indices.get(state.current_diff_idx).copied();
                let row_h = 18.0_f32;
                let mut sa = egui::ScrollArea::vertical()
                    .id_salt("diff_rows_scroll")
                    .auto_shrink([false, false]);
                if let Some(target_row) = state.scroll_to_row.take() {
                    sa = sa.vertical_scroll_offset(target_row as f32 * row_h);
                }

                let half_w = ((ui.available_width() - 16.0) / 2.0).max(160.0);
                sa.show_rows(ui, row_h, res.rows.len(), |ui, row_range| {
                    for row_idx in row_range {
                        let r = &res.rows[row_idx];
                        let is_focused = focused_row == Some(row_idx);

                        let (l_bg, r_bg) = match r.kind {
                            DiffLineKind::Equal => (Color32::TRANSPARENT, Color32::TRANSPARENT),
                            DiffLineKind::Delete => (
                                Color32::from_rgb(68, 22, 26),
                                Color32::from_rgb(24, 28, 38),
                            ),
                            DiffLineKind::Insert => (
                                Color32::from_rgb(24, 28, 38),
                                Color32::from_rgb(20, 62, 34),
                            ),
                            DiffLineKind::Replace => (
                                Color32::from_rgb(64, 48, 16),
                                Color32::from_rgb(64, 48, 16),
                            ),
                        };

                        ui.horizontal(|ui| {
                            let l_stroke = if is_focused {
                                Stroke::new(1.0_f32, Color32::from_rgb(0, 200, 255))
                            } else {
                                Stroke::NONE
                            };
                            egui::Frame::NONE
                                .fill(l_bg)
                                .stroke(l_stroke)
                                .show(ui, |ui| {
                                    ui.set_width(half_w);
                                    ui.horizontal(|ui| {
                                        let ln = r
                                            .left_line_no
                                            .map(|n| format!("{:>4}", n))
                                            .unwrap_or_else(|| "    ".to_string());
                                        ui.label(
                                            RichText::new(ln)
                                                .monospace()
                                                .size(11.5)
                                                .color(Color32::from_rgb(100, 118, 145)),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&r.left_text)
                                                    .monospace()
                                                    .size(12.0),
                                            )
                                            .truncate(),
                                        );
                                    });
                                });

                            egui::Frame::NONE
                                .fill(r_bg)
                                .stroke(l_stroke)
                                .show(ui, |ui| {
                                    ui.set_width(half_w);
                                    ui.horizontal(|ui| {
                                        let rn = r
                                            .right_line_no
                                            .map(|n| format!("{:>4}", n))
                                            .unwrap_or_else(|| "    ".to_string());
                                        ui.label(
                                            RichText::new(rn)
                                                .monospace()
                                                .size(11.5)
                                                .color(Color32::from_rgb(100, 118, 145)),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&r.right_text)
                                                    .monospace()
                                                    .size(12.0),
                                            )
                                            .truncate(),
                                        );
                                    });
                                });
                        });
                    }
                });
            }
        });

    if !is_open {
        action = Some(DiffDialogAction::Close);
    }

    if matches!(action, Some(DiffDialogAction::Close)) {
        state.is_open = false;
    }

    action
}
