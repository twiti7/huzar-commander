use std::path::{Path, PathBuf};

use egui::{Color32, RichText, Stroke};

use crate::fs_item::{FileItem, SortColumn, SortOrder};
use crate::i18n::I18n;
use crate::ui::state::{ContextMenuState, ContextMenuSubmenu, ContextMenuTarget};

pub mod icons {
    pub const OPEN_FOLDER: &str = "📂";
    pub const OPEN_FILE: &str = "▶";
    pub const VIEW: &str = "👁";
    pub const EDIT: &str = "✏";
    pub const OPEN_WITH: &str = "⚙";
    pub const RUN_AS_ADMIN: &str = "🛡";
    pub const CUT: &str = "✂";
    pub const COPY: &str = "📋";
    pub const PASTE: &str = "📥";
    pub const COPY_AS_PATH: &str = "🔗";
    pub const COPY_PANEL: &str = "➡";
    pub const MOVE_PANEL: &str = "🚚";
    pub const EXTRACT: &str = "📦";
    pub const EXTRACT_OUT: &str = "📤";
    pub const FOLDER: &str = "📁";
    pub const COMPRESS_ZIP: &str = "📦";
    pub const CALC_SIZE: &str = "📏";
    pub const RENAME: &str = "🏷";
    pub const DELETE: &str = "🗑";
    pub const EXPLORER: &str = "🔍";
    pub const TERMINAL: &str = "💻";
    pub const PROPERTIES: &str = "ℹ";
    pub const WIN_PROPERTIES: &str = "🔧";
    pub const OTHER_PANEL: &str = "↔";
    pub const REFRESH: &str = "🔄";
    pub const SORT_BY: &str = "↕";
    pub const NEW_ITEM: &str = "✨";
    pub const SELECT_ALL: &str = "☑";
    pub const DESELECT_ALL: &str = "☐";
    pub const CHECKMARK: &str = "✔";
    pub const TEXT_FILE: &str = "📄";
    pub const CUSTOM_FILE: &str = "📝";
    pub const SUBMENU_ARROW: &str = "▸";
}

pub const ALL_MENU_ICONS: &[(&str, &str)] = &[
    ("OPEN_FOLDER", icons::OPEN_FOLDER),
    ("OPEN_FILE", icons::OPEN_FILE),
    ("VIEW", icons::VIEW),
    ("EDIT", icons::EDIT),
    ("OPEN_WITH", icons::OPEN_WITH),
    ("RUN_AS_ADMIN", icons::RUN_AS_ADMIN),
    ("CUT", icons::CUT),
    ("COPY", icons::COPY),
    ("PASTE", icons::PASTE),
    ("COPY_AS_PATH", icons::COPY_AS_PATH),
    ("COPY_PANEL", icons::COPY_PANEL),
    ("MOVE_PANEL", icons::MOVE_PANEL),
    ("EXTRACT", icons::EXTRACT),
    ("EXTRACT_OUT", icons::EXTRACT_OUT),
    ("FOLDER", icons::FOLDER),
    ("COMPRESS_ZIP", icons::COMPRESS_ZIP),
    ("CALC_SIZE", icons::CALC_SIZE),
    ("RENAME", icons::RENAME),
    ("DELETE", icons::DELETE),
    ("EXPLORER", icons::EXPLORER),
    ("TERMINAL", icons::TERMINAL),
    ("PROPERTIES", icons::PROPERTIES),
    ("WIN_PROPERTIES", icons::WIN_PROPERTIES),
    ("OTHER_PANEL", icons::OTHER_PANEL),
    ("REFRESH", icons::REFRESH),
    ("SORT_BY", icons::SORT_BY),
    ("NEW_ITEM", icons::NEW_ITEM),
    ("SELECT_ALL", icons::SELECT_ALL),
    ("DESELECT_ALL", icons::DESELECT_ALL),
    ("CHECKMARK", icons::CHECKMARK),
    ("TEXT_FILE", icons::TEXT_FILE),
    ("CUSTOM_FILE", icons::CUSTOM_FILE),
    ("SUBMENU_ARROW", icons::SUBMENU_ARROW),
];

