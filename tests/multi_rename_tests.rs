use std::path::PathBuf;

use huzar_commander::fs_item::FileItem;
use huzar_commander::multi_rename::{
    compute_rename_preview, execute_batch_rename, CaseTransform, MultiRenameConfig,
};

fn make_item(path: &std::path::Path, is_dir: bool) -> FileItem {
    FileItem {
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        ext: if is_dir {
            String::new()
        } else {
            path.extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        },
        path: path.to_path_buf(),
        is_dir,
        is_parent: false,
        size: 10,
        modified: None,
        is_hidden: false,
        is_readonly: false,
        is_symlink: false,
    }
}

#[test]
fn test_multi_rename_placeholders_regex_and_case() {
    let dir = PathBuf::from(r"C:\tmp_virtual");
    let items = vec![
        make_item(&dir.join("photo_alpha.JPG"), false),
        make_item(&dir.join("photo_beta.jpg"), false),
    ];

    let cfg = MultiRenameConfig {
        name_mask: "img_[C]_[N]".to_string(),
        ext_mask: "[E]".to_string(),
        search: r"photo_(\w+)".to_string(),
        replace: "$1".to_string(),
        use_regex: true,
        case_sensitive: false,
        counter_start: 10,
        counter_step: 5,
        counter_digits: 3,
        case_transform: CaseTransform::Lower,
    };

    let rows = compute_rename_preview(&items, &cfg).expect("Valid regex preview");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].new_name, "img_010_alpha.jpg");
    assert_eq!(rows[1].new_name, "img_015_beta.jpg");
    assert!(rows.iter().all(|r| r.conflict.is_none() && r.changed));
}

#[test]
fn test_multi_rename_collision_detection() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_mr_coll_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let a = temp_dir.join("a.txt");
    let b = temp_dir.join("b.txt");
    let existing = temp_dir.join("taken.txt");
    std::fs::write(&a, "1").unwrap();
    std::fs::write(&b, "2").unwrap();
    std::fs::write(&existing, "3").unwrap();

    let items = vec![make_item(&a, false), make_item(&b, false)];

    // Duplicate target inside batch
    let dup_cfg = MultiRenameConfig {
        name_mask: "same".to_string(),
        ..Default::default()
    };
    let dup_rows = compute_rename_preview(&items, &dup_cfg).unwrap();
    assert!(dup_rows.iter().all(|r| r.conflict.is_some()));

    // Conflict with existing file on disk
    let disk_cfg = MultiRenameConfig {
        name_mask: "taken".to_string(),
        ..Default::default()
    };
    let disk_rows = compute_rename_preview(&[make_item(&a, false)], &disk_cfg).unwrap();
    assert!(disk_rows[0].conflict.is_some());

    // Invalid regex returns Err
    let bad_regex_cfg = MultiRenameConfig {
        search: "[unclosed".to_string(),
        use_regex: true,
        ..Default::default()
    };
    assert!(compute_rename_preview(&items, &bad_regex_cfg).is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_execute_batch_rename_swap_cycle_and_undo() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_mr_exec_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let a = temp_dir.join("a.txt");
    let b = temp_dir.join("b.txt");
    std::fs::write(&a, "CONTENT_A").unwrap();
    std::fs::write(&b, "CONTENT_B").unwrap();

    // Swap cycle a.txt <-> b.txt
    let pairs = vec![(a.clone(), b.clone()), (b.clone(), a.clone())];
    let done = execute_batch_rename(&pairs).unwrap();
    assert_eq!(done.len(), 2);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "CONTENT_B");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "CONTENT_A");

    // Undo batch
    let undo_pairs: Vec<(PathBuf, PathBuf)> =
        done.into_iter().rev().map(|(old, new)| (new, old)).collect();
    execute_batch_rename(&undo_pairs).unwrap();
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "CONTENT_A");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "CONTENT_B");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
