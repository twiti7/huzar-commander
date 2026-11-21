use std::collections::HashSet;
use std::path::{Path, PathBuf};

use huzar_commander::app::PanelSide;
use huzar_commander::fs_item::{FileItem, SortColumn, SortOrder};
use huzar_commander::i18n::{I18n, Language};
use huzar_commander::opener::{generate_non_colliding_file_name, is_executable_file};
use huzar_commander::panel::PanelState;
use huzar_commander::ui::context_menu::ALL_MENU_ICONS;
use huzar_commander::ui::fonts::build_font_definitions;
use huzar_commander::ui::panels::sort_indicator;
use huzar_commander::ui::state::{ContextMenuState, ContextMenuSubmenu, ContextMenuTarget};

fn make_test_panel() -> PanelState {
    let (dir_size_tx, dir_size_rx) = crossbeam_channel::unbounded();
    let parent = FileItem::new_parent(PathBuf::from("C:\\test"));
    let item1 = FileItem {
        name: "dir_one".to_string(),
        ext: String::new(),
        path: PathBuf::from("C:\\test\\dir_one"),
        is_dir: true,
        is_parent: false,
        size: 0,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };
    let item2 = FileItem {
        name: "a.txt".to_string(),
        ext: "txt".to_string(),
        path: PathBuf::from("C:\\test\\a.txt"),
        is_dir: false,
        is_parent: false,
        size: 100,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };
    let item3 = FileItem {
        name: "b.txt".to_string(),
        ext: "txt".to_string(),
        path: PathBuf::from("C:\\test\\b.txt"),
        is_dir: false,
        is_parent: false,
        size: 250,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };
    let item4 = FileItem {
        name: "c.txt".to_string(),
        ext: "txt".to_string(),
        path: PathBuf::from("C:\\test\\c.txt"),
        is_dir: false,
        is_parent: false,
        size: 50,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };

    let all = vec![parent, item1, item2, item3, item4];
    let mut panel = PanelState {
        current_path: PathBuf::from("C:\\test"),
        all_items: all.clone(),
        items: all,
        selected_paths: HashSet::new(),
        cursor_idx: 0,
        scroll_to_cursor: false,
        sort_column: SortColumn::Name,
        sort_order: SortOrder::Ascending,
        filter: String::new(),
        is_filter_open: false,
        show_hidden: true,
        custom_title: None,
        zip_location: None,
        total_bytes: 0,
        total_files: 0,
        total_dirs: 0,
        selected_bytes: 0,
        selected_count: 0,
        error_message: None,
        loading_handle: None,
        calculating_dir_sizes: HashSet::new(),
        calculated_dirs: HashSet::new(),
        dir_size_tx,
        dir_size_rx,
    };
    panel.recalculate_totals();
    panel
}

#[test]
fn test_context_menu_state_lifecycle_and_hit_testing() {
    let mut state = ContextMenuState::default();
    assert!(!state.is_open);
    assert!(!state.contains_pos(egui::pos2(10.0, 10.0)));

    let sample_item = FileItem {
        name: "notes.txt".to_string(),
        ext: "txt".to_string(),
        path: PathBuf::from("C:\\test\\notes.txt"),
        is_dir: false,
        is_parent: false,
        size: 42,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };

    state.open_for_item(
        PanelSide::Left,
        egui::pos2(120.0, 150.0),
        2,
        sample_item,
        3,
        false,
        true,
        true,
        SortColumn::Name,
        SortOrder::Ascending,
    );

    assert!(state.is_open);
    assert!(state.opened_this_frame);
    assert!(state.can_paste);
    assert!(state.has_selection);
    assert_eq!(state.side, PanelSide::Left);
    assert!(matches!(
        state.target,
        Some(ContextMenuTarget::Item {
            index: 2,
            selected_count: 3,
            ..
        })
    ));

    state.menu_rect = Some(egui::Rect::from_min_size(
        egui::pos2(120.0, 150.0),
        egui::vec2(260.0, 300.0),
    ));
    state.submenu_rect = Some(egui::Rect::from_min_size(
        egui::pos2(380.0, 200.0),
        egui::vec2(200.0, 140.0),
    ));

    assert!(state.contains_pos(egui::pos2(150.0, 200.0)));
    assert!(state.contains_pos(egui::pos2(450.0, 250.0)));
    assert!(!state.contains_pos(egui::pos2(50.0, 50.0)));

    state.open_submenu = ContextMenuSubmenu::SortBy;
    state.close();
    assert!(!state.is_open);
    assert!(state.target.is_none());
    assert_eq!(state.open_submenu, ContextMenuSubmenu::None);
    assert!(state.menu_rect.is_none());
    assert!(state.submenu_rect.is_none());
    assert!(!state.contains_pos(egui::pos2(150.0, 200.0)));

    state.open_for_background(
        PanelSide::Right,
        egui::pos2(500.0, 220.0),
        PathBuf::from("C:\\test"),
        false,
        false,
        false,
        false,
        SortColumn::Size,
        SortOrder::Descending,
    );
    assert!(state.is_open);
    assert_eq!(state.side, PanelSide::Right);
    assert_eq!(state.sort_column, SortColumn::Size);
    assert_eq!(state.sort_order, SortOrder::Descending);
    assert!(matches!(
        state.target,
        Some(ContextMenuTarget::Background { .. })
    ));
}

