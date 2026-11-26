use std::collections::VecDeque;
use egui::{Color32, Key, RichText, Stroke};
use humansize::{format_size, DECIMAL};

use crate::i18n::I18n;
use crate::ui::state::ProgressUiState;
use crate::worker::{IoWorker, QueueStateSnapshot};

/// Wykres prędkości w czasie (neonowa fala cyan z poświatą)
pub fn render_speed_graph(
    ui: &mut egui::Ui,
    history: &VecDeque<f32>,
    current_speed: f32,
    i18n: &I18n,
) {
    let desired_size = egui::vec2(ui.available_width(), 80.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Ciemne tło ramki wykresu
    painter.rect(
        rect,
        4.0,
        Color32::from_rgb(10, 16, 28),
        Stroke::new(1.0_f32, Color32::from_rgb(35, 50, 75)),
        egui::StrokeKind::Inside,
    );

    // Linia siatki (pozioma 50%)
    let grid_stroke = Stroke::new(1.0_f32, Color32::from_rgb(22, 34, 52));
    let y_mid = rect.center().y;
    painter.line_segment([egui::pos2(rect.left(), y_mid), egui::pos2(rect.right(), y_mid)], grid_stroke);

    // Szczytowa prędkość (minimum 5 MB/s jako podstawa)
    let max_speed = history.iter().cloned().fold(5.0 * 1024.0 * 1024.0, f32::max);

    // Rysowanie fali
    if history.len() >= 2 {
        let step_x = rect.width() / (history.len() - 1) as f32;
        let mut points = Vec::with_capacity(history.len());

        for (i, &spd) in history.iter().enumerate() {
            let x = rect.left() + i as f32 * step_x;
            let ratio = (spd / max_speed).clamp(0.0, 1.0);
            let y = rect.bottom() - 4.0 - ratio * (rect.height() - 16.0);
            points.push(egui::pos2(x, y));
        }

        // Wypełnienie obszaru pod wykresem poświatą
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.right(), rect.bottom() - 2.0));
        fill_points.push(egui::pos2(rect.left(), rect.bottom() - 2.0));
        let fill_color = Color32::from_rgba_premultiplied(0, 160, 255, 35);
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, Stroke::NONE));

        // Elektryczna linia cyan
        let line_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 200, 255));
        for window in points.windows(2) {
            painter.line_segment([window[0], window[1]], line_stroke);
        }
    }

    // Etykieta lewy górny róg: Szczyt
    let max_speed_str = format_size(max_speed as u64, DECIMAL);
    let max_str = i18n.t_fmt("queue.peak_speed", &[("speed", &max_speed_str)]);
    painter.text(
        rect.left_top() + egui::vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        max_str,
        egui::FontId::monospace(10.0),
        Color32::from_rgb(130, 150, 180),
    );

    // Etykieta lewy dolny róg: 0 B/s
    painter.text(
        rect.left_bottom() + egui::vec2(6.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        "0 B/s",
        egui::FontId::monospace(10.0),
        Color32::from_rgb(90, 110, 130),
    );

    // Etykieta prawy górny róg: Aktualna prędkość
    let cur_str = format!("{}/s", format_size(current_speed as u64, DECIMAL));
    painter.text(
        rect.right_top() + egui::vec2(-6.0, 4.0),
        egui::Align2::RIGHT_TOP,
        cur_str,
        egui::FontId::monospace(14.0),
        Color32::from_rgb(100, 255, 140),
    );
}

