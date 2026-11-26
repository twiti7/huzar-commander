use std::io::Write;
use huzar_commander::opener::ViewerMode;
use huzar_commander::ui::state::{PropertiesDialogState, SearchDialogState, ViewerWindowState};

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

#[test]
fn test_properties_dialog_state_async_calculation() {
    let mut state = PropertiesDialogState::default();
    assert!(!state.is_open);
    assert!(!state.is_calculating);
    assert!(state.rx.is_none());

    let (tx, rx) = crossbeam_channel::unbounded();
    state.is_open = true;
    state.is_dir = true;
    state.is_calculating = true;
    state.rx = Some(rx);

    // Simulate worker calculation
    tx.send((1024 * 1024, 42, 5)).unwrap();

    if let Some(ref rx) = state.rx {
        let (size, files, dirs) = rx.try_recv().unwrap();
        state.size = size;
        state.files_count = files;
        state.dirs_count = dirs;
        state.is_calculating = false;
        state.rx = None;
    }

    assert_eq!(state.size, 1024 * 1024);
    assert_eq!(state.files_count, 42);
    assert_eq!(state.dirs_count, 5);
    assert!(!state.is_calculating);
    assert!(state.rx.is_none());
}

#[test]
fn test_viewer_search_forward_backward_and_utf8() {
    use std::sync::atomic::AtomicBool;
    use huzar_commander::opener::TextEncoding;
    use huzar_commander::ui::state::search_file_offset;

    let temp_dir = std::env::temp_dir().join(format!("cmdr_viewer_search_utf8_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("big_utf8.txt");

    // Build a >150 KB file with Polish diacritics across 128 KB block boundary
    let mut data = String::from("poczatek: Żółć pierwsza ");
    data.push_str(&"x".repeat(130 * 1024));
    data.push_str(" srodek: żółć druga ");
    data.push_str(&"y".repeat(20 * 1024));
    data.push_str(" koniec: ŻÓŁĆ trzecia\n");
    std::fs::write(&file_path, &data).unwrap();

    let cancel = AtomicBool::new(false);
    let m1 = search_file_offset(&file_path, "żółć", 0, true, TextEncoding::Utf8, &cancel)
        .unwrap()
        .expect("Should find first occurrence");
    let m2 = search_file_offset(&file_path, "żółć", m1, true, TextEncoding::Utf8, &cancel)
        .unwrap()
        .expect("Should find second occurrence across 128KB boundary");
    let m3 = search_file_offset(&file_path, "żółć", m2, true, TextEncoding::Utf8, &cancel)
        .unwrap()
        .expect("Should find third occurrence");
    assert!(m1 < m2 && m2 < m3);

    // Search backward from m3 -> must hit m2, then from m2 -> must hit m1
    let back_to_2 = search_file_offset(&file_path, "ŻÓŁĆ", m3, false, TextEncoding::Utf8, &cancel)
        .unwrap()
        .expect("Backward search from m3 must find m2");
    assert_eq!(back_to_2, m2);

    let back_to_1 = search_file_offset(&file_path, "żółć", back_to_2, false, TextEncoding::Utf8, &cancel)
        .unwrap()
        .expect("Backward search from m2 must find m1");
    assert_eq!(back_to_1, m1);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_viewer_encoding_windows1250_and_utf16() {
    use huzar_commander::opener::{decode_bytes_with_encoding, detect_encoding, TextEncoding};

    // Windows-1250 bytes: "zażółć" -> z, a, 0xBF(ż), 0xF3(ó), 0xB3(ł), 0xE6(ć)
    let win1250_bytes = vec![b'z', b'a', 0xBF, 0xF3, 0xB3, 0xE6];
    let decoded_win = decode_bytes_with_encoding(&win1250_bytes, TextEncoding::Windows1250);
    assert_eq!(decoded_win, "zażółć");

    // UTF-16 LE with BOM
    let mut utf16_bytes = vec![0xFF, 0xFE];
    for unit in "Cześć\n".encode_utf16() {
        utf16_bytes.extend_from_slice(&unit.to_le_bytes());
    }
    assert_eq!(detect_encoding(&utf16_bytes), TextEncoding::Utf16Le);

    let temp_dir = std::env::temp_dir().join(format!("cmdr_viewer_utf16_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let utf16_file = temp_dir.join("utf16_note.txt");
    std::fs::write(&utf16_file, &utf16_bytes).unwrap();

    let mut viewer = ViewerWindowState::default();
    viewer.open_file("utf16".to_string(), utf16_file).unwrap();
    // Must not be misclassified as Hex binary despite 0x00 bytes!
    assert_eq!(viewer.mode, ViewerMode::Text);
    assert_eq!(viewer.encoding, TextEncoding::Utf16Le);
    assert!(viewer.content.contains("Cześć"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_viewer_code_mode_and_tokenizer() {
    use std::path::Path;
    use huzar_commander::ui::code_viewer::{detect_code_lang, tokenize_code_line, CodeLang, TokenKind};

    assert_eq!(detect_code_lang(Some(Path::new("main.rs"))), CodeLang::Rust);
    assert_eq!(detect_code_lang(Some(Path::new("app.py"))), CodeLang::Python);

    let tokens = tokenize_code_line(r#"let x: String = "abc"; // hi 42"#, CodeLang::Rust);
    assert!(tokens.iter().any(|(k, t)| *k == TokenKind::Keyword && t == "let"));
    assert!(tokens.iter().any(|(k, t)| *k == TokenKind::TypeName && t == "String"));
    assert!(tokens.iter().any(|(k, t)| *k == TokenKind::StringLit && t == "\"abc\""));
    assert!(tokens.iter().any(|(k, t)| *k == TokenKind::Comment && t == "// hi 42"));

    let temp_dir = std::env::temp_dir().join(format!("cmdr_viewer_code_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let rs_file = temp_dir.join("sample.rs");
    std::fs::write(&rs_file, "fn main() {}\n").unwrap();

    let mut viewer = ViewerWindowState::default();
    viewer.open_file("sample.rs".to_string(), rs_file).unwrap();
    assert_eq!(viewer.mode, ViewerMode::Code);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
