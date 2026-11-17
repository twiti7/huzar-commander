use std::collections::HashSet;
use std::path::PathBuf;
use huzar_commander::fs_item::{FileItem, SortColumn, SortOrder};
use huzar_commander::panel::{matches_filter, PanelState};

fn create_test_panel() -> PanelState {
    let (dir_size_tx, dir_size_rx) = crossbeam_channel::unbounded();
    let parent = FileItem::new_parent(PathBuf::from("C:\\test"));
    let dir_b = FileItem {
        name: "BetaDir".to_string(),
        ext: String::new(),
        path: PathBuf::from("C:\\test\\BetaDir"),
        is_dir: true,
        is_parent: false,
        size: 0,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };
    let dir_a = FileItem {
        name: "AlphaDir".to_string(),
        ext: String::new(),
        path: PathBuf::from("C:\\test\\AlphaDir"),
        is_dir: true,
        is_parent: false,
        size: 0,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };
    let file_z = FileItem {
        name: "zeta.txt".to_string(),
        ext: "txt".to_string(),
        path: PathBuf::from("C:\\test\\zeta.txt"),
        is_dir: false,
        is_parent: false,
        size: 500,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };
    let file_a = FileItem {
        name: "alpha.txt".to_string(),
        ext: "txt".to_string(),
        path: PathBuf::from("C:\\test\\alpha.txt"),
        is_dir: false,
        is_parent: false,
        size: 1500,
        modified: None,
        is_hidden: false,
        is_readonly: false,
    };

    let all = vec![parent, dir_b, dir_a, file_z, file_a];
    PanelState {
        current_path: PathBuf::from("C:\\test"),
        all_items: all.clone(),
        items: all,
        selected_paths: HashSet::new(),
        cursor_idx: 0,
        scroll_to_cursor: false,
        sort_column: SortColumn::Name,
        sort_order: SortOrder::Ascending,
        filter: String::new(),
        is_filter_open: false,
        show_hidden: true,
        custom_title: None,
        zip_location: None,
        total_bytes: 0,
        total_files: 0,
        total_dirs: 0,
        selected_bytes: 0,
        selected_count: 0,
        error_message: None,
        loading_handle: None,
        calculating_dir_sizes: HashSet::new(),
        calculated_dirs: HashSet::new(),
        dir_size_tx,
        dir_size_rx,
    }
}

#[test]
fn test_sorting_and_cursor_preservation() {
    let mut panel = create_test_panel();
    panel.sort_items();

    assert_eq!(panel.items[0].name, "..");
    assert_eq!(panel.items[1].name, "AlphaDir");
    assert_eq!(panel.items[2].name, "BetaDir");
    assert_eq!(panel.items[3].name, "alpha.txt");
    assert_eq!(panel.items[4].name, "zeta.txt");

    panel.cursor_idx = 3;
    assert_eq!(panel.items[panel.cursor_idx].name, "alpha.txt");

    panel.sort_order = SortOrder::Descending;
    panel.sort_items();

    assert_eq!(panel.items[0].name, "..");
    assert_eq!(panel.items[1].name, "BetaDir");
    assert_eq!(panel.items[2].name, "AlphaDir");
    assert_eq!(panel.items[3].name, "zeta.txt");
    assert_eq!(panel.items[4].name, "alpha.txt");

    assert_eq!(panel.cursor_idx, 4);
    assert_eq!(panel.items[panel.cursor_idx].name, "alpha.txt");
}

#[test]
fn test_selection_totals() {
    let mut panel = create_test_panel();
    panel.toggle_select(3); // alpha.txt
    panel.toggle_select(4); // zeta.txt
    assert_eq!(panel.selected_count, 2);
    assert_eq!(panel.selected_bytes, 2000);
}

#[test]
fn test_filter_matching() {
    assert!(matches_filter("main.rs", "*.rs"));
    assert!(!matches_filter("main.txt", "*.rs"));
    assert!(matches_filter("cargo.toml", "*tom*"));
    assert!(matches_filter("test_app.rs", "app"));
    assert!(matches_filter("test.txt", "*.rs;*.txt"));
    assert!(matches_filter("anything", "*"));
}

#[test]
fn test_panel_filtering() {
    let mut panel = create_test_panel();
    panel.set_filter("alpha".to_string());

    // Should keep ".." and any item with "alpha" (AlphaDir and alpha.txt)
    assert_eq!(panel.items.len(), 3);
    assert_eq!(panel.items[0].name, "..");
    assert_eq!(panel.items[1].name, "AlphaDir");
    assert_eq!(panel.items[2].name, "alpha.txt");

    panel.clear_filter();
    assert_eq!(panel.items.len(), 5);
}

