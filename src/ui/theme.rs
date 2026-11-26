use egui::Color32;

use crate::config::{AppearanceConfig, ThemeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub egui_panel_fill: Color32,
    pub egui_window_fill: Color32,
    pub egui_text: Color32,
    pub egui_selection_bg: Color32,

    pub panel_bg_active: Color32,
    pub panel_bg_inactive: Color32,
    pub panel_border_active: Color32,
    pub panel_border_inactive: Color32,
    pub drop_target_border: Color32,

    pub path_bar_bg_active: Color32,
    pub path_bar_bg_inactive: Color32,
    pub path_bar_border: Color32,
    pub path_text_active: Color32,
    pub path_text_inactive: Color32,
    pub breadcrumb_parent_active: Color32,
    pub breadcrumb_parent_inactive: Color32,

    pub tab_bg_active: Color32,
    pub tab_border_active: Color32,
    pub tab_fg_active: Color32,
    pub tab_bg_inactive: Color32,
    pub tab_border_inactive: Color32,
    pub tab_fg_inactive: Color32,

    pub drive_btn_active_bg: Color32,
    pub drive_btn_active_fg: Color32,
    pub drive_btn_bg: Color32,
    pub drive_btn_fg: Color32,
    pub drive_btn_border: Color32,

    pub fkey_btn_bg: Color32,
    pub fkey_btn_fg: Color32,
    pub fkey_btn_border: Color32,

    pub splitter_idle: Color32,
    pub splitter_hot: Color32,
    pub accent: Color32,
    pub muted_text: Color32,
    pub cli_prompt: Color32,
    pub status_msg: Color32,

    pub item_selected: Color32,
    pub item_cursor_active: Color32,
    pub item_parent: Color32,
    pub item_dir: Color32,
    pub item_exec: Color32,
    pub item_archive: Color32,
    pub item_code: Color32,
    pub item_normal: Color32,
}

