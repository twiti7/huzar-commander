use huzar_commander::diff::{compute_side_by_side_diff, load_and_diff_files, DiffLineKind};

#[test]
fn test_compute_side_by_side_diff_and_whitespace() {
    let left = "line1\nline2\n  indented  \nremoved\n";
    let right = "line1\nline2_changed\nindented\nadded\n";

    let strict = compute_side_by_side_diff(left, right, false);
    assert_eq!(strict.rows[0].kind, DiffLineKind::Equal);
    assert!(!strict.diff_row_indices.is_empty());
    assert!(strict.changed_count >= 1);

    let ignore_ws = compute_side_by_side_diff("alpha\n  beta  \n", "alpha\nbeta\n", true);
    assert!(ignore_ws.diff_row_indices.is_empty());
    assert_eq!(ignore_ws.added_count, 0);
    assert_eq!(ignore_ws.removed_count, 0);
    assert_eq!(ignore_ws.changed_count, 0);
    assert!(ignore_ws
        .rows
        .iter()
        .all(|r| r.kind == DiffLineKind::Equal));
}

#[test]
fn test_load_and_diff_files_text_and_binary_rejection() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_diff_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let f1 = temp_dir.join("left.txt");
    let f2 = temp_dir.join("right.txt");
    let bin = temp_dir.join("bin.dat");

    std::fs::write(&f1, "hello\nworld\n").unwrap();
    std::fs::write(&f2, "hello\nrust\nextra\n").unwrap();
    std::fs::write(&bin, [0u8, 1, 2, 0]).unwrap();

    let res = load_and_diff_files(&f1, &f2, false).unwrap();
    assert_eq!(res.changed_count, 1);
    assert_eq!(res.added_count, 1);

    assert!(load_and_diff_files(&f1, &bin, false).is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
