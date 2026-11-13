use std::path::{Path, PathBuf};
use egui::{Color32, Key, RichText, Stroke};
use humansize::{format_size, DECIMAL};

use crate::app::PanelSide;
use crate::drives::DriveInfo;
use crate::i18n::I18n;
use crate::ui::state::{AboutDialogState, AboutTab, DriveMenuState, FileOpsState, LicenseViewKind};
use crate::worker::WorkerCommand;

pub enum FileOpsAction {
    EnqueueCommand(WorkerCommand),
    EnqueueCommandWithBackground(WorkerCommand),
    CreateDir { path: PathBuf, name: String },
    StatusMessage(String),
}

pub fn letter_to_key(ch: char) -> Option<Key> {
    match ch.to_ascii_uppercase() {
        'A' => Some(Key::A),
        'B' => Some(Key::B),
        'C' => Some(Key::C),
        'D' => Some(Key::D),
        'E' => Some(Key::E),
        'F' => Some(Key::F),
        'G' => Some(Key::G),
        'H' => Some(Key::H),
        'I' => Some(Key::I),
        'J' => Some(Key::J),
        'K' => Some(Key::K),
        'L' => Some(Key::L),
        'M' => Some(Key::M),
        'N' => Some(Key::N),
        'O' => Some(Key::O),
        'P' => Some(Key::P),
        'Q' => Some(Key::Q),
        'R' => Some(Key::R),
        'S' => Some(Key::S),
        'T' => Some(Key::T),
        'U' => Some(Key::U),
        'V' => Some(Key::V),
        'W' => Some(Key::W),
        'X' => Some(Key::X),
        'Y' => Some(Key::Y),
        'Z' => Some(Key::Z),
        '/' => Some(Key::Slash),
        '0' => Some(Key::Num0),
        '1' => Some(Key::Num1),
        '2' => Some(Key::Num2),
        '3' => Some(Key::Num3),
        '4' => Some(Key::Num4),
        '5' => Some(Key::Num5),
        '6' => Some(Key::Num6),
        '7' => Some(Key::Num7),
        '8' => Some(Key::Num8),
        '9' => Some(Key::Num9),
        _ => None,
    }
}

pub fn render_mkdir_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    active_dir: &Path,
    i18n: &I18n,
) -> Option<FileOpsAction> {
    if !state.show_mkdir {
        return None;
    }

    let mut action = None;

    egui::Window::new(i18n.t("dialogs.mkdir_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(i18n.t("dialogs.mkdir_prompt"));
            let text_edit = ui.text_edit_singleline(&mut state.mkdir_input);
            text_edit.request_focus();

            ui.horizontal(|ui| {
                let name = state.mkdir_input.trim().to_string();
                let can_create = !name.is_empty();
                if (ui.add_enabled(can_create, egui::Button::new(i18n.t("dialogs.mkdir_create"))).clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter)))
                    && can_create
                {
                    let new_dir_path = active_dir.join(&name);
                    action = Some(FileOpsAction::CreateDir {
                        path: new_dir_path,
                        name,
                    });
                    state.show_mkdir = false;
                }
                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_mkdir = false;
                }
            });
        });

    action
}

pub fn render_delete_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    i18n: &I18n,
) -> Option<FileOpsAction> {
    if !state.show_delete {
        return None;
    }

    let mut action = None;

    egui::Window::new(i18n.t("dialogs.delete_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let prompt = i18n.t_fmt("dialogs.delete_prompt", &[("count", &state.delete_targets.len().to_string())]);
            ui.label(prompt);
            ui.checkbox(&mut state.delete_use_trash, i18n.t("dialogs.delete_trash"));
            if !state.delete_use_trash {
                ui.label(RichText::new(i18n.t("dialogs.delete_warning")).size(11.0).color(Color32::from_rgb(255, 120, 120)));
            }

            ui.horizontal(|ui| {
                if ui.button(i18n.t("dialogs.delete_btn")).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    action = Some(FileOpsAction::EnqueueCommand(WorkerCommand::Delete {
                        targets: state.delete_targets.clone(),
                        use_trash: state.delete_use_trash,
                    }));
                    state.show_delete = false;
                }
                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_delete = false;
                }
            });
        });

    action
}

