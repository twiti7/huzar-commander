use huzar_commander::drives::scan_drives;

#[test]
fn test_scan_drives() {
    let drives = scan_drives();
    assert!(!drives.is_empty(), "Should detect at least one drive");

    for drive in &drives {
        assert!(drive.path.exists(), "Drive path {:?} should exist", drive.path);
        assert!(drive.letter.is_ascii_alphabetic(), "Drive letter should be alphabetic");
        #[cfg(target_os = "windows")]
        {
            assert!(drive.total_space > 0, "System drive should have total space");
        }
    }
}
