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
    assert!(item.size > 0);
}