pub fn render_copy_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    in_zip: bool,
    zip_location: Option<(&Path, &str)>,
    is_busy: bool,
    i18n: &I18n,
) -> Option<FileOpsAction> {
    if !state.show_copy {
        return None;
    }

    let mut action = None;
    let title = if in_zip { "F5 - Rozpakowywanie archiwum ZIP" } else { i18n.t("dialogs.copy_title") };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let count_str = state.copy_sources.len().to_string();
            let prompt = if in_zip {
                format!("Wypakuj {} elementów do:", count_str)
            } else {
                i18n.t_fmt("dialogs.copy_prompt", &[("count", &count_str)])
            };
            ui.label(prompt);
            ui.text_edit_singleline(&mut state.copy_dest_path);

            ui.horizontal(|ui| {
                let btn_label = if is_busy {
                    i18n.t("dialogs.copy_background")
                } else if in_zip {
                    "Rozpakuj"
                } else {
                    i18n.t("dialogs.copy_btn")
                };
                if ui.button(RichText::new(btn_label).strong()).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    let target_dir = PathBuf::from(state.copy_dest_path.trim());
                    let cmd = if in_zip {
                        if let Some((archive_path, internal_dir)) = zip_location {
                            let filter_prefix = if internal_dir.is_empty() { None } else { Some(internal_dir.to_string()) };
                            Some(WorkerCommand::ExtractZip {
                                archive_path: archive_path.to_path_buf(),
                                target_dir,
                                filter_prefix,
                            })
                        } else {
                            None
                        }
                    } else {
                        Some(WorkerCommand::Copy {
                            sources: state.copy_sources.clone(),
                            target_dir,
                        })
                    };

                    if let Some(cmd) = cmd {
                        if is_busy {
                            action = Some(FileOpsAction::EnqueueCommandWithBackground(cmd));
                        } else {
                            action = Some(FileOpsAction::EnqueueCommand(cmd));
                        }
                    }
                    state.show_copy = false;
                }

                if !is_busy && ui.button(i18n.t("dialogs.copy_background")).clicked() {
                    let target_dir = PathBuf::from(state.copy_dest_path.trim());
                    let cmd = if in_zip {
                        if let Some((archive_path, internal_dir)) = zip_location {
                            let filter_prefix = if internal_dir.is_empty() { None } else { Some(internal_dir.to_string()) };
                            Some(WorkerCommand::ExtractZip {
                                archive_path: archive_path.to_path_buf(),
                                target_dir,
                                filter_prefix,
                            })
                        } else {
                            None
                        }
                    } else {
                        Some(WorkerCommand::Copy {
                            sources: state.copy_sources.clone(),
                            target_dir,
                        })
                    };

                    if let Some(cmd) = cmd {
                        action = Some(FileOpsAction::EnqueueCommandWithBackground(cmd));
                    }
                    state.show_copy = false;
                }

                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_copy = false;
                }
            });
        });

    action
}

pub fn render_move_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    is_busy: bool,
    i18n: &I18n,
) -> Option<FileOpsAction> {
    if !state.show_move {
        return None;
    }

    let mut action = None;

    egui::Window::new(i18n.t("dialogs.move_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let count_str = state.move_sources.len().to_string();
            let prompt = i18n.t_fmt("dialogs.move_prompt", &[("count", &count_str)]);
            ui.label(prompt);
            ui.text_edit_singleline(&mut state.move_dest_path);

            ui.horizontal(|ui| {
                let btn_label = if is_busy { i18n.t("dialogs.move_background") } else { i18n.t("dialogs.move_btn") };
                if ui.button(RichText::new(btn_label).strong()).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    let target_dir = PathBuf::from(state.move_dest_path.trim());
                    let cmd = WorkerCommand::Move {
                        sources: state.move_sources.clone(),
                        target_dir,
                    };
                    if is_busy {
                        action = Some(FileOpsAction::EnqueueCommandWithBackground(cmd));
                    } else {
                        action = Some(FileOpsAction::EnqueueCommand(cmd));
                    }
                    state.show_move = false;
                }

                if !is_busy && ui.button(i18n.t("dialogs.move_background")).clicked() {
                    let target_dir = PathBuf::from(state.move_dest_path.trim());
                    action = Some(FileOpsAction::EnqueueCommandWithBackground(WorkerCommand::Move {
                        sources: state.move_sources.clone(),
                        target_dir,
                    }));
                    state.show_move = false;
                }

                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_move = false;
                }
            });
        });

    action
}