/// Modalne okno postępu transferu ze statystykami, wykresem i dwoma paskami postępu
pub fn render_progress_dialog(
    ctx: &egui::Context,
    state: &mut ProgressUiState,
    worker: &IoWorker,
    queue_snapshot: &QueueStateSnapshot,
    i18n: &I18n,
) {
    if !state.is_open || state.in_background {
        return;
    }

    egui::Window::new(&state.title)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;

            if queue_snapshot.total_tasks > 1 {
                ui.horizontal(|ui| {
                    let q_badge = i18n.t_fmt(
                        "queue.queue_badge",
                        &[
                            ("index", &queue_snapshot.active_index.to_string()),
                            ("total", &queue_snapshot.total_tasks.to_string()),
                            ("pending", &queue_snapshot.pending_tasks.len().to_string()),
                        ],
                    );
                    ui.label(RichText::new(q_badge).color(Color32::from_rgb(0, 210, 255)).strong());
                });
                ui.add_space(2.0);
            }

            // Bieżący plik
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n.t("queue.current_task")).strong());
                ui.add(egui::Label::new(RichText::new(&state.current_item).color(Color32::from_rgb(140, 210, 255))).truncate());
            });

            let file_ratio = if state.file_bytes_total > 0 {
                (state.file_bytes_done as f32 / state.file_bytes_total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(file_ratio).show_percentage());

            ui.horizontal(|ui| {
                let done_s = format_size(state.file_bytes_done, DECIMAL);
                let total_s = format_size(state.file_bytes_total, DECIMAL);
                let cur_file_info = i18n.t_fmt("queue.file_progress", &[("done", &done_s), ("total", &total_s)]);
                ui.label(RichText::new(cur_file_info).size(11.0).color(Color32::LIGHT_GRAY));
            });

            ui.add_space(2.0);

            // Całkowity postęp zadania
            let total_ratio = if state.bytes_total > 0 {
                (state.bytes_done as f32 / state.bytes_total as f32).clamp(0.0, 1.0)
            } else if state.items_total > 0 {
                (state.items_done as f32 / state.items_total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.label(RichText::new(i18n.t("queue.queue_progress")).strong());
            ui.add(egui::ProgressBar::new(total_ratio).show_percentage());

            ui.horizontal(|ui| {
                let done_s = format_size(state.bytes_done, DECIMAL);
                let total_s = format_size(state.bytes_total, DECIMAL);
                let items_done_s = state.items_done.min(state.items_total).to_string();
                let items_total_s = state.items_total.to_string();
                let total_info = i18n.t_fmt(
                    "queue.total_progress",
                    &[
                        ("done", &done_s),
                        ("total", &total_s),
                        ("items_done", &items_done_s),
                        ("items_total", &items_total_s),
                    ],
                );
                ui.label(RichText::new(total_info).size(11.0).color(Color32::LIGHT_GRAY));
            });

            ui.separator();

            // Metryki: Prędkość, ETA, Czas trwania
            let elapsed_secs = state.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);
            let elapsed_str = format!("{:02}:{:02}:{:02}", elapsed_secs / 3600, (elapsed_secs % 3600) / 60, elapsed_secs % 60);

            let eta_str = if let Some(eta) = state.eta_secs {
                format!("{:02}:{:02}:{:02}", eta / 3600, (eta % 3600) / 60, eta % 60)
            } else {
                "...".to_string()
            };

            let speed_str = format!("{}/s", format_size(state.speed_bps as u64, DECIMAL));

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(i18n.t("queue.speed")).size(11.0).color(Color32::GRAY));
                    ui.label(RichText::new(speed_str).color(Color32::from_rgb(100, 255, 140)).strong().monospace());
                });

                ui.add_space(20.0);

                ui.vertical(|ui| {
                    ui.label(RichText::new(i18n.t("queue.eta")).size(11.0).color(Color32::GRAY));
                    ui.label(RichText::new(eta_str).color(Color32::YELLOW).strong().monospace());
                });

                ui.add_space(20.0);

                ui.vertical(|ui| {
                    ui.label(RichText::new(i18n.t("queue.elapsed")).size(11.0).color(Color32::GRAY));
                    ui.label(RichText::new(elapsed_str).color(Color32::WHITE).monospace());
                });
            });

            ui.add_space(4.0);

            // Wykres prędkości
            ui.label(RichText::new(i18n.t("queue.speed_chart")).size(11.0).color(Color32::GRAY));
            render_speed_graph(ui, &state.speed_history, state.speed_bps, i18n);

            ui.add_space(6.0);

            // Przyciski sterowania
            ui.horizontal(|ui| {
                let bg_lbl = format!("⏬ {} (Esc)", i18n.t("dialogs.btn_background"));
                let bg_btn = egui::Button::new(RichText::new(bg_lbl).color(Color32::from_rgb(110, 200, 255)))
                    .fill(Color32::from_rgb(20, 35, 60));
                if ui.add(bg_btn).clicked() {
                    state.in_background = true;
                }

                let q_lbl = format!("📋 {}", i18n.t("queue.queue_title"));
                let queue_btn = egui::Button::new(RichText::new(q_lbl).color(Color32::from_rgb(255, 220, 100)))
                    .fill(Color32::from_rgb(30, 42, 60));
                if ui.add(queue_btn).clicked() {
                    state.show_queue_dialog = true;
                }

                let pause_label = if queue_snapshot.is_paused { i18n.t("dialogs.btn_resume") } else { i18n.t("dialogs.btn_pause") };
                if ui.button(pause_label).clicked() {
                    worker.toggle_pause();
                }

                let skip_btn = egui::Button::new(RichText::new(i18n.t("dialogs.btn_skip")).color(Color32::from_rgb(255, 200, 100)))
                    .fill(Color32::from_rgb(45, 40, 20));
                if ui.add(skip_btn).clicked() {
                    worker.skip_current();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cancel_lbl = i18n.t("queue.cancel_operation");
                    let cancel_btn = egui::Button::new(RichText::new(cancel_lbl).color(Color32::from_rgb(255, 120, 120)))
                        .fill(Color32::from_rgb(45, 25, 30));
                    if ui.add(cancel_btn).clicked() {
                        worker.cancel();
                        state.is_open = false;
                        state.in_background = false;
                    }
                });
            });
        });
}