pub enum ContextMenuAction {
    OpenItem(FileItem),
    OpenInOtherPanel(PathBuf),
    OpenWith(PathBuf),
    RunAsAdmin(PathBuf),
    ViewF3,
    EditF4,
    Cut,
    Copy,
    Paste(PathBuf),
    CopyAsPath(Vec<PathBuf>),
    CopyName(String),
    CopyToOtherPanelF5,
    MoveToOtherPanelF6,
    PackZip,
    ExtractZipHere(PathBuf),
    ExtractZipToFolder(PathBuf),
    CalculateDirSize,
    Rename,
    Delete,
    NewFolder,
    NewTextFile(PathBuf),
    NewCustomFile,
    Refresh,
    SortBy(SortColumn),
    SetSortOrder(SortOrder),
    SelectAll,
    DeselectAll,
    ToggleHidden,
    OpenTerminal(PathBuf),
    ShowInExplorer { path: PathBuf, select_item: bool },
    Properties(Option<PathBuf>),
    SystemProperties(PathBuf),
}

struct MenuRowParams<'a> {
    icon: &'a str,
    label: &'a str,
    shortcut: &'a str,
    bold: bool,
    enabled: bool,
    danger: bool,
    close_submenu_on_hover: bool,
}

fn with_count_badge(label: &str, selected_count: usize) -> String {
    if selected_count > 1 {
        format!("{} ({})", label, selected_count)
    } else {
        label.to_string()
    }
}

fn menu_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(Color32::from_rgb(20, 28, 44))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 85, 130)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(4, 5))
        .shadow(egui::epaint::Shadow {
            offset: [2, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(150),
        })
}

fn menu_separator(ui: &mut egui::Ui) {
    let width = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 7.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let y = rect.center().y;
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 6.0, y),
                egui::pos2(rect.right() - 6.0, y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(45, 62, 92)),
        );
    }
}

fn menu_item_row(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    params: MenuRowParams<'_>,
) -> bool {
    let height = 24.0_f32;
    let width = ui.available_width().max(200.0);
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(width, height),
        if params.enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let hovered = params.enabled && resp.hovered();
    if hovered && params.close_submenu_on_hover {
        state.open_submenu = ContextMenuSubmenu::None;
    }

    if ui.is_rect_visible(rect) {
        if hovered {
            let bg = if params.danger {
                Color32::from_rgb(150, 40, 45)
            } else {
                Color32::from_rgb(0, 95, 175)
            };
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(4), bg);
        }

        let text_color = if !params.enabled {
            Color32::from_rgb(100, 115, 135)
        } else if hovered {
            Color32::WHITE
        } else if params.danger {
            Color32::from_rgb(255, 130, 130)
        } else {
            Color32::from_rgb(220, 230, 245)
        };

        let inner_rect = rect.shrink2(egui::vec2(8.0, 2.0));
        let mut row_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        row_ui.add_sized(
            [20.0, 20.0],
            egui::Label::new(RichText::new(params.icon).size(13.0).color(text_color))
                .selectable(false),
        );

        let mut rt = RichText::new(params.label).size(13.0).color(text_color);
        if params.bold {
            rt = rt.strong();
        }
        row_ui.add(egui::Label::new(rt).selectable(false));

        if !params.shortcut.is_empty() {
            row_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let sc_color = if !params.enabled {
                    Color32::from_rgb(85, 98, 118)
                } else if hovered {
                    Color32::from_rgb(220, 235, 255)
                } else {
                    Color32::from_rgb(135, 150, 175)
                };
                ui.add(
                    egui::Label::new(RichText::new(params.shortcut).size(11.5).color(sc_color))
                        .selectable(false),
                );
            });
        }
    }

    params.enabled && resp.clicked()
}