pub fn render_drive_menu(
    ctx: &egui::Context,
    state: &mut DriveMenuState,
    drives: &[DriveInfo],
    left_path: &Path,
    right_path: &Path,
    i18n: &I18n,
) -> Option<(PanelSide, PathBuf)> {
    let target_side = state.side?;

    let side_title = match target_side {
        PanelSide::Left => i18n.t("dialogs.drive_title_left"),
        PanelSide::Right => i18n.t("dialogs.drive_title_right"),
    };

    let anchor_offset = match target_side {
        PanelSide::Left => egui::vec2(-180.0, -30.0),
        PanelSide::Right => egui::vec2(180.0, -30.0),
    };

    let mut chosen_drive: Option<DriveInfo> = None;
    let mut close_dialog = false;

    ctx.input(|i| {
        if i.key_pressed(Key::Escape) {
            close_dialog = true;
        }
        if i.key_pressed(Key::ArrowUp) {
            if state.cursor_idx > 0 {
                state.cursor_idx -= 1;
            } else if !drives.is_empty() {
                state.cursor_idx = drives.len() - 1;
            }
        }
        if i.key_pressed(Key::ArrowDown) && !drives.is_empty() {
            if state.cursor_idx + 1 < drives.len() {
                state.cursor_idx += 1;
            } else {
                state.cursor_idx = 0;
            }
        }
        if i.key_pressed(Key::Enter) {
            if let Some(drive) = drives.get(state.cursor_idx) {
                chosen_drive = Some(drive.clone());
            }
        }

        // Direct letter selection (e.g. typing 'C', 'D')
        for (idx, drive) in drives.iter().enumerate() {
            if let Some(k) = letter_to_key(drive.letter) {
                if i.key_pressed(k) {
                    state.cursor_idx = idx;
                    chosen_drive = Some(drive.clone());
                    break;
                }
            }
        }
    });

    egui::Window::new(side_title)
        .collapsible(false)
        .resizable(false)
        .default_width(450.0)
        .anchor(egui::Align2::CENTER_CENTER, anchor_offset)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;

            let prompt_text = if i18n.language() == crate::i18n::Language::Pl {
                "Wybierz dysk docelowy (kliknij, naciśnij literę lub strzałki + Enter):"
            } else {
                "Select target drive (click, press drive letter or arrows + Enter):"
            };
            ui.label(
                RichText::new(prompt_text)
                    .size(12.0)
                    .color(Color32::from_rgb(170, 195, 230)),
            );
            ui.separator();

            let current_panel_path = match target_side {
                PanelSide::Left => left_path,
                PanelSide::Right => right_path,
            };

            for (idx, drive) in drives.iter().enumerate() {
                let is_cursor = idx == state.cursor_idx;
                let is_current = current_panel_path.starts_with(&drive.path);

                let bg_color = if is_cursor {
                    Color32::from_rgb(0, 95, 175)
                } else if is_current {
                    Color32::from_rgb(22, 38, 68)
                } else {
                    Color32::from_rgb(14, 20, 32)
                };

                let border_stroke = if is_cursor {
                    Stroke::new(1.5_f32, Color32::from_rgb(80, 200, 255))
                } else if is_current {
                    Stroke::new(1.0_f32, Color32::from_rgb(40, 80, 130))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(25, 35, 52))
                };

                let row_resp = egui::Frame::NONE
                    .fill(bg_color)
                    .stroke(border_stroke)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Drive badge [ - C - ]
                            let badge_color = if is_cursor {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(255, 220, 100)
                            };
                            ui.label(
                                RichText::new(format!("[-{}-]", drive.letter))
                                    .strong()
                                    .monospace()
                                    .size(14.0)
                                    .color(badge_color),
                            );

                            ui.add_space(6.0);

                            // Volume label & path
                            let label_text = if !drive.volume_label.is_empty() {
                                format!("{} ({})", drive.volume_label, drive.path.to_string_lossy())
                            } else {
                                drive.path.to_string_lossy().to_string()
                            };
                            ui.label(
                                RichText::new(label_text)
                                    .strong()
                                    .size(13.0)
                                    .color(if is_cursor { Color32::WHITE } else { Color32::from_rgb(220, 230, 245) }),
                            );

                            // Right-aligned disk space and usage bar
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if drive.total_space > 0 {
                                    let free_str = format_size(drive.free_space, DECIMAL);
                                    let total_str = format_size(drive.total_space, DECIMAL);
                                    let space_info = i18n.t_fmt("dialogs.drive_free_total", &[("free", &free_str), ("total", &total_str)]);
                                    ui.label(
                                        RichText::new(space_info)
                                            .size(11.0)
                                            .monospace()
                                            .color(if is_cursor { Color32::from_rgb(200, 240, 255) } else { Color32::from_rgb(140, 160, 190) }),
                                    );

                                    let used_bytes = drive.total_space.saturating_sub(drive.free_space);
                                    let usage_ratio = (used_bytes as f32 / drive.total_space as f32).clamp(0.0, 1.0);
                                    ui.add(egui::ProgressBar::new(usage_ratio).desired_width(55.0));
                                } else {
                                    let removable_label = if i18n.language() == crate::i18n::Language::Pl { "Dysk wymienny / Sieciowy" } else { "Removable / Network drive" };
                                    ui.label(
                                        RichText::new(removable_label)
                                            .size(11.0)
                                            .color(Color32::GRAY),
                                    );
                                }
                            });
                        });
                    });

                let interact = ui.interact(row_resp.response.rect, ui.id().with(idx), egui::Sense::click());
                if interact.hovered() && !is_cursor {
                    state.cursor_idx = idx;
                }
                if interact.clicked() {
                    chosen_drive = Some(drive.clone());
                }
            }

            ui.add_space(4.0);
            ui.separator();

            ui.horizontal(|ui| {
                let hint_footer = if i18n.language() == crate::i18n::Language::Pl {
                    "Naciśnij [Enter] lub literę dysku, [Esc] aby zamknąć"
                } else {
                    "Press [Enter] or drive letter, [Esc] to close"
                };
                ui.label(
                    RichText::new(hint_footer)
                        .size(11.0)
                        .color(Color32::from_rgb(130, 150, 180)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(format!("{} (Esc)", i18n.t("dialogs.btn_close"))).clicked() {
                        close_dialog = true;
                    }
                });
            });
        });

    if close_dialog {
        state.side = None;
    }

    if let Some(drive) = chosen_drive {
        let side = target_side;
        state.side = None;
        return Some((side, drive.path));
    }

    None
}

