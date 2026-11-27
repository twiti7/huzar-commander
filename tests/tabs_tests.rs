use std::path::PathBuf;

use huzar_commander::panel::PanelState;
use huzar_commander::tabs::{PanelTab, PanelTabManager};

#[test]
fn test_tab_manager_duplicate_switch_close_and_lock() {
    let temp_dir = std::env::temp_dir().join(format!("cmdr_tabs_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let sub_a = temp_dir.join("alpha");
    let sub_b = temp_dir.join("beta");
    std::fs::create_dir_all(&sub_a).unwrap();
    std::fs::create_dir_all(&sub_b).unwrap();

    let mut mgr = PanelTabManager::new(PanelState::new(sub_a.clone()));
    assert_eq!(mgr.len(), 1);
    assert_eq!(mgr.active_idx, 0);
    assert_eq!(PanelTabManager::tab_title(mgr.active_tab()), "alpha");

    // Cannot close the only tab
    assert!(!mgr.close_active_tab());

    // Duplicate creates independent tab & activates it
    let new_idx = mgr.duplicate_active_tab();
    assert_eq!(new_idx, 1);
    assert_eq!(mgr.len(), 2);
    assert_eq!(mgr.active_idx, 1);

    mgr.active_mut().change_dir(sub_b.clone());
    assert_eq!(mgr.tabs[0].panel.current_path, sub_a);
    assert_eq!(mgr.tabs[1].panel.current_path, sub_b);
    assert_eq!(PanelTabManager::tab_title(&mgr.tabs[1]), "beta");

    // Cycle tabs
    mgr.next_tab();
    assert_eq!(mgr.active_idx, 0);
    mgr.prev_tab();
    assert_eq!(mgr.active_idx, 1);

    // Lock prevents closing
    assert_eq!(mgr.toggle_lock(1), Some(true));
    assert!(!mgr.close_tab(1));
    assert_eq!(mgr.len(), 2);

    // Unlock allows closing
    assert_eq!(mgr.toggle_lock(1), Some(false));
    assert!(mgr.close_tab(1));
    assert_eq!(mgr.len(), 1);
    assert_eq!(mgr.active_idx, 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tab_title_drive_and_unc_root() {
    let unc_tab = PanelTab {
        panel: PanelState::new_deferred(PathBuf::from(r"\\TRUENAS"), true),
        is_locked: false,
    };
    assert_eq!(PanelTabManager::tab_title(&unc_tab), r"\\TRUENAS");

    let custom_tab = PanelTab {
        panel: PanelState {
            custom_title: Some("Wyniki".to_string()),
            ..PanelState::new_deferred(PathBuf::from("."), true)
        },
        is_locked: false,
    };
    assert_eq!(PanelTabManager::tab_title(&custom_tab), "[Wyniki]");
}