fn submenu_trigger_row(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    icon: &str,
    label: &str,
    submenu_kind: ContextMenuSubmenu,
) {
    let height = 24.0_f32;
    let width = ui.available_width().max(200.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let is_open = state.open_submenu == submenu_kind;
    let hovered = resp.hovered();

    if hovered || resp.clicked() {
        state.open_submenu = submenu_kind;
        state.submenu_anchor_y = rect.min.y;
    }

    if ui.is_rect_visible(rect) {
        if hovered || is_open {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(4),
                Color32::from_rgb(0, 95, 175),
            );
        }

        let text_color = if hovered || is_open {
            Color32::WHITE
        } else {
            Color32::from_rgb(220, 230, 245)
        };

        let inner_rect = rect.shrink2(egui::vec2(8.0, 2.0));
        let mut row_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        row_ui.add_sized(
            [20.0, 20.0],
            egui::Label::new(RichText::new(icon).size(13.0).color(text_color)).selectable(false),
        );
        row_ui.add(
            egui::Label::new(RichText::new(label).size(13.0).color(text_color)).selectable(false),
        );
        row_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(RichText::new(icons::SUBMENU_ARROW).size(12.0).color(text_color))
                    .selectable(false),
            );
        });
    }
}

fn main_row(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    icon: &str,
    label: &str,
    shortcut: &str,
) -> bool {
    menu_item_row(
        ui,
        state,
        MenuRowParams {
            icon,
            label,
            shortcut,
            bold: false,
            enabled: true,
            danger: false,
            close_submenu_on_hover: true,
        },
    )
}

fn sub_row(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    icon: &str,
    label: &str,
    shortcut: &str,
) -> bool {
    menu_item_row(
        ui,
        state,
        MenuRowParams {
            icon,
            label,
            shortcut,
            bold: false,
            enabled: true,
            danger: false,
            close_submenu_on_hover: false,
        },
    )
}

fn render_zip_item_menu(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    item: &FileItem,
    i18n: &I18n,
) -> Option<ContextMenuAction> {
    let mut action = None;

    if menu_item_row(
        ui,
        state,
        MenuRowParams {
            icon: icons::OPEN_FOLDER,
            label: i18n.t("ctx_menu.open"),
            shortcut: "Enter",
            bold: true,
            enabled: true,
            danger: false,
            close_submenu_on_hover: true,
        },
    ) {
        action = Some(ContextMenuAction::OpenItem(item.clone()));
    }

    if !item.is_dir {
        if main_row(ui, state, icons::VIEW, i18n.t("ctx_menu.view"), "F3") {
            action = Some(ContextMenuAction::ViewF3);
        }
        menu_separator(ui);
        if main_row(
            ui,
            state,
            icons::EXTRACT_OUT,
            i18n.t("ctx_menu.extract_other_panel"),
            "F5",
        ) {
            action = Some(ContextMenuAction::CopyToOtherPanelF5);
        }
        if main_row(ui, state, icons::COPY, i18n.t("ctx_menu.copy_name"), "") {
            action = Some(ContextMenuAction::CopyName(item.name.clone()));
        }
    } else if main_row(
        ui,
        state,
        icons::EXTRACT_OUT,
        i18n.t("ctx_menu.extract_other_panel"),
        "F5",
    ) {
        action = Some(ContextMenuAction::CopyToOtherPanelF5);
    }

    action
}

