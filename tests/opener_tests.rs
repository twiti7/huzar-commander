use std::fs::File;
use std::io::Write;
use std::path::Path;
use huzar_commander::opener::*;

#[test]
fn test_is_image_and_media_file() {
    assert!(is_image_file("photo.JPG"));
    assert!(is_image_file("image.png"));
    assert!(is_image_file("animation.gif"));
    assert!(is_image_file("vector.svg"));
    assert!(!is_image_file("document.pdf"));
    assert!(!is_image_file("code.rs"));

    assert!(is_media_file("movie.mp4"));
    assert!(is_media_file("clip.mkv"));
    assert!(is_media_file("song.mp3"));
    assert!(!is_media_file("image.png"));
}

#[test]
fn test_read_file_preview_stream_large_and_binary() {
    let temp_dir = std::env::temp_dir().join("cmdr_test_stream_preview_it");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    // 1. Text file with 10,000 bytes, stream reading with max 500 bytes limit
    let text_file = temp_dir.join("sample.txt");
    {
        let mut f = File::create(&text_file).unwrap();
        let chunk = "Linia tekstu podgladu strumieniowego.\n";
        for _ in 0..300 {
            f.write_all(chunk.as_bytes()).unwrap();
        }
    }
    let (content, total_size, is_binary) = read_file_preview_stream(&text_file, 500).unwrap();
    assert!(!is_binary);
    assert!(total_size > 500);
    assert!(content.contains("Podgląd strumieniowy ucięto"));

    // 2. Binary file with null bytes
    let bin_file = temp_dir.join("sample.bin");
    {
        let mut f = File::create(&bin_file).unwrap();
        let data = vec![0u8, 1, 2, 3, 255, 0, 77, 90, 144, 0];
        f.write_all(&data).unwrap();
    }
    let (content, total_size, is_binary) = read_file_preview_stream(&bin_file, 1024).unwrap();
    assert!(is_binary);
    assert_eq!(total_size, 10);
    assert!(content.contains("Offset(h)"));
    assert!(content.contains("PLIK BINARNY"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_open_file_not_found() {
    let non_existent = Path::new("C:/__non_existent_folder__/file.xyz123");
    let res = open_file(non_existent);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("nie istnieje"));
}

#[test]
fn test_open_zip_entry_extract_and_path() {
    let temp_dir = std::env::temp_dir().join("cmdr_test_opener_it");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let zip_path = temp_dir.join("test_media.zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("videos/sample.mp4", options).unwrap();
        zip.write_all(b"fake mp4 video bytes header").unwrap();
        zip.finish().unwrap();
    }

    let target_file = temp_dir.join("extracted_sample.mp4");
    let ext_res = huzar_commander::zip_support::extract_single_zip_entry(
        &zip_path,
        "videos/sample.mp4",
        &target_file,
    );
    assert!(ext_res.is_ok());
    assert!(target_file.exists());
    assert_eq!(std::fs::read(&target_file).unwrap(), b"fake mp4 video bytes header");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_read_file_chunk_seeking_and_modes() {
    let temp_dir = std::env::temp_dir().join("cmdr_test_chunk_seek_it");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_file = temp_dir.join("large_data.dat");
    {
        let mut f = File::create(&test_file).unwrap();
        let mut buffer = vec![0u8; 100_000];
        for (i, b) in buffer.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        f.write_all(&buffer).unwrap();
    }

    let (chunk, total_size) = read_file_chunk(&test_file, 50_000, 64 * 1024).unwrap();
    assert_eq!(total_size, 100_000);
    assert_eq!(chunk.len(), 50_000);
    assert_eq!(chunk[0], (50_000 % 256) as u8);

    let hex_str = format_hex_chunk(&chunk[..32], 50_000, total_size);
    assert!(hex_str.contains("0000C350"));
    assert!(hex_str.contains("PODGLĄD HEX"));

    let text_str = format_text_chunk(&chunk[..64], 50_000, total_size);
    assert!(text_str.contains("PODGLĄD TEKSTOWY"));
    assert!(text_str.contains("50.0%"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_is_markdown_file() {
    assert!(is_markdown_file(Path::new("README.md")));
    assert!(is_markdown_file(Path::new("docs/GUIDE.MARKDOWN")));
    assert!(is_markdown_file(Path::new("C:/Workspace/AGENTS.md")));
    assert!(!is_markdown_file(Path::new("main.rs")));
    assert!(!is_markdown_file(Path::new("archive.zip")));
    assert!(!is_markdown_file(Path::new("document.pdf")));
}

#[test]
fn test_is_json_and_yaml_file() {
    assert!(is_json_file(Path::new("package.json")));
    assert!(is_json_file(Path::new("DATA/CONFIG.JSON")));
    assert!(!is_json_file(Path::new("README.md")));

    assert!(is_yaml_file(Path::new("docker-compose.yaml")));
    assert!(is_yaml_file(Path::new(".github/workflows/ci.yml")));
    assert!(is_yaml_file(Path::new("SETTINGS.YML")));
    assert!(!is_yaml_file(Path::new("package.json")));
}

#[test]
fn test_open_with_editor_nonexistent_file() {
    let res = open_with_editor(Path::new("non_existent_file_xyz_123.txt"), "notepad.exe");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("nie istnieje"));
}

#[test]
fn test_split_editor_cmd() {
    let (prog, args) = split_editor_cmd("code");
    assert_eq!(prog, "code");
    assert!(args.is_empty());

    let (prog, args) = split_editor_cmd("code -n --wait");
    assert_eq!(prog, "code");
    assert_eq!(args, vec!["-n", "--wait"]);

    let (prog, args) = split_editor_cmd("\"C:\\Program Files\\Notepad++\\notepad++.exe\" -multiInst");
    assert_eq!(prog, "C:\\Program Files\\Notepad++\\notepad++.exe");
    assert_eq!(args, vec!["-multiInst"]);

    let (prog, args) = split_editor_cmd("'C:\\Custom Path\\editor.exe' file.txt");
    assert_eq!(prog, "C:\\Custom Path\\editor.exe");
    assert_eq!(args, vec!["file.txt"]);
}

#[test]
fn test_resolve_editor_command_for_code() {
    if let Some(resolved) = resolve_editor_command("code") {
        assert!(resolved.is_file(), "Resolved code editor must be a valid file");
    }
}

#[test]
fn test_open_zip_entry_rejects_traversal_name() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_open_zip_safe_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let archive = temp_dir.join("dummy.zip");

    assert!(open_zip_entry(&archive, "..").is_err());
    assert!(open_zip_entry(&archive, "sub/..").is_err());
    assert!(open_zip_entry(&archive, "").is_err());
    assert!(open_zip_entry(&archive, "evil:name.txt").is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cleanup_preview_temp_dir_at() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_cleanup_preview_{}", std::process::id()));
    let nested = temp_dir.join("sub");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("preview.txt"), b"temp").unwrap();
    assert!(temp_dir.exists());

    cleanup_preview_temp_dir_at(&temp_dir);
    assert!(!temp_dir.exists());

    // Calling again on non-existent directory must not panic
    cleanup_preview_temp_dir_at(&temp_dir);
}