/// Okno dialogowe zarządzania kolejką operacji w tle (Alt+F7 / menu / pasek)
pub fn render_queue_dialog(
    ctx: &egui::Context,
    state: &mut ProgressUiState,
    worker: &IoWorker,
    queue_snapshot: &QueueStateSnapshot,
    i18n: &I18n,
) {
    if !state.show_queue_dialog {
        return;
    }

    egui::Window::new(format!("📋 {}", i18n.t("queue.queue_title")))
        .collapsible(false)
        .default_size([620.0, 420.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            let is_paused = queue_snapshot.is_paused;
            let queue_len = queue_snapshot.pending_tasks.len() + if queue_snapshot.active_task.is_some() { 1 } else { 0 };

            ui.horizontal(|ui| {
                let tasks_lbl = i18n.t_fmt("queue.tasks_count", &[("count", &queue_len.to_string())]);
                ui.label(RichText::new(tasks_lbl).strong().size(14.0));

                let speed_str = format!("{}/s", format_size(state.speed_bps as u64, DECIMAL));
                ui.label(RichText::new(speed_str).color(Color32::from_rgb(100, 255, 140)).strong());

                if let Some(eta) = state.eta_secs {
                    let eta_str = format!("{:02}:{:02}:{:02}", eta / 3600, (eta % 3600) / 60, eta % 60);
                    let eta_lbl = i18n.t_fmt("queue.total_eta", &[("eta", &eta_str)]);
                    ui.label(RichText::new(eta_lbl).color(Color32::YELLOW).strong());
                }
            });

            // Pasek łącznego postępu kolejki
            let queue_ratio = if state.queue_total_bytes > 0 {
                (state.queue_bytes_done as f32 / state.queue_total_bytes as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(queue_ratio).show_percentage());
            let q_done_s = format_size(state.queue_bytes_done, DECIMAL);
            let q_total_s = format_size(state.queue_total_bytes, DECIMAL);
            let queue_prog_lbl = i18n.t_fmt(
                "queue.total_queue_progress",
                &[("done", &q_done_s), ("total", &q_total_s)],
            );
            ui.label(RichText::new(queue_prog_lbl).size(11.0).color(Color32::GRAY));

            ui.separator();

            // Karta aktywnego zadania
            if let Some(ref active) = queue_snapshot.active_task {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(20, 32, 52))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0, 160, 255)))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let status_badge = if is_paused {
                                RichText::new(format!("⏸ {}", i18n.t("queue.status_paused"))).color(Color32::YELLOW).strong()
                            } else {
                                RichText::new(format!("▶ {}", i18n.t("queue.status_running"))).color(Color32::from_rgb(100, 255, 140)).strong()
                            };
                            ui.label(status_badge);
                            ui.label(RichText::new(&active.title).strong().color(Color32::WHITE));
                            ui.label(RichText::new(&active.detail).color(Color32::LIGHT_GRAY));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(i18n.t("dialogs.btn_skip")).clicked() {
                                    worker.skip_current();
                                }
                            });
                        });

                        let file_ratio = if state.file_bytes_total > 0 {
                            (state.file_bytes_done as f32 / state.file_bytes_total as f32).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        ui.add(egui::ProgressBar::new(file_ratio).show_percentage());
                        ui.label(RichText::new(format!("{}: {}", i18n.t("queue.current_task"), state.current_item)).size(11.0).color(Color32::LIGHT_BLUE));
                    });
            } else {
                ui.label(RichText::new(i18n.t("queue.no_active_tasks")).italics().color(Color32::GRAY));
            }

            // Lista oczekujących zadań
            ui.label(RichText::new(i18n.t("queue.pending_header")).strong());
            egui::ScrollArea::vertical()
                .id_salt("dialog_queue_pending_scroll")
                .max_height(120.0)
                .show(ui, |ui| {
                if queue_snapshot.pending_tasks.is_empty() {
                    ui.label(RichText::new(i18n.t("queue.no_active_tasks")).color(Color32::GRAY));
                } else {
                    for (idx, task) in queue_snapshot.pending_tasks.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("#{}.", idx + 1)).color(Color32::YELLOW));
                            ui.label(RichText::new(&task.title).strong());
                            ui.label(RichText::new(&task.detail).color(Color32::GRAY));
                            if task.total_bytes > 0 {
                                let size_str = format_size(task.total_bytes, DECIMAL);
                                ui.label(RichText::new(format!("(~{})", size_str)).color(Color32::LIGHT_GRAY));
                            }
                        });
                    }
                }
            });

            ui.separator();

            // Przyciski sterujące
            ui.horizontal(|ui| {
                let pause_label = if is_paused { i18n.t("dialogs.btn_resume") } else { i18n.t("dialogs.btn_pause") };
                if ui.button(pause_label).clicked() {
                    worker.toggle_pause();
                }

                if ui.button(RichText::new(i18n.t("queue.cancel_all")).color(Color32::from_rgb(255, 120, 120))).clicked() {
                    worker.cancel_all();
                    state.is_open = false;
                    state.in_background = false;
                    state.show_queue_dialog = false;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_close")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                        state.show_queue_dialog = false;
                    }
                });
            });
        });
}