#[test]
fn test_batch_selection_and_inversion() {
    let mut panel = create_test_panel();
    // select_all nie powinno zaznaczać elementu rodzica ".."
    panel.select_all();
    assert_eq!(panel.selected_count, 4);

    // deselect_all czyści wszystko
    panel.deselect_all();
    assert_eq!(panel.selected_count, 0);

    // invert_selection odwraca zaznaczenie
    panel.toggle_select(3); // alpha.txt
    assert_eq!(panel.selected_count, 1);
    panel.invert_selection();
    assert_eq!(panel.selected_count, 3); // BetaDir, AlphaDir, zeta.txt
    assert!(!panel.selected_paths.contains(&panel.items[3].path)); // alpha.txt odznaczony
    assert!(!panel.selected_paths.contains(&panel.items[0].path)); // ".." nie zaznaczony

    // selected_or_cursor_items
    let selected = panel.selected_or_cursor_items();
    assert_eq!(selected.len(), 3);

    panel.deselect_all();
    panel.cursor_idx = 3;
    let cursor_only = panel.selected_or_cursor_items();
    assert_eq!(cursor_only.len(), 1);
    assert_eq!(cursor_only[0], panel.items[3].path);
}

#[test]
fn test_hidden_files_filtering() {
    let mut panel = create_test_panel();
    let hidden = FileItem {
        name: ".hidden_file".to_string(),
        ext: String::new(),
        path: PathBuf::from("C:\\test\\.hidden_file"),
        is_dir: false,
        is_parent: false,
        size: 100,
        modified: None,
        is_hidden: true,
        is_readonly: false,
    };
    panel.all_items.push(hidden);
    panel.apply_filter();
    assert_eq!(panel.items.len(), 6);

    // Ukryj pliki ukryte
    panel.toggle_hidden();
    assert_eq!(panel.items.len(), 5);
    assert!(!panel.items.iter().any(|it| it.name == ".hidden_file"));

    // Przywróć pliki ukryte
    panel.toggle_hidden();
    assert_eq!(panel.items.len(), 6);
}