fn render_normal_file_menu(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    item: &FileItem,
    selected_count: usize,
    i18n: &I18n,
) -> Option<ContextMenuAction> {
    let mut action = None;
    let single = selected_count <= 1;

    if single {
        if menu_item_row(
            ui,
            state,
            MenuRowParams {
                icon: icons::OPEN_FILE,
                label: i18n.t("ctx_menu.open"),
                shortcut: "Enter",
                bold: true,
                enabled: true,
                danger: false,
                close_submenu_on_hover: true,
            },
        ) {
            action = Some(ContextMenuAction::OpenItem(item.clone()));
        }

        #[cfg(windows)]
        if crate::opener::is_executable_file(&item.path)
            && main_row(
                ui,
                state,
                icons::RUN_AS_ADMIN,
                i18n.t("ctx_menu.run_as_admin"),
                "",
            )
        {
            action = Some(ContextMenuAction::RunAsAdmin(item.path.clone()));
        }

        if main_row(ui, state, icons::OPEN_WITH, i18n.t("ctx_menu.open_with"), "") {
            action = Some(ContextMenuAction::OpenWith(item.path.clone()));
        }
        if main_row(ui, state, icons::VIEW, i18n.t("ctx_menu.view"), "F3") {
            action = Some(ContextMenuAction::ViewF3);
        }
        if main_row(ui, state, icons::EDIT, i18n.t("ctx_menu.edit"), "F4") {
            action = Some(ContextMenuAction::EditF4);
        }
        menu_separator(ui);
    }

    let cut_lbl = with_count_badge(i18n.t("ctx_menu.cut"), selected_count);
    if main_row(ui, state, icons::CUT, &cut_lbl, "Ctrl+X") {
        action = Some(ContextMenuAction::Cut);
    }

    let copy_lbl = with_count_badge(i18n.t("ctx_menu.copy"), selected_count);
    if main_row(ui, state, icons::COPY, &copy_lbl, "Ctrl+C") {
        action = Some(ContextMenuAction::Copy);
    }

    let copy_path_lbl = with_count_badge(i18n.t("ctx_menu.copy_as_path"), selected_count);
    if main_row(ui, state, icons::COPY_AS_PATH, &copy_path_lbl, "") {
        let paths = if single {
            vec![item.path.clone()]
        } else {
            Vec::new()
        };
        action = Some(ContextMenuAction::CopyAsPath(paths));
    }

    menu_separator(ui);

    let copy_panel_lbl = with_count_badge(i18n.t("ctx_menu.copy_other_panel"), selected_count);
    if main_row(ui, state, icons::COPY_PANEL, &copy_panel_lbl, "F5") {
        action = Some(ContextMenuAction::CopyToOtherPanelF5);
    }

    let move_panel_lbl = with_count_badge(i18n.t("ctx_menu.move_other_panel"), selected_count);
    if main_row(ui, state, icons::MOVE_PANEL, &move_panel_lbl, "F6") {
        action = Some(ContextMenuAction::MoveToOtherPanelF6);
    }

    menu_separator(ui);

    if single && crate::zip_support::is_zip_file(&item.path) {
        if main_row(ui, state, icons::EXTRACT, i18n.t("ctx_menu.extract_here"), "") {
            action = Some(ContextMenuAction::ExtractZipHere(item.path.clone()));
        }
        let stem = item
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string());
        let extract_folder_lbl = i18n.t_fmt("ctx_menu.extract_to_folder", &[("folder", &stem)]);
        if main_row(ui, state, icons::FOLDER, &extract_folder_lbl, "") {
            action = Some(ContextMenuAction::ExtractZipToFolder(item.path.clone()));
        }
    }

    let zip_lbl = with_count_badge(i18n.t("ctx_menu.compress_zip"), selected_count);
    if main_row(ui, state, icons::COMPRESS_ZIP, &zip_lbl, "Alt+F5") {
        action = Some(ContextMenuAction::PackZip);
    }

    menu_separator(ui);

    if single && main_row(ui, state, icons::RENAME, i18n.t("ctx_menu.rename"), "F2") {
        action = Some(ContextMenuAction::Rename);
    }

    let del_lbl = with_count_badge(i18n.t("ctx_menu.delete"), selected_count);
    if menu_item_row(
        ui,
        state,
        MenuRowParams {
            icon: icons::DELETE,
            label: &del_lbl,
            shortcut: "Del / F8",
            bold: false,
            enabled: true,
            danger: true,
            close_submenu_on_hover: true,
        },
    ) {
        action = Some(ContextMenuAction::Delete);
    }

    if single {
        menu_separator(ui);
        if main_row(
            ui,
            state,
            icons::EXPLORER,
            i18n.t("ctx_menu.show_in_explorer"),
            "",
        ) {
            action = Some(ContextMenuAction::ShowInExplorer {
                path: item.path.clone(),
                select_item: true,
            });
        }
        if main_row(
            ui,
            state,
            icons::PROPERTIES,
            i18n.t("ctx_menu.properties"),
            "Alt+Enter",
        ) {
            action = Some(ContextMenuAction::Properties(None));
        }
        #[cfg(windows)]
        if main_row(
            ui,
            state,
            icons::WIN_PROPERTIES,
            i18n.t("ctx_menu.windows_properties"),
            "",
        ) {
            action = Some(ContextMenuAction::SystemProperties(item.path.clone()));
        }
    }

    action
}