use std::sync::OnceLock;

static ICON_COLOR_IMAGE: OnceLock<egui::ColorImage> = OnceLock::new();

pub fn get_app_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let color_image = ICON_COLOR_IMAGE.get_or_init(|| {
        let png_bytes = include_bytes!("../../assets/huzar_icon.png");
        let img = image::load_from_memory(png_bytes)
            .expect("Nie udało się załadować ikony programu")
            .into_rgba8();
        let (w, h) = img.dimensions();
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &img.into_raw())
    });

    ctx.load_texture("huzar_app_icon", color_image.clone(), egui::TextureOptions::LINEAR)
}

const LICENSE_MIT_TEXT: &str = include_str!("../../LICENSE-MIT");
const LICENSE_APACHE_TEXT: &str = include_str!("../../LICENSE-APACHE");

struct OpenSourceCrate {
    name: &'static str,
    version: &'static str,
    license: &'static str,
    desc_pl: &'static str,
    desc_en: &'static str,
    url: &'static str,
}

const CRATES: &[OpenSourceCrate] = &[
    OpenSourceCrate {
        name: "eframe",
        version: "0.31",
        license: "MIT OR Apache-2.0",
        desc_pl: "Oficjalny framework okienkowy egui i natywna pętla GUI z akceleracją Glow/OpenGL",
        desc_en: "Official framework for egui apps with hardware Glow/OpenGL acceleration",
        url: "https://crates.io/crates/eframe",
    },
    OpenSourceCrate {
        name: "egui",
        version: "0.31",
        license: "MIT OR Apache-2.0",
        desc_pl: "Silnik Immediate Mode GUI w Rust zoptymalizowany pod kątem szybkości i responsywności",
        desc_en: "Fast and lightweight immediate mode GUI library in Rust",
        url: "https://crates.io/crates/egui",
    },
    OpenSourceCrate {
        name: "egui_extras",
        version: "0.31",
        license: "MIT OR Apache-2.0",
        desc_pl: "Komponenty wirtualnych tabel TableBuilder ze scrollem dla widoków paneli",
        desc_en: "Virtual TableBuilder and extra widgets for egui panel views",
        url: "https://crates.io/crates/egui_extras",
    },
    OpenSourceCrate {
        name: "crossbeam-channel",
        version: "0.5",
        license: "MIT OR Apache-2.0",
        desc_pl: "Wielowątkowe, nieblokujące kolejki kanałów dla wątków roboczych I/O i wyszukiwarki",
        desc_en: "High-performance multi-producer multi-consumer channels for background I/O",
        url: "https://crates.io/crates/crossbeam-channel",
    },
    OpenSourceCrate {
        name: "zip",
        version: "9.5.1",
        license: "MIT",
        desc_pl: "Odczyt, wirtualne drzewo folderów, dekompresja i tworzenie archiwów ZIP",
        desc_en: "ZIP archive browsing, virtual folder tree, and Deflate compression/extraction",
        url: "https://crates.io/crates/zip",
    },
    OpenSourceCrate {
        name: "pulldown-cmark",
        version: "0.12",
        license: "MIT",
        desc_pl: "Wydajny parser CommonMark Markdown dla zaawansowanego podglądu F3",
        desc_en: "Fast CommonMark Markdown parser for F3 preview window",
        url: "https://crates.io/crates/pulldown-cmark",
    },
    OpenSourceCrate {
        name: "serde & serde_json",
        version: "1.0",
        license: "MIT OR Apache-2.0",
        desc_pl: "Serializacja i parsowanie formatu JSON, konfiguracji oraz bazy tłumaczeń i18n",
        desc_en: "Serialization framework and JSON parser for config and i18n translations",
        url: "https://crates.io/crates/serde_json",
    },
    OpenSourceCrate {
        name: "serde_yaml",
        version: "0.9",
        license: "MIT OR Apache-2.0",
        desc_pl: "Parsowanie i podgląd strukturalny plików YAML w oknie F3",
        desc_en: "YAML parser and serializer for F3 structured file viewer",
        url: "https://crates.io/crates/serde_yaml",
    },
    OpenSourceCrate {
        name: "trash",
        version: "5.2",
        license: "MIT",
        desc_pl: "Bezpieczne usuwanie plików do systemowego kosza Windows i Linux",
        desc_en: "Cross-platform system recycle bin / trash integration",
        url: "https://crates.io/crates/trash",
    },
    OpenSourceCrate {
        name: "open",
        version: "5.3",
        license: "MIT",
        desc_pl: "Otwieranie plików i odnośników w domyślnych programach powiązanych w systemie",
        desc_en: "Open paths and URLs with system default applications",
        url: "https://crates.io/crates/open",
    },
    OpenSourceCrate {
        name: "walkdir",
        version: "2.5",
        license: "Unlicense OR MIT",
        desc_pl: "Rekurencyjne skanowanie drzewa folderów dla obliczania rozmiaru i wyszukiwania",
        desc_en: "Recursive directory walker for size calculation and search",
        url: "https://crates.io/crates/walkdir",
    },
    OpenSourceCrate {
        name: "humansize",
        version: "2.1",
        license: "MIT OR Apache-2.0",
        desc_pl: "Czytelna konwersja rozmiarów plików na jednostki B, KB, MB, GB, TB",
        desc_en: "Human-readable file size formatting (B, KB, MB, GB, TB)",
        url: "https://crates.io/crates/humansize",
    },
    OpenSourceCrate {
        name: "chrono",
        version: "0.4",
        license: "MIT OR Apache-2.0",
        desc_pl: "Formatowanie dat i stempli czasowych ostatniej modyfikacji plików",
        desc_en: "Date and time library for file modification timestamps",
        url: "https://crates.io/crates/chrono",
    },
    OpenSourceCrate {
        name: "image",
        version: "0.25",
        license: "MIT OR Apache-2.0",
        desc_pl: "Dekodowanie grafiki rastrowej PNG dla ikony aplikacji i podglądu",
        desc_en: "Image rasterization and decoding for application icon and preview",
        url: "https://crates.io/crates/image",
    },
    OpenSourceCrate {
        name: "libc",
        version: "0.2",
        license: "MIT OR Apache-2.0",
        desc_pl: "Niskopoziomowe wiązania systemowe C dla platform Unix / Linux",
        desc_en: "Low-level C bindings for Unix / Linux operating systems",
        url: "https://crates.io/crates/libc",
    },
];

