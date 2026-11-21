use std::path::PathBuf;

use huzar_commander::clipboard::{
    decode_dropfiles_wide, encode_dropfiles_wide, ClipboardOp, FileClipboard,
};

#[test]
fn test_dropfiles_wide_roundtrip_with_spaces_and_diacritics() {
    let paths = vec![
        PathBuf::from("C:\\Workspace\\zażółć gęślą jaźń\\plik testowy.txt"),
        PathBuf::from("D:\\Dane\\Raport 2026\\Ważne żółte źródło.md"),
        PathBuf::from("/home/użytkownik/Mój Folder/dokument.pdf"),
    ];

    let wide = encode_dropfiles_wide(&paths);
    // Double-NUL terminated
    assert!(wide.len() >= 2);
    assert_eq!(wide[wide.len() - 1], 0);
    assert_eq!(wide[wide.len() - 2], 0);

    let decoded = decode_dropfiles_wide(&wide);
    assert_eq!(decoded, paths);
}

#[test]
fn test_dropfiles_wide_empty_slice() {
    let wide = encode_dropfiles_wide(&[]);
    assert_eq!(wide, vec![0u16, 0u16]);

    let decoded = decode_dropfiles_wide(&wide);
    assert!(decoded.is_empty());
}

#[test]
fn test_file_clipboard_memory_operations() {
    let mut cb = FileClipboard::default();
    assert!(cb.get_files_memory().is_none());
    assert!(cb.cut_paths().is_none());

    let p1 = PathBuf::from("C:\\tmp\\a.txt");
    let p2 = PathBuf::from("C:\\tmp\\b.txt");
    cb.set_files_memory(vec![p1.clone(), p2.clone()], ClipboardOp::Copy);
    assert_eq!(
        cb.get_files_memory(),
        Some((vec![p1.clone(), p2.clone()], ClipboardOp::Copy))
    );
    assert!(cb.cut_paths().is_none());

    cb.set_files_memory(vec![p1.clone()], ClipboardOp::Cut);
    assert_eq!(
        cb.get_files_memory(),
        Some((vec![p1.clone()], ClipboardOp::Cut))
    );
    assert_eq!(cb.cut_paths(), Some(std::slice::from_ref(&p1)));

    cb.clear_memory();
    assert!(cb.get_files_memory().is_none());
    assert!(cb.cut_paths().is_none());

    cb.set_files_memory(Vec::new(), ClipboardOp::Cut);
    assert!(cb.get_files_memory().is_none());
}
