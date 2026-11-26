use std::path::PathBuf;
use huzar_commander::fs_item::FileItem;

#[test]
fn test_new_parent() {
    let parent_path = PathBuf::from("C:\\test");
    let item = FileItem::new_parent(parent_path.clone());
    assert_eq!(item.name, "..");
    assert_eq!(item.path, parent_path);
    assert!(item.is_dir);
    assert!(item.is_parent);
    assert_eq!(item.size, 0);
    assert_eq!(item.ext, "");
}

#[test]
fn test_from_path() {
    let current_dir = std::env::current_dir().unwrap();
    let cargo_toml = current_dir.join("Cargo.toml");
    let item = FileItem::from_path(&cargo_toml).expect("Cargo.toml should exist");

    assert_eq!(item.name, "Cargo.toml");
    assert_eq!(item.ext, "toml");
    assert!(!item.is_dir);
    assert!(!item.is_parent);
    assert!(!item.is_symlink);
    assert!(item.size > 0);
}

#[test]
fn test_symlink_from_path_handling() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_fs_item_symlink_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let target = temp_dir.join("target.txt");
    std::fs::write(&target, "hello symlink").unwrap();
    let link_path = temp_dir.join("link.txt");

    #[cfg(unix)]
    let link_res = std::os::unix::fs::symlink(&target, &link_path);
    #[cfg(windows)]
    let link_res = std::os::windows::fs::symlink_file(&target, &link_path);

    if link_res.is_ok() {
        let item = FileItem::from_path(&link_path).expect("Valid symlink should be read");
        assert!(item.is_symlink);
        assert!(!item.is_dir);
        assert_eq!(item.size, 13);

        // Remove target to create a broken symlink — FileItem::from_path must still return it
        std::fs::remove_file(&target).unwrap();
        let broken = FileItem::from_path(&link_path).expect("Broken symlink should remain visible");
        assert!(broken.is_symlink);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
