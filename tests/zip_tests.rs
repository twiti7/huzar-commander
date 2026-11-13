use std::fs::{self, File};
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use huzar_commander::zip_support::{extract_zip, is_zip_file, list_zip_entries, read_zip_text};

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
