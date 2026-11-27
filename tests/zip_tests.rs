use std::fs::{self, File};
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use huzar_commander::zip_support::{
    extract_zip, extract_zip_entries, is_zip_file, list_zip_entries, read_zip_text,
    sanitize_zip_rel_path,
};

#[test]
fn test_zip_create_browse_and_extract() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_zip_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let zip_path = temp_dir.join("test_archive.zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        writer.start_file("hello.txt", options).unwrap();
        writer.write_all(b"Hello Total Commander!").unwrap();

        writer.start_file("sub/nested.txt", options).unwrap();
        writer.write_all(b"Nested content inside ZIP").unwrap();

        writer.finish().unwrap();
    }

    assert!(is_zip_file(&zip_path));

    // Test root entries
    let root_entries = list_zip_entries(&zip_path, "").unwrap();
    assert_eq!(root_entries.len(), 2);
    let names: Vec<String> = root_entries.iter().map(|it| it.name.clone()).collect();
    assert!(names.contains(&"hello.txt".to_string()));
    assert!(names.contains(&"sub".to_string()));

    // Test subfolder entries
    let sub_entries = list_zip_entries(&zip_path, "sub/").unwrap();
    assert_eq!(sub_entries.len(), 1);
    assert_eq!(sub_entries[0].name, "nested.txt");

    // Test text preview
    let text = read_zip_text(&zip_path, "hello.txt", 1000).unwrap();
    assert_eq!(text, "Hello Total Commander!");

    // Test extraction
    let extract_dir = temp_dir.join("extracted");
    let cancel = Arc::new(AtomicBool::new(false));
    let count = extract_zip(&zip_path, &extract_dir, None, &cancel, |_, _, _| {}).unwrap();
    assert_eq!(count, 2);
    assert!(extract_dir.join("hello.txt").exists());
    assert!(extract_dir.join("sub").join("nested.txt").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_sanitize_zip_rel_path_blocks_traversal() {
    use std::path::PathBuf;

    assert_eq!(sanitize_zip_rel_path("../evil.txt"), None);
    assert_eq!(sanitize_zip_rel_path("a/../../evil.txt"), None);
    assert_eq!(sanitize_zip_rel_path("/etc/passwd"), None);
    assert_eq!(sanitize_zip_rel_path("C:\\evil.txt"), None);
    assert_eq!(sanitize_zip_rel_path("C:/evil.txt"), None);
    assert_eq!(sanitize_zip_rel_path("a\\..\\b"), None);
    assert_eq!(sanitize_zip_rel_path("foo\0bar"), None);
    assert_eq!(sanitize_zip_rel_path(""), None);
    assert_eq!(sanitize_zip_rel_path("."), None);

    assert_eq!(
        sanitize_zip_rel_path("sub/file.txt"),
        Some(PathBuf::from("sub").join("file.txt"))
    );
    assert_eq!(
        sanitize_zip_rel_path("./sub/file.txt"),
        Some(PathBuf::from("sub").join("file.txt"))
    );
}

#[test]
fn test_extract_zip_blocks_zip_slip() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_zip_slip_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let extract_dir = temp_dir.join("out");
    fs::create_dir_all(&extract_dir).unwrap();

    let zip_path = temp_dir.join("slip.zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        writer.start_file("safe/ok.txt", options).unwrap();
        writer.write_all(b"safe content").unwrap();

        // Try adding a traversal entry; if zip crate allows it, extract_zip must block it.
        // We use same-length placeholder "xx/escaped.txt" and patch it to "../escaped.txt" if needed.
        writer.start_file("xx/escaped.txt", options).unwrap();
        writer.write_all(b"evil content").unwrap();

        writer.finish().unwrap();
    }

    // Patch "xx/escaped.txt" -> "../escaped.txt" in raw ZIP bytes (same length = 14 bytes, valid CRC/offsets)
    let mut raw_zip = fs::read(&zip_path).unwrap();
    let needle = b"xx/escaped.txt";
    let replacement = b"../escaped.txt";
    let mut replaced = 0;
    for i in 0..=raw_zip.len().saturating_sub(needle.len()) {
        if &raw_zip[i..i + needle.len()] == needle {
            raw_zip[i..i + needle.len()].copy_from_slice(replacement);
            replaced += 1;
        }
    }
    assert!(replaced >= 2, "Expected both local header and central directory entry to be patched");
    fs::write(&zip_path, raw_zip).unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    let count = extract_zip(&zip_path, &extract_dir, None, &cancel, |_, _, _| {}).unwrap();
    assert_eq!(count, 1);
    assert!(extract_dir.join("safe").join("ok.txt").exists());
    assert!(!temp_dir.join("escaped.txt").exists());
    assert!(!extract_dir.join("escaped.txt").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_extract_zip_selected_entries_only() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_zip_selected_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let extract_dir = temp_dir.join("out");
    fs::create_dir_all(&extract_dir).unwrap();

    let zip_path = temp_dir.join("multi.zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        writer.start_file("keep.txt", options).unwrap();
        writer.write_all(b"keep me").unwrap();

        writer.start_file("skip.txt", options).unwrap();
        writer.write_all(b"skip me").unwrap();

        writer.start_file("folder_a/inside.txt", options).unwrap();
        writer.write_all(b"inside folder_a").unwrap();

        writer.start_file("folder_b/other.txt", options).unwrap();
        writer.write_all(b"inside folder_b").unwrap();

        writer.finish().unwrap();
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let selected = vec!["keep.txt".to_string(), "folder_a".to_string()];
    let count = extract_zip_entries(
        &zip_path,
        &extract_dir,
        None,
        Some(&selected),
        &cancel,
        |_, _, _| {},
    )
    .unwrap();

    assert_eq!(count, 2);
    assert!(extract_dir.join("keep.txt").exists());
    assert!(extract_dir.join("folder_a").join("inside.txt").exists());
    assert!(!extract_dir.join("skip.txt").exists());
    assert!(!extract_dir.join("folder_b").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}
