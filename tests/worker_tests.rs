use std::fs;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use huzar_commander::i18n::Language;
use huzar_commander::worker::{ConflictResolution, IoWorker, WorkerCommand, WorkerEvent};

#[test]
fn test_worker_create_dir() {
    let worker = IoWorker::new();
    let temp_dir =
        std::env::temp_dir().join(format!("cmdr_test_mkdir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    worker
        .cmd_tx
        .send(WorkerCommand::CreateDir {
            path: temp_dir.clone(),
        })
        .unwrap();

    let event = worker
        .event_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("Should receive finished event");
    match event {
        WorkerEvent::QueueUpdated(_) | WorkerEvent::TaskStarted { .. } => {
            let mut got_finished = false;
            while let Ok(ev) = worker.event_rx.recv_timeout(Duration::from_secs(2)) {
                if let WorkerEvent::TaskFinished { message, .. } = ev {
                    assert!(message.contains("utworzony"));
                    got_finished = true;
                    break;
                }
            }
            assert!(got_finished);
        }
        WorkerEvent::TaskFinished { message, .. } => {
            assert!(message.contains("utworzony"));
        }
        _ => panic!("Unexpected event"),
    }
    assert!(temp_dir.exists());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_copy_file() {
    let worker = IoWorker::new();
    let temp_dir =
        std::env::temp_dir().join(format!("cmdr_test_copy_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let src_file = temp_dir.join("src.txt");
    let dst_dir = temp_dir.join("dst");
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(&src_file, "Total Commander In Rust Worker Queue Test").unwrap();

    worker
        .cmd_tx
        .send(WorkerCommand::Copy {
            sources: vec![src_file.clone()],
            target_dir: dst_dir.clone(),
        })
        .unwrap();

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    let dst_file = dst_dir.join("src.txt");
    assert!(dst_file.exists());
    let content = fs::read_to_string(&dst_file).unwrap();
    assert_eq!(content, "Total Commander In Rust Worker Queue Test");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_multi_task_queue() {
    let worker = IoWorker::new();
    let temp_dir =
        std::env::temp_dir().join(format!("cmdr_test_queue_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let f1 = temp_dir.join("f1.txt");
    let f2 = temp_dir.join("f2.txt");
    fs::write(&f1, "file 1 content").unwrap();
    fs::write(&f2, "file 2 content").unwrap();

    let dst1 = temp_dir.join("d1");
    let dst2 = temp_dir.join("d2");

    // Enqueue two tasks simultaneously
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![f1.clone()],
        target_dir: dst1.clone(),
    });
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![f2.clone()],
        target_dir: dst2.clone(),
    });

    let mut queue_finished = false;
    let mut tasks_finished = 0;

    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(5)) {
        match event {
            WorkerEvent::TaskFinished { .. } => {
                tasks_finished += 1;
            }
            WorkerEvent::QueueFinished { .. } => {
                queue_finished = true;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(tasks_finished, 2);
    assert!(queue_finished);
    assert!(dst1.join("f1.txt").exists());
    assert!(dst2.join("f2.txt").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_skip_current_task() {
    let worker = IoWorker::new();
    let temp_dir =
        std::env::temp_dir().join(format!("cmdr_test_skip_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a 15MB file for task 1 so it takes time to copy
    let f1 = temp_dir.join("f1_large.dat");
    let f2 = temp_dir.join("f2_small.txt");
    fs::write(&f1, vec![0xAA; 15 * 1024 * 1024]).unwrap();
    fs::write(&f2, "second task small file").unwrap();

    let dst1 = temp_dir.join("d1");
    let dst2 = temp_dir.join("d2");

    worker.enqueue(WorkerCommand::Copy {
        sources: vec![f1.clone()],
        target_dir: dst1.clone(),
    });
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![f2.clone()],
        target_dir: dst2.clone(),
    });

    // Wait until task 1 starts
    let mut task1_started = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskStarted { .. } = event {
            task1_started = true;
            break;
        }
    }
    assert!(task1_started);

    // Immediately skip task 1!
    worker.skip_current();

    let mut got_task1_skipped = false;
    let mut got_task2_finished = false;
    let mut got_queue_finished = false;

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(event) = worker.event_rx.recv_timeout(Duration::from_millis(500)) {
            match event {
                WorkerEvent::TaskFinished { message, .. } => {
                    if message.contains("Pominięto") {
                        got_task1_skipped = true;
                    } else if message.contains("Skopiowano") {
                        got_task2_finished = true;
                    }
                }
                WorkerEvent::QueueFinished { .. } => {
                    got_queue_finished = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(got_task1_skipped, "Task 1 should have been skipped");
    assert!(got_task2_finished, "Task 2 should have finished successfully");
    assert!(got_queue_finished, "Queue should have finished");
    assert!(dst2.join("f2_small.txt").exists(), "Destination file for Task 2 must exist");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_enqueue_while_running() {
    let worker = IoWorker::new();
    let temp_dir =
        std::env::temp_dir().join(format!("cmdr_test_dyn_queue_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // 10MB file for task 1
    let f1 = temp_dir.join("f1.dat");
    let f2 = temp_dir.join("f2.txt");
    fs::write(&f1, vec![0xBB; 10 * 1024 * 1024]).unwrap();
    fs::write(&f2, "dynamic queue task 2 content").unwrap();

    let dst1 = temp_dir.join("d1");
    let dst2 = temp_dir.join("d2");

    // Enqueue only task 1 first
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![f1.clone()],
        target_dir: dst1.clone(),
    });

    // Wait until task 1 starts
    let mut task1_started = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskStarted { .. } = event {
            task1_started = true;
            break;
        }
    }
    assert!(task1_started);

    // While task 1 is running, dynamically enqueue task 2!
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![f2.clone()],
        target_dir: dst2.clone(),
    });

    // Immediately verify that a QueueUpdated event is received showing 2 total tasks
    let mut got_queue_update_with_pending = false;
    let mut task1_done = false;
    let mut task2_done = false;
    let mut queue_done = false;

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        if let Ok(event) = worker.event_rx.recv_timeout(Duration::from_millis(500)) {
            match event {
                WorkerEvent::QueueUpdated(snapshot) => {
                    if snapshot.total_tasks >= 2 {
                        got_queue_update_with_pending = true;
                    }
                }
                WorkerEvent::TaskFinished { message, .. } => {
                    if message.contains("f1.dat") || !task1_done {
                        task1_done = true;
                    } else {
                        task2_done = true;
                    }
                }
                WorkerEvent::QueueFinished { .. } => {
                    queue_done = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(got_queue_update_with_pending, "Queue should immediately receive updated snapshot with 2 tasks");
    assert!(task1_done, "Task 1 should complete");
    assert!(task2_done, "Task 2 should complete");
    assert!(queue_done, "Queue should finish");
    assert!(dst1.join("f1.dat").exists());
    assert!(dst2.join("f2.txt").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_move_file() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_move_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let src_file = temp_dir.join("move_me.txt");
    let dst_dir = temp_dir.join("moved_dst");
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(&src_file, "Moving test content").unwrap();

    worker.enqueue(WorkerCommand::Move {
        sources: vec![src_file.clone()],
        target_dir: dst_dir.clone(),
    });

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    assert!(!src_file.exists(), "Source file should have been moved away");
    let dst_file = dst_dir.join("move_me.txt");
    assert!(dst_file.exists(), "Target file should exist in destination directory");
    assert_eq!(fs::read_to_string(&dst_file).unwrap(), "Moving test content");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_delete_file() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_delete_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let file_to_delete = temp_dir.join("delete_me.txt");
    fs::write(&file_to_delete, "Delete me now").unwrap();
    assert!(file_to_delete.exists());

    worker.enqueue(WorkerCommand::Delete {
        targets: vec![file_to_delete.clone()],
        use_trash: false,
    });

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    assert!(!file_to_delete.exists(), "File should be permanently deleted");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_pause_resume() {
    let worker = IoWorker::new();
    // Sprawdź początkowy stan pauzy
    assert!(!worker.is_paused.load(Ordering::Relaxed));

    // Włącz pauzę
    worker.toggle_pause();
    assert!(worker.is_paused.load(Ordering::Relaxed));

    // Wyłącz pauzę
    worker.toggle_pause();
    assert!(!worker.is_paused.load(Ordering::Relaxed));
}

#[test]
fn test_worker_copy_same_file_generates_copy_name() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_copy_same_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let src_file = temp_dir.join("document.txt");
    fs::write(&src_file, "Wazna zawartosc dokumentu").unwrap();

    // Kopiowanie do tego samego katalogu (src == dest)
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![src_file.clone()],
        target_dir: temp_dir.clone(),
    });

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    // Oryginalny plik nie może być wyzerowany!
    let orig_content = fs::read_to_string(&src_file).unwrap();
    assert_eq!(orig_content, "Wazna zawartosc dokumentu");

    // Powinien powstać plik z dopiskiem " - Kopia"
    let copy_file = temp_dir.join("document - Kopia.txt");
    assert!(copy_file.exists(), "Kopia pliku powinna istnieć");
    let copy_content = fs::read_to_string(&copy_file).unwrap();
    assert_eq!(copy_content, "Wazna zawartosc dokumentu");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_move_same_file_is_noop() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_move_same_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let src_file = temp_dir.join("data.bin");
    fs::write(&src_file, "Nie ruszaj mnie").unwrap();

    worker.enqueue(WorkerCommand::Move {
        sources: vec![src_file.clone()],
        target_dir: temp_dir.clone(),
    });

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    assert!(src_file.exists());
    assert_eq!(fs::read_to_string(&src_file).unwrap(), "Nie ruszaj mnie");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_copy_dir_into_subfolder_rejected() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_loop_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let parent_dir = temp_dir.join("parent");
    let sub_dir = parent_dir.join("child");
    fs::create_dir_all(&sub_dir).unwrap();

    // Próba skopiowania parent_dir do parent_dir/child (pętla)
    worker.enqueue(WorkerCommand::Copy {
        sources: vec![parent_dir.clone()],
        target_dir: sub_dir.clone(),
    });

    let mut got_error = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::Error { message, .. } = event {
            assert!(message.contains("podkatalogu"));
            got_error = true;
            break;
        }
    }

    assert!(got_error, "Powinien wystąpić błąd zapobiegający nieskończonej rekurencji");
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_rename_file() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_rename_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let old_file = temp_dir.join("old_name.txt");
    fs::write(&old_file, "Zawartosc do zmiany nazwy").unwrap();

    worker.enqueue(WorkerCommand::Rename {
        source: old_file.clone(),
        new_name: "brand_new_name.txt".to_string(),
    });

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    assert!(!old_file.exists());
    let new_file = temp_dir.join("brand_new_name.txt");
    assert!(new_file.exists());
    assert_eq!(fs::read_to_string(&new_file).unwrap(), "Zawartosc do zmiany nazwy");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_create_zip() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_create_zip_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let f1 = temp_dir.join("pack1.txt");
    let f2 = temp_dir.join("pack2.txt");
    fs::write(&f1, "Plik 1 do spakowania").unwrap();
    fs::write(&f2, "Plik 2 do spakowania").unwrap();

    let zip_out = temp_dir.join("result.zip");

    worker.enqueue(WorkerCommand::CreateZip {
        sources: vec![f1.clone(), f2.clone()],
        archive_path: zip_out.clone(),
    });

    let mut completed = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            completed = true;
            break;
        }
    }

    assert!(completed);
    assert!(zip_out.exists());
    assert!(huzar_commander::zip_support::is_zip_file(&zip_out));

    let entries = huzar_commander::zip_support::list_zip_entries(&zip_out, "").unwrap();
    assert_eq!(entries.len(), 2);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_repaint_signal() {
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    let worker = IoWorker::new();
    let repaint_count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&repaint_count);

    worker.set_repaint_signal(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });

    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_repaint_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    worker.enqueue(WorkerCommand::CreateDir {
        path: temp_dir.clone(),
    });

    let mut got_event = false;
    while let Ok(event) = worker.event_rx.recv_timeout(Duration::from_secs(3)) {
        if let WorkerEvent::TaskFinished { .. } = event {
            got_event = true;
            break;
        }
    }

    assert!(got_event);
    assert!(repaint_count.load(Ordering::SeqCst) > 0);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_conflict_overwrite_and_skip() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_conflict_ow_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let src_dir = temp_dir.join("src");
    let dst_dir = temp_dir.join("dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();

    let s1 = src_dir.join("a.txt");
    let s2 = src_dir.join("b.txt");
    fs::write(&s1, "NEW A").unwrap();
    fs::write(&s2, "NEW B").unwrap();

    fs::write(dst_dir.join("a.txt"), "OLD A").unwrap();
    fs::write(dst_dir.join("b.txt"), "OLD B").unwrap();

    worker.enqueue(WorkerCommand::Copy {
        sources: vec![s1, s2],
        target_dir: dst_dir.clone(),
    });

    let mut finished = false;
    while let Ok(ev) = worker.event_rx.recv_timeout(Duration::from_secs(5)) {
        match ev {
            WorkerEvent::ConflictDetected { destination, reply_tx, .. } => {
                if destination.name == "a.txt" {
                    reply_tx.send(ConflictResolution::Skip).unwrap();
                } else {
                    reply_tx.send(ConflictResolution::Overwrite).unwrap();
                }
            }
            WorkerEvent::QueueFinished { .. } => {
                finished = true;
                break;
            }
            _ => {}
        }
    }

    assert!(finished);
    assert_eq!(fs::read_to_string(dst_dir.join("a.txt")).unwrap(), "OLD A");
    assert_eq!(fs::read_to_string(dst_dir.join("b.txt")).unwrap(), "NEW B");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_conflict_rename() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_conflict_ren_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let src_dir = temp_dir.join("src");
    let dst_dir = temp_dir.join("dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();

    let src = src_dir.join("file.txt");
    fs::write(&src, "NEW CONTENT").unwrap();
    fs::write(dst_dir.join("file.txt"), "EXISTING CONTENT").unwrap();

    worker.enqueue(WorkerCommand::Copy {
        sources: vec![src],
        target_dir: dst_dir.clone(),
    });

    let mut finished = false;
    while let Ok(ev) = worker.event_rx.recv_timeout(Duration::from_secs(5)) {
        match ev {
            WorkerEvent::ConflictDetected { reply_tx, .. } => {
                reply_tx
                    .send(ConflictResolution::Rename("renamed.txt".to_string()))
                    .unwrap();
            }
            WorkerEvent::QueueFinished { .. } => {
                finished = true;
                break;
            }
            _ => {}
        }
    }

    assert!(finished);
    assert_eq!(
        fs::read_to_string(dst_dir.join("file.txt")).unwrap(),
        "EXISTING CONTENT"
    );
    assert_eq!(
        fs::read_to_string(dst_dir.join("renamed.txt")).unwrap(),
        "NEW CONTENT"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_conflict_overwrite_older() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_conflict_older_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let src_dir = temp_dir.join("src");
    let dst_dir = temp_dir.join("dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();

    let now = std::time::SystemTime::now();
    let older = now - Duration::from_secs(3600);
    let newer = now - Duration::from_secs(60);

    let s_newer = src_dir.join("newer.txt");
    let s_older = src_dir.join("older.txt");
    let d_newer = dst_dir.join("newer.txt");
    let d_older = dst_dir.join("older.txt");

    fs::write(&s_newer, "SRC NEWER").unwrap();
    fs::File::options().write(true).open(&s_newer).unwrap().set_modified(newer).unwrap();
    fs::write(&d_newer, "DST OLDER").unwrap();
    fs::File::options().write(true).open(&d_newer).unwrap().set_modified(older).unwrap();

    fs::write(&s_older, "SRC OLDER").unwrap();
    fs::File::options().write(true).open(&s_older).unwrap().set_modified(older).unwrap();
    fs::write(&d_older, "DST NEWER").unwrap();
    fs::File::options().write(true).open(&d_older).unwrap().set_modified(newer).unwrap();

    worker.enqueue(WorkerCommand::Copy {
        sources: vec![s_newer, s_older],
        target_dir: dst_dir.clone(),
    });

    let mut prompt_count = 0;
    let mut finished = false;
    while let Ok(ev) = worker.event_rx.recv_timeout(Duration::from_secs(5)) {
        match ev {
            WorkerEvent::ConflictDetected { reply_tx, .. } => {
                prompt_count += 1;
                reply_tx.send(ConflictResolution::OverwriteOlder).unwrap();
            }
            WorkerEvent::QueueFinished { .. } => {
                finished = true;
                break;
            }
            _ => {}
        }
    }

    assert!(finished);
    assert_eq!(prompt_count, 1, "OverwriteOlder applies to remaining conflicts in the task");
    assert_eq!(fs::read_to_string(&d_newer).unwrap(), "SRC NEWER");
    assert_eq!(fs::read_to_string(&d_older).unwrap(), "DST NEWER");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_preserves_mtime() {
    let worker = IoWorker::new();
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_mtime_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let dst_dir = temp_dir.join("dst");
    fs::create_dir_all(&dst_dir).unwrap();

    let src = temp_dir.join("dated.txt");
    fs::write(&src, "preserve my mtime").unwrap();
    let target_time = std::time::SystemTime::now() - Duration::from_secs(86_400);
    fs::File::options().write(true).open(&src).unwrap().set_modified(target_time).unwrap();
    let expected_mtime = fs::metadata(&src).unwrap().modified().unwrap();

    worker.enqueue(WorkerCommand::Copy {
        sources: vec![src],
        target_dir: dst_dir.clone(),
    });

    let mut finished = false;
    while let Ok(ev) = worker.event_rx.recv_timeout(Duration::from_secs(5)) {
        if let WorkerEvent::QueueFinished { .. } = ev {
            finished = true;
            break;
        }
    }
    assert!(finished);

    let copied_mtime = fs::metadata(dst_dir.join("dated.txt")).unwrap().modified().unwrap();
    let diff = expected_mtime
        .duration_since(copied_mtime)
        .or_else(|e| Ok::<Duration, std::time::SystemTimeError>(e.duration()))
        .unwrap();
    assert!(diff < Duration::from_secs(2), "Copied file mtime should match source");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_worker_english_messages() {
    let worker = IoWorker::new();
    worker.set_language(Language::En);
    let temp_dir = std::env::temp_dir().join(format!("cmdr_test_en_msg_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    worker.enqueue(WorkerCommand::CreateDir {
        path: temp_dir.clone(),
    });

    let mut finished_msg = String::new();
    while let Ok(ev) = worker.event_rx.recv_timeout(Duration::from_secs(5)) {
        if let WorkerEvent::TaskFinished { message, .. } = ev {
            finished_msg = message;
            break;
        }
    }

    assert_eq!(finished_msg, "Folder created");
    let _ = fs::remove_dir_all(&temp_dir);
}
