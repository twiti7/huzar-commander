use egui::{Color32, Key, RichText};
use humansize::{format_size, DECIMAL};

use crate::i18n::I18n;
use crate::opener::ViewerMode;
use crate::ui::state::ViewerWindowState;

pub fn render_viewer_window(ctx: &egui::Context, state: &mut ViewerWindowState, i18n: &I18n) {
    if !state.is_open {
        return;
    }

    let mut is_open = true;
    let mut offset_changed = false;
    let mut mode_changed = false;

    let screen = ctx.screen_rect();
    let app_width = screen.width();
    let app_height = screen.height();

    // Okno podglądu NIGDY nie może być szersze ani wyższe niż rozmiar okna aplikacji
    let max_w = (app_width - 24.0).max(280.0).min(app_width);
    let max_h = (app_height - 24.0).max(220.0).min(app_height);
    let default_w = (app_width * 0.88).clamp(320.0, max_w);
    let default_h = (app_height * 0.88).clamp(240.0, max_h);
    let min_w = 280.0_f32.min(max_w);
    let min_h = 200.0_f32.min(max_h);

    let window_title = i18n.t_fmt("viewer.title", &[("name", &state.title)]);

    egui::Window::new(window_title)
        .id(egui::Id::new("f3_viewer_window"))
        .open(&mut is_open)
        .resizable(true)
        .collapsible(false)
        .constrain(true)
        .default_pos(screen.center())
        .pivot(egui::Align2::CENTER_CENTER)
        .default_size([default_w, default_h])
        .min_size([min_w, min_h])
        .max_size([max_w, max_h])
        .show(ctx, |ui| {
            // Skróty klawiszowe wewnątrz okna podglądu
            let mut prev_page = false;
            let mut next_page = false;
            let mut go_home = false;
            let mut go_end = false;

            ui.input(|i| {
                if i.key_pressed(Key::Escape) {
                    if state.is_search_open {
                        state.is_search_open = false;
                        state.search_status = None;
                    } else {
                        state.is_open = false;
                    }
                } else if i.modifiers.ctrl && i.key_pressed(Key::F) {
                    state.is_search_open = !state.is_search_open;
                } else if i.key_pressed(Key::PageDown) {
                    next_page = true;
                } else if i.key_pressed(Key::PageUp) {
                    prev_page = true;
                } else if i.key_pressed(Key::Home) && !i.modifiers.any() {
                    go_home = true;
                } else if i.key_pressed(Key::End) && !i.modifiers.any() {
                    go_end = true;
                }
            });

            if !state.is_open {
                return;
            }

            let chunk_step = state.chunk_size() as u64;
            let max_offset = state.total_size.saturating_sub(chunk_step);

            if next_page {
                state.seek_next();
            } else if prev_page {
                state.seek_prev();
            } else if go_home {
                state.offset = 0;
                offset_changed = true;
            } else if go_end {
                state.offset = max_offset;
                offset_changed = true;
            }

            // Górny pasek narzędziowy: Wybór trybu + Info o pozycji + Zamknij
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(i18n.t("viewer.mode_label")).strong());
                let prev_mode = state.mode;
                ui.selectable_value(&mut state.mode, ViewerMode::Text, i18n.t("viewer.mode_text"));
                ui.selectable_value(&mut state.mode, ViewerMode::Hex, i18n.t("viewer.mode_hex"));
                ui.selectable_value(&mut state.mode, ViewerMode::Markdown, i18n.t("viewer.mode_markdown"));
                ui.selectable_value(&mut state.mode, ViewerMode::Json, i18n.t("viewer.mode_json"));
                ui.selectable_value(&mut state.mode, ViewerMode::Yaml, i18n.t("viewer.mode_yaml"));
                if state.mode != prev_mode {
                    mode_changed = true;
                }

                ui.separator();
                ui.toggle_value(&mut state.is_search_open, i18n.t("viewer.search_toggle"));

                ui.separator();

                let percent = state.calculate_view_percentage();
                let size_str = format_size(state.total_size, DECIMAL);
                let current_offset = state.current_visible_offset();
                let hex_offset = format!("{:08X}", current_offset);
                let pct_str = format!("{:.1}", percent);
                let info_text = i18n.t_fmt(
                    "viewer.info_status",
                    &[
                        ("size", &size_str),
                        ("offset", &hex_offset),
                        ("percent", &pct_str),
                    ],
                );
                ui.label(
                    RichText::new(info_text)
                        .color(Color32::from_rgb(180, 205, 235))
                        .monospace(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            RichText::new(i18n.t("viewer.close_btn"))
                                .color(Color32::from_rgb(255, 120, 120)),
                        )
                        .clicked()
                    {
                        state.is_open = false;
                    }
                });
            });

            // Pasek wyszukiwania w pliku
            if state.is_search_open {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(i18n.t("viewer.search_label")).color(Color32::from_rgb(0, 200, 255)).strong());
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.search_query)
                            .hint_text(i18n.t("viewer.search_hint"))
                            .desired_width(200.0),
                    );
                    if search_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        state.find_next(true);
                    }
                    if ui.button(i18n.t("viewer.search_next")).clicked() {
                        state.find_next(true);
                    }
                    if ui.button(i18n.t("viewer.search_prev")).clicked() {
                        state.find_next(false);
                    }
                    if let Some(ref status) = state.search_status {
                        ui.label(RichText::new(status).color(Color32::YELLOW).size(11.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(i18n.t("viewer.search_close")).clicked() {
                            state.is_search_open = false;
                            state.search_status = None;
                        }
                    });
                });
            }

            // Pasek nawigacji i suwak przewijania całego pliku (dla plików większych niż rozmiar porcji)
            if state.total_size > chunk_step {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(i18n.t("viewer.btn_home")).clicked() {
                        state.offset = 0;
                        offset_changed = true;
                    }
                    let step_lbl = if chunk_step >= 1024 * 1024 { "1 MB" } else { "64 KB" };
                    if ui.button(format!("◀ -{} (PgUp)", step_lbl)).clicked() {
                        state.seek_prev();
                    }
                    if ui.button(format!("▶ +{} (PgDn)", step_lbl)).clicked() {
                        state.seek_next();
                    }
                    if ui.button(i18n.t("viewer.btn_end")).clicked() {
                        state.offset = max_offset;
                        offset_changed = true;
                    }

                    ui.separator();

                    let prev_offset = state.offset;
                    let slider = egui::Slider::new(&mut state.offset, 0..=max_offset)
                        .text(i18n.t("viewer.slider_label"))
                        .smart_aim(false);
                    if ui.add(slider).changed() && state.offset != prev_offset {
                        offset_changed = true;
                    }
                });
            }

            if mode_changed {
                state.reload_chunk();
            }
            if offset_changed {
                state.reload_chunk();
            }

            ui.separator();

            // Główny obszar przewijania treści (wspiera przewijanie pionowe i poziome)
            let mut sa = egui::ScrollArea::both()
                .id_salt("viewer_main_scroll")
                .auto_shrink([false, false]);
            if state.scroll_to_top {
                sa = sa.vertical_scroll_offset(0.0);
                state.scroll_to_top = false;
            } else if state.scroll_to_bottom {
                sa = sa.vertical_scroll_offset(f32::INFINITY);
                state.scroll_to_bottom = false;
            }
            let scroll_output = sa.show(ui, |ui| {
                    if state.total_size > chunk_step
                        && state.offset > 0
                        && ui.button("▲ Wczytaj poprzednią część (PgUp)").clicked()
                    {
                        state.seek_prev();
                    }

                    match state.mode {
                        ViewerMode::Text | ViewerMode::Hex => {
                            ui.add(
                                egui::TextEdit::multiline(&mut state.content.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY),
                            );
                        }
                        ViewerMode::Markdown => {
                            let avail_w = ui.available_width();
                            ui.scope(|ui| {
                                ui.set_max_width(avail_w);
                                crate::ui::markdown_viewer::render_markdown(ui, &state.content);
                            });
                        }
                        ViewerMode::Json => {
                            let avail_w = ui.available_width();
                            ui.scope(|ui| {
                                ui.set_max_width(avail_w);
                                crate::ui::structured_viewer::render_json(
                                    ui,
                                    &state.content,
                                    &mut state.structured_subview,
                                    i18n,
                                );
                            });
                        }
                        ViewerMode::Yaml => {
                            let avail_w = ui.available_width();
                            ui.scope(|ui| {
                                ui.set_max_width(avail_w);
                                crate::ui::structured_viewer::render_yaml(
                                    ui,
                                    &state.content,
                                    &mut state.structured_subview,
                                    i18n,
                                );
                            });
                        }
                    }

                    if state.total_size > chunk_step
                        && state.offset + (state.chunk.len() as u64) < state.total_size
                        && ui.button("▼ Wczytaj następną część (PgDn)").clicked()
                    {
                        state.seek_next();
                    }
                });

            let max_scroll_y = (scroll_output.content_size.y - scroll_output.inner_rect.height()).max(0.0);
            let scroll_ratio = if max_scroll_y <= 1.0 {
                1.0
            } else {
                (scroll_output.state.offset.y / max_scroll_y).clamp(0.0, 1.0)
            };

            if (state.scroll_ratio - scroll_ratio).abs() > 0.001 {
                state.scroll_ratio = scroll_ratio;
                ctx.request_repaint();
            }

            // Automatyczne doładowanie kolejnej / poprzedniej części przy przewijaniu kółkiem myszy
            if state.total_size > chunk_step {
                let mouse_wheel = ctx.input(|i| i.raw_scroll_delta.y);
                if scroll_ratio >= 0.999 && mouse_wheel < -2.0 && state.offset + (state.chunk.len() as u64) < state.total_size {
                    state.seek_next();
                    state.scroll_to_top = true;
                    ctx.request_repaint();
                } else if scroll_ratio <= 0.001 && mouse_wheel > 2.0 && state.offset > 0 {
                    state.seek_prev();
                    state.scroll_to_bottom = true;
                    ctx.request_repaint();
                }
            }
        });

    if !is_open {
        state.is_open = false;
    }
}
