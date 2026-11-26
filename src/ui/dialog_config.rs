use egui::{Color32, Key, RichText, Stroke};

use crate::config::{AppearanceConfig, AppConfig, SizeFormat, ThemeKind};
use crate::i18n::I18n;
use crate::ui::menu_bars::theme_label_key;
use crate::ui::state::{ConfigDialogState, ConfigTab};
use crate::ui::theme::palette_for;

pub enum ConfigDialogAction {
    Save(Box<AppConfig>),
    PreviewAppearance(AppearanceConfig),
    RevertAppearance(AppearanceConfig),
    Close,
}

pub fn render_config_dialog(
    ctx: &egui::Context,
    state: &mut ConfigDialogState,
    i18n: &I18n,
) -> Option<ConfigDialogAction> {
    if !state.is_open {
        return None;
    }

    let mut action = None;
    let mut is_open = true;

    egui::Window::new(i18n.t("config.title"))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(true)
        .default_size([670.0, 520.0])
        .min_size([520.0, 400.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.active_tab,
                    ConfigTab::Editors,
                    i18n.t("config.tab_editors"),
                );
                ui.selectable_value(
                    &mut state.active_tab,
                    ConfigTab::Appearance,
                    i18n.t("config.tab_appearance"),
                );
                ui.selectable_value(
                    &mut state.active_tab,
                    ConfigTab::Behavior,
                    i18n.t("config.tab_behavior"),
                );
            });

            ui.separator();

            match state.active_tab {
                ConfigTab::Editors => {
                    ui.label(RichText::new(i18n.t("config.default_editor")).strong());
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.default_editor)
                                .hint_text(i18n.t("config.default_hint"))
                                .desired_width(360.0),
                        );
                        if ui
                            .button(format!("📁 {}", i18n.t("config.browse_btn")))
                            .on_hover_text(i18n.t("config.browse_hint"))
                            .clicked()
                        {
                            if let Some(picked) = crate::opener::pick_editor_executable() {
                                state.default_editor = picked.to_string_lossy().to_string();
                            }
                        }
                    });

                    ui.separator();

                    ui.label(RichText::new(i18n.t("config.associations_title")).strong());

                    let mut to_remove = None;

                    egui::ScrollArea::vertical()
                        .id_salt("config_associations_scroll")
                        .max_height(210.0)
                        .show(ui, |ui| {
                            egui::Grid::new("config_associations_grid")
                                .striped(true)
                                .num_columns(4)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new(i18n.t("config.col_ext")).strong());
                                    ui.label(RichText::new(i18n.t("config.col_editor")).strong());
                                    ui.label("");
                                    ui.label("");
                                    ui.end_row();

                                    for (idx, (ext, editor)) in
                                        state.associations.iter_mut().enumerate()
                                    {
                                        ui.label(
                                            RichText::new(format!(".{}", ext))
                                                .monospace()
                                                .color(Color32::from_rgb(120, 210, 255)),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(editor).desired_width(280.0),
                                        );
                                        if ui
                                            .button("📁")
                                            .on_hover_text(i18n.t("config.browse_hint"))
                                            .clicked()
                                        {
                                            if let Some(picked) =
                                                crate::opener::pick_editor_executable()
                                            {
                                                *editor = picked.to_string_lossy().to_string();
                                            }
                                        }
                                        if ui
                                            .button(
                                                RichText::new("✖")
                                                    .color(Color32::from_rgb(255, 120, 120)),
                                            )
                                            .on_hover_text(i18n.t("config.remove_btn"))
                                            .clicked()
                                        {
                                            to_remove = Some(idx);
                                        }
                                        ui.end_row();
                                    }
                                });
                        });

                    if let Some(idx) = to_remove {
                        state.associations.remove(idx);
                    }

                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.new_ext)
                                .hint_text(i18n.t("config.ext_hint"))
                                .desired_width(70.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut state.new_editor)
                                .hint_text(i18n.t("config.editor_hint"))
                                .desired_width(230.0),
                        );
                        if ui
                            .button(format!("📁 {}", i18n.t("config.browse_btn")))
                            .on_hover_text(i18n.t("config.browse_hint"))
                            .clicked()
                        {
                            if let Some(picked) = crate::opener::pick_editor_executable() {
                                state.new_editor = picked.to_string_lossy().to_string();
                            }
                        }
                        let can_add =
                            !state.new_ext.trim().is_empty() && !state.new_editor.trim().is_empty();
                        if ui
                            .add_enabled(can_add, egui::Button::new(i18n.t("config.add_btn")))
                            .clicked()
                        {
                            let ext = state.new_ext.trim_start_matches('.').trim().to_lowercase();
                            let ed = state.new_editor.trim().to_string();
                            if let Some(existing) =
                                state.associations.iter_mut().find(|(k, _)| k == &ext)
                            {
                                existing.1 = ed;
                            } else {
                                state.associations.push((ext, ed));
                                state.associations.sort_by(|a, b| a.0.cmp(&b.0));
                            }
                            state.new_ext.clear();
                            state.new_editor.clear();
                        }
                    });
                }

                ConfigTab::Appearance => {
                    ui.label(RichText::new(i18n.t("config.theme_label")).strong());
                    ui.horizontal_wrapped(|ui| {
                        for theme in ThemeKind::ALL {
                            let pal = palette_for(theme);
                            let selected = state.appearance.theme == theme;
                            let btn_frame = egui::Frame::NONE
                                .fill(pal.panel_bg_active)
                                .stroke(Stroke::new(
                                    if selected { 2.0_f32 } else { 1.0_f32 },
                                    if selected {
                                        pal.panel_border_active
                                    } else {
                                        pal.panel_border_inactive
                                    },
                                ))
                                .corner_radius(4.0_f32)
                                .inner_margin(egui::Margin::symmetric(10, 6));

                            let resp = btn_frame
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (swatch_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(14.0, 14.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(swatch_rect, 2.0_f32, pal.accent);
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(i18n.t(theme_label_key(theme)))
                                                    .color(pal.item_normal)
                                                    .strong(),
                                            )
                                            .selectable(false),
                                        );
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());

                            if resp.clicked() {
                                state.appearance.theme = theme;
                                action = Some(ConfigDialogAction::PreviewAppearance(
                                    state.appearance.clone(),
                                ));
                            }
                        }
                    });

                    ui.separator();

                    egui::Grid::new("config_appearance_grid")
                        .num_columns(2)
                        .spacing([14.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new(i18n.t("config.font_size_label")).strong());
                            let font_resp = ui.add(
                                egui::Slider::new(&mut state.appearance.font_size, 11.0..=18.0)
                                    .step_by(0.5)
                                    .suffix(" pt"),
                            );
                            if font_resp.changed() {
                                action = Some(ConfigDialogAction::PreviewAppearance(
                                    state.appearance.clone(),
                                ));
                            }
                            ui.end_row();

                            ui.label(RichText::new(i18n.t("config.row_height_label")).strong());
                            ui.add(
                                egui::Slider::new(&mut state.appearance.row_height, 16.0..=30.0)
                                    .step_by(1.0)
                                    .suffix(" px"),
                            );
                            ui.end_row();
                        });

                    ui.separator();

                    ui.label(RichText::new(i18n.t("config.size_format_label")).strong());
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut state.appearance.size_format,
                            SizeFormat::Decimal,
                            i18n.t("config.size_format_decimal"),
                        );
                        ui.radio_value(
                            &mut state.appearance.size_format,
                            SizeFormat::Binary,
                            i18n.t("config.size_format_binary"),
                        );
                        ui.radio_value(
                            &mut state.appearance.size_format,
                            SizeFormat::Bytes,
                            i18n.t("config.size_format_bytes"),
                        );
                    });
                }

                ConfigTab::Behavior => {
                    ui.checkbox(
                        &mut state.behavior.confirm_delete,
                        i18n.t("config.behavior_confirm_delete"),
                    );
                    ui.checkbox(
                        &mut state.behavior.save_session_on_exit,
                        i18n.t("config.behavior_save_session"),
                    );
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new(format!("💾 {}", i18n.t("dialogs.btn_ok"))).strong())
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter))
                {
                    let mut associations = std::collections::HashMap::new();
                    for (k, v) in &state.associations {
                        let k_clean = k.trim_start_matches('.').trim().to_lowercase();
                        let v_clean = v.trim().to_string();
                        if !k_clean.is_empty() && !v_clean.is_empty() {
                            associations.insert(k_clean, v_clean);
                        }
                    }
                    let cfg = AppConfig {
                        language: state.language.clone(),
                        default_editor: state.default_editor.trim().to_string(),
                        associations,
                        session: crate::config::SessionState::default(),
                        bookmarks: Vec::new(),
                        appearance: state.appearance.clone(),
                        behavior: state.behavior.clone(),
                    };
                    action = Some(ConfigDialogAction::Save(Box::new(cfg)));
                    state.is_open = false;
                }

                if ui.button(i18n.t("config.reset_btn")).clicked() {
                    let prev_tab = state.active_tab;
                    let prev_orig = state.original_appearance.clone();
                    let default_cfg = AppConfig::default();
                    state.open_with_config(&default_cfg);
                    state.active_tab = prev_tab;
                    state.original_appearance = prev_orig;
                    action = Some(ConfigDialogAction::PreviewAppearance(
                        state.appearance.clone(),
                    ));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_cancel")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape))
                    {
                        action = Some(if state.appearance != state.original_appearance {
                            ConfigDialogAction::RevertAppearance(state.original_appearance.clone())
                        } else {
                            ConfigDialogAction::Close
                        });
                        state.is_open = false;
                    }
                });
            });
        });

    if !is_open {
        if action.is_none() {
            action = Some(if state.appearance != state.original_appearance {
                ConfigDialogAction::RevertAppearance(state.original_appearance.clone())
            } else {
                ConfigDialogAction::Close
            });
        }
        state.is_open = false;
    }

    action
}
