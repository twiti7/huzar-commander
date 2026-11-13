use egui::{Color32, Key, RichText};
use crate::config::AppConfig;
use crate::i18n::I18n;
use crate::ui::state::ConfigDialogState;

pub enum ConfigDialogAction {
    Save(AppConfig),
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
        .default_size([650.0, 500.0])
        .min_size([500.0, 380.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            // Domyślny edytor
            ui.label(RichText::new(i18n.t("config.default_editor")).strong());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.default_editor)
                        .hint_text(i18n.t("config.default_hint"))
                        .desired_width(360.0),
                );
                if ui.button(format!("📁 {}", i18n.t("config.browse_btn"))).on_hover_text(i18n.t("config.browse_hint")).clicked() {
                    if let Some(picked) = crate::opener::pick_editor_executable() {
                        state.default_editor = picked.to_string_lossy().to_string();
                    }
                }
            });

            ui.separator();

            // Lista skojarzeń
            ui.label(RichText::new(i18n.t("config.associations_title")).strong());

            let mut to_remove = None;

            egui::ScrollArea::vertical()
                .id_salt("config_associations_scroll")
                .max_height(220.0)
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

                            for (idx, (ext, editor)) in state.associations.iter_mut().enumerate() {
                                ui.label(RichText::new(format!(".{}", ext)).monospace().color(Color32::from_rgb(120, 210, 255)));
                                ui.add(egui::TextEdit::singleline(editor).desired_width(280.0));
                                if ui.button("📁").on_hover_text(i18n.t("config.browse_hint")).clicked() {
                                    if let Some(picked) = crate::opener::pick_editor_executable() {
                                        *editor = picked.to_string_lossy().to_string();
                                    }
                                }
                                if ui.button(RichText::new("✖").color(Color32::from_rgb(255, 120, 120))).on_hover_text(i18n.t("config.remove_btn")).clicked() {
                                    to_remove = Some(idx);
                                }
                                ui.end_row();
                            }
                        });
                });

            if let Some(idx) = to_remove {
                state.associations.remove(idx);
            }

            // Dodawanie nowego skojarzenia
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
                if ui.button(format!("📁 {}", i18n.t("config.browse_btn"))).on_hover_text(i18n.t("config.browse_hint")).clicked() {
                    if let Some(picked) = crate::opener::pick_editor_executable() {
                        state.new_editor = picked.to_string_lossy().to_string();
                    }
                }
                let can_add = !state.new_ext.trim().is_empty() && !state.new_editor.trim().is_empty();
                if ui.add_enabled(can_add, egui::Button::new(i18n.t("config.add_btn"))).clicked() {
                    let ext = state.new_ext.trim_start_matches('.').trim().to_lowercase();
                    let ed = state.new_editor.trim().to_string();
                    if let Some(existing) = state.associations.iter_mut().find(|(k, _)| k == &ext) {
                        existing.1 = ed;
                    } else {
                        state.associations.push((ext, ed));
                        state.associations.sort_by(|a, b| a.0.cmp(&b.0));
                    }
                    state.new_ext.clear();
                    state.new_editor.clear();
                }
            });

            ui.separator();

            // Przyciski akcji
            ui.horizontal(|ui| {
                if ui.button(RichText::new(format!("💾 {}", i18n.t("dialogs.btn_ok"))).strong()).clicked()
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
                    };
                    action = Some(ConfigDialogAction::Save(cfg));
                    state.is_open = false;
                }

                if ui.button(i18n.t("config.reset_btn")).clicked() {
                    let default_cfg = AppConfig::default();
                    state.open_with_config(&default_cfg);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_cancel")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape))
                    {
                        action = Some(ConfigDialogAction::Close);
                        state.is_open = false;
                    }
                });
            });
        });

    if !is_open {
        action = Some(ConfigDialogAction::Close);
        state.is_open = false;
    }

    action
}
