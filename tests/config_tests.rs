use std::collections::HashMap;
use huzar_commander::config::AppConfig;

#[test]
fn test_config_defaults_and_lookup() {
    let mut cfg = AppConfig {
        language: None,
        default_editor: "notepad.exe".to_string(),
        associations: HashMap::new(),
        ..Default::default()
    };

    // When no association is present, return default editor
    assert_eq!(cfg.get_editor_for_ext("rs"), "notepad.exe");
    assert_eq!(cfg.get_editor_for_ext(".RS"), "notepad.exe");

    // Add custom association
    cfg.set_association("rs", "code");
    assert_eq!(cfg.get_editor_for_ext("rs"), "code");
    assert_eq!(cfg.get_editor_for_ext(".RS"), "code");

    // Remove association
    cfg.remove_association("rs");
    assert_eq!(cfg.get_editor_for_ext("rs"), "notepad.exe");
}

#[test]
fn test_config_serialization() {
    let mut cfg = AppConfig::default();
    cfg.set_association("xyz", "custom-editor");

    let serialized = serde_json::to_string(&cfg).expect("Serialization must succeed");
    let deserialized: AppConfig = serde_json::from_str(&serialized).expect("Deserialization must succeed");

    assert_eq!(cfg, deserialized);
    assert_eq!(deserialized.get_editor_for_ext("xyz"), "custom-editor");
}

#[test]
fn test_config_path_portable_and_user_dirs() {
    use std::fs;
    use std::path::PathBuf;

    let temp_dir = std::env::temp_dir().join(format!("cmdr_config_path_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let exe_portable_flag = temp_dir.join("exe_flag");
    let exe_existing_cfg = temp_dir.join("exe_existing");
    let exe_clean = temp_dir.join("exe_clean");
    fs::create_dir_all(&exe_portable_flag).unwrap();
    fs::create_dir_all(&exe_existing_cfg).unwrap();
    fs::create_dir_all(&exe_clean).unwrap();

    fs::write(exe_portable_flag.join("portable.flag"), b"").unwrap();
    fs::write(exe_existing_cfg.join("config.json"), b"{}").unwrap();

    let app_data = temp_dir.join("AppData");
    let xdg_home = temp_dir.join("xdg");
    let home_dir = temp_dir.join("home");

    // 1. Portable flag next to exe takes priority
    assert_eq!(
        AppConfig::resolve_config_path_with(
            Some(&exe_portable_flag),
            Some(&app_data),
            Some(&xdg_home),
            Some(&home_dir),
        ),
        exe_portable_flag.join("config.json")
    );

    // 2. Existing config.json next to exe keeps portable mode
    assert_eq!(
        AppConfig::resolve_config_path_with(
            Some(&exe_existing_cfg),
            Some(&app_data),
            Some(&xdg_home),
            Some(&home_dir),
        ),
        exe_existing_cfg.join("config.json")
    );

    // 3. Otherwise resolves to platform user config directory
    let resolved = AppConfig::resolve_config_path_with(
        Some(&exe_clean),
        Some(&app_data),
        Some(&xdg_home),
        Some(&home_dir),
    );
    #[cfg(target_os = "windows")]
    assert_eq!(resolved, app_data.join("HuzarCommander").join("config.json"));
    #[cfg(not(target_os = "windows"))]
    assert_eq!(resolved, xdg_home.join("huzar-commander").join("config.json"));

    // 4. Fallback to CWD config.json when no env dirs are provided
    assert_eq!(
        AppConfig::resolve_config_path_with(Some(&exe_clean), None, None, None),
        PathBuf::from("config.json")
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_config_save_to_creates_parent_directories() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_config_saveto_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    let nested_cfg = temp_dir.join("nested").join("dir").join("config.json");
    let mut cfg = AppConfig::default();
    cfg.set_association("rs", "hx");

    cfg.save_to(&nested_cfg).expect("save_to should create missing parent directories");
    assert!(nested_cfg.is_file());

    let content = std::fs::read_to_string(&nested_cfg).unwrap();
    let parsed: AppConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.get_editor_for_ext("rs"), "hx");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_config_session_persistence_and_backward_compat() {
    use huzar_commander::config::{PanelSessionConfig, SessionState};
    use huzar_commander::fs_item::{SortColumn, SortOrder};

    // 1. Backward compatibility with legacy config JSON lacking "session"
    let legacy_json = r#"{
        "language": "pl",
        "default_editor": "notepad.exe",
        "associations": { "rs": "code" }
    }"#;
    let parsed_legacy: AppConfig =
        serde_json::from_str(legacy_json).expect("Legacy config without session must deserialize");
    assert_eq!(parsed_legacy.session.split_ratio, 0.5);
    assert!(parsed_legacy.session.show_hidden);
    assert!(parsed_legacy.session.window_maximized);
    assert_eq!(parsed_legacy.session.left_panel.path, None);

    // 2. Roundtrip with populated SessionState
    let cfg = AppConfig {
        session: SessionState {
            left_panel: PanelSessionConfig {
                path: Some("C:\\Workspace".to_string()),
                sort_column: Some(SortColumn::Size),
                sort_order: Some(SortOrder::Descending),
                ..Default::default()
            },
            right_panel: PanelSessionConfig {
                path: Some("D:\\Backup".to_string()),
                sort_column: Some(SortColumn::Date),
                sort_order: Some(SortOrder::Ascending),
                ..Default::default()
            },
            show_hidden: false,
            split_ratio: 0.62,
            window_width: Some(1440.0),
            window_height: Some(900.0),
            window_maximized: false,
        },
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, cfg);
}

#[test]
fn test_config_bookmarks_and_tabs_persistence() {
    use huzar_commander::config::{Bookmark, PanelSessionConfig, SessionState, TabSessionConfig};
    use huzar_commander::fs_item::{SortColumn, SortOrder};

    let cfg = AppConfig {
        bookmarks: vec![
            Bookmark {
                name: "Workspace".to_string(),
                path: r"C:\Workspace".to_string(),
            },
            Bookmark {
                name: "NAS Media".to_string(),
                path: r"\\TRUENAS\media".to_string(),
            },
        ],
        session: SessionState {
            left_panel: PanelSessionConfig {
                tabs: vec![
                    TabSessionConfig {
                        path: r"C:\Workspace".to_string(),
                        is_locked: true,
                        sort_column: Some(SortColumn::Name),
                        sort_order: Some(SortOrder::Ascending),
                    },
                    TabSessionConfig {
                        path: r"C:\Users".to_string(),
                        is_locked: false,
                        sort_column: Some(SortColumn::Date),
                        sort_order: Some(SortOrder::Descending),
                    },
                ],
                active_tab: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, cfg);
    assert_eq!(decoded.bookmarks.len(), 2);
    assert_eq!(decoded.session.left_panel.tabs.len(), 2);
    assert!(decoded.session.left_panel.tabs[0].is_locked);
    assert_eq!(decoded.session.left_panel.active_tab, 1);

    // Backward compatibility when bookmarks / tabs fields are absent
    let legacy = r#"{ "default_editor": "code", "associations": {} }"#;
    let parsed: AppConfig = serde_json::from_str(legacy).unwrap();
    assert!(parsed.bookmarks.is_empty());
    assert!(parsed.session.left_panel.tabs.is_empty());
    assert_eq!(parsed.session.left_panel.active_tab, 0);
}