/// Pasek zadań w tle (wyświetlany na dole ekranu nad statusem gdy transfer działa w tle)
pub fn render_background_tasks_bar(
    ctx: &egui::Context,
    state: &mut ProgressUiState,
    worker: &IoWorker,
    queue_snapshot: &QueueStateSnapshot,
    i18n: &I18n,
) {
    if !state.is_open || !state.in_background {
        return;
    }

    egui::TopBottomPanel::bottom("background_tasks_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let queue_count = queue_snapshot.pending_tasks.len() + if queue_snapshot.active_task.is_some() { 1 } else { 0 };
            let queue_tag = if queue_count > 1 {
                i18n.t_fmt(
                    "queue.bg_queue_tag",
                    &[
                        ("index", &queue_snapshot.active_index.to_string()),
                        ("total", &queue_snapshot.total_tasks.to_string()),
                    ],
                )
            } else {
                i18n.t("queue.bg_task_tag").to_string()
            };
            ui.label(RichText::new(queue_tag).strong().color(Color32::from_rgb(0, 190, 255)));

            let speed_str = format!("{}/s", format_size(state.speed_bps as u64, DECIMAL));
            ui.label(RichText::new(speed_str).color(Color32::from_rgb(100, 255, 140)).monospace());

            let queue_ratio = if state.queue_total_bytes > 0 {
                (state.queue_bytes_done as f32 / state.queue_total_bytes as f32).clamp(0.0, 1.0)
            } else if state.bytes_total > 0 {
                (state.bytes_done as f32 / state.bytes_total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(queue_ratio).desired_width(120.0).show_percentage());

            if let Some(eta) = state.eta_secs {
                let eta_str = format!("{:02}:{:02}", (eta % 3600) / 60, eta % 60);
                ui.label(RichText::new(format!("ETA: {}", eta_str)).color(Color32::YELLOW).monospace());
            }

            ui.add(egui::Label::new(RichText::new(&state.current_item).color(Color32::from_rgb(150, 180, 220))).truncate());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let cancel_btn = egui::Button::new(RichText::new(i18n.t("queue.cancel_all")).color(Color32::from_rgb(255, 120, 120)))
                    .fill(Color32::from_rgb(45, 25, 30));
                if ui.add(cancel_btn).clicked() {
                    worker.cancel();
                    state.is_open = false;
                    state.in_background = false;
                }

                let pause_label = if queue_snapshot.is_paused { i18n.t("dialogs.btn_resume") } else { i18n.t("dialogs.btn_pause") };
                if ui.button(pause_label).clicked() {
                    worker.toggle_pause();
                }

                let skip_btn = egui::Button::new(RichText::new(i18n.t("dialogs.btn_skip")).color(Color32::from_rgb(255, 200, 100)))
                    .fill(Color32::from_rgb(45, 40, 20));
                if ui.add(skip_btn).clicked() {
                    worker.skip_current();
                }

                let q_lbl = format!("📋 {}", i18n.t("queue.queue_title"));
                let queue_btn = egui::Button::new(RichText::new(q_lbl).strong().color(Color32::from_rgb(255, 220, 100)))
                    .fill(Color32::from_rgb(30, 42, 60));
                if ui.add(queue_btn).clicked() {
                    state.show_queue_dialog = true;
                }

                let show_btn = egui::Button::new(RichText::new(i18n.t("queue.btn_chart")).strong().color(Color32::from_rgb(100, 210, 255)))
                    .fill(Color32::from_rgb(20, 35, 60));
                if ui.add(show_btn).clicked() {
                    state.in_background = false;
                }
            });
        });
    });
}