pub fn render_about_dialog(ctx: &egui::Context, state: &mut AboutDialogState, i18n: &I18n) {
    if !state.is_open {
        return;
    }

    let icon_texture = get_app_icon_texture(ctx);
    let mut open = state.is_open;
    let mut close_requested = false;

    egui::Window::new(format!("ℹ {}", i18n.t("dialogs.about_title")))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .default_height(480.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            // Nagłówek wspólny z logo husarii
            ui.horizontal(|ui| {
                ui.add(egui::Image::from_texture(&icon_texture).fit_to_exact_size(egui::vec2(56.0, 56.0)));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Huzar Commander").size(19.0).strong().color(Color32::from_rgb(0, 190, 255)));
                        ui.label(RichText::new(format!("{} 0.1.0", i18n.t("dialogs.about_version"))).size(13.0).color(Color32::from_rgb(150, 175, 210)));
                    });
                    ui.label(
                        RichText::new(i18n.t("dialogs.about_author"))
                            .size(12.0)
                            .color(Color32::from_rgb(205, 220, 240)),
                    );
                    let hussar_sub = if i18n.language() == crate::i18n::Language::Pl {
                        "Dwa skrzydła husarii — niezawodność, bezpieczeństwo i prędkość szarży I/O"
                    } else {
                        "Two wings of the Polish Hussars — reliability, memory safety, and lightning I/O"
                    };
                    ui.label(RichText::new(hussar_sub).size(11.0).italics().color(Color32::from_rgb(230, 190, 120)));
                });
            });

            ui.add_space(2.0);
            ui.separator();

            // Pasek zakładek (Tabs)
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.current_tab, AboutTab::Overview, i18n.t("dialogs.about_tab_overview"));
                ui.selectable_value(&mut state.current_tab, AboutTab::License, i18n.t("dialogs.about_tab_license"));
                ui.selectable_value(&mut state.current_tab, AboutTab::Libraries, i18n.t("dialogs.about_tab_libraries"));
            });

            ui.separator();

            match state.current_tab {
                AboutTab::Overview => {
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(14, 20, 32))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(35, 50, 75)))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.label(RichText::new(i18n.t("dialogs.about_shortcuts_header")).strong().color(Color32::from_rgb(255, 220, 100)));
                            let shortcuts = [
                                i18n.t("dialogs.about_f1"),
                                i18n.t("dialogs.about_f3"),
                                i18n.t("dialogs.about_f4"),
                                i18n.t("dialogs.about_f5"),
                                i18n.t("dialogs.about_f6"),
                                i18n.t("dialogs.about_f7"),
                                i18n.t("dialogs.about_f8"),
                                i18n.t("dialogs.about_alt_f7"),
                                i18n.t("dialogs.about_tab"),
                                i18n.t("dialogs.about_space"),
                                i18n.t("dialogs.about_alt_f1_f2"),
                            ];
                            for sc in shortcuts {
                                ui.label(RichText::new(format!("• {}", sc)).size(11.5).color(Color32::from_rgb(190, 205, 225)));
                            }
                        });

                    ui.horizontal(|ui| {
                        let lic_info = if i18n.language() == crate::i18n::Language::Pl {
                            "Licencja: MIT lub Apache 2.0 (Dual License Open Source)"
                        } else {
                            "License: MIT or Apache 2.0 (Dual License Open Source)"
                        };
                        ui.label(RichText::new(lic_info).size(11.5).color(Color32::from_rgb(140, 200, 255)));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn_lbl = if i18n.language() == crate::i18n::Language::Pl {
                                "Zobacz licencję i biblioteki ➔"
                            } else {
                                "View license & libraries ➔"
                            };
                            if ui.button(btn_lbl).clicked() {
                                state.current_tab = AboutTab::License;
                            }
                        });
                    });
                }
                AboutTab::License => {
                    ui.label(RichText::new(i18n.t("dialogs.about_license_header")).size(12.0).color(Color32::from_rgb(220, 230, 245)));

                    let current_license_text = match state.selected_license {
                        LicenseViewKind::Mit => LICENSE_MIT_TEXT,
                        LicenseViewKind::Apache2 => LICENSE_APACHE_TEXT,
                    };

                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut state.selected_license, LicenseViewKind::Mit, i18n.t("dialogs.about_license_mit_desc"));
                        ui.selectable_value(&mut state.selected_license, LicenseViewKind::Apache2, i18n.t("dialogs.about_license_apache_desc"));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(i18n.t("dialogs.about_license_copy")).clicked() {
                                ctx.copy_text(current_license_text.to_string());
                                state.copy_notice_timer = Some(std::time::Instant::now());
                            }
                            if let Some(t) = state.copy_notice_timer {
                                if t.elapsed().as_secs() < 3 {
                                    ui.label(RichText::new(format!("✓ {}", i18n.t("dialogs.about_license_copied"))).size(11.0).color(Color32::from_rgb(100, 240, 150)));
                                }
                            }
                        });
                    });

                    egui::Frame::NONE
                        .fill(Color32::from_rgb(10, 14, 22))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 45, 70)))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(240.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(current_license_text)
                                                .font(egui::FontId::monospace(10.5))
                                                .color(Color32::from_rgb(180, 200, 220)),
                                        )
                                        .wrap(),
                                    );
                                });
                        });
                }
                AboutTab::Libraries => {
                    ui.label(RichText::new(i18n.t("dialogs.about_libraries_header")).size(12.0).color(Color32::from_rgb(220, 230, 245)));

                    egui::ScrollArea::vertical()
                        .max_height(250.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;

                            for c in CRATES {
                                egui::Frame::NONE
                                    .fill(Color32::from_rgb(15, 21, 33))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(32, 46, 68)))
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(c.name).strong().size(13.0).color(Color32::from_rgb(80, 200, 255)));
                                            ui.label(RichText::new(format!("v{}", c.version)).size(11.0).color(Color32::from_rgb(140, 160, 185)));

                                            let badge_color = if c.license.contains("MIT") {
                                                Color32::from_rgb(120, 220, 140)
                                            } else {
                                                Color32::from_rgb(240, 200, 100)
                                            };
                                            ui.label(
                                                RichText::new(format!("[{}]", c.license))
                                                    .size(10.5)
                                                    .color(badge_color),
                                            );

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.small_button(i18n.t("dialogs.about_btn_visit")).clicked() {
                                                    let _ = open::that(c.url);
                                                }
                                            });
                                        });

                                        let desc = if i18n.language() == crate::i18n::Language::Pl {
                                            c.desc_pl
                                        } else {
                                            c.desc_en
                                        };
                                        ui.label(RichText::new(desc).size(11.0).color(Color32::from_rgb(175, 195, 220)));
                                    });
                            }
                        });
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new("Esc / Enter").size(11.0).color(Color32::GRAY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n.t("dialogs.btn_close")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
                    {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        open = false;
    }
    state.is_open = open;
}

