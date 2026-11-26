use std::path::PathBuf;

use egui::{Color32, Key, RichText, Stroke};
use egui_extras::{Column, TableBuilder};

use crate::i18n::I18n;
use crate::multi_rename::{compute_rename_preview, CaseTransform, MultiRenameConfig};
use crate::ui::state::MultiRenameDialogState;

pub enum MultiRenameAction {
    Execute(Vec<(PathBuf, PathBuf)>),
    Undo(Vec<(PathBuf, PathBuf)>),
    Close,
}

fn case_label(t: CaseTransform, i18n: &I18n) -> &str {
    match t {
        CaseTransform::Unchanged => i18n.t("multi_rename.case_unchanged"),
        CaseTransform::Lower => i18n.t("multi_rename.case_lower"),
        CaseTransform::Upper => i18n.t("multi_rename.case_upper"),
        CaseTransform::Title => i18n.t("multi_rename.case_title"),
        CaseTransform::FirstUpper => i18n.t("multi_rename.case_first_upper"),
    }
}

pub fn render_multi_rename_dialog(
    ctx: &egui::Context,
    state: &mut MultiRenameDialogState,
    i18n: &I18n,
) -> Option<MultiRenameAction> {
    if !state.is_open {
        return None;
    }

    let mut action = None;
    let mut is_open = true;

    let preview_res = compute_rename_preview(&state.items, &state.config);

    egui::Window::new(i18n.t("multi_rename.title"))
        .id(egui::Id::new("multi_rename_dialog"))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(true)
        .constrain(true)
        .default_size([780.0, 560.0])
        .min_size([620.0, 420.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;

            egui::Grid::new("multi_rename_masks_grid")
                .num_columns(4)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label(i18n.t("multi_rename.name_mask"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.config.name_mask)
                                .desired_width(210.0),
                        );
                        for token in ["[N]", "[C]", "[YMD]"] {
                            if ui.small_button(token).clicked() {
                                state.config.name_mask.push_str(token);
                            }
                        }
                    });

                    ui.label(i18n.t("multi_rename.ext_mask"));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.config.ext_mask)
                                .desired_width(110.0),
                        );
                        if ui.small_button("[E]").clicked() {
                            state.config.ext_mask.push_str("[E]");
                        }
                    });
                    ui.end_row();

                    ui.label(i18n.t("multi_rename.search"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.search).desired_width(250.0),
                    );
                    ui.label(i18n.t("multi_rename.replace"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.replace).desired_width(180.0),
                    );
                    ui.end_row();
                });

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut state.config.use_regex, i18n.t("multi_rename.regex"));
                ui.checkbox(
                    &mut state.config.case_sensitive,
                    i18n.t("multi_rename.case_sensitive"),
                );

                ui.separator();
                ui.label(i18n.t("multi_rename.counter_start"));
                ui.add(egui::DragValue::new(&mut state.config.counter_start).range(0..=999_999));
                ui.label(i18n.t("multi_rename.counter_step"));
                ui.add(egui::DragValue::new(&mut state.config.counter_step).range(1..=10_000));
                ui.label(i18n.t("multi_rename.counter_digits"));
                ui.add(egui::DragValue::new(&mut state.config.counter_digits).range(1..=8));

                ui.separator();
                ui.label(i18n.t("multi_rename.case_label"));
                egui::ComboBox::from_id_salt("multi_rename_case_combo")
                    .selected_text(case_label(state.config.case_transform, i18n))
                    .show_ui(ui, |ui| {
                        for t in [
                            CaseTransform::Unchanged,
                            CaseTransform::Lower,
                            CaseTransform::Upper,
                            CaseTransform::Title,
                            CaseTransform::FirstUpper,
                        ] {
                            ui.selectable_value(
                                &mut state.config.case_transform,
                                t,
                                case_label(t, i18n),
                            );
                        }
                    });
            });

            ui.separator();

            let mut can_execute = false;
            let mut execute_pairs = Vec::new();
            let mut conflict_count = 0usize;
            let mut changed_count = 0usize;

            match &preview_res {
                Err(err) => {
                    ui.label(
                        RichText::new(i18n.t_fmt("multi_rename.regex_error", &[("err", err)]))
                            .color(Color32::from_rgb(255, 110, 110))
                            .strong(),
                    );
                }
                Ok(rows) => {
                    for r in rows {
                        if r.conflict.is_some() {
                            conflict_count += 1;
                        }
                        if r.changed {
                            changed_count += 1;
                            execute_pairs.push((r.source.clone(), r.target.clone()));
                        }
                    }
                    can_execute = conflict_count == 0 && changed_count > 0;

                    egui::Frame::NONE
                        .fill(Color32::from_rgb(13, 19, 32))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 56, 85)))
                        .inner_margin(4.0_f32)
                        .show(ui, |ui| {
                            TableBuilder::new(ui)
                                .striped(true)
                                .vscroll(true)
                                .max_scroll_height(260.0)
                                .column(Column::initial(230.0).at_least(120.0).resizable(true))
                                .column(Column::initial(250.0).at_least(120.0).resizable(true))
                                .column(Column::remainder().at_least(120.0))
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(i18n.t("multi_rename.col_old"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(i18n.t("multi_rename.col_new"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(i18n.t("multi_rename.col_status"));
                                    });
                                })
                                .body(|body| {
                                    body.rows(20.0, rows.len(), |mut row_ui| {
                                        let r = &rows[row_ui.index()];
                                        let new_color = if r.conflict.is_some() {
                                            Color32::from_rgb(255, 110, 110)
                                        } else if r.changed {
                                            Color32::from_rgb(120, 235, 160)
                                        } else {
                                            Color32::GRAY
                                        };
                                        row_ui.col(|ui| {
                                            ui.add(egui::Label::new(&r.old_name).truncate());
                                        });
                                        row_ui.col(|ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&r.new_name).color(new_color),
                                                )
                                                .truncate(),
                                            );
                                        });
                                        row_ui.col(|ui| {
                                            if let Some(ref c) = r.conflict {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(c).color(Color32::from_rgb(
                                                            255, 110, 110,
                                                        )),
                                                    )
                                                    .truncate(),
                                                );
                                            } else if r.changed {
                                                ui.label(
                                                    RichText::new(i18n.t("multi_rename.status_ok"))
                                                        .color(Color32::from_rgb(120, 235, 160)),
                                                );
                                            } else {
                                                ui.label(
                                                    RichText::new(
                                                        i18n.t("multi_rename.status_unchanged"),
                                                    )
                                                    .color(Color32::GRAY),
                                                );
                                            }
                                        });
                                    });
                                });
                        });
                }
            }

            ui.label(
                RichText::new(i18n.t_fmt(
                    "multi_rename.summary",
                    &[
                        ("total", &state.items.len().to_string()),
                        ("changed", &changed_count.to_string()),
                        ("conflicts", &conflict_count.to_string()),
                    ],
                ))
                .size(11.5)
                .color(Color32::LIGHT_GRAY),
            );

            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        can_execute,
                        egui::Button::new(RichText::new(i18n.t("multi_rename.btn_rename")).strong()),
                    )
                    .clicked()
                {
                    action = Some(MultiRenameAction::Execute(execute_pairs));
                }

                if ui
                    .add_enabled(
                        !state.last_batch.is_empty(),
                        egui::Button::new(i18n.t("multi_rename.btn_undo")),
                    )
                    .clicked()
                {
                    action = Some(MultiRenameAction::Undo(state.last_batch.clone()));
                }

                if ui.button(i18n.t("multi_rename.btn_reset")).clicked() {
                    state.config = MultiRenameConfig::default();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_close")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape))
                    {
                        action = Some(MultiRenameAction::Close);
                    }
                });
            });
        });

    if !is_open {
        action = Some(MultiRenameAction::Close);
    }

    if matches!(
        action,
        Some(MultiRenameAction::Close | MultiRenameAction::Execute(_) | MultiRenameAction::Undo(_))
    ) {
        state.is_open = false;
    }

    action
}
