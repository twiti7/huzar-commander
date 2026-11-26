use std::fs;
use std::time::Duration;
use huzar_commander::search::{start_search, SearchMessage, SearchParams};

#[test]
fn test_search_mask_and_text() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_search_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let sub_dir = temp_dir.join("nested");
    fs::create_dir_all(&sub_dir).unwrap();

    let file_a = temp_dir.join("find_me.rs");
    fs::write(&file_a, "fn secret_function() { println!(\"magic_key\"); }").unwrap();

    let file_b = sub_dir.join("other.txt");
    fs::write(&file_b, "no magic here").unwrap();

    let params = SearchParams {
        start_dir: temp_dir.clone(),
        mask: "*.rs".to_string(),
        text: "magic_key".to_string(),
        case_sensitive: false,
        recursive: true,
    };

    let handle = start_search(params);
    let mut found = Vec::new();

    while let Ok(msg) = handle.rx.recv_timeout(Duration::from_secs(3)) {
        match msg {
            SearchMessage::Found(item) => found.push(item),
            SearchMessage::Finished(_) => break,
            _ => {}
        }
    }

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "find_me.rs");
    assert!(found[0].match_line.as_ref().unwrap().contains("magic_key"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_search_case_sensitive() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_search_case_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let file = temp_dir.join("sample.txt");
    fs::write(&file, "Hello WORLD Case Test").unwrap();

    // 1. Z opcją case_sensitive: true i małym "world" -> brak trafień
    let params_case = SearchParams {
        start_dir: temp_dir.clone(),
        mask: "*".to_string(),
        text: "world".to_string(),
        case_sensitive: true,
        recursive: false,
    };
    let handle_case = start_search(params_case);
    let mut found_case = Vec::new();
    while let Ok(msg) = handle_case.rx.recv_timeout(Duration::from_secs(2)) {
        match msg {
            SearchMessage::Found(item) => found_case.push(item),
            SearchMessage::Finished(_) => break,
            _ => {}
        }
    }
    assert_eq!(found_case.len(), 0);

    // 2. Z opcją case_sensitive: false i małym "world" -> 1 trafienie
    let params_nocase = SearchParams {
        start_dir: temp_dir.clone(),
        mask: "*".to_string(),
        text: "world".to_string(),
        case_sensitive: false,
        recursive: false,
    };
    let handle_nocase = start_search(params_nocase);
    let mut found_nocase = Vec::new();
    while let Ok(msg) = handle_nocase.rx.recv_timeout(Duration::from_secs(2)) {
        match msg {
            SearchMessage::Found(item) => found_nocase.push(item),
            SearchMessage::Finished(_) => break,
            _ => {}
        }
    }
    assert_eq!(found_nocase.len(), 1);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_search_cancellation() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_search_cancel_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Utwórz 20 plików
    for i in 0..20 {
        fs::write(temp_dir.join(format!("file_{}.txt", i)), "data").unwrap();
    }

    let params = SearchParams {
        start_dir: temp_dir.clone(),
        mask: "*.txt".to_string(),
        text: "data".to_string(),
        case_sensitive: false,
        recursive: true,
    };

    let handle = start_search(params);
    // Natychmiast anuluj
    handle.cancel.store(true, std::sync::atomic::Ordering::Relaxed);

    let mut finished = false;
    while let Ok(msg) = handle.rx.recv_timeout(Duration::from_secs(2)) {
        if let SearchMessage::Finished(_) = msg {
            finished = true;
            break;
        }
    }

    assert!(finished, "Search should stop and finish after cancellation");
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_search_non_utf8_encoding() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_search_nonutf8_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Plik z bajtami Windows-1250 / ISO-8859 zawierający nie-UTF8 bajty 0xB9, 0xE6
    let non_utf8_file = temp_dir.join("encoded.txt");
    let mut raw_bytes = b"witaj w pliku z polskimi znakami: klucz_specjalny ".to_vec();
    raw_bytes.push(0xB9); // ą w Win1250 (niepoprawne w UTF-8)
    raw_bytes.push(0xE6); // ć w Win1250
    raw_bytes.push(b'\n');
    fs::write(&non_utf8_file, raw_bytes).unwrap();

    let params = SearchParams {
        start_dir: temp_dir.clone(),
        mask: "*.txt".to_string(),
        text: "klucz_specjalny".to_string(),
        case_sensitive: false,
        recursive: false,
    };

    let handle = start_search(params);
    let mut found = Vec::new();
    while let Ok(msg) = handle.rx.recv_timeout(Duration::from_secs(3)) {
        match msg {
            SearchMessage::Found(item) => found.push(item),
            SearchMessage::Finished(_) => break,
            _ => {}
        }
    }

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "encoded.txt");
    assert!(found[0].match_line.as_ref().unwrap().contains("klucz_specjalny"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_truncate_preview_utf8_boundaries() {
    use huzar_commander::search::truncate_preview;

    let short = "zażółć gęślą jaźń 🚀";
    assert_eq!(truncate_preview(short, 120), short);

    // 118 ASCII bytes + multibyte Polish chars and emoji crossing byte 120
    let mut line = "a".repeat(118);
    line.push_str("ąęćłńóśźż🚀");
    let truncated = truncate_preview(&line, 120);
    assert!(truncated.ends_with("..."));
    assert_eq!(truncated.chars().count(), 123);
    assert!(truncated.starts_with(&format!("{}ąę", "a".repeat(118))));
}

#[test]
fn test_search_utf8_multibyte_long_line_no_panic() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_search_utf8_boundary_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let file = temp_dir.join("utf8_long.txt");
    // Place multibyte chars across the 120-byte mark, then keyword beyond 120 chars
    let content = format!("{}ąęćłńóśźż🚀_więcej_tekstu_szukany_klucz\n", "x".repeat(119));
    fs::write(&file, content).unwrap();

    let params = SearchParams {
        start_dir: temp_dir.clone(),
        mask: "*.txt".to_string(),
        text: "szukany_klucz".to_string(),
        case_sensitive: false,
        recursive: false,
    };

    let handle = start_search(params);
    let mut found = Vec::new();
    while let Ok(msg) = handle.rx.recv_timeout(Duration::from_secs(3)) {
        match msg {
            SearchMessage::Found(item) => found.push(item),
            SearchMessage::Finished(_) => break,
            _ => {}
        }
    }

    assert_eq!(found.len(), 1);
    let preview = found[0].match_line.as_ref().unwrap();
    assert!(preview.ends_with("..."));
    assert_eq!(preview.chars().count(), 123);

    let _ = fs::remove_dir_all(&temp_dir);
}