pub fn render_rename_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    i18n: &I18n,
) -> Option<FileOpsAction> {
    if !state.show_rename {
        return None;
    }

    let mut action = None;

    egui::Window::new(i18n.t("dialogs.rename_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(i18n.t("dialogs.rename_prompt"));
            let text_edit = ui.text_edit_singleline(&mut state.rename_input);
            text_edit.request_focus();

            ui.horizontal(|ui| {
                let can_rename = !state.rename_input.trim().is_empty();
                if (ui.add_enabled(can_rename, egui::Button::new(i18n.t("dialogs.rename_btn"))).clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter)))
                    && can_rename
                {
                    if let Some(ref src) = state.rename_source {
                        action = Some(FileOpsAction::EnqueueCommand(WorkerCommand::Rename {
                            source: src.clone(),
                            new_name: state.rename_input.trim().to_string(),
                        }));
                    }
                    state.show_rename = false;
                }
                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_rename = false;
                }
            });
        });

    action
}

pub fn render_pack_zip_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    is_busy: bool,
    i18n: &I18n,
) -> Option<FileOpsAction> {
    if !state.show_pack_zip {
        return None;
    }

    let mut action = None;

    egui::Window::new(i18n.t("dialogs.zip_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(i18n.t("dialogs.zip_prompt"));
            ui.text_edit_singleline(&mut state.pack_zip_dest);

            ui.horizontal(|ui| {
                let can_pack = !state.pack_zip_dest.trim().is_empty();
                let btn_label = if is_busy {
                    i18n.t("dialogs.copy_background")
                } else {
                    i18n.t("dialogs.zip_btn")
                };
                if (ui
                    .add_enabled(can_pack, egui::Button::new(RichText::new(btn_label).strong()))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter)))
                    && can_pack
                {
                    let archive_path = PathBuf::from(state.pack_zip_dest.trim());
                    action = Some(FileOpsAction::EnqueueCommand(WorkerCommand::CreateZip {
                        sources: state.pack_zip_sources.clone(),
                        archive_path,
                    }));
                    if is_busy {
                        action = Some(FileOpsAction::StatusMessage(
                            "Dodano zadanie pakowania do kolejki w tle".to_string(),
                        ));
                    }
                    state.show_pack_zip = false;
                }

                if !is_busy && can_pack && ui.button(i18n.t("dialogs.copy_background")).clicked() {
                    let archive_path = PathBuf::from(state.pack_zip_dest.trim());
                    action = Some(FileOpsAction::EnqueueCommandWithBackground(
                        WorkerCommand::CreateZip {
                            sources: state.pack_zip_sources.clone(),
                            archive_path,
                        },
                    ));
                    state.show_pack_zip = false;
                }

                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_pack_zip = false;
                }
            });
        });

    action
}