#[test]
fn test_panel_select_range_skips_parent_and_works_both_directions() {
    let mut panel = make_test_panel();

    // Forward range including index 0 (..)
    panel.select_range(0, 2);
    assert_eq!(panel.selected_count, 2);
    assert!(!panel.selected_paths.contains(&PathBuf::from("C:\\test")));
    assert!(panel
        .selected_paths
        .contains(&PathBuf::from("C:\\test\\dir_one")));
    assert!(panel
        .selected_paths
        .contains(&PathBuf::from("C:\\test\\a.txt")));

    panel.deselect_all();
    assert_eq!(panel.selected_count, 0);

    // Backward range from index 4 to 2
    panel.select_range(4, 2);
    assert_eq!(panel.selected_count, 3);
    assert_eq!(panel.selected_bytes, 100 + 250 + 50);
    assert!(panel
        .selected_paths
        .contains(&PathBuf::from("C:\\test\\a.txt")));
    assert!(panel
        .selected_paths
        .contains(&PathBuf::from("C:\\test\\b.txt")));
    assert!(panel
        .selected_paths
        .contains(&PathBuf::from("C:\\test\\c.txt")));
}

#[test]
fn test_is_executable_file_and_non_colliding_name() {
    assert!(is_executable_file(Path::new("setup.exe")));
    assert!(is_executable_file(Path::new("SCRIPT.BAT")));
    assert!(is_executable_file(Path::new("run.Cmd")));
    assert!(is_executable_file(Path::new("installer.msi")));
    assert!(!is_executable_file(Path::new("readme.txt")));
    assert!(!is_executable_file(Path::new("archive.zip")));

    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_ctx_menu_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let first = generate_non_colliding_file_name(&temp_dir, "Nowy dokument tekstowy", "txt");
    assert_eq!(
        first.file_name().unwrap().to_string_lossy(),
        "Nowy dokument tekstowy.txt"
    );
    std::fs::write(&first, "").unwrap();

    let second = generate_non_colliding_file_name(&temp_dir, "Nowy dokument tekstowy", "txt");
    assert_eq!(
        second.file_name().unwrap().to_string_lossy(),
        "Nowy dokument tekstowy (2).txt"
    );
    std::fs::write(&second, "").unwrap();

    let third = generate_non_colliding_file_name(&temp_dir, "Nowy dokument tekstowy", "txt");
    assert_eq!(
        third.file_name().unwrap().to_string_lossy(),
        "Nowy dokument tekstowy (3).txt"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_context_menu_i18n_keys_pl_and_en() {
    let pl = I18n::new(Language::Pl);
    let en = I18n::new(Language::En);

    for key in [
        "ctx_menu.open",
        "ctx_menu.open_with",
        "ctx_menu.run_as_admin",
        "ctx_menu.cut",
        "ctx_menu.copy",
        "ctx_menu.paste",
        "ctx_menu.copy_as_path",
        "ctx_menu.rename",
        "ctx_menu.delete",
        "ctx_menu.sort_by",
        "ctx_menu.new_menu",
        "ctx_menu.new_text_file",
        "ctx_menu.show_in_explorer",
        "ctx_menu.windows_properties",
    ] {
        assert_ne!(pl.t(key), key, "Missing PL translation for {}", key);
        assert_ne!(en.t(key), key, "Missing EN translation for {}", key);
    }
}

#[test]
fn test_all_context_menu_and_sort_icons_have_bundled_glyphs() {
    let mut fonts = egui::epaint::text::FontsImpl::new(1.0, 2048, build_font_definitions(false));
    let prop = egui::FontId::proportional(13.0);

    let mut missing = Vec::new();
    for &(name, icon) in ALL_MENU_ICONS {
        if !fonts.has_glyphs(&prop, icon) {
            let cps: Vec<String> = icon
                .chars()
                .map(|c| format!("U+{:04X}", c as u32))
                .collect();
            missing.push(format!("{} ('{}' [{}])", name, icon, cps.join(", ")));
        }
    }

    let asc = sort_indicator(SortColumn::Name, SortColumn::Name, SortOrder::Ascending);
    if !fonts.has_glyphs(&prop, asc) {
        missing.push(format!("SORT_ASC ('{}')", asc));
    }
    let desc = sort_indicator(SortColumn::Name, SortColumn::Name, SortOrder::Descending);
    if !fonts.has_glyphs(&prop, desc) {
        missing.push(format!("SORT_DESC ('{}')", desc));
    }

    assert!(
        missing.is_empty(),
        "Icons missing glyphs in bundled Proportional font stack: {:?}",
        missing
    );
}