#[test]
fn test_mkdir_and_refresh_cursor_position() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_panel_mkdir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut panel = PanelState::new(temp_dir.clone());
    assert_eq!(panel.items.len(), 1); // Tylko ".."

    // Tworzenie nowego folderu
    let new_folder = temp_dir.join("NowyFolder");
    std::fs::create_dir_all(&new_folder).unwrap();

    // Odświeżenie panelu i pozycjonowanie kursora
    panel.refresh();
    let pos = panel.items.iter().position(|it| it.name == "NowyFolder");
    assert!(pos.is_some());
    let idx = pos.unwrap();
    panel.cursor_idx = idx;
    panel.scroll_to_cursor = true;

    assert_eq!(panel.cursor_idx, idx);
    assert_eq!(panel.items[panel.cursor_idx].name, "NowyFolder");
    assert!(panel.items[panel.cursor_idx].is_dir);
    assert!(panel.scroll_to_cursor);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_async_dir_loading_success() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_async_load_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(temp_dir.join("file1.txt"), "hello").unwrap();
    std::fs::write(temp_dir.join("file2.txt"), "world").unwrap();
    std::fs::create_dir(temp_dir.join("subfolder")).unwrap();

    let mut panel = create_test_panel();
    assert!(!panel.is_loading());

    panel.start_loading_dir(temp_dir.clone(), None, None);
    assert!(panel.is_loading());
    assert_eq!(panel.loading_target(), Some(&temp_dir));

    // Poll until complete
    let start = std::time::Instant::now();
    let mut loaded = false;
    while start.elapsed() < std::time::Duration::from_secs(3) {
        if let Some(res) = panel.poll_loading() {
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), temp_dir);
            loaded = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(loaded, "Directory should have been loaded asynchronously");
    assert!(!panel.is_loading());
    assert_eq!(panel.current_path, temp_dir);
    assert!(panel.items.iter().any(|it| it.name == "subfolder"));
    assert!(panel.items.iter().any(|it| it.name == "file1.txt"));
    assert!(panel.items.iter().any(|it| it.name == "file2.txt"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_async_dir_loading_cancel() {
    let mut panel = create_test_panel();
    let original_path = panel.current_path.clone();

    // Start loading a slow/network path
    panel.start_loading_dir(PathBuf::from("Z:\\slow_or_offline_network_drive"), None, None);
    assert!(panel.is_loading());
    assert_eq!(panel.loading_target(), Some(&PathBuf::from("Z:\\slow_or_offline_network_drive")));

    // Anulowanie operacji
    let cancelled = panel.cancel_loading();
    assert!(cancelled);
    assert!(!panel.is_loading());
    assert_eq!(panel.loading_target(), None);

    // Ponowne anulowanie zwraca false
    assert!(!panel.cancel_loading());

    // Oryginalna ścieżka i elementy panelu nie uległy zmianie
    assert_eq!(panel.current_path, original_path);
    assert_eq!(panel.items.len(), 5);
}

#[test]
fn test_async_dir_loading_go_up() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_async_goup_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let sub = temp_dir.join("child_dir");
    std::fs::create_dir_all(&sub).unwrap();

    let mut panel = PanelState::new(sub.clone());
    assert_eq!(panel.current_path, sub);

    panel.go_up_async(None);
    assert!(panel.is_loading());

    let start = std::time::Instant::now();
    let mut loaded = false;
    while start.elapsed() < std::time::Duration::from_secs(3) {
        if let Some(res) = panel.poll_loading() {
            assert!(res.is_ok());
            loaded = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(loaded);
    assert_eq!(panel.current_path, temp_dir);
    // Kursor powinien być ustawiony na "child_dir"
    let pos = panel.items.iter().position(|it| it.name == "child_dir");
    assert!(pos.is_some());
    assert_eq!(panel.cursor_idx, pos.unwrap());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_space_select_and_async_calculate_dir() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_space_calc_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let sub = temp_dir.join("my_folder");
    std::fs::create_dir_all(&sub).unwrap();
    // 3 files totaling 6000 bytes
    std::fs::write(sub.join("a.bin"), vec![0u8; 1000]).unwrap();
    std::fs::write(sub.join("b.bin"), vec![0u8; 2000]).unwrap();
    std::fs::write(sub.join("c.bin"), vec![0u8; 3000]).unwrap();

    let mut panel = PanelState::new(temp_dir.clone());
    let folder_idx = panel.items.iter().position(|it| it.name == "my_folder").unwrap();
    panel.cursor_idx = folder_idx;

    // Press Space on folder
    let calc = panel.space_select_and_calculate(None);
    assert_eq!(calc, Some(sub.clone()));

    // Folder is selected
    assert!(panel.selected_paths.contains(&sub));
    assert!(panel.is_calculating_dir(&sub));

    // Wait for background calculation
    let start = std::time::Instant::now();
    let mut finished = false;
    while start.elapsed() < std::time::Duration::from_secs(3) {
        let res = panel.poll_dir_sizes();
        if !res.is_empty() {
            assert_eq!(res[0].0, sub);
            assert_eq!(res[0].1, 6000);
            assert_eq!(res[0].2, 3);
            finished = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(finished);
    assert!(!panel.is_calculating_dir(&sub));
    assert!(panel.is_calculated_dir(&sub));

    // Verify folder size updated
    let updated_folder = panel.items.iter().find(|it| it.name == "my_folder").unwrap();
    assert_eq!(updated_folder.size, 6000);
    // Selected bytes includes 6000
    assert_eq!(panel.selected_bytes, 6000);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_space_multiple_dirs_rapid_selection() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_space_rapid_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let sub1 = temp_dir.join("folder1");
    let sub2 = temp_dir.join("folder2");
    std::fs::create_dir_all(&sub1).unwrap();
    std::fs::create_dir_all(&sub2).unwrap();
    std::fs::write(sub1.join("x.txt"), "hello").unwrap();
    std::fs::write(sub2.join("y.txt"), "world!").unwrap();

    let mut panel = PanelState::new(temp_dir.clone());
    // Find the first folder
    let f1_idx = panel.items.iter().position(|it| it.name == "folder1").unwrap();
    panel.cursor_idx = f1_idx;

    // Rapid selection of multiple items with Space without waiting
    panel.space_select_and_calculate(None);
    panel.space_select_and_calculate(None);

    // Both should be marked selected
    assert!(panel.selected_paths.contains(&sub1));
    assert!(panel.selected_paths.contains(&sub2));

    // Both calculate concurrently in background
    let start = std::time::Instant::now();
    let mut total_finished = 0;
    while start.elapsed() < std::time::Duration::from_secs(3) {
        let res = panel.poll_dir_sizes();
        total_finished += res.len();
        if total_finished >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(total_finished, 2);
    assert!(!panel.has_calculating_dirs());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
