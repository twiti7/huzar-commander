use egui::{Color32, Key, RichText};
use humansize::{format_size, DECIMAL};

use crate::i18n::I18n;
use crate::opener::{is_native_preview_image, TextEncoding, ViewerMode};
use crate::ui::code_viewer::{detect_code_lang, render_code_view, CodeLang};
use crate::ui::state::{QuickViewState, ViewerWindowState};

pub enum QuickViewAction {
    Close,
    OpenInF3,
}

fn render_viewer_body(
    ui: &mut egui::Ui,
    state: &mut ViewerWindowState,
    i18n: &I18n,
    scroll_id: &str,
) {
    let chunk_step = state.chunk_size() as u64;

    if state.mode == ViewerMode::Image {
        if state.image_loader.is_some() {
            ui.vertical_centered(|ui| {
                ui.add_space(36.0);
                ui.add(egui::Spinner::new().size(28.0));
                ui.add_space(8.0);
                ui.label(i18n.t("viewer.img_loading"));
            });
            return;
        }
        if let Some(ref err) = state.image_error {
            ui.label(
                RichText::new(err)
                    .color(Color32::from_rgb(255, 130, 130))
                    .strong(),
            );
            return;
        }
        if let Some(ref tex) = state.image_texture {
            let [iw, ih] = tex.size();
            let iw_f = (iw.max(1)) as f32;
            let ih_f = (ih.max(1)) as f32;

            // Support Ctrl + mouse wheel zoom inside image area
            if ui.rect_contains_pointer(ui.max_rect()) {
                let zoom_delta = ui.input(|i| {
                    if i.modifiers.ctrl && i.raw_scroll_delta.y.abs() > 0.1 {
                        Some(i.raw_scroll_delta.y)
                    } else {
                        None
                    }
                });
                if let Some(dy) = zoom_delta {
                    state.image_fit = false;
                    let factor = if dy > 0.0 { 1.15 } else { 1.0 / 1.15 };
                    state.image_zoom = (state.image_zoom * factor).clamp(0.1, 8.0);
                }
            }

            let avail = ui.available_size();
            let display_size = if state.image_fit {
                let scale = (avail.x.max(64.0) / iw_f)
                    .min(avail.y.max(64.0) / ih_f)
                    .min(1.0);
                egui::vec2(iw_f * scale, ih_f * scale)
            } else {
                egui::vec2(iw_f * state.image_zoom, ih_f * state.image_zoom)
            };

            egui::ScrollArea::both()
                .id_salt(format!("{}_img", scroll_id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.add(egui::Image::new((tex.id(), display_size)));
                    });
                });
        }
        return;
    }

    let mut sa = egui::ScrollArea::both()
        .id_salt(scroll_id)
        .auto_shrink([false, false]);
    if state.scroll_to_top {
        sa = sa.vertical_scroll_offset(0.0);
        state.scroll_to_top = false;
    } else if state.scroll_to_bottom {
        sa = sa.vertical_scroll_offset(f32::INFINITY);
        state.scroll_to_bottom = false;
    }

    let query_opt = {
        let q = state.search_query.trim();
        if q.is_empty() {
            None
        } else {
            Some(q.to_string())
        }
    };

    let scroll_output = sa.show(ui, |ui| {
        if state.total_size > chunk_step
            && state.offset > 0
            && ui.button(i18n.t("viewer.load_prev_chunk")).clicked()
        {
            state.seek_prev();
        }

        match state.mode {
            ViewerMode::Text => {
                if query_opt.is_some() || state.word_wrap {
                    render_code_view(
                        ui,
                        &state.content,
                        CodeLang::Plain,
                        state.word_wrap,
                        query_opt.as_deref(),
                    );
                } else {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.content.as_str())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                }
            }
            ViewerMode::Code => {
                let lang = detect_code_lang(state.path.as_deref());
                render_code_view(
                    ui,
                    &state.content,
                    lang,
                    state.word_wrap,
                    query_opt.as_deref(),
                );
            }
            ViewerMode::Hex => {
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
            ViewerMode::Image => {}
        }

        if state.total_size > chunk_step
            && state.offset + (state.chunk.len() as u64) < state.total_size
            && ui.button(i18n.t("viewer.load_next_chunk")).clicked()
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
        ui.ctx().request_repaint();
    }

    if state.total_size > chunk_step && ui.rect_contains_pointer(ui.max_rect()) {
        let mouse_wheel = ui.ctx().input(|i| i.raw_scroll_delta.y);
        if scroll_ratio >= 0.999
            && mouse_wheel < -2.0
            && state.offset + (state.chunk.len() as u64) < state.total_size
        {
            state.seek_next();
            state.scroll_to_top = true;
            ui.ctx().request_repaint();
        } else if scroll_ratio <= 0.001 && mouse_wheel > 2.0 && state.offset > 0 {
            state.seek_prev();
            state.scroll_to_bottom = true;
            ui.ctx().request_repaint();
        }
    }
}

