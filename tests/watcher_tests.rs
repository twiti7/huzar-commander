use std::path::Path;
use std::time::{Duration, Instant};

use huzar_commander::watcher::{is_watchable_dir, paths_match, DirWatcher};

#[test]
fn test_watcher_paths_match_and_is_watchable() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_watch_check_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    assert!(is_watchable_dir(&temp_dir));
    assert!(!is_watchable_dir(Path::new(r"\\TRUENAS")));
    assert!(!is_watchable_dir(&temp_dir.join("missing_subdir")));

    let with_slash = format!("{}\\", temp_dir.to_string_lossy());
    assert!(paths_match(&temp_dir, Path::new(&with_slash)));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_dir_watcher_detects_file_creation_with_debounce() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_watch_live_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut watcher = DirWatcher::new(Duration::from_millis(60), None);
    watcher.sync_watched_dirs(std::slice::from_ref(&temp_dir));

    // Give OS directory watcher a brief moment to register
    std::thread::sleep(Duration::from_millis(60));
    std::fs::write(temp_dir.join("created.txt"), "hello").unwrap();

    let start = Instant::now();
    let mut detected = false;
    while start.elapsed() < Duration::from_secs(3) {
        let ready = watcher.poll_ready_dirs();
        if ready.iter().any(|p| paths_match(p, &temp_dir)) {
            detected = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    assert!(detected, "DirWatcher should report modified watched directory after debounce");
    let _ = std::fs::remove_dir_all(&temp_dir);
}
