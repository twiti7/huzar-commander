use egui::{Key, RichText, Stroke};

use crate::app::PanelSide;
use crate::config::ThemeKind;
use crate::i18n::{I18n, Language};
use crate::ui::theme::ThemePalette;

pub fn theme_label_key(kind: ThemeKind) -> &'static str {
    match kind {
        ThemeKind::MidnightDark => "menu.theme_midnight",
        ThemeKind::ClassicNortonBlue => "menu.theme_norton",
        ThemeKind::Light => "menu.theme_light",
        ThemeKind::HighContrast => "menu.theme_high_contrast",
    }
}

pub enum MenuAction {
    TriggerF1About,
    TriggerF3View,
    TriggerF4Edit,
    TriggerF5Copy,
    TriggerF6Move,
    TriggerRename,
    TriggerPackZip,
    TriggerProperties,
    TriggerCalculateDirSize,
    TriggerF7Mkdir,
    TriggerF8Delete,
    ExitApp,

    SelectAll,
    DeselectAll,
    InvertSelection,
    SelectGroup,
    DeselectGroup,

    OpenSearchDialog,
    OpenBookmarks,
    ToggleQuickView,
    OpenMultiRename,
    CompareDirs,
    CompareFiles,
    NewTab,
    CloseTab,
    OpenQueueDialog,
    ToggleFilter,
    ToggleHidden,
    SwapPanels,
    OpenDriveMenu(PanelSide),
    RefreshPanels,
    OpenTerminal,
    OpenConfigDialog,
    OpenLicensesDialog,
    SetLanguage(Option<Language>),
    SetTheme(ThemeKind),
}