pub fn render_viewer_window(ctx: &egui::Context, state: &mut ViewerWindowState, i18n: &I18n) {
    if !state.is_open {
        return;
    }

    state.poll_background(ctx, i18n);

    let mut is_open = true;
    let mut offset_changed = false;
    let mut mode_changed = false;
    let repaint_sig: std::sync::Arc<dyn Fn() + Send + Sync> = {
        let c = ctx.clone();
        std::sync::Arc::new(move || c.request_repaint())
    };

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
            let mut prev_page = false;
            let mut next_page = false;
            let mut go_home = false;
            let mut go_end = false;
            let mut find_dir = None;

            ui.input(|i| {
                if i.key_pressed(Key::Escape) {
                    if state.search_handle.is_some() {
                        state.stop_find();
                    } else if state.is_search_open {
                        state.is_search_open = false;
                        state.search_status = None;
                    } else {
                        state.is_open = false;
                    }
                } else if i.modifiers.ctrl && i.key_pressed(Key::F) {
                    state.is_search_open = !state.is_search_open;
                } else if i.key_pressed(Key::F3) && !state.search_query.trim().is_empty() {
                    find_dir = Some(!i.modifiers.shift);
                } else if i.key_pressed(Key::PageDown) {
                    next_page = true;
                } else if i.key_pressed(Key::PageUp) {
                    prev_page = true;
                } else if i.key_pressed(Key::Home) && !i.modifiers.any() && !ctx.wants_keyboard_input()
                {
                    go_home = true;
                } else if i.key_pressed(Key::End) && !i.modifiers.any() && !ctx.wants_keyboard_input()
                {
                    go_end = true;
                }
            });

            if !state.is_open {
                state.stop_find();
                return;
            }

            if let Some(fwd) = find_dir {
                state.start_find(fwd, Some(std::sync::Arc::clone(&repaint_sig)));
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

            // Górny pasek narzędziowy: Wybór trybu + Info + Zamknij
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(i18n.t("viewer.mode_label")).strong());
                let prev_mode = state.mode;
                ui.selectable_value(&mut state.mode, ViewerMode::Text, i18n.t("viewer.mode_text"));
                ui.selectable_value(&mut state.mode, ViewerMode::Code, i18n.t("viewer.mode_code"));
                ui.selectable_value(&mut state.mode, ViewerMode::Hex, i18n.t("viewer.mode_hex"));
                ui.selectable_value(
                    &mut state.mode,
                    ViewerMode::Markdown,
                    i18n.t("viewer.mode_markdown"),
                );
                ui.selectable_value(&mut state.mode, ViewerMode::Json, i18n.t("viewer.mode_json"));
                ui.selectable_value(&mut state.mode, ViewerMode::Yaml, i18n.t("viewer.mode_yaml"));
                let show_img_tab = state.mode == ViewerMode::Image
                    || state.path.as_deref().is_some_and(is_native_preview_image);
                if show_img_tab {
                    ui.selectable_value(
                        &mut state.mode,
                        ViewerMode::Image,
                        i18n.t("viewer.mode_image"),
                    );
                }
                if state.mode != prev_mode {
                    mode_changed = true;
                }

                if state.mode != ViewerMode::Image {
                    ui.separator();
                    ui.toggle_value(&mut state.is_search_open, i18n.t("viewer.search_toggle"));
                }

                ui.separator();

                if state.mode == ViewerMode::Image {
                    let dims_str = state
                        .image_dims
                        .map(|(w, h)| format!("{}×{} px | ", w, h))
                        .unwrap_or_default();
                    let size_str = format_size(state.total_size, DECIMAL);
                    ui.label(
                        RichText::new(format!("{}{}", dims_str, size_str))
                            .color(Color32::from_rgb(180, 205, 235))
                            .monospace(),
                    );
                } else {
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
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            RichText::new(i18n.t("viewer.close_btn"))
                                .color(Color32::from_rgb(255, 120, 120)),
                        )
                        .clicked()
                    {
                        state.stop_find();
                        state.is_open = false;
                    }
                });
            });

            // Drugi pasek opcji: Wrap / Kodowanie / Tail lub kontrolki Zoom dla obrazów
            ui.horizontal_wrapped(|ui| {
                if state.mode == ViewerMode::Image {
                    ui.toggle_value(&mut state.image_fit, i18n.t("viewer.img_fit"));
                    if ui.button(i18n.t("viewer.img_100")).clicked() {
                        state.image_fit = false;
                        state.image_zoom = 1.0;
                    }
                    if ui.button("−").clicked() {
                        state.image_fit = false;
                        state.image_zoom = (state.image_zoom / 1.25).clamp(0.1, 8.0);
                    }
                    let zoom_pct = if state.image_fit {
                        i18n.t("viewer.img_fit").to_string()
                    } else {
                        format!("{:.0}%", state.image_zoom * 100.0)
                    };
                    ui.label(RichText::new(zoom_pct).monospace());
                    if ui.button("+").clicked() {
                        state.image_fit = false;
                        state.image_zoom = (state.image_zoom * 1.25).clamp(0.1, 8.0);
                    }
                    if let Some(ref img_path) = state.path {
                        ui.separator();
                        if ui.button(i18n.t("viewer.img_open_external")).clicked() {
                            let _ = crate::opener::open_file(img_path);
                        }
                    }
                } else {
                    if state.mode == ViewerMode::Text || state.mode == ViewerMode::Code {
                        ui.toggle_value(&mut state.word_wrap, i18n.t("viewer.wrap_toggle"));
                        ui.separator();
                    }

                    ui.label(i18n.t("viewer.encoding_label"));
                    let prev_enc = state.encoding;
                    egui::ComboBox::from_id_salt("viewer_encoding_combo")
                        .selected_text(state.encoding.label())
                        .show_ui(ui, |ui| {
                            for enc in [
                                TextEncoding::Utf8,
                                TextEncoding::Windows1250,
                                TextEncoding::Utf16Le,
                                TextEncoding::Utf16Be,
                            ] {
                                ui.selectable_value(&mut state.encoding, enc, enc.label());
                            }
                        });
                    if state.encoding != prev_enc {
                        state.refresh_content();
                    }

                    ui.separator();
                    let prev_tail = state.tail_mode;
                    ui.toggle_value(&mut state.tail_mode, i18n.t("viewer.tail_toggle"));
                    if state.tail_mode && !prev_tail {
                        state.offset = max_offset;
                        state.reload_chunk();
                        state.scroll_to_bottom = true;
                        state.last_tail_check = Some(std::time::Instant::now());
                    }
                }
            });

            // Pasek wyszukiwania w pliku (asynchroniczny)
            if state.is_search_open && state.mode != ViewerMode::Image {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(i18n.t("viewer.search_label"))
                            .color(Color32::from_rgb(0, 200, 255))
                            .strong(),
                    );
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.search_query)
                            .hint_text(i18n.t("viewer.search_hint"))
                            .desired_width(200.0),
                    );
                    if search_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        state.start_find(true, Some(std::sync::Arc::clone(&repaint_sig)));
                    }
                    if ui.button(i18n.t("viewer.search_next")).clicked() {
                        state.start_find(true, Some(std::sync::Arc::clone(&repaint_sig)));
                    }
                    if ui.button(i18n.t("viewer.search_prev")).clicked() {
                        state.start_find(false, Some(std::sync::Arc::clone(&repaint_sig)));
                    }
                    if state.search_handle.is_some() {
                        ui.add(egui::Spinner::new().size(14.0));
                        ui.label(RichText::new(i18n.t("viewer.search_running")).color(Color32::LIGHT_BLUE));
                        if ui.button(i18n.t("viewer.search_stop")).clicked() {
                            state.stop_find();
                        }
                    } else if let Some(ref status) = state.search_status {
                        ui.label(RichText::new(status).color(Color32::YELLOW).size(11.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(i18n.t("viewer.search_close")).clicked() {
                            state.stop_find();
                            state.is_search_open = false;
                            state.search_status = None;
                        }
                    });
                });
            }

            // Pasek nawigacji i suwak przewijania całego pliku
            if state.mode != ViewerMode::Image && state.total_size > chunk_step {
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

            if mode_changed || offset_changed {
                state.reload_chunk();
            }

            ui.separator();

            render_viewer_body(ui, state, i18n, "viewer_main_scroll");
        });

    if !is_open {
        state.stop_find();
        state.is_open = false;
    }
}

