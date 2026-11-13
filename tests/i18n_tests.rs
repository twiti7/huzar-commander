use huzar_commander::i18n::{I18n, Language};

#[test]
fn test_i18n_basic_translations() {
    let mut i18n = I18n::new(Language::Pl);
    assert_eq!(i18n.t("menu.file"), "Plik");
    assert_eq!(i18n.t("f_keys.f5"), "Kopiuj");

    i18n.set_language(Language::En);
    assert_eq!(i18n.t("menu.file"), "File");
    assert_eq!(i18n.t("f_keys.f5"), "Copy");
}

#[test]
fn test_i18n_formatting() {
    let mut i18n = I18n::new(Language::Pl);
    let formatted = i18n.t_fmt("panels.items_count", &[("filtered", "5"), ("total", "10")]);
    assert_eq!(formatted, "5/10 el.");

    let viewer_fmt_pl = i18n.t_fmt(
        "viewer.info_status",
        &[("size", "9.33 kB"), ("offset", "00000000"), ("percent", "0.0")],
    );
    assert_eq!(viewer_fmt_pl, "Rozmiar: 9.33 kB | Offset: 0x00000000 (0.0%)");

    i18n.set_language(Language::En);
    let formatted_en = i18n.t_fmt("panels.items_count", &[("filtered", "5"), ("total", "10")]);
    assert_eq!(formatted_en, "5/10 items");

    let viewer_fmt_en = i18n.t_fmt(
        "viewer.info_status",
        &[("size", "9.33 kB"), ("offset", "00000000"), ("percent", "0.0")],
    );
    assert_eq!(viewer_fmt_en, "Size: 9.33 kB | Offset: 0x00000000 (0.0%)");
}

#[test]
fn test_i18n_fallback() {
    let i18n = I18n::new(Language::Pl);
    assert_eq!(i18n.t("nonexistent.key.xyz"), "nonexistent.key.xyz");
}

#[test]
fn test_i18n_pl_en_key_parity() {
    let i18n = I18n::new(Language::Pl);
    let pl_keys = i18n.get_all_keys(Language::Pl);
    let en_keys = i18n.get_all_keys(Language::En);

    assert!(!pl_keys.is_empty(), "PL keys should not be empty");
    assert!(!en_keys.is_empty(), "EN keys should not be empty");

    // Verify that every key in PL exists in EN
    for key in &pl_keys {
        assert!(
            en_keys.contains(key),
            "Missing translation key in en.json: {}",
            key
        );
    }

    // Verify that every key in EN exists in PL
    for key in &en_keys {
        assert!(
            pl_keys.contains(key),
            "Missing translation key in pl.json: {}",
            key
        );
    }

    assert_eq!(pl_keys.len(), en_keys.len(), "PL and EN must have equal number of keys");
}

#[test]
fn test_language_from_locale_str() {
    assert_eq!(Language::from_locale_str("pl_PL"), Some(Language::Pl));
    assert_eq!(Language::from_locale_str("pl-PL"), Some(Language::Pl));
    assert_eq!(Language::from_locale_str("PL"), Some(Language::Pl));
    assert_eq!(Language::from_locale_str("polish"), Some(Language::Pl));

    assert_eq!(Language::from_locale_str("en_US"), Some(Language::En));
    assert_eq!(Language::from_locale_str("en-GB"), Some(Language::En));
    assert_eq!(Language::from_locale_str("EN"), Some(Language::En));

    // Unsupported languages return None
    assert_eq!(Language::from_locale_str("de_DE"), None);
    assert_eq!(Language::from_locale_str("fr_FR"), None);
    assert_eq!(Language::from_locale_str("es_ES"), None);
    assert_eq!(Language::from_locale_str("ja_JP"), None);
}

#[test]
fn test_detect_system_language_returns_valid_language() {
    let lang = Language::detect_system_language();
    assert!(lang == Language::Pl || lang == Language::En);
}
