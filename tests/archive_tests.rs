use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use huzar_commander::archive::{
    extract_archive_entries, extract_single_archive_entry, list_archive_entries, ArchiveFormat,
};

fn temp_dir(name: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("huzar_archive_test_{}_{}", name, ts));
    fs::create_dir_all(&p).expect("create temp archive test dir");
    p
}

fn build_sample_tar_stream<W: Write>(writer: W) {
    let mut builder = tar::Builder::new(writer);

    let mut header1 = tar::Header::new_gnu();
    let data1 = b"root content";
    header1.set_size(data1.len() as u64);
    header1.set_mode(0o644);
    header1.set_cksum();
    builder
        .append_data(&mut header1, "root.txt", &data1[..])
        .unwrap();

    let mut header2 = tar::Header::new_gnu();
    let data2 = b"nested hello";
    header2.set_size(data2.len() as u64);
    header2.set_mode(0o644);
    header2.set_cksum();
    builder
        .append_data(&mut header2, "dir/hello.txt", &data2[..])
        .unwrap();

    builder.finish().unwrap();
}

#[test]
fn test_archive_format_detection_and_stem() {
    assert_eq!(
        ArchiveFormat::from_path(Path::new("a.zip")),
        Some(ArchiveFormat::Zip)
    );
    assert_eq!(
        ArchiveFormat::from_path(Path::new("a.tar")),
        Some(ArchiveFormat::Tar)
    );
    assert_eq!(
        ArchiveFormat::from_path(Path::new("a.tar.gz")),
        Some(ArchiveFormat::TarGz)
    );
    assert_eq!(
        ArchiveFormat::from_path(Path::new("a.TGZ")),
        Some(ArchiveFormat::TarGz)
    );
    assert_eq!(
        ArchiveFormat::from_path(Path::new("a.7z")),
        Some(ArchiveFormat::SevenZ)
    );
    assert_eq!(ArchiveFormat::from_path(Path::new("a.txt")), None);

    assert_eq!(
        ArchiveFormat::strip_archive_stem(Path::new(r"C:\tmp\backup.tar.gz")),
        "backup"
    );
    assert_eq!(
        ArchiveFormat::strip_archive_stem(Path::new(r"C:\tmp\release.7z")),
        "release"
    );
}

#[test]
fn test_tar_and_targz_browse_and_extract() {
    let dir = temp_dir("tar_tgz");
    let tar_path = dir.join("bundle.tar");
    let tgz_path = dir.join("bundle.tar.gz");

    {
        let f = File::create(&tar_path).unwrap();
        build_sample_tar_stream(f);
    }
    {
        let f = File::create(&tgz_path).unwrap();
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        build_sample_tar_stream(gz);
    }

    for archive in [&tar_path, &tgz_path] {
        let root_items = list_archive_entries(archive, "").expect("list root");
        assert!(root_items.iter().any(|i| i.name == "dir" && i.is_dir));
        assert!(root_items.iter().any(|i| i.name == "root.txt" && !i.is_dir));

        let sub_items = list_archive_entries(archive, "dir/").expect("list dir");
        assert_eq!(sub_items.len(), 1);
        assert_eq!(sub_items[0].name, "hello.txt");

        let single_out = dir.join("single.txt");
        let n = extract_single_archive_entry(archive, "dir/hello.txt", &single_out).unwrap();
        assert_eq!(n, 12);
        assert_eq!(fs::read_to_string(&single_out).unwrap(), "nested hello");
        let _ = fs::remove_file(&single_out);

        let extract_dir = dir.join("out_sel");
        let cancel = Arc::new(AtomicBool::new(false));
        let sel = vec!["dir/".to_string()];
        let count = extract_archive_entries(
            archive,
            &extract_dir,
            None,
            Some(&sel),
            &cancel,
            |_, _, _| {},
        )
        .unwrap();
        assert_eq!(count, 1);
        assert!(!extract_dir.join("root.txt").exists());
        assert_eq!(
            fs::read_to_string(extract_dir.join("dir").join("hello.txt")).unwrap(),
            "nested hello"
        );
        let _ = fs::remove_dir_all(&extract_dir);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_tar_zip_slip_protection() {
    let dir = temp_dir("tar_slip");
    let tar_path = dir.join("evil.tar");
    let extract_dir = dir.join("extract");
    fs::create_dir_all(&extract_dir).unwrap();

    // Craft a tar containing a safe file and a traversal header ("../evil.txt")
    {
        let mut raw_tar = Vec::new();
        build_sample_tar_stream(&mut raw_tar);
        // Strip trailing 1024 zero trailer blocks so we can append a raw malicious entry
        raw_tar.truncate(raw_tar.len().saturating_sub(1024));

        let mut evil_header = tar::Header::new_gnu();
        let payload = b"pwned";
        evil_header.set_size(payload.len() as u64);
        evil_header.set_mode(0o644);
        let name_bytes = b"../evil.txt";
        evil_header.as_mut_bytes()[..name_bytes.len()].copy_from_slice(name_bytes);
        evil_header.as_mut_bytes()[name_bytes.len()] = 0;
        evil_header.set_cksum();

        raw_tar.extend_from_slice(evil_header.as_bytes());
        let mut padded = vec![0u8; 512];
        padded[..payload.len()].copy_from_slice(payload);
        raw_tar.extend_from_slice(&padded);
        raw_tar.extend_from_slice(&[0u8; 1024]);

        fs::write(&tar_path, &raw_tar).unwrap();
    }

    let entries = list_archive_entries(&tar_path, "").unwrap();
    assert!(!entries.iter().any(|i| i.name.contains("evil")));

    let cancel = Arc::new(AtomicBool::new(false));
    let count =
        extract_archive_entries(&tar_path, &extract_dir, None, None, &cancel, |_, _, _| {})
            .unwrap();
    assert_eq!(count, 2);
    assert!(!dir.join("evil.txt").exists());
    assert!(extract_dir.join("root.txt").exists());

    let _ = fs::remove_dir_all(&dir);
}