fn render_normal_dir_menu(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    item: &FileItem,
    selected_count: usize,
    i18n: &I18n,
) -> Option<ContextMenuAction> {
    let mut action = None;
    let single = selected_count <= 1;
    let can_paste = state.can_paste;

    if single {
        if menu_item_row(
            ui,
            state,
            MenuRowParams {
                icon: icons::OPEN_FOLDER,
                label: i18n.t("ctx_menu.open"),
                shortcut: "Enter",
                bold: true,
                enabled: true,
                danger: false,
                close_submenu_on_hover: true,
            },
        ) {
            action = Some(ContextMenuAction::OpenItem(item.clone()));
        }
        if main_row(
            ui,
            state,
            icons::OTHER_PANEL,
            i18n.t("ctx_menu.open_in_other_panel"),
            "",
        ) {
            action = Some(ContextMenuAction::OpenInOtherPanel(item.path.clone()));
        }
        if main_row(
            ui,
            state,
            icons::TERMINAL,
            i18n.t("ctx_menu.open_in_terminal"),
            "",
        ) {
            action = Some(ContextMenuAction::OpenTerminal(item.path.clone()));
        }
        menu_separator(ui);
    }

    let cut_lbl = with_count_badge(i18n.t("ctx_menu.cut"), selected_count);
    if main_row(ui, state, icons::CUT, &cut_lbl, "Ctrl+X") {
        action = Some(ContextMenuAction::Cut);
    }

    let copy_lbl = with_count_badge(i18n.t("ctx_menu.copy"), selected_count);
    if main_row(ui, state, icons::COPY, &copy_lbl, "Ctrl+C") {
        action = Some(ContextMenuAction::Copy);
    }

    if single
        && menu_item_row(
            ui,
            state,
            MenuRowParams {
                icon: icons::PASTE,
                label: i18n.t("ctx_menu.paste_into_folder"),
                shortcut: "Ctrl+V",
                bold: false,
                enabled: can_paste,
                danger: false,
                close_submenu_on_hover: true,
            },
        )
    {
        action = Some(ContextMenuAction::Paste(item.path.clone()));
    }

    let copy_path_lbl = with_count_badge(i18n.t("ctx_menu.copy_as_path"), selected_count);
    if main_row(ui, state, icons::COPY_AS_PATH, &copy_path_lbl, "") {
        let paths = if single {
            vec![item.path.clone()]
        } else {
            Vec::new()
        };
        action = Some(ContextMenuAction::CopyAsPath(paths));
    }

    menu_separator(ui);

    let copy_panel_lbl = with_count_badge(i18n.t("ctx_menu.copy_other_panel"), selected_count);
    if main_row(ui, state, icons::COPY_PANEL, &copy_panel_lbl, "F5") {
        action = Some(ContextMenuAction::CopyToOtherPanelF5);
    }

    let move_panel_lbl = with_count_badge(i18n.t("ctx_menu.move_other_panel"), selected_count);
    if main_row(ui, state, icons::MOVE_PANEL, &move_panel_lbl, "F6") {
        action = Some(ContextMenuAction::MoveToOtherPanelF6);
    }

    menu_separator(ui);

    let zip_lbl = with_count_badge(i18n.t("ctx_menu.compress_zip"), selected_count);
    if main_row(ui, state, icons::COMPRESS_ZIP, &zip_lbl, "Alt+F5") {
        action = Some(ContextMenuAction::PackZip);
    }

    if single
        && main_row(
            ui,
            state,
            icons::CALC_SIZE,
            i18n.t("ctx_menu.calc_dir_size"),
            "Space",
        )
    {
        action = Some(ContextMenuAction::CalculateDirSize);
    }

    menu_separator(ui);

    if single && main_row(ui, state, icons::RENAME, i18n.t("ctx_menu.rename"), "F2") {
        action = Some(ContextMenuAction::Rename);
    }

    let del_lbl = with_count_badge(i18n.t("ctx_menu.delete"), selected_count);
    if menu_item_row(
        ui,
        state,
        MenuRowParams {
            icon: icons::DELETE,
            label: &del_lbl,
            shortcut: "Del / F8",
            bold: false,
            enabled: true,
            danger: true,
            close_submenu_on_hover: true,
        },
    ) {
        action = Some(ContextMenuAction::Delete);
    }

    if single {
        menu_separator(ui);
        if main_row(
            ui,
            state,
            icons::EXPLORER,
            i18n.t("ctx_menu.show_in_explorer"),
            "",
        ) {
            action = Some(ContextMenuAction::ShowInExplorer {
                path: item.path.clone(),
                select_item: true,
            });
        }
        if main_row(
            ui,
            state,
            icons::PROPERTIES,
            i18n.t("ctx_menu.properties"),
            "Alt+Enter",
        ) {
            action = Some(ContextMenuAction::Properties(None));
        }
        #[cfg(windows)]
        if main_row(
            ui,
            state,
            icons::WIN_PROPERTIES,
            i18n.t("ctx_menu.windows_properties"),
            "",
        ) {
            action = Some(ContextMenuAction::SystemProperties(item.path.clone()));
        }
    }

    action
}