pub fn render_quick_view_panel(
    ui: &mut egui::Ui,
    qv: &mut QuickViewState,
    i18n: &I18n,
) -> Option<QuickViewAction> {
    let mut action = None;

    egui::Frame::NONE
        .fill(Color32::from_rgb(13, 18, 32))
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(45, 55, 75)))
        .inner_margin(6.0_f32)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());

            ui.horizontal(|ui| {
                let target_title = if qv.is_dir_summary {
                    qv.dir_name.as_str()
                } else if qv.loaded_path.is_some() {
                    qv.viewer.title.as_str()
                } else {
                    ""
                };
                let title = if target_title.is_empty() {
                    i18n.t("viewer.quick_view_title").to_string()
                } else {
                    format!("{}: {}", i18n.t("viewer.quick_view_title"), target_title)
                };
                ui.add(
                    egui::Label::new(
                        RichText::new(title)
                            .strong()
                            .color(Color32::from_rgb(0, 200, 255)),
                    )
                    .truncate(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            RichText::new("✖")
                                .size(12.0)
                                .color(Color32::from_rgb(255, 130, 130)),
                        )
                        .clicked()
                    {
                        action = Some(QuickViewAction::Close);
                    }
                    if !qv.is_dir_summary
                        && qv.viewer.path.is_some()
                        && ui.button(i18n.t("viewer.quick_view_open_f3")).clicked()
                    {
                        action = Some(QuickViewAction::OpenInF3);
                    }
                });
            });

            ui.separator();

            if let Some(ref msg) = qv.placeholder_msg {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(if msg == ".." {
                            i18n.t("viewer.quick_view_empty")
                        } else {
                            msg.as_str()
                        })
                        .color(Color32::GRAY),
                    );
                });
            } else if qv.is_dir_summary {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(format!("📁 {}", qv.dir_name))
                            .strong()
                            .size(16.0)
                            .color(Color32::from_rgb(255, 225, 120)),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(i18n.t("viewer.quick_view_dir_summary"))
                            .color(Color32::from_rgb(160, 185, 220)),
                    );
                    ui.add_space(8.0);
                    if qv.dir_calc_rx.is_some() {
                        ui.add(egui::Spinner::new().size(24.0));
                        ui.label(
                            RichText::new(i18n.t("dialogs.props_calculating"))
                                .color(Color32::LIGHT_BLUE),
                        );
                    } else {
                        let size_s = format_size(qv.dir_bytes, DECIMAL);
                        let summary = i18n.t_fmt(
                            "dialogs.props_contains_fmt",
                            &[
                                ("files", &qv.dir_files.to_string()),
                                ("dirs", &qv.dir_dirs.to_string()),
                            ],
                        );
                        ui.label(RichText::new(summary).size(14.0));
                        ui.label(
                            RichText::new(size_s)
                                .strong()
                                .size(15.0)
                                .color(Color32::from_rgb(120, 235, 175)),
                        );
                    }
                });
            } else if qv.viewer.path.is_some() {
                render_viewer_body(ui, &mut qv.viewer, i18n, "quick_view_scroll");
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(i18n.t("viewer.quick_view_empty")).color(Color32::GRAY));
                });
            }
        });

    action
}
