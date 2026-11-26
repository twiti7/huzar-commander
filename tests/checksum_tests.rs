use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use huzar_commander::checksum::{
    compute_file_checksums, normalize_checksum_token, verify_checksum_input, ChecksumAlgorithm,
    ChecksumVerifyResult,
};

fn make_temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("huzar_checksum_test_{}_{}", name, ts));
    let _ = std::fs::create_dir_all(&dir);
    let file_path = dir.join("sample.bin");
    std::fs::write(&file_path, bytes).expect("write temp checksum sample");
    file_path
}

#[test]
fn test_checksum_known_vectors() {
    let path = make_temp_file("vectors", b"hello world");
    let cancel = AtomicBool::new(false);
    let mut progress_calls = 0usize;

    let hashes = compute_file_checksums(&path, &cancel, |done, total| {
        progress_calls += 1;
        assert_eq!(total, 11);
        assert!(done <= total);
    })
    .expect("checksum computation should succeed");

    assert!(progress_calls >= 1);
    assert_eq!(
        hashes.sha256,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
    assert_eq!(hashes.md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    assert_eq!(hashes.crc32, "0d4a1185");

    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[test]
fn test_checksum_verify_formats() {
    let path = make_temp_file("verify", b"hello world");
    let cancel = AtomicBool::new(false);
    let hashes = compute_file_checksums(&path, &cancel, |_, _| {}).unwrap();

    assert_eq!(
        verify_checksum_input("", &hashes),
        ChecksumVerifyResult::Empty
    );
    assert_eq!(
        verify_checksum_input(
            "  B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9  sample.bin ",
            &hashes
        ),
        ChecksumVerifyResult::Matched(ChecksumAlgorithm::Sha256)
    );
    assert_eq!(
        verify_checksum_input(
            "MD5 (sample.bin) = 5EB63BBBE01EEED093CB22BB8F5ACDC3",
            &hashes
        ),
        ChecksumVerifyResult::Matched(ChecksumAlgorithm::Md5)
    );
    assert_eq!(
        verify_checksum_input("0x0D4A1185", &hashes),
        ChecksumVerifyResult::Matched(ChecksumAlgorithm::Crc32)
    );
    assert_eq!(
        verify_checksum_input("deadbeef", &hashes),
        ChecksumVerifyResult::Mismatch
    );
    assert_eq!(
        normalize_checksum_token("SHA256 (a.iso) = 0xAA-BB:CC"),
        "aabbcc"
    );

    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[test]
fn test_checksum_cancellation() {
    let path = make_temp_file("cancel", b"hello world");
    let cancel = AtomicBool::new(true);
    let res = compute_file_checksums(&path, &cancel, |_, _| {});
    assert!(res.is_err());

    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