fn render_background_menu(
    ui: &mut egui::Ui,
    state: &mut ContextMenuState,
    dir_path: &Path,
    i18n: &I18n,
) -> Option<ContextMenuAction> {
    let mut action = None;

    if state.in_zip {
        if main_row(ui, state, icons::REFRESH, i18n.t("ctx_menu.refresh"), "Ctrl+R") {
            action = Some(ContextMenuAction::Refresh);
        }
        submenu_trigger_row(
            ui,
            state,
            icons::SORT_BY,
            i18n.t("ctx_menu.sort_by"),
            ContextMenuSubmenu::SortBy,
        );
        return action;
    }

    let can_paste = state.can_paste;
    let has_selection = state.has_selection;
    let show_hidden = state.show_hidden;

    if main_row(ui, state, icons::REFRESH, i18n.t("ctx_menu.refresh"), "Ctrl+R") {
        action = Some(ContextMenuAction::Refresh);
    }
    submenu_trigger_row(
        ui,
        state,
        icons::SORT_BY,
        i18n.t("ctx_menu.sort_by"),
        ContextMenuSubmenu::SortBy,
    );

    menu_separator(ui);

    if menu_item_row(
        ui,
        state,
        MenuRowParams {
            icon: icons::PASTE,
            label: i18n.t("ctx_menu.paste"),
            shortcut: "Ctrl+V",
            bold: false,
            enabled: can_paste,
            danger: false,
            close_submenu_on_hover: true,
        },
    ) {
        action = Some(ContextMenuAction::Paste(dir_path.to_path_buf()));
    }

    menu_separator(ui);

    submenu_trigger_row(
        ui,
        state,
        icons::NEW_ITEM,
        i18n.t("ctx_menu.new_menu"),
        ContextMenuSubmenu::NewItem,
    );

    menu_separator(ui);

    if main_row(
        ui,
        state,
        icons::SELECT_ALL,
        i18n.t("ctx_menu.select_all"),
        "Ctrl+A",
    ) {
        action = Some(ContextMenuAction::SelectAll);
    }
    if has_selection
        && main_row(
            ui,
            state,
            icons::DESELECT_ALL,
            i18n.t("ctx_menu.deselect_all"),
            "",
        )
    {
        action = Some(ContextMenuAction::DeselectAll);
    }
    let hidden_icon = if show_hidden { icons::CHECKMARK } else { "" };
    if main_row(
        ui,
        state,
        hidden_icon,
        i18n.t("ctx_menu.show_hidden"),
        "Ctrl+H",
    ) {
        action = Some(ContextMenuAction::ToggleHidden);
    }

    menu_separator(ui);

    if main_row(
        ui,
        state,
        icons::TERMINAL,
        i18n.t("ctx_menu.open_in_terminal"),
        "",
    ) {
        action = Some(ContextMenuAction::OpenTerminal(dir_path.to_path_buf()));
    }
    if main_row(
        ui,
        state,
        icons::EXPLORER,
        i18n.t("ctx_menu.open_in_explorer"),
        "",
    ) {
        action = Some(ContextMenuAction::ShowInExplorer {
            path: dir_path.to_path_buf(),
            select_item: false,
        });
    }
    if main_row(
        ui,
        state,
        icons::COPY_AS_PATH,
        i18n.t("ctx_menu.copy_as_path"),
        "",
    ) {
        action = Some(ContextMenuAction::CopyAsPath(vec![dir_path.to_path_buf()]));
    }

    menu_separator(ui);

    if main_row(
        ui,
        state,
        icons::PROPERTIES,
        i18n.t("ctx_menu.folder_properties"),
        "",
    ) {
        action = Some(ContextMenuAction::Properties(Some(dir_path.to_path_buf())));
    }
    #[cfg(windows)]
    if main_row(
        ui,
        state,
        icons::WIN_PROPERTIES,
        i18n.t("ctx_menu.windows_properties"),
        "",
    ) {
        action = Some(ContextMenuAction::SystemProperties(dir_path.to_path_buf()));
    }

    action
}

