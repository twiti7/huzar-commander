use std::path::{Path, PathBuf};
use huzar_commander::fs_item::{SortColumn, SortOrder};
use huzar_commander::ui::panels::{build_breadcrumb_segments, sort_indicator};

#[test]
fn test_sort_indicator_display() {
    assert_eq!(sort_indicator(SortColumn::Name, SortColumn::Name, SortOrder::Ascending), "▲");
    assert_eq!(sort_indicator(SortColumn::Name, SortColumn::Name, SortOrder::Descending), "▼");
    assert_eq!(sort_indicator(SortColumn::Size, SortColumn::Name, SortOrder::Ascending), "");
}

#[test]
fn test_build_breadcrumb_segments() {
    let win = build_breadcrumb_segments(Path::new(r"C:\Workspace\commander"));
    assert_eq!(
        win,
        vec![
            ("C:\\".to_string(), PathBuf::from(r"C:\")),
            ("Workspace".to_string(), PathBuf::from(r"C:\Workspace")),
            ("commander".to_string(), PathBuf::from(r"C:\Workspace\commander")),
        ]
    );

    let unc_root = build_breadcrumb_segments(Path::new(r"\\TRUENAS"));
    assert_eq!(
        unc_root,
        vec![(r"\\TRUENAS".to_string(), PathBuf::from(r"\\TRUENAS"))]
    );

    let unc = build_breadcrumb_segments(Path::new(r"\\server\share\dir"));
    assert_eq!(
        unc,
        vec![
            (r"\\server".to_string(), PathBuf::from(r"\\server")),
            ("share".to_string(), PathBuf::from(r"\\server\share")),
            ("dir".to_string(), PathBuf::from(r"\\server\share\dir")),
        ]
    );

    let unix = build_breadcrumb_segments(Path::new("/home/user/docs"));
    assert_eq!(
        unix,
        vec![
            ("/".to_string(), PathBuf::from("/")),
            ("home".to_string(), PathBuf::from("/home")),
            ("user".to_string(), PathBuf::from("/home/user")),
            ("docs".to_string(), PathBuf::from("/home/user/docs")),
        ]
    );
}
