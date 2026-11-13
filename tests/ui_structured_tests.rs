use huzar_commander::ui::structured_viewer::find_yaml_colon;

#[test]
fn test_json_parsing_and_tokenization() {
    let sample = r#"{
        "title": "Rust Commander",
        "version": 1.0,
        "active": true,
        "author": null,
        "tags": ["cli", "gui"]
    }"#;

    let parsed: Result<serde_json::Value, _> = serde_json::from_str(sample);
    assert!(parsed.is_ok(), "Prawidłowy JSON powinien zostać sparsowany");
    let val = parsed.unwrap();
    assert_eq!(val["title"], "Rust Commander");
    assert_eq!(val["version"], 1.0);
    assert_eq!(val["active"], true);
    assert_eq!(val["author"], serde_json::Value::Null);
    assert_eq!(val["tags"].as_array().unwrap().len(), 2);
}

#[test]
fn test_yaml_parsing_and_tokenization() {
    let sample = r#"
name: commander
version: 0.1.0
features:
  - dual-panel
  - fast-io
enabled: true
"#;

    let parsed: Result<serde_yaml::Value, _> = serde_yaml::from_str(sample);
    assert!(parsed.is_ok(), "Prawidłowy YAML powinien zostać sparsowany");
    let val = parsed.unwrap();
    assert_eq!(val["name"].as_str(), Some("commander"));
    assert_eq!(val["features"].as_sequence().unwrap().len(), 2);
    assert_eq!(val["enabled"].as_bool(), Some(true));
}

#[test]
fn test_yaml_colon_finder() {
    assert_eq!(find_yaml_colon("name: commander"), Some(4));
    assert_eq!(find_yaml_colon("- key: value"), Some(5));
    assert_eq!(find_yaml_colon("http://example.com"), None);
    assert_eq!(find_yaml_colon("\"quoted:key\": value"), Some(12));
}