fn render_submenu(
    ctx: &egui::Context,
    state: &mut ContextMenuState,
    menu_rect: egui::Rect,
    dir_path_for_new: PathBuf,
    i18n: &I18n,
) -> Option<ContextMenuAction> {
    let submenu = state.open_submenu;
    if submenu == ContextMenuSubmenu::None {
        state.submenu_rect = None;
        return None;
    }

    let screen = ctx.screen_rect();
    let sub_w = 220.0_f32;
    let sub_est_h = match submenu {
        ContextMenuSubmenu::SortBy => 185.0_f32,
        ContextMenuSubmenu::NewItem => 92.0_f32,
        ContextMenuSubmenu::None => 0.0_f32,
    };

    let sub_x = if menu_rect.right() + sub_w <= screen.right() - 4.0 {
        menu_rect.right() - 2.0
    } else {
        (menu_rect.left() - sub_w + 2.0).max(screen.left() + 4.0)
    };
    let sub_y = state.submenu_anchor_y.clamp(
        screen.top() + 4.0,
        (screen.bottom() - sub_est_h - 4.0).max(screen.top() + 4.0),
    );

    let mut action = None;
    let area_resp = egui::Area::new(egui::Id::new("commander_context_submenu"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(sub_x, sub_y))
        .interactable(true)
        .show(ctx, |ui| {
            menu_frame().show(ui, |ui| {
                ui.set_min_width(sub_w);
                ui.set_max_width(sub_w + 40.0);
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

                match submenu {
                    ContextMenuSubmenu::SortBy => {
                        let cur_col = state.sort_column;
                        let cur_ord = state.sort_order;
                        let cols = [
                            (SortColumn::Name, i18n.t("ctx_menu.sort_name")),
                            (SortColumn::Ext, i18n.t("ctx_menu.sort_ext")),
                            (SortColumn::Size, i18n.t("ctx_menu.sort_size")),
                            (SortColumn::Date, i18n.t("ctx_menu.sort_date")),
                        ];
                        for (col, label) in cols {
                            let icon = if cur_col == col { icons::CHECKMARK } else { "" };
                            if sub_row(ui, state, icon, label, "") {
                                action = Some(ContextMenuAction::SortBy(col));
                            }
                        }
                        menu_separator(ui);
                        let asc_icon = if cur_ord == SortOrder::Ascending {
                            icons::CHECKMARK
                        } else {
                            ""
                        };
                        if sub_row(ui, state, asc_icon, i18n.t("ctx_menu.sort_asc"), "") {
                            action = Some(ContextMenuAction::SetSortOrder(SortOrder::Ascending));
                        }
                        let desc_icon = if cur_ord == SortOrder::Descending {
                            icons::CHECKMARK
                        } else {
                            ""
                        };
                        if sub_row(ui, state, desc_icon, i18n.t("ctx_menu.sort_desc"), "") {
                            action = Some(ContextMenuAction::SetSortOrder(SortOrder::Descending));
                        }
                    }
                    ContextMenuSubmenu::NewItem => {
                        if sub_row(ui, state, icons::FOLDER, i18n.t("ctx_menu.new_folder"), "F7") {
                            action = Some(ContextMenuAction::NewFolder);
                        }
                        if sub_row(
                            ui,
                            state,
                            icons::TEXT_FILE,
                            i18n.t("ctx_menu.new_text_file"),
                            ".txt",
                        ) {
                            action = Some(ContextMenuAction::NewTextFile(dir_path_for_new));
                        }
                        if sub_row(
                            ui,
                            state,
                            icons::CUSTOM_FILE,
                            i18n.t("ctx_menu.new_custom_file"),
                            "",
                        ) {
                            action = Some(ContextMenuAction::NewCustomFile);
                        }
                    }
                    ContextMenuSubmenu::None => {}
                }
            });
        });

    state.submenu_rect = Some(area_resp.response.rect);
    action
}

pub fn render_context_menu(
    ctx: &egui::Context,
    state: &mut ContextMenuState,
    i18n: &I18n,
) -> Option<ContextMenuAction> {
    if !state.is_open {
        return None;
    }

    let target = state.target.clone()?;
    let screen = ctx.screen_rect();
    let est_w = state.last_menu_size.x.max(280.0);
    let est_h = state.last_menu_size.y.max(120.0);

    let clamped_x = state
        .pos
        .x
        .clamp(screen.left() + 4.0, (screen.right() - est_w - 4.0).max(screen.left() + 4.0));
    let clamped_y = state
        .pos
        .y
        .clamp(screen.top() + 4.0, (screen.bottom() - est_h - 4.0).max(screen.top() + 4.0));

    let mut action = None;
    let area_resp = egui::Area::new(egui::Id::new("commander_context_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(clamped_x, clamped_y))
        .interactable(true)
        .show(ctx, |ui| {
            menu_frame().show(ui, |ui| {
                ui.set_min_width(280.0);
                ui.set_max_width(340.0);
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

                match &target {
                    ContextMenuTarget::Item {
                        item,
                        selected_count,
                        ..
                    } => {
                        if state.in_zip {
                            action = render_zip_item_menu(ui, state, item, i18n);
                        } else if item.is_dir {
                            action =
                                render_normal_dir_menu(ui, state, item, *selected_count, i18n);
                        } else {
                            action =
                                render_normal_file_menu(ui, state, item, *selected_count, i18n);
                        }
                    }
                    ContextMenuTarget::Background { dir_path } => {
                        action = render_background_menu(ui, state, dir_path, i18n);
                    }
                }
            });
        });

    let menu_rect = area_resp.response.rect;
    state.menu_rect = Some(menu_rect);
    state.last_menu_size = menu_rect.size();

    let dir_for_new = match &target {
        ContextMenuTarget::Background { dir_path } => dir_path.clone(),
        ContextMenuTarget::Item { item, .. } => item
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default(),
    };

    if action.is_none() {
        action = render_submenu(ctx, state, menu_rect, dir_for_new, i18n);
    }

    if action.is_some() {
        state.close();
    } else if state.opened_this_frame {
        state.opened_this_frame = false;
        ctx.request_repaint();
    } else if ctx.input(|i| i.pointer.secondary_pressed()) {
        let outside = ctx
            .input(|i| i.pointer.interact_pos())
            .map(|p| !state.contains_pos(p))
            .unwrap_or(true);
        if outside {
            state.close();
        }
    }

    action
}
