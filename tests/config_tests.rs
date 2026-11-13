use std::collections::HashMap;
use huzar_commander::config::AppConfig;

#[test]
fn test_config_defaults_and_lookup() {
    let mut cfg = AppConfig {
        language: None,
        default_editor: "notepad.exe".to_string(),
        associations: HashMap::new(),
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