pub fn render_mask_select_dialog(
    ctx: &egui::Context,
    state: &mut FileOpsState,
    i18n: &I18n,
) -> Option<(bool, String)> {
    if !state.show_mask_select {
        return None;
    }

    let mut apply = None;
    let title = if state.mask_is_select {
        i18n.t("dialogs.mask_select_title")
    } else {
        i18n.t("dialogs.mask_deselect_title")
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(i18n.t("dialogs.mask_prompt"));
            let text_edit = ui.text_edit_singleline(&mut state.mask_input);
            text_edit.request_focus();

            ui.horizontal(|ui| {
                let btn_text = i18n.t("dialogs.btn_apply");
                if (ui.button(RichText::new(btn_text).strong()).clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter)))
                    && !state.mask_input.trim().is_empty()
                {
                    apply = Some((state.mask_is_select, state.mask_input.trim().to_string()));
                    state.show_mask_select = false;
                }
                if ui.button(i18n.t("dialogs.btn_cancel")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.show_mask_select = false;
                }
            });
        });

    apply
}

pub fn render_properties_dialog(
    ctx: &egui::Context,
    state: &mut crate::ui::state::PropertiesDialogState,
    i18n: &I18n,
) {
    if !state.is_open {
        return;
    }

    egui::Window::new(format!("ℹ {}: {}", i18n.t("dialogs.props_title"), state.name))
        .collapsible(false)
        .resizable(false)
        .default_width(450.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;

            let icon = if state.is_dir { "📁" } else { "📄" };
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(24.0));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&state.name)
                            .strong()
                            .size(15.0)
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(state.path.to_string_lossy())
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                });
            });

            ui.separator();

            egui::Grid::new("prop_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new(i18n.t("dialogs.props_type")).strong());
                    ui.label(if state.is_dir {
                        i18n.t("dialogs.props_type_dir")
                    } else {
                        i18n.t("dialogs.props_type_file")
                    });
                    ui.end_row();

                    ui.label(RichText::new(i18n.t("dialogs.props_size")).strong());
                    let size_text =
                        format!("{} ({} B)", format_size(state.size, DECIMAL), state.size);
                    ui.label(size_text);
                    ui.end_row();

                    if state.is_dir {
                        let contains_lbl = if i18n.language() == crate::i18n::Language::Pl { "Zawiera:" } else { "Contains:" };
                        let items_lbl = if i18n.language() == crate::i18n::Language::Pl {
                            format!("{} plików, {} folderów", state.files_count, state.dirs_count)
                        } else {
                            format!("{} files, {} folders", state.files_count, state.dirs_count)
                        };
                        ui.label(RichText::new(contains_lbl).strong());
                        ui.label(items_lbl);
                        ui.end_row();
                    }

                    if let Some(ref mod_date) = state.modified {
                        ui.label(RichText::new(i18n.t("dialogs.props_modified")).strong());
                        ui.label(mod_date);
                        ui.end_row();
                    }

                    let attrs_lbl = if i18n.language() == crate::i18n::Language::Pl { "Atrybuty:" } else { "Attributes:" };
                    ui.label(RichText::new(attrs_lbl).strong());
                    let mut attrs = Vec::new();
                    if state.readonly {
                        attrs.push(i18n.t("dialogs.props_readonly"));
                    }
                    if state.hidden {
                        let hidden_lbl = if i18n.language() == crate::i18n::Language::Pl { "Ukryty" } else { "Hidden" };
                        attrs.push(hidden_lbl);
                    }
                    if attrs.is_empty() {
                        let normal_lbl = if i18n.language() == crate::i18n::Language::Pl { "Zwykły (A)" } else { "Normal (A)" };
                        attrs.push(normal_lbl);
                    }
                    ui.label(attrs.join(", "));
                    ui.end_row();
                });

            ui.separator();

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(format!("{} (Esc)", i18n.t("dialogs.btn_close"))).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
                    {
                        state.is_open = false;
                    }
                });
            });
        });
}


