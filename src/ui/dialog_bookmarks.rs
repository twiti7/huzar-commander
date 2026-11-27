use std::path::{Path, PathBuf};

use egui::{Color32, Key, RichText, Stroke};

use crate::config::Bookmark;
use crate::i18n::I18n;
use crate::ui::state::BookmarksDialogState;

pub enum BookmarksDialogAction {
    Navigate(PathBuf),
    NavigateOtherPanel(PathBuf),
    Add { name: String, path: String },
    Update { idx: usize, name: String, path: String },
    Remove(usize),
    MoveUp(usize),
    MoveDown(usize),
    Close,
}

pub fn render_bookmarks_dialog(
    ctx: &egui::Context,
    state: &mut BookmarksDialogState,
    bookmarks: &[Bookmark],
    active_dir: &Path,
    i18n: &I18n,
) -> Option<BookmarksDialogAction> {
    if !state.is_open {
        return None;
    }

    if !bookmarks.is_empty() && state.cursor_idx >= bookmarks.len() {
        state.cursor_idx = bookmarks.len() - 1;
    }

    let mut action = None;
    let mut is_open = true;

    if !ctx.wants_keyboard_input() {
        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                action = Some(BookmarksDialogAction::Close);
            } else if i.key_pressed(Key::ArrowUp) && !bookmarks.is_empty() {
                state.cursor_idx = state.cursor_idx.saturating_sub(1);
            } else if i.key_pressed(Key::ArrowDown)
                && !bookmarks.is_empty()
                && state.cursor_idx + 1 < bookmarks.len()
            {
                state.cursor_idx += 1;
            } else if i.key_pressed(Key::Enter) && !bookmarks.is_empty() {
                let target = PathBuf::from(&bookmarks[state.cursor_idx].path);
                action = Some(BookmarksDialogAction::Navigate(target));
            }
        });
    } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
        action = Some(BookmarksDialogAction::Close);
    }

    egui::Window::new(i18n.t("bookmarks.title"))
        .id(egui::Id::new("bookmarks_hotlist_dialog"))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(true)
        .default_size([600.0, 440.0])
        .min_size([460.0, 320.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;

            egui::Frame::NONE
                .fill(Color32::from_rgb(13, 19, 32))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 56, 85)))
                .inner_margin(6.0_f32)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("bookmarks_list_scroll")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            if bookmarks.is_empty() {
                                ui.label(
                                    RichText::new(i18n.t("bookmarks.empty"))
                                        .color(Color32::GRAY)
                                        .italics(),
                                );
                            } else {
                                let total = bookmarks.len();
                                for (idx, bm) in bookmarks.iter().enumerate() {
                                    let selected = idx == state.cursor_idx;
                                    let row_bg = if selected {
                                        Color32::from_rgb(24, 52, 92)
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    egui::Frame::NONE
                                        .fill(row_bg)
                                        .inner_margin(egui::Margin::symmetric(6, 4))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                let main_btn = ui.add(
                                                    egui::Button::new(
                                                        RichText::new(format!("★ {}", bm.name))
                                                            .strong()
                                                            .color(if selected {
                                                                Color32::from_rgb(255, 225, 110)
                                                            } else {
                                                                Color32::from_rgb(220, 232, 248)
                                                            }),
                                                    )
                                                    .frame(false),
                                                );
                                                if main_btn.clicked() {
                                                    state.cursor_idx = idx;
                                                }
                                                if main_btn.double_clicked() {
                                                    action = Some(BookmarksDialogAction::Navigate(
                                                        PathBuf::from(&bm.path),
                                                    ));
                                                }

                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&bm.path)
                                                            .size(11.5)
                                                            .monospace()
                                                            .color(Color32::from_rgb(135, 155, 185)),
                                                    )
                                                    .truncate(),
                                                );

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        if ui
                                                            .button("🗑")
                                                            .on_hover_text(
                                                                i18n.t("bookmarks.btn_delete"),
                                                            )
                                                            .clicked()
                                                        {
                                                            action = Some(
                                                                BookmarksDialogAction::Remove(idx),
                                                            );
                                                        }
                                                        if ui
                                                            .button("✏")
                                                            .on_hover_text(
                                                                i18n.t("bookmarks.btn_edit"),
                                                            )
                                                            .clicked()
                                                        {
                                                            state.editing_idx = Some(idx);
                                                            state.new_name = bm.name.clone();
                                                            state.new_path = bm.path.clone();
                                                        }
                                                        if ui
                                                            .add_enabled(
                                                                idx + 1 < total,
                                                                egui::Button::new("▼"),
                                                            )
                                                            .on_hover_text(
                                                                i18n.t("bookmarks.btn_move_down"),
                                                            )
                                                            .clicked()
                                                        {
                                                            action = Some(
                                                                BookmarksDialogAction::MoveDown(idx),
                                                            );
                                                        }
                                                        if ui
                                                            .add_enabled(
                                                                idx > 0,
                                                                egui::Button::new("▲"),
                                                            )
                                                            .on_hover_text(
                                                                i18n.t("bookmarks.btn_move_up"),
                                                            )
                                                            .clicked()
                                                        {
                                                            action = Some(
                                                                BookmarksDialogAction::MoveUp(idx),
                                                            );
                                                        }
                                                        if ui
                                                            .button("↔")
                                                            .on_hover_text(
                                                                i18n.t("bookmarks.btn_open_other"),
                                                            )
                                                            .clicked()
                                                        {
                                                            action = Some(
                                                                BookmarksDialogAction::NavigateOtherPanel(
                                                                    PathBuf::from(&bm.path),
                                                                ),
                                                            );
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                }
                            }
                        });
                });

            ui.separator();

            egui::Grid::new("bookmarks_form_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label(i18n.t("bookmarks.name_label"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_name)
                            .hint_text(i18n.t("bookmarks.name_hint"))
                            .desired_width(420.0),
                    );
                    ui.end_row();

                    ui.label(i18n.t("bookmarks.path_label"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_path)
                            .hint_text(i18n.t("bookmarks.path_hint"))
                            .desired_width(420.0),
                    );
                    ui.end_row();
                });

            ui.horizontal(|ui| {
                let valid = !state.new_name.trim().is_empty() && !state.new_path.trim().is_empty();
                if let Some(edit_idx) = state.editing_idx {
                    if ui
                        .add_enabled(valid, egui::Button::new(i18n.t("bookmarks.btn_save")))
                        .clicked()
                    {
                        action = Some(BookmarksDialogAction::Update {
                            idx: edit_idx,
                            name: state.new_name.trim().to_string(),
                            path: state.new_path.trim().to_string(),
                        });
                        state.editing_idx = None;
                    }
                    if ui.button(i18n.t("bookmarks.btn_cancel_edit")).clicked() {
                        state.editing_idx = None;
                    }
                } else {
                    if ui
                        .add_enabled(valid, egui::Button::new(i18n.t("bookmarks.btn_add")))
                        .clicked()
                    {
                        action = Some(BookmarksDialogAction::Add {
                            name: state.new_name.trim().to_string(),
                            path: state.new_path.trim().to_string(),
                        });
                    }
                    if ui.button(i18n.t("bookmarks.btn_current_dir")).clicked() {
                        state.open_for_dir(active_dir);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_close")).clicked() {
                        action = Some(BookmarksDialogAction::Close);
                    }
                });
            });
        });

    if !is_open {
        action = Some(BookmarksDialogAction::Close);
    }

    if matches!(
        action,
        Some(
            BookmarksDialogAction::Close
                | BookmarksDialogAction::Navigate(_)
                | BookmarksDialogAction::NavigateOtherPanel(_)
        )
    ) {
        state.is_open = false;
    }

    action
}
