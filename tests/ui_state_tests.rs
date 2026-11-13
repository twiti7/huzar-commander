use std::io::Write;
use huzar_commander::opener::ViewerMode;
use huzar_commander::ui::state::{SearchDialogState, ViewerWindowState};

#[test]
fn test_viewer_window_state_paging_and_modes() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_ui_viewer_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_file = temp_dir.join("test_large_paging.txt");

    // Utwórz plik 150 KB (powyżej 64 KB, więc wymaga paginacji)
    let data = vec![b'A'; 150 * 1024];
    let mut f = std::fs::File::create(&test_file).unwrap();
    f.write_all(&data).unwrap();
    drop(f);

    let mut viewer = ViewerWindowState::default();
    assert!(viewer.open_file("test".to_string(), test_file.clone()).is_ok());
    assert_eq!(viewer.total_size, 150 * 1024);
    assert_eq!(viewer.offset, 0);
    assert_eq!(viewer.chunk.len(), 64 * 1024);
    assert_eq!(viewer.mode, ViewerMode::Text);

    // Nawigacja w przód (seek_next)
    viewer.seek_next();
    assert_eq!(viewer.offset, 64 * 1024);

    // Nawigacja do końca (max offset = 150 KB - 64 KB = 86 KB)
    viewer.seek_next();
    assert_eq!(viewer.offset, 150 * 1024 - 64 * 1024);

    // Próba seek_next poza zakres nie przekracza max offset
    viewer.seek_next();
    assert_eq!(viewer.offset, 150 * 1024 - 64 * 1024);

    // Nawigacja wstecz (seek_prev)
    viewer.seek_prev();
    assert_eq!(viewer.offset, (150 * 1024u64 - 64 * 1024u64).saturating_sub(64 * 1024u64));

    let _ = std::fs::remove_file(&test_file);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_viewer_window_state_markdown_mode() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_ui_md_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let md_file = temp_dir.join("README.md");

    std::fs::write(&md_file, "# Nagłówek testowy\n\nTreść markdown").unwrap();

    let mut viewer = ViewerWindowState::default();
    assert!(viewer.open_file("README.md".to_string(), md_file.clone()).is_ok());
    assert_eq!(viewer.mode, ViewerMode::Markdown);
    assert!(viewer.content.contains("# Nagłówek testowy"));

    // Przełączenie na tryb Hex
    viewer.mode = ViewerMode::Hex;
    viewer.refresh_content();
    assert!(viewer.content.contains("PODGLĄD HEX"));

    // Przełączenie na tryb Text
    viewer.mode = ViewerMode::Text;
    viewer.refresh_content();
    assert!(viewer.content.contains("# Nagłówek testowy"));

    let _ = std::fs::remove_file(&md_file);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_viewer_window_state_json_and_yaml_modes() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_ui_json_yaml_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let json_file = temp_dir.join("data.json");
    std::fs::write(&json_file, r#"{"app": "commander", "count": 42}"#).unwrap();

    let mut viewer = ViewerWindowState::default();
    assert!(viewer.open_file("data.json".to_string(), json_file.clone()).is_ok());
    assert_eq!(viewer.mode, ViewerMode::Json);
    assert!(viewer.content.contains("\"commander\""));

    let yaml_file = temp_dir.join("config.yaml");
    std::fs::write(&yaml_file, "server:\n  port: 8080\n  debug: true\n").unwrap();

    let mut yaml_viewer = ViewerWindowState::default();
    assert!(yaml_viewer.open_file("config.yaml".to_string(), yaml_file.clone()).is_ok());
    assert_eq!(yaml_viewer.mode, ViewerMode::Yaml);
    assert!(yaml_viewer.content.contains("port: 8080"));

    let _ = std::fs::remove_file(&json_file);
    let _ = std::fs::remove_file(&yaml_file);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_search_dialog_state_stop() {
    let mut state = SearchDialogState::default();
    // stop bez aktywnego handle nie powinien panikować
    state.stop();
    assert!(state.handle.is_none());
}

#[test]
fn test_about_dialog_state_tabs_and_modes() {
    use huzar_commander::ui::state::{AboutDialogState, AboutTab, LicenseViewKind};

    let mut state = AboutDialogState::default();
    assert!(!state.is_open);
    assert_eq!(state.current_tab, AboutTab::Overview);
    assert_eq!(state.selected_license, LicenseViewKind::Mit);

    state.open_overview();
    assert!(state.is_open);
    assert_eq!(state.current_tab, AboutTab::Overview);

    state.open_license();
    assert!(state.is_open);
    assert_eq!(state.current_tab, AboutTab::License);

    state.open_libraries();
    assert!(state.is_open);
    assert_eq!(state.current_tab, AboutTab::Libraries);
}

#[test]
fn test_viewer_window_state_percentage_calculation() {
    let mut viewer = ViewerWindowState {
        total_size: 0,
        ..Default::default()
    };
    assert_eq!(viewer.calculate_view_percentage(), 100.0);

    // Plik 10 KB mieszczący się w całości w chunku (64 KB)
    viewer.total_size = 10 * 1024;
    viewer.chunk = vec![0u8; 10 * 1024];
    viewer.offset = 0;

    // Na początku pliku (scroll_ratio = 0.0)
    viewer.scroll_ratio = 0.0;
    assert_eq!(viewer.calculate_view_percentage(), 0.0);

    // W połowie pliku (scroll_ratio = 0.5)
    viewer.scroll_ratio = 0.5;
    assert_eq!(viewer.calculate_view_percentage(), 50.0);

    // Na końcu pliku (scroll_ratio = 1.0)
    viewer.scroll_ratio = 1.0;
    assert_eq!(viewer.calculate_view_percentage(), 100.0);

    // Duży plik 200 KB (paginowany chunkami 64 KB)
    viewer.total_size = 200 * 1024;
    viewer.chunk = vec![0u8; 64 * 1024];

    // Ostatnia strona (offset = 200 KB - 64 KB = 136 KB) przewinięta do końca
    viewer.offset = 136 * 1024;
    viewer.scroll_ratio = 1.0;
    assert_eq!(viewer.calculate_view_percentage(), 100.0);

    // Sprawdzenie dynamicznego offsetu widoku current_visible_offset
    viewer.offset = 0;
    viewer.total_size = 10 * 1024;
    viewer.chunk = vec![0u8; 10 * 1024];

    viewer.scroll_ratio = 0.0;
    assert_eq!(viewer.current_visible_offset(), 0);

    viewer.scroll_ratio = 0.5;
    assert_eq!(viewer.current_visible_offset(), 5 * 1024);

    viewer.scroll_ratio = 1.0;
    assert_eq!(viewer.current_visible_offset(), 10 * 1024);
}
