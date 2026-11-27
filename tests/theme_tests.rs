use std::collections::HashSet;

use huzar_commander::config::{
    format_file_size, AppConfig, AppearanceConfig, BehaviorConfig, SizeFormat, ThemeKind,
};
use huzar_commander::ui::theme::palette_for;

#[test]
fn test_all_theme_palettes_distinct_and_valid() {
    let mut active_bgs = HashSet::new();
    for kind in ThemeKind::ALL {
        let pal = palette_for(kind);
        assert_ne!(pal.panel_bg_active, pal.item_normal);
        assert_ne!(pal.panel_bg_active, pal.panel_border_active);
        assert_ne!(pal.fkey_btn_bg, pal.fkey_btn_fg);
        active_bgs.insert((
            pal.panel_bg_active.r(),
            pal.panel_bg_active.g(),
            pal.panel_bg_active.b(),
        ));
    }
    assert_eq!(active_bgs.len(), ThemeKind::ALL.len());
    assert!(palette_for(ThemeKind::MidnightDark).is_dark);
    assert!(palette_for(ThemeKind::ClassicNortonBlue).is_dark);
    assert!(!palette_for(ThemeKind::Light).is_dark);
    assert!(palette_for(ThemeKind::HighContrast).is_dark);
}

#[test]
fn test_format_file_size_modes() {
    assert_eq!(format_file_size(0, SizeFormat::Bytes), "0 B");
    assert_eq!(format_file_size(1024, SizeFormat::Bytes), "1 024 B");
    assert_eq!(format_file_size(1_234_567, SizeFormat::Bytes), "1 234 567 B");

    let binary = format_file_size(1024, SizeFormat::Binary);
    assert!(binary.contains("KiB"), "unexpected binary format: {}", binary);

    let decimal = format_file_size(1000, SizeFormat::Decimal);
    assert!(
        decimal.contains("kB") || decimal.contains("KB"),
        "unexpected decimal format: {}",
        decimal
    );
}

#[test]
fn test_config_appearance_and_behavior_roundtrip_and_legacy_compat() {
    let cfg = AppConfig {
        appearance: AppearanceConfig {
            theme: ThemeKind::ClassicNortonBlue,
            font_size: 15.0,
            row_height: 24.0,
            size_format: SizeFormat::Binary,
        },
        behavior: BehaviorConfig {
            confirm_delete: false,
            save_session_on_exit: false,
        },
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, cfg);

    // Legacy JSON lacking appearance & behavior falls back to exact defaults
    let legacy = r#"{ "default_editor": "code", "associations": {} }"#;
    let parsed: AppConfig = serde_json::from_str(legacy).unwrap();
    assert_eq!(parsed.appearance, AppearanceConfig::default());
    assert_eq!(parsed.behavior, BehaviorConfig::default());
}