pub fn render_top_menu_bar(
    ctx: &egui::Context,
    i18n: &I18n,
    config_lang: Option<&str>,
    current_theme: ThemeKind,
    palette: &ThemePalette,
) -> Option<MenuAction> {
    let mut action = None;

    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            let m_file = ui.menu_button(i18n.t("menu.file"), |ui| {
                if ui.button(i18n.t("menu.view")).clicked() {
                    action = Some(MenuAction::TriggerF3View);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.edit")).clicked() {
                    action = Some(MenuAction::TriggerF4Edit);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.copy")).clicked() {
                    action = Some(MenuAction::TriggerF5Copy);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.move")).clicked() {
                    action = Some(MenuAction::TriggerF6Move);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.rename")).clicked() {
                    action = Some(MenuAction::TriggerRename);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.pack_zip")).clicked() {
                    action = Some(MenuAction::TriggerPackZip);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.new_folder")).clicked() {
                    action = Some(MenuAction::TriggerF7Mkdir);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.delete")).clicked() {
                    action = Some(MenuAction::TriggerF8Delete);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(i18n.t("menu.properties")).clicked() {
                    action = Some(MenuAction::TriggerProperties);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.calc_dir_size")).clicked() {
                    action = Some(MenuAction::TriggerCalculateDirSize);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(i18n.t("menu.exit")).clicked() {
                    action = Some(MenuAction::ExitApp);
                }
            });
            m_file.response.surrender_focus();

            let m_sel = ui.menu_button(i18n.t("menu.select"), |ui| {
                if ui.button(i18n.t("menu.select_all")).clicked() {
                    action = Some(MenuAction::SelectAll);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.deselect_all")).clicked() {
                    action = Some(MenuAction::DeselectAll);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.invert_selection")).clicked() {
                    action = Some(MenuAction::InvertSelection);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(i18n.t("menu.select_group")).clicked() {
                    action = Some(MenuAction::SelectGroup);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.deselect_group")).clicked() {
                    action = Some(MenuAction::DeselectGroup);
                    ui.close_menu();
                }
            });
            m_sel.response.surrender_focus();

            let m_cmd = ui.menu_button(i18n.t("menu.commands"), |ui| {
                if ui.button(i18n.t("menu.search")).clicked() {
                    action = Some(MenuAction::OpenSearchDialog);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.bookmarks")).clicked() {
                    action = Some(MenuAction::OpenBookmarks);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.quick_view")).clicked() {
                    action = Some(MenuAction::ToggleQuickView);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.multi_rename")).clicked() {
                    action = Some(MenuAction::OpenMultiRename);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.compare_dirs")).clicked() {
                    action = Some(MenuAction::CompareDirs);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.compare_files")).clicked() {
                    action = Some(MenuAction::CompareFiles);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.new_tab")).clicked() {
                    action = Some(MenuAction::NewTab);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.close_tab")).clicked() {
                    action = Some(MenuAction::CloseTab);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.queue")).clicked() {
                    action = Some(MenuAction::OpenQueueDialog);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.filter")).clicked() {
                    action = Some(MenuAction::ToggleFilter);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.toggle_hidden")).clicked() {
                    action = Some(MenuAction::ToggleHidden);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(i18n.t("menu.left_drive")).clicked() {
                    action = Some(MenuAction::OpenDriveMenu(PanelSide::Left));
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.right_drive")).clicked() {
                    action = Some(MenuAction::OpenDriveMenu(PanelSide::Right));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(i18n.t("menu.swap_panels")).clicked() {
                    action = Some(MenuAction::SwapPanels);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.refresh")).clicked() {
                    action = Some(MenuAction::RefreshPanels);
                    ui.close_menu();
                }
                if ui.button(i18n.t("menu.open_terminal")).clicked() {
                    action = Some(MenuAction::OpenTerminal);
                    ui.close_menu();
                }
            });
            m_cmd.response.surrender_focus();

            let m_opt = ui.menu_button(i18n.t("menu.options"), |ui| {
                if ui.button(i18n.t("menu.configure_editors")).clicked() {
                    action = Some(MenuAction::OpenConfigDialog);
                    ui.close_menu();
                }
                ui.menu_button(i18n.t("menu.theme"), |ui| {
                    for theme in ThemeKind::ALL {
                        let is_selected = current_theme == theme;
                        if ui
                            .selectable_label(is_selected, i18n.t(theme_label_key(theme)))
                            .clicked()
                        {
                            action = Some(MenuAction::SetTheme(theme));
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                ui.menu_button(i18n.t("menu.language"), |ui| {
                    let is_system = config_lang.is_none();
                    if ui.selectable_label(is_system, i18n.t("menu.lang_system")).clicked() {
                        action = Some(MenuAction::SetLanguage(None));
                        ui.close_menu();
                    }
                    ui.separator();
                    let is_pl = config_lang == Some("pl") || (is_system && i18n.language() == Language::Pl);
                    if ui.selectable_label(is_pl, "Polski (PL)").clicked() {
                        action = Some(MenuAction::SetLanguage(Some(Language::Pl)));
                        ui.close_menu();
                    }
                    let is_en = config_lang == Some("en") || (is_system && i18n.language() == Language::En);
                    if ui.selectable_label(is_en, "English (EN)").clicked() {
                        action = Some(MenuAction::SetLanguage(Some(Language::En)));
                        ui.close_menu();
                    }
                });
            });
            m_opt.response.surrender_focus();

            let m_help = ui.menu_button(i18n.t("menu.help"), |ui| {
                if ui.button(i18n.t("menu.about")).clicked() {
                    action = Some(MenuAction::TriggerF1About);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(i18n.t("menu.licenses")).clicked() {
                    action = Some(MenuAction::OpenLicensesDialog);
                    ui.close_menu();
                }
            });
            m_help.response.surrender_focus();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let icon_tex = crate::ui::file_dialogs::get_app_icon_texture(ctx);
                let brand_resp = ui.horizontal(|ui| {
                    ui.label(RichText::new("Huzar Commander").size(12.0).strong().color(palette.accent));
                    ui.add(egui::Image::from_texture(&icon_tex).fit_to_exact_size(egui::vec2(16.0, 16.0)));
                }).response;
                if brand_resp.interact(egui::Sense::click()).clicked() {
                    action = Some(MenuAction::TriggerF1About);
                }
            });
        });
    });

    action
}

pub fn render_bottom_f_keys(ctx: &egui::Context, i18n: &I18n, palette: &ThemePalette) -> Option<MenuAction> {
    let mut action = None;

    egui::TopBottomPanel::bottom("f_keys_bar").show(ctx, |ui| {
        let f_buttons = [
            ("F1", i18n.t("f_keys.f1"), 1),
            ("F3", i18n.t("f_keys.f3"), 3),
            ("F4", i18n.t("f_keys.f4"), 4),
            ("F5", i18n.t("f_keys.f5"), 5),
            ("F6", i18n.t("f_keys.f6"), 6),
            ("F7", i18n.t("f_keys.f7"), 7),
            ("F8", i18n.t("f_keys.f8"), 8),
            ("Alt+F4", i18n.t("f_keys.f10"), 10),
        ];

        let count = f_buttons.len();
        let total_width = ui.available_width();
        let btn_width = ((total_width - (count as f32 - 1.0) * 4.0) / count as f32).max(60.0);

        ui.horizontal(|ui| {
            for (key_badge, label, key_num) in f_buttons {
                let text = RichText::new(format!("{} {}", key_badge, label))
                    .size(12.0)
                    .color(palette.fkey_btn_fg)
                    .strong();

                let btn = egui::Button::new(text)
                    .fill(palette.fkey_btn_bg)
                    .stroke(Stroke::new(1.0_f32, palette.fkey_btn_border))
                    .min_size(egui::vec2(btn_width, 24.0));

                let resp = ui.add(btn);
                if resp.clicked() {
                    action = match key_num {
                        1 => Some(MenuAction::TriggerF1About),
                        3 => Some(MenuAction::TriggerF3View),
                        4 => Some(MenuAction::TriggerF4Edit),
                        5 => Some(MenuAction::TriggerF5Copy),
                        6 => Some(MenuAction::TriggerF6Move),
                        7 => Some(MenuAction::TriggerF7Mkdir),
                        8 => Some(MenuAction::TriggerF8Delete),
                        10 => Some(MenuAction::ExitApp),
                        _ => None,
                    };
                }
                resp.surrender_focus();
            }
        });
    });

    action
}

#[allow(clippy::too_many_arguments)]
pub fn render_cli_status_bar(
    ctx: &egui::Context,
    active_path: &str,
    cmd_line: &mut String,
    cmd_history: &[String],
    cmd_history_idx: &mut Option<usize>,
    focus_cli: &mut bool,
    status_bar_msg: Option<&(String, std::time::Instant)>,
    palette: &ThemePalette,
) -> bool {
    let mut execute_cmd = false;

    egui::TopBottomPanel::bottom("cmd_status_bar").show(ctx, |ui| {
        // Command prompt line
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}>", active_path)).color(palette.cli_prompt).monospace());
            let resp = ui.add(
                egui::TextEdit::singleline(cmd_line).desired_width(ui.available_width().max(120.0)),
            );
            if *focus_cli {
                resp.request_focus();
                *focus_cli = false;
            }
            if resp.has_focus() && !cmd_history.is_empty() {
                let (up, down) = ui.input(|i| {
                    (
                        !i.modifiers.any() && i.key_pressed(Key::ArrowUp),
                        !i.modifiers.any() && i.key_pressed(Key::ArrowDown),
                    )
                });
                if up {
                    let next_idx = match *cmd_history_idx {
                        None => cmd_history.len().saturating_sub(1),
                        Some(idx) => idx.saturating_sub(1),
                    };
                    *cmd_history_idx = Some(next_idx);
                    if let Some(entry) = cmd_history.get(next_idx) {
                        *cmd_line = entry.clone();
                    }
                } else if down {
                    match *cmd_history_idx {
                        Some(idx) if idx + 1 < cmd_history.len() => {
                            let next_idx = idx + 1;
                            *cmd_history_idx = Some(next_idx);
                            if let Some(entry) = cmd_history.get(next_idx) {
                                *cmd_line = entry.clone();
                            }
                        }
                        Some(_) => {
                            *cmd_history_idx = None;
                            cmd_line.clear();
                        }
                        None => {}
                    }
                }
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                execute_cmd = true;
            }
        });

        // Status message
        if let Some((msg, time)) = status_bar_msg {
            if time.elapsed().as_secs() < 5 {
                ui.label(RichText::new(msg).color(palette.status_msg));
            }
        }
    });

    execute_cmd
}
