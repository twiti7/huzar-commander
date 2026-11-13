use egui::{Color32, Key, RichText};
use humansize::{format_size, DECIMAL};

use crate::i18n::I18n;
use crate::search::SearchResult;
use crate::ui::state::SearchDialogState;

pub enum SearchDialogAction {
    StartSearch,
    StopSearch,
    JumpTo(SearchResult),
    PreviewFile(SearchResult),
    FeedToListbox(Vec<SearchResult>),
    Close,
}

pub fn render_search_dialog(
    ctx: &egui::Context,
    state: &mut SearchDialogState,
    i18n: &I18n,
) -> Option<SearchDialogAction> {
    if !state.is_open {
        return None;
    }

    let mut action = None;
    let mut is_open = true;

    egui::Window::new(i18n.t("search.title"))
        .open(&mut is_open)
        .resizable(true)
        .collapsible(false)
        .default_size([740.0, 500.0])
        .min_size([500.0, 350.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;

            egui::Grid::new("search_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new(i18n.t("search.mask")).strong());
                    ui.add(egui::TextEdit::singleline(&mut state.mask).desired_width(420.0));
                    ui.end_row();

                    ui.label(RichText::new(i18n.t("search.dir")).strong());
                    ui.add(egui::TextEdit::singleline(&mut state.dir).desired_width(420.0));
                    ui.end_row();

                    ui.label(RichText::new(i18n.t("search.text")).strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut state.text)
                            .hint_text(i18n.t("search.text_hint"))
                            .desired_width(420.0),
                    );
                    ui.end_row();

                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.recursive, i18n.t("search.recursive"));
                        ui.checkbox(&mut state.case_sensitive, i18n.t("search.case_sensitive"));
                    });
                    ui.end_row();
                });

            ui.horizontal(|ui| {
                let is_searching = state.handle.is_some();
                if is_searching {
                    if ui
                        .button(
                            RichText::new(i18n.t("search.btn_stop"))
                                .color(Color32::from_rgb(255, 120, 120)),
                        )
                        .clicked()
                    {
                        action = Some(SearchDialogAction::StopSearch);
                    }
                } else {
                    let enter_start =
                        ui.input(|i| i.key_pressed(Key::Enter)) && state.selected_idx.is_none();
                    if ui
                        .button(
                            RichText::new(i18n.t("search.btn_start"))
                                .strong()
                                .color(Color32::from_rgb(100, 255, 150)),
                        )
                        .clicked()
                        || enter_start
                    {
                        action = Some(SearchDialogAction::StartSearch);
                    }
                }

                if !state.status_text.is_empty() {
                    ui.label(
                        RichText::new(&state.status_text).color(Color32::from_rgb(180, 210, 240)),
                    );
                }
            });

            ui.separator();

            let found_count_str = state.results.len().to_string();
            ui.label(RichText::new(i18n.t_fmt("search.found_count", &[("count", &found_count_str)])).strong());

            let mut jump_target = None;
            let total_results = state.results.len();

            let col_dir_title = if i18n.language() == crate::i18n::Language::Pl {
                "Katalog / Trafienie"
            } else {
                "Folder / Match"
            };

            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .column(egui_extras::Column::initial(180.0).at_least(100.0))
                .column(egui_extras::Column::remainder().at_least(150.0))
                .column(egui_extras::Column::exact(80.0))
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong(i18n.t("panels.col_name"));
                    });
                    header.col(|ui| {
                        ui.strong(col_dir_title);
                    });
                    header.col(|ui| {
                        ui.strong(i18n.t("panels.col_size"));
                    });
                })
                .body(|body| {
                    body.rows(20.0, total_results, |mut row| {
                        let idx = row.index();
                        let res = &state.results[idx];
                        let is_selected = state.selected_idx == Some(idx);
                        row.set_selected(is_selected);

                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&res.name)
                                        .color(Color32::from_rgb(130, 215, 255)),
                                )
                                .truncate(),
                            );
                        });

                        row.col(|ui| {
                            let display_text = if let Some(ref line) = res.match_line {
                                format!("[{}] {}", res.dir.display(), line)
                            } else {
                                res.dir.to_string_lossy().to_string()
                            };
                            ui.add(
                                egui::Label::new(RichText::new(display_text).color(Color32::GRAY))
                                    .truncate(),
                            );
                        });

                        row.col(|ui| {
                            let size_str = format_size(res.size, DECIMAL);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(size_str);
                                },
                            );
                        });

                        let resp = row.response();
                        if resp.clicked() {
                            state.selected_idx = Some(idx);
                        }
                        if resp.double_clicked() {
                            jump_target = Some(res.clone());
                        }
                    });
                });

            if let Some(res) = jump_target {
                action = Some(SearchDialogAction::JumpTo(res));
            }

            ui.separator();

            ui.horizontal(|ui| {
                let has_sel = state.selected_idx.is_some()
                    && state.selected_idx.unwrap() < state.results.len();

                let jump_btn = ui.add_enabled(
                    has_sel,
                    egui::Button::new(
                        RichText::new(format!("➔ {}", i18n.t("search.btn_goto")))
                            .strong()
                            .color(Color32::from_rgb(100, 240, 255)),
                    ),
                );
                if jump_btn.clicked() {
                    if let Some(idx) = state.selected_idx {
                        let res = state.results[idx].clone();
                        action = Some(SearchDialogAction::JumpTo(res));
                    }
                }

                let f3_btn = ui.add_enabled(has_sel, egui::Button::new(i18n.t("search.btn_view")));
                if f3_btn.clicked() {
                    if let Some(idx) = state.selected_idx {
                        let res = state.results[idx].clone();
                        action = Some(SearchDialogAction::PreviewFile(res));
                    }
                }

                let has_results = !state.results.is_empty();
                let feed_btn = ui.add_enabled(
                    has_results,
                    egui::Button::new(
                        RichText::new(format!("📥 {}", i18n.t("search.btn_feed_list")))
                            .strong()
                            .color(Color32::from_rgb(255, 220, 100)),
                    ),
                );
                if feed_btn.clicked() {
                    action = Some(SearchDialogAction::FeedToListbox(state.results.clone()));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(format!("{} (Esc)", i18n.t("dialogs.btn_close"))).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape))
                    {
                        action = Some(SearchDialogAction::Close);
                    }
                });
            });
        });

    if !is_open {
        action = Some(SearchDialogAction::Close);
    }

    action
}