pub fn palette_for(kind: ThemeKind) -> ThemePalette {
    match kind {
        ThemeKind::MidnightDark => ThemePalette {
            is_dark: true,
            egui_panel_fill: Color32::from_rgb(13, 17, 23),
            egui_window_fill: Color32::from_rgb(18, 24, 38),
            egui_text: Color32::from_rgb(220, 230, 245),
            egui_selection_bg: Color32::from_rgb(0, 82, 148),

            panel_bg_active: Color32::from_rgb(16, 24, 42),
            panel_bg_inactive: Color32::from_rgb(13, 18, 32),
            panel_border_active: Color32::from_rgb(0, 170, 255),
            panel_border_inactive: Color32::from_rgb(45, 55, 75),
            drop_target_border: Color32::from_rgb(0, 220, 140),

            path_bar_bg_active: Color32::from_rgb(22, 38, 68),
            path_bar_bg_inactive: Color32::from_rgb(18, 26, 46),
            path_bar_border: Color32::from_rgb(40, 60, 95),
            path_text_active: Color32::from_rgb(255, 240, 100),
            path_text_inactive: Color32::from_rgb(180, 190, 210),
            breadcrumb_parent_active: Color32::from_rgb(175, 210, 255),
            breadcrumb_parent_inactive: Color32::from_rgb(150, 165, 190),

            tab_bg_active: Color32::from_rgb(28, 62, 110),
            tab_border_active: Color32::from_rgb(0, 175, 255),
            tab_fg_active: Color32::WHITE,
            tab_bg_inactive: Color32::from_rgb(20, 28, 46),
            tab_border_inactive: Color32::from_rgb(45, 60, 88),
            tab_fg_inactive: Color32::from_rgb(160, 178, 204),

            drive_btn_active_bg: Color32::from_rgb(0, 110, 200),
            drive_btn_active_fg: Color32::WHITE,
            drive_btn_bg: Color32::from_rgb(25, 35, 55),
            drive_btn_fg: Color32::from_rgb(180, 200, 230),
            drive_btn_border: Color32::from_rgb(50, 65, 95),

            fkey_btn_bg: Color32::from_rgb(28, 42, 68),
            fkey_btn_fg: Color32::WHITE,
            fkey_btn_border: Color32::from_rgb(50, 75, 115),

            splitter_idle: Color32::from_rgb(40, 52, 72),
            splitter_hot: Color32::from_rgb(0, 170, 255),
            accent: Color32::from_rgb(0, 190, 255),
            muted_text: Color32::from_rgb(140, 160, 190),
            cli_prompt: Color32::from_rgb(100, 255, 120),
            status_msg: Color32::YELLOW,

            item_selected: Color32::from_rgb(255, 80, 80),
            item_cursor_active: Color32::WHITE,
            item_parent: Color32::from_rgb(255, 220, 100),
            item_dir: Color32::from_rgb(255, 215, 0),
            item_exec: Color32::from_rgb(100, 255, 140),
            item_archive: Color32::from_rgb(255, 170, 90),
            item_code: Color32::from_rgb(130, 205, 255),
            item_normal: Color32::from_rgb(230, 235, 245),
        },

        ThemeKind::ClassicNortonBlue => ThemePalette {
            is_dark: true,
            egui_panel_fill: Color32::from_rgb(0, 0, 92),
            egui_window_fill: Color32::from_rgb(0, 28, 122),
            egui_text: Color32::from_rgb(210, 235, 255),
            egui_selection_bg: Color32::from_rgb(0, 128, 150),

            panel_bg_active: Color32::from_rgb(0, 0, 128),
            panel_bg_inactive: Color32::from_rgb(0, 0, 96),
            panel_border_active: Color32::from_rgb(0, 220, 255),
            panel_border_inactive: Color32::from_rgb(40, 85, 160),
            drop_target_border: Color32::from_rgb(85, 255, 140),

            path_bar_bg_active: Color32::from_rgb(0, 120, 150),
            path_bar_bg_inactive: Color32::from_rgb(0, 45, 115),
            path_bar_border: Color32::from_rgb(70, 150, 215),
            path_text_active: Color32::from_rgb(255, 255, 110),
            path_text_inactive: Color32::from_rgb(185, 215, 245),
            breadcrumb_parent_active: Color32::WHITE,
            breadcrumb_parent_inactive: Color32::from_rgb(160, 200, 240),

            tab_bg_active: Color32::from_rgb(0, 130, 160),
            tab_border_active: Color32::from_rgb(0, 235, 255),
            tab_fg_active: Color32::WHITE,
            tab_bg_inactive: Color32::from_rgb(0, 25, 105),
            tab_border_inactive: Color32::from_rgb(55, 105, 175),
            tab_fg_inactive: Color32::from_rgb(175, 210, 245),

            drive_btn_active_bg: Color32::from_rgb(0, 145, 175),
            drive_btn_active_fg: Color32::WHITE,
            drive_btn_bg: Color32::from_rgb(0, 35, 120),
            drive_btn_fg: Color32::from_rgb(180, 225, 255),
            drive_btn_border: Color32::from_rgb(70, 130, 205),

            fkey_btn_bg: Color32::from_rgb(0, 110, 128),
            fkey_btn_fg: Color32::WHITE,
            fkey_btn_border: Color32::from_rgb(60, 195, 220),

            splitter_idle: Color32::from_rgb(60, 115, 195),
            splitter_hot: Color32::from_rgb(0, 225, 255),
            accent: Color32::from_rgb(0, 230, 255),
            muted_text: Color32::from_rgb(165, 205, 245),
            cli_prompt: Color32::from_rgb(85, 255, 85),
            status_msg: Color32::from_rgb(255, 255, 85),

            item_selected: Color32::from_rgb(255, 255, 85),
            item_cursor_active: Color32::WHITE,
            item_parent: Color32::from_rgb(255, 255, 85),
            item_dir: Color32::WHITE,
            item_exec: Color32::from_rgb(85, 255, 85),
            item_archive: Color32::from_rgb(255, 170, 85),
            item_code: Color32::from_rgb(130, 255, 255),
            item_normal: Color32::from_rgb(210, 235, 255),
        },

        ThemeKind::Light => ThemePalette {
            is_dark: false,
            egui_panel_fill: Color32::from_rgb(244, 246, 250),
            egui_window_fill: Color32::WHITE,
            egui_text: Color32::from_rgb(24, 32, 46),
            egui_selection_bg: Color32::from_rgb(200, 224, 250),

            panel_bg_active: Color32::WHITE,
            panel_bg_inactive: Color32::from_rgb(242, 245, 249),
            panel_border_active: Color32::from_rgb(0, 110, 210),
            panel_border_inactive: Color32::from_rgb(198, 208, 222),
            drop_target_border: Color32::from_rgb(16, 155, 88),

            path_bar_bg_active: Color32::from_rgb(228, 238, 252),
            path_bar_bg_inactive: Color32::from_rgb(236, 240, 246),
            path_bar_border: Color32::from_rgb(176, 194, 218),
            path_text_active: Color32::from_rgb(0, 75, 165),
            path_text_inactive: Color32::from_rgb(75, 88, 110),
            breadcrumb_parent_active: Color32::from_rgb(35, 92, 168),
            breadcrumb_parent_inactive: Color32::from_rgb(90, 104, 126),

            tab_bg_active: Color32::from_rgb(216, 232, 252),
            tab_border_active: Color32::from_rgb(0, 110, 210),
            tab_fg_active: Color32::from_rgb(15, 28, 52),
            tab_bg_inactive: Color32::from_rgb(235, 239, 245),
            tab_border_inactive: Color32::from_rgb(196, 206, 220),
            tab_fg_inactive: Color32::from_rgb(65, 78, 98),

            drive_btn_active_bg: Color32::from_rgb(0, 110, 210),
            drive_btn_active_fg: Color32::WHITE,
            drive_btn_bg: Color32::from_rgb(232, 237, 245),
            drive_btn_fg: Color32::from_rgb(30, 42, 62),
            drive_btn_border: Color32::from_rgb(180, 194, 214),

            fkey_btn_bg: Color32::from_rgb(228, 234, 243),
            fkey_btn_fg: Color32::from_rgb(22, 32, 48),
            fkey_btn_border: Color32::from_rgb(175, 190, 210),

            splitter_idle: Color32::from_rgb(190, 200, 216),
            splitter_hot: Color32::from_rgb(0, 110, 210),
            accent: Color32::from_rgb(0, 105, 205),
            muted_text: Color32::from_rgb(92, 106, 128),
            cli_prompt: Color32::from_rgb(16, 120, 56),
            status_msg: Color32::from_rgb(170, 95, 0),

            item_selected: Color32::from_rgb(195, 28, 38),
            item_cursor_active: Color32::from_rgb(15, 25, 40),
            item_parent: Color32::from_rgb(145, 92, 0),
            item_dir: Color32::from_rgb(15, 78, 168),
            item_exec: Color32::from_rgb(14, 122, 58),
            item_archive: Color32::from_rgb(176, 82, 12),
            item_code: Color32::from_rgb(45, 86, 160),
            item_normal: Color32::from_rgb(24, 32, 46),
        },

        ThemeKind::HighContrast => ThemePalette {
            is_dark: true,
            egui_panel_fill: Color32::BLACK,
            egui_window_fill: Color32::from_rgb(12, 12, 12),
            egui_text: Color32::WHITE,
            egui_selection_bg: Color32::from_rgb(0, 70, 140),

            panel_bg_active: Color32::BLACK,
            panel_bg_inactive: Color32::from_rgb(10, 10, 10),
            panel_border_active: Color32::from_rgb(255, 255, 0),
            panel_border_inactive: Color32::from_rgb(140, 140, 140),
            drop_target_border: Color32::from_rgb(0, 255, 120),

            path_bar_bg_active: Color32::from_rgb(24, 24, 24),
            path_bar_bg_inactive: Color32::from_rgb(16, 16, 16),
            path_bar_border: Color32::WHITE,
            path_text_active: Color32::from_rgb(255, 255, 0),
            path_text_inactive: Color32::from_rgb(220, 220, 220),
            breadcrumb_parent_active: Color32::from_rgb(0, 255, 255),
            breadcrumb_parent_inactive: Color32::from_rgb(200, 200, 200),

            tab_bg_active: Color32::from_rgb(0, 75, 150),
            tab_border_active: Color32::from_rgb(255, 255, 0),
            tab_fg_active: Color32::WHITE,
            tab_bg_inactive: Color32::from_rgb(20, 20, 20),
            tab_border_inactive: Color32::from_rgb(140, 140, 140),
            tab_fg_inactive: Color32::from_rgb(230, 230, 230),

            drive_btn_active_bg: Color32::from_rgb(0, 95, 190),
            drive_btn_active_fg: Color32::WHITE,
            drive_btn_bg: Color32::from_rgb(20, 20, 20),
            drive_btn_fg: Color32::WHITE,
            drive_btn_border: Color32::WHITE,

            fkey_btn_bg: Color32::from_rgb(20, 20, 20),
            fkey_btn_fg: Color32::WHITE,
            fkey_btn_border: Color32::WHITE,

            splitter_idle: Color32::from_rgb(150, 150, 150),
            splitter_hot: Color32::from_rgb(255, 255, 0),
            accent: Color32::from_rgb(0, 255, 255),
            muted_text: Color32::from_rgb(200, 200, 200),
            cli_prompt: Color32::from_rgb(0, 255, 0),
            status_msg: Color32::from_rgb(255, 255, 0),

            item_selected: Color32::from_rgb(255, 75, 75),
            item_cursor_active: Color32::WHITE,
            item_parent: Color32::from_rgb(255, 255, 0),
            item_dir: Color32::from_rgb(255, 255, 0),
            item_exec: Color32::from_rgb(0, 255, 0),
            item_archive: Color32::from_rgb(255, 145, 0),
            item_code: Color32::from_rgb(0, 255, 255),
            item_normal: Color32::WHITE,
        },
    }
}

pub fn apply_theme_and_font_size(ctx: &egui::Context, appearance: &AppearanceConfig) -> ThemePalette {
    let palette = palette_for(appearance.theme);
    let mut visuals = if palette.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.egui_panel_fill;
    visuals.window_fill = palette.egui_window_fill;
    visuals.override_text_color = Some(palette.egui_text);
    visuals.selection.bg_fill = palette.egui_selection_bg;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, palette.panel_border_active);
    ctx.set_visuals(visuals);

    let base = appearance.font_size.clamp(11.0, 18.0);
    ctx.style_mut(|style| {
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(base));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(base));
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace((base - 0.5).max(10.0)),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional((base - 2.0).max(9.5)),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(base + 4.5),
        );
    });

    palette
}
