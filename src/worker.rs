use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::i18n::Language;

fn lock_unpoison<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone)]
pub struct ConflictFileInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictResolution {
    Overwrite,
    OverwriteAll,
    OverwriteOlder,
    Skip,
    SkipAll,
    Rename(String),
    Cancel,
}

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    Copy {
        sources: Vec<PathBuf>,
        target_dir: PathBuf,
    },
    Move {
        sources: Vec<PathBuf>,
        target_dir: PathBuf,
    },
    Delete {
        targets: Vec<PathBuf>,
        use_trash: bool,
    },
    CreateDir {
        path: PathBuf,
    },
    ExtractZip {
        archive_path: PathBuf,
        target_dir: PathBuf,
        filter_prefix: Option<String>,
        selected_entries: Vec<String>,
    },
    Rename {
        source: PathBuf,
        new_name: String,
    },
    BatchRename {
        pairs: Vec<(PathBuf, PathBuf)>,
    },
    CreateZip {
        sources: Vec<PathBuf>,
        archive_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct TaskItem {
    pub id: u64,
    pub title: String,
    pub detail: String,
    pub command: WorkerCommand,
    pub total_items: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct QueueStateSnapshot {
    pub active_task: Option<TaskItem>,
    pub pending_tasks: Vec<TaskItem>,
    pub active_index: usize,
    pub total_tasks: usize,
    pub queue_total_items: usize,
    pub queue_items_done: usize,
    pub queue_total_bytes: u64,
    pub queue_bytes_done: u64,
    pub is_paused: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WorkerControl {
    Enqueue(WorkerCommand),
    Pause,
    Resume,
    TogglePause,
    SkipCurrent,
    CancelAll,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WorkerEvent {
    QueueUpdated(QueueStateSnapshot),
    TaskStarted {
        task_id: u64,
        title: String,
        total_items: usize,
        total_bytes: u64,
    },
    Progress {
        task_id: u64,
        current_name: String,
        task_items_done: usize,
        task_total_items: usize,
        task_bytes_done: u64,
        task_total_bytes: u64,
        current_file_bytes: u64,
        current_file_size: u64,
        queue_items_done: usize,
        queue_total_items: usize,
        queue_bytes_done: u64,
        queue_total_bytes: u64,
        speed_bps: f32,
        eta_secs: Option<u64>,
    },
    TaskFinished {
        task_id: u64,
        message: String,
    },
    QueueFinished {
        message: String,
    },
    Error {
        task_id: u64,
        message: String,
    },
    ConflictDetected {
        task_id: u64,
        source: ConflictFileInfo,
        destination: ConflictFileInfo,
        reply_tx: Sender<ConflictResolution>,
    },
}

#[derive(Debug, Default)]
pub struct WorkerQueueState {
    pub pending_queue: VecDeque<TaskItem>,
    pub active_task: Option<TaskItem>,
    pub task_id_gen: u64,
    pub total_tasks_counter: usize,
    pub completed_tasks_counter: usize,
    pub queue_total_items: usize,
    pub queue_items_done: usize,
    pub queue_total_bytes: u64,
    pub queue_bytes_done_base: u64,
}

impl WorkerQueueState {
    pub fn snapshot(&self, is_paused: bool) -> QueueStateSnapshot {
        QueueStateSnapshot {
            active_task: self.active_task.clone(),
            pending_tasks: self.pending_queue.iter().cloned().collect(),
            active_index: if self.active_task.is_some() {
                self.completed_tasks_counter + 1
            } else {
                self.completed_tasks_counter
            },
            total_tasks: self.total_tasks_counter,
            queue_total_items: self.queue_total_items,
            queue_items_done: self.queue_items_done,
            queue_total_bytes: self.queue_total_bytes,
            queue_bytes_done: self.queue_bytes_done_base,
            is_paused,
        }
    }
}

pub type RepaintSignal = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

#[derive(Clone)]
pub struct IoWorker {
    pub control_tx: Sender<WorkerControl>,
    pub cmd_tx: Sender<WorkerCommand>,
    pub event_rx: Receiver<WorkerEvent>,
    pub is_paused: Arc<AtomicBool>,
    pub skip_current_flag: Arc<AtomicBool>,
    pub cancel_all_flag: Arc<AtomicBool>,
    pub state: Arc<Mutex<WorkerQueueState>>,
    pub wake_condvar: Arc<Condvar>,
    pub event_tx: Sender<WorkerEvent>,
    pub repaint_signal: RepaintSignal,
    pub language: Arc<Mutex<Language>>,
}

impl IoWorker {
    pub fn new() -> Self {
        let (control_tx, control_rx) = crossbeam_channel::unbounded::<WorkerControl>();
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<WorkerCommand>();
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<WorkerEvent>();

        let is_paused = Arc::new(AtomicBool::new(false));
        let skip_current_flag = Arc::new(AtomicBool::new(false));
        let cancel_all_flag = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(WorkerQueueState::default()));
        let wake_condvar = Arc::new(Condvar::new());
        let repaint_signal: RepaintSignal = Arc::new(Mutex::new(None));
        let language = Arc::new(Mutex::new(Language::Pl));

        let is_paused_thread = Arc::clone(&is_paused);
        let skip_current_thread = Arc::clone(&skip_current_flag);
        let cancel_all_thread = Arc::clone(&cancel_all_flag);
        let state_thread = Arc::clone(&state);
        let condvar_thread = Arc::clone(&wake_condvar);
        let event_tx_thread = event_tx.clone();
        let repaint_sig_thread = Arc::clone(&repaint_signal);
        let lang_thread = Arc::clone(&language);

        // Worker execution loop
        thread::spawn(move || {
            let send_event = |ev: WorkerEvent| {
                let _ = event_tx_thread.send(ev);
                if let Ok(guard) = repaint_sig_thread.lock() {
                    if let Some(ref cb) = *guard {
                        cb();
                    }
                }
            };

            loop {
                // 1. Wait for a task in pending_queue
                let (mut current_task, initial_items) = {
                    let mut q = lock_unpoison(&state_thread);
                    while q.pending_queue.is_empty() {
                        q.active_task = None;
                        if cancel_all_thread.load(Ordering::Relaxed) {
                            cancel_all_thread.store(false, Ordering::Relaxed);
                        }
                        q = condvar_thread.wait(q).unwrap_or_else(|e| e.into_inner());
                    }

                    if cancel_all_thread.load(Ordering::Relaxed) {
                        cancel_all_thread.store(false, Ordering::Relaxed);
                        q.pending_queue.clear();
                        continue;
                    }

                    let task = match q.pending_queue.pop_front() {
                        Some(t) => t,
                        None => continue,
                    };

                    let init_items = task.total_items;
                    q.active_task = Some(task.clone());
                    skip_current_thread.store(false, Ordering::SeqCst);

                    let snapshot = q.snapshot(is_paused_thread.load(Ordering::Relaxed));
                    send_event(WorkerEvent::QueueUpdated(snapshot));
                    (task, init_items)
                };

                // 2. Calculate exact totals in background thread (never on UI thread)
                let (actual_items, actual_bytes) = calculate_command_totals_cancellable(
                    &current_task.command,
                    &skip_current_thread,
                    &cancel_all_thread,
                );

                let scan_cancelled = cancel_all_thread.load(Ordering::Relaxed);
                let scan_skipped = skip_current_thread.load(Ordering::Relaxed);
                let lang = *lock_unpoison(&lang_thread);

                if scan_cancelled {
                    {
                        let mut q = lock_unpoison(&state_thread);
                        q.active_task = None;
                        q.pending_queue.clear();
                        q.total_tasks_counter = 0;
                        q.completed_tasks_counter = 0;
                        q.queue_total_items = 0;
                        q.queue_items_done = 0;
                        q.queue_total_bytes = 0;
                        q.queue_bytes_done_base = 0;
                    }
                    send_event(WorkerEvent::QueueFinished {
                        message: msg_cancelled(lang).to_string(),
                    });
                    continue;
                }

                if scan_skipped {
                    let queue_is_empty = {
                        let mut q = lock_unpoison(&state_thread);
                        q.active_task = None;
                        q.completed_tasks_counter += 1;
                        q.queue_total_items = q.queue_total_items.saturating_sub(initial_items);
                        if q.pending_queue.is_empty() {
                            q.total_tasks_counter = 0;
                            q.completed_tasks_counter = 0;
                            q.queue_total_items = 0;
                            q.queue_items_done = 0;
                            q.queue_total_bytes = 0;
                            q.queue_bytes_done_base = 0;
                            true
                        } else {
                            let snapshot = q.snapshot(is_paused_thread.load(Ordering::Relaxed));
                            send_event(WorkerEvent::QueueUpdated(snapshot));
                            false
                        }
                    };
                    skip_current_thread.store(false, Ordering::SeqCst);
                    send_event(WorkerEvent::TaskFinished {
                        task_id: current_task.id,
                        message: msg_skipped_task(lang, &current_task.title),
                    });
                    if queue_is_empty {
                        send_event(WorkerEvent::QueueFinished {
                            message: msg_queue_finished(lang).to_string(),
                        });
                        let q = lock_unpoison(&state_thread);
                        let snapshot = q.snapshot(false);
                        send_event(WorkerEvent::QueueUpdated(snapshot));
                    }
                    continue;
                }

                {
                    let mut q = lock_unpoison(&state_thread);
                    current_task.total_items = actual_items;
                    current_task.total_bytes = actual_bytes;
                    q.queue_total_items = q.queue_total_items.saturating_sub(initial_items) + actual_items;
                    q.queue_total_bytes = q.queue_total_bytes.saturating_add(actual_bytes);
                    q.active_task = Some(current_task.clone());
                    let snapshot = q.snapshot(is_paused_thread.load(Ordering::Relaxed));
                    send_event(WorkerEvent::QueueUpdated(snapshot));
                }

                // 3. Notify task started
                send_event(WorkerEvent::TaskStarted {
                    task_id: current_task.id,
                    title: current_task.title.clone(),
                    total_items: current_task.total_items,
                    total_bytes: current_task.total_bytes,
                });

                // 4. Progress tracker referencing shared state
                let mut tracker = QueueProgressTracker::new(
                    current_task.id,
                    current_task.total_items,
                    current_task.total_bytes,
                    Arc::clone(&state_thread),
                );

                // 5. Execute the task
                let runtime_ctx = WorkerRuntimeContext {
                    is_paused: &is_paused_thread,
                    skip_current: &skip_current_thread,
                    cancel_all: &cancel_all_thread,
                    wake_condvar: &condvar_thread,
                    state: &state_thread,
                    repaint_signal: &repaint_sig_thread,
                    lang,
                };

                let res = execute_task(
                    &current_task,
                    &mut tracker,
                    &event_tx_thread,
                    &runtime_ctx,
                );

                let is_skip = skip_current_thread.load(Ordering::Relaxed)
                    || res.as_ref().err().map(|e| e == "SKIP").unwrap_or(false);
                let is_cancel = cancel_all_thread.load(Ordering::Relaxed)
                    || res.as_ref().err().map(|e| e == "CANCEL").unwrap_or(false);

                // 6. Update state counters
                let queue_is_empty = {
                    let mut q = lock_unpoison(&state_thread);
                    q.active_task = None;
                    q.completed_tasks_counter += 1;

                    if is_skip || is_cancel {
                        let uncopied_bytes = current_task.total_bytes.saturating_sub(tracker.task_bytes_done);
                        let uncopied_items = current_task.total_items.saturating_sub(tracker.task_items_done);
                        q.queue_total_bytes = q.queue_total_bytes.saturating_sub(uncopied_bytes);
                        q.queue_total_items = q.queue_total_items.saturating_sub(uncopied_items);
                    }

                    q.queue_items_done += tracker.task_items_done;
                    q.queue_bytes_done_base += tracker.task_bytes_done;

                    if q.pending_queue.is_empty() {
                        q.total_tasks_counter = 0;
                        q.completed_tasks_counter = 0;
                        q.queue_total_items = 0;
                        q.queue_items_done = 0;
                        q.queue_total_bytes = 0;
                        q.queue_bytes_done_base = 0;
                        true
                    } else {
                        let snapshot = q.snapshot(is_paused_thread.load(Ordering::Relaxed));
                        send_event(WorkerEvent::QueueUpdated(snapshot));
                        false
                    }
                };

                skip_current_thread.store(false, Ordering::SeqCst);

                // 7. Notify completion / skip / cancel
                if is_cancel {
                    send_event(WorkerEvent::QueueFinished {
                        message: msg_cancelled(lang).to_string(),
                    });
                } else if is_skip {
                    send_event(WorkerEvent::TaskFinished {
                        task_id: current_task.id,
                        message: msg_skipped_task(lang, &current_task.title),
                    });
                } else {
                    match res {
                        Ok(msg) => {
                            send_event(WorkerEvent::TaskFinished {
                                task_id: current_task.id,
                                message: msg,
                            });
                        }
                        Err(e) => {
                            send_event(WorkerEvent::Error {
                                task_id: current_task.id,
                                message: e,
                            });
                        }
                    }
                }

                if queue_is_empty && !is_cancel {
                    send_event(WorkerEvent::QueueFinished {
                        message: msg_queue_finished(lang).to_string(),
                    });
                    let q = lock_unpoison(&state_thread);
                    let snapshot = q.snapshot(false);
                    send_event(WorkerEvent::QueueUpdated(snapshot));
                }
            }
        });

        let worker = Self {
            control_tx: control_tx.clone(),
            cmd_tx: cmd_tx.clone(),
            event_rx,
            is_paused,
            skip_current_flag,
            cancel_all_flag,
            state,
            wake_condvar,
            event_tx,
            repaint_signal,
            language,
        };

        // Forward compatibility thread for control_tx
        let worker_ref_ctrl = worker.clone();
        thread::spawn(move || {
            while let Ok(ctrl) = control_rx.recv() {
                match ctrl {
                    WorkerControl::Enqueue(cmd) => worker_ref_ctrl.enqueue(cmd),
                    WorkerControl::Pause => worker_ref_ctrl.pause(),
                    WorkerControl::Resume => worker_ref_ctrl.resume(),
                    WorkerControl::TogglePause => worker_ref_ctrl.toggle_pause(),
                    WorkerControl::SkipCurrent => worker_ref_ctrl.skip_current(),
                    WorkerControl::CancelAll => worker_ref_ctrl.cancel_all(),
                }
            }
        });

        // Forward compatibility thread for cmd_tx
        let worker_ref_cmd = worker.clone();
        thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                worker_ref_cmd.enqueue(cmd);
            }
        });

        worker
    }

    pub fn enqueue(&self, cmd: WorkerCommand) {
        let items = initial_command_items(&cmd);
        let lang = *lock_unpoison(&self.language);
        let (title, detail) = describe_command(&cmd, lang);

        // Reset cancellation flags if a new task is added
        self.cancel_all_flag.store(false, Ordering::SeqCst);
        self.skip_current_flag.store(false, Ordering::SeqCst);

        let snapshot = {
            let mut q = lock_unpoison(&self.state);
            if q.task_id_gen == 0 {
                q.task_id_gen = 1;
            }
            let task = TaskItem {
                id: q.task_id_gen,
                title,
                detail,
                command: cmd,
                total_items: items,
                total_bytes: 0,
            };
            q.task_id_gen += 1;
            q.total_tasks_counter += 1;
            q.queue_total_items += items;
            q.pending_queue.push_back(task);

            q.snapshot(self.is_paused.load(Ordering::Relaxed))
        };

        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
        if let Ok(guard) = self.repaint_signal.lock() {
            if let Some(ref cb) = *guard {
                cb();
            }
        }
        self.wake_condvar.notify_all();
    }

    pub fn set_language(&self, lang: Language) {
        *lock_unpoison(&self.language) = lang;
    }

    pub fn is_busy(&self) -> bool {
        let q = lock_unpoison(&self.state);
        q.active_task.is_some() || !q.pending_queue.is_empty()
    }

    pub fn set_repaint_signal<F: Fn() + Send + Sync + 'static>(&self, callback: F) {
        let mut sig = lock_unpoison(&self.repaint_signal);
        *sig = Some(Arc::new(callback));
    }

    #[allow(dead_code)]
    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
        let snapshot = {
            let q = lock_unpoison(&self.state);
            q.snapshot(true)
        };
        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
    }

    #[allow(dead_code)]
    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
        let snapshot = {
            let q = lock_unpoison(&self.state);
            q.snapshot(false)
        };
        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
        self.wake_condvar.notify_all();
    }

    pub fn toggle_pause(&self) {
        let old = self.is_paused.fetch_xor(true, Ordering::SeqCst);
        let new_val = !old;
        let snapshot = {
            let q = lock_unpoison(&self.state);
            q.snapshot(new_val)
        };
        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
        if !new_val {
            self.wake_condvar.notify_all();
        }
    }

    pub fn skip_current(&self) {
        self.skip_current_flag.store(true, Ordering::SeqCst);
        self.is_paused.store(false, Ordering::SeqCst);
        self.wake_condvar.notify_all();
    }

    pub fn cancel_all(&self) {
        self.cancel_all_flag.store(true, Ordering::SeqCst);
        self.is_paused.store(false, Ordering::SeqCst);
        let snapshot = {
            let mut q = lock_unpoison(&self.state);
            q.pending_queue.clear();
            q.active_task = None;
            q.total_tasks_counter = 0;
            q.completed_tasks_counter = 0;
            q.queue_total_items = 0;
            q.queue_items_done = 0;
            q.queue_total_bytes = 0;
            q.queue_bytes_done_base = 0;
            q.snapshot(false)
        };
        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
        self.wake_condvar.notify_all();
    }

    pub fn cancel(&self) {
        self.cancel_all();
    }
}

impl Default for IoWorker {
    fn default() -> Self {
        Self::new()
    }
}

fn msg_cancelled(lang: Language) -> &'static str {
    match lang {
        Language::Pl => "Operacje anulowane przez użytkownika",
        Language::En => "Operations cancelled by user",
    }
}

fn msg_queue_finished(lang: Language) -> &'static str {
    match lang {
        Language::Pl => "Wszystkie zadania w kolejce zostały zakończone",
        Language::En => "All queued tasks completed",
    }
}

fn msg_skipped_task(lang: Language, title: &str) -> String {
    match lang {
        Language::Pl => format!("Pominięto zadanie: {}", title),
        Language::En => format!("Skipped task: {}", title),
    }
}

fn initial_command_items(cmd: &WorkerCommand) -> usize {
    match cmd {
        WorkerCommand::Copy { sources, .. }
        | WorkerCommand::Move { sources, .. }
        | WorkerCommand::CreateZip { sources, .. } => sources.len().max(1),
        WorkerCommand::Delete { targets, .. } => targets.len().max(1),
        WorkerCommand::BatchRename { pairs } => pairs.len().max(1),
        WorkerCommand::CreateDir { .. }
        | WorkerCommand::ExtractZip { .. }
        | WorkerCommand::Rename { .. } => 1,
    }
}

fn describe_command(cmd: &WorkerCommand, lang: Language) -> (String, String) {
    let is_pl = lang == Language::Pl;
    match cmd {
        WorkerCommand::Copy {
            sources,
            target_dir,
        } => {
            let title = if sources.len() == 1 {
                let name = sources[0]
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| if is_pl { "plik" } else { "file" }.to_string());
                if is_pl {
                    format!("Kopiowanie '{}'", name)
                } else {
                    format!("Copying '{}'", name)
                }
            } else if is_pl {
                format!("Kopiowanie {} elementów", sources.len())
            } else {
                format!("Copying {} items", sources.len())
            };
            let detail = if is_pl {
                format!("do {}", target_dir.display())
            } else {
                format!("to {}", target_dir.display())
            };
            (title, detail)
        }
        WorkerCommand::Move {
            sources,
            target_dir,
        } => {
            let title = if sources.len() == 1 {
                let name = sources[0]
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| if is_pl { "plik" } else { "file" }.to_string());
                if is_pl {
                    format!("Przenoszenie '{}'", name)
                } else {
                    format!("Moving '{}'", name)
                }
            } else if is_pl {
                format!("Przenoszenie {} elementów", sources.len())
            } else {
                format!("Moving {} items", sources.len())
            };
            let detail = if is_pl {
                format!("do {}", target_dir.display())
            } else {
                format!("to {}", target_dir.display())
            };
            (title, detail)
        }
        WorkerCommand::Delete { targets, use_trash } => {
            let title = if *use_trash {
                if is_pl {
                    format!("Kosz: {} el.", targets.len())
                } else {
                    format!("Recycle Bin: {} item(s)", targets.len())
                }
            } else if is_pl {
                format!("Usuwanie: {} el.", targets.len())
            } else {
                format!("Deleting: {} item(s)", targets.len())
            };
            let detail = if targets.len() == 1 {
                targets[0].to_string_lossy().to_string()
            } else if is_pl {
                format!("{} wybranych ścieżek", targets.len())
            } else {
                format!("{} selected paths", targets.len())
            };
            (title, detail)
        }
        WorkerCommand::CreateDir { path } => {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| if is_pl { "katalog" } else { "folder" }.to_string());
            let title = if is_pl {
                format!("Tworzenie katalogu '{}'", name)
            } else {
                format!("Creating folder '{}'", name)
            };
            (title, path.display().to_string())
        }
        WorkerCommand::ExtractZip {
            archive_path,
            target_dir,
            ..
        } => {
            let name = archive_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "archive.zip".to_string());
            let title = if is_pl {
                format!("Rozpakowywanie '{}'", name)
            } else {
                format!("Extracting '{}'", name)
            };
            let detail = if is_pl {
                format!("do {}", target_dir.display())
            } else {
                format!("to {}", target_dir.display())
            };
            (title, detail)
        }
        WorkerCommand::Rename { source, new_name } => {
            let old_name = source
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| if is_pl { "plik" } else { "file" }.to_string());
            let title = if is_pl {
                format!("Zmiana nazwy '{}'", old_name)
            } else {
                format!("Renaming '{}'", old_name)
            };
            let detail = if is_pl {
                format!("na '{}'", new_name)
            } else {
                format!("to '{}'", new_name)
            };
            (title, detail)
        }
        WorkerCommand::BatchRename { pairs } => {
            let title = if is_pl {
                format!("Wielokrotna zmiana nazw ({} el.)", pairs.len())
            } else {
                format!("Multi-rename ({} items)", pairs.len())
            };
            let detail = if is_pl {
                format!("{} plików/katalogów", pairs.len())
            } else {
                format!("{} files/folders", pairs.len())
            };
            (title, detail)
        }
        WorkerCommand::CreateZip {
            sources,
            archive_path,
        } => {
            let name = archive_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "archive.zip".to_string());
            let title = if sources.len() == 1 {
                if is_pl {
                    format!("Pakowanie '{}' do ZIP", name)
                } else {
                    format!("Packing '{}' to ZIP", name)
                }
            } else if is_pl {
                format!("Pakowanie {} elementów do ZIP", sources.len())
            } else {
                format!("Packing {} items to ZIP", sources.len())
            };
            let detail = if is_pl {
                format!("do {}", archive_path.display())
            } else {
                format!("to {}", archive_path.display())
            };
            (title, detail)
        }
    }
}

fn calculate_command_totals_cancellable(
    cmd: &WorkerCommand,
    skip_current: &Arc<AtomicBool>,
    cancel_all: &Arc<AtomicBool>,
) -> (usize, u64) {
    match cmd {
        WorkerCommand::Copy { sources, .. } | WorkerCommand::Move { sources, .. } => {
            calculate_sources_totals(sources, skip_current, cancel_all)
        }
        WorkerCommand::Delete { targets, .. } => (targets.len(), 0),
        WorkerCommand::CreateDir { .. } => (1, 0),
        WorkerCommand::ExtractZip { archive_path, .. } => {
            let size = archive_path.metadata().map(|m| m.len()).unwrap_or(0);
            (1, size)
        }
        WorkerCommand::Rename { .. } => (1, 0),
        WorkerCommand::BatchRename { pairs } => (pairs.len(), 0),
        WorkerCommand::CreateZip { sources, .. } => {
            calculate_sources_totals(sources, skip_current, cancel_all)
        }
    }
}

fn calculate_sources_totals(
    sources: &[PathBuf],
    skip_current: &Arc<AtomicBool>,
    cancel_all: &Arc<AtomicBool>,
) -> (usize, u64) {
    let mut total_files = 0;
    let mut total_bytes = 0;

    for src in sources {
        if skip_current.load(Ordering::Relaxed) || cancel_all.load(Ordering::Relaxed) {
            break;
        }
        let Ok(sym_meta) = fs::symlink_metadata(src) else {
            continue;
        };
        if crate::fs_item::is_symlink_or_reparse(&sym_meta) {
            if !src.is_dir() {
                total_files += 1;
                if let Ok(m) = src.metadata() {
                    total_bytes += m.len();
                }
            }
            continue;
        }
        if sym_meta.is_file() {
            total_files += 1;
            total_bytes += sym_meta.len();
        } else if sym_meta.is_dir() {
            for entry in walkdir::WalkDir::new(src)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if skip_current.load(Ordering::Relaxed) || cancel_all.load(Ordering::Relaxed) {
                    break;
                }
                if entry.file_type().is_file() {
                    total_files += 1;
                    total_bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
    }

    (total_files.max(sources.len()), total_bytes)
}

struct QueueProgressTracker {
    pub task_id: u64,
    pub task_total_items: usize,
    pub task_items_done: usize,
    pub task_total_bytes: u64,
    pub task_bytes_done: u64,
    pub state: Arc<Mutex<WorkerQueueState>>,
    pub last_event_time: Instant,
    pub last_queue_bytes_done: u64,
    pub speed_bps: f32,
}

impl QueueProgressTracker {
    pub fn new(
        task_id: u64,
        task_total_items: usize,
        task_total_bytes: u64,
        state: Arc<Mutex<WorkerQueueState>>,
    ) -> Self {
        let (queue_bytes_base, _) = {
            let q = lock_unpoison(&state);
            (q.queue_bytes_done_base, q.queue_total_bytes)
        };
        let now = Instant::now();
        Self {
            task_id,
            task_total_items,
            task_items_done: 0,
            task_total_bytes,
            task_bytes_done: 0,
            state,
            last_event_time: now,
            last_queue_bytes_done: queue_bytes_base,
            speed_bps: 0.0,
        }
    }

    pub fn update(
        &mut self,
        current_name: &str,
        current_file_bytes: u64,
        current_file_size: u64,
        event_tx: &Sender<WorkerEvent>,
    ) {
        let now = Instant::now();
        let elapsed_since_last = now.duration_since(self.last_event_time).as_secs_f32();

        let (queue_bytes_base, queue_total_bytes, queue_items_base, queue_total_items) = {
            let q = lock_unpoison(&self.state);
            (
                q.queue_bytes_done_base,
                q.queue_total_bytes,
                q.queue_items_done,
                q.queue_total_items,
            )
        };
        let current_queue_bytes_done = queue_bytes_base + self.task_bytes_done;

        if elapsed_since_last >= 0.05
            || (self.task_total_bytes > 0 && self.task_bytes_done >= self.task_total_bytes)
        {
            if elapsed_since_last > 0.0 {
                let bytes_diff =
                    current_queue_bytes_done.saturating_sub(self.last_queue_bytes_done);
                let instant_speed = bytes_diff as f32 / elapsed_since_last;
                self.speed_bps = if self.speed_bps == 0.0 {
                    instant_speed
                } else {
                    self.speed_bps * 0.7 + instant_speed * 0.3
                };
            }

            self.last_event_time = now;
            self.last_queue_bytes_done = current_queue_bytes_done;

            let remaining_queue_bytes =
                queue_total_bytes.saturating_sub(current_queue_bytes_done);
            let eta_secs = if self.speed_bps > 1024.0 {
                Some((remaining_queue_bytes as f32 / self.speed_bps) as u64)
            } else {
                None
            };

            let _ = event_tx.send(WorkerEvent::Progress {
                task_id: self.task_id,
                current_name: current_name.to_string(),
                task_items_done: self.task_items_done,
                task_total_items: self.task_total_items,
                task_bytes_done: self.task_bytes_done,
                task_total_bytes: self.task_total_bytes,
                current_file_bytes,
                current_file_size,
                queue_items_done: queue_items_base + self.task_items_done,
                queue_total_items,
                queue_bytes_done: current_queue_bytes_done,
                queue_total_bytes,
                speed_bps: self.speed_bps,
                eta_secs,
            });
        }
    }
}

pub struct WorkerRuntimeContext<'a> {
    pub is_paused: &'a Arc<AtomicBool>,
    pub skip_current: &'a Arc<AtomicBool>,
    pub cancel_all: &'a Arc<AtomicBool>,
    pub wake_condvar: &'a Arc<Condvar>,
    pub state: &'a Arc<Mutex<WorkerQueueState>>,
    pub repaint_signal: &'a RepaintSignal,
    pub lang: Language,
}

pub fn normalize_path(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        let s = c.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{}", stripped));
        }
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
        return c;
    }
    p.to_path_buf()
}

pub fn is_same_file_or_dir(p1: &Path, p2: &Path) -> bool {
    if p1 == p2 {
        return true;
    }
    let n1 = normalize_path(p1);
    let n2 = normalize_path(p2);
    n1 == n2
}

pub fn generate_non_colliding_copy_name(dst: &Path) -> PathBuf {
    generate_non_colliding_copy_name_for_lang(dst, Language::Pl)
}

pub fn generate_non_colliding_copy_name_for_lang(dst: &Path, lang: Language) -> PathBuf {
    if !dst.exists() {
        return dst.to_path_buf();
    }
    let parent = dst.parent().unwrap_or_else(|| Path::new(""));
    let stem = dst.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = dst.extension().map(|e| e.to_string_lossy().to_string());
    let copy_word = match lang {
        Language::Pl => "Kopia",
        Language::En => "Copy",
    };

    let mut counter = 1;
    loop {
        let new_file_name = if counter == 1 {
            match &ext {
                Some(e) if !e.is_empty() => format!("{} - {}.{}", stem, copy_word, e),
                _ => format!("{} - {}", stem, copy_word),
            }
        } else {
            match &ext {
                Some(e) if !e.is_empty() => format!("{} - {} ({}).{}", stem, copy_word, counter, e),
                _ => format!("{} - {} ({})", stem, copy_word, counter),
            }
        };
        let candidate = parent.join(new_file_name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn check_pause_and_cancellation(ctx: &WorkerRuntimeContext) -> Result<(), &'static str> {
    if ctx.cancel_all.load(Ordering::Relaxed) {
        return Err("CANCEL");
    }
    if ctx.skip_current.load(Ordering::Relaxed) {
        return Err("SKIP");
    }
    if ctx.is_paused.load(Ordering::Relaxed) {
        let mut q = lock_unpoison(ctx.state);
        while ctx.is_paused.load(Ordering::Relaxed) {
            if ctx.cancel_all.load(Ordering::Relaxed) {
                return Err("CANCEL");
            }
            if ctx.skip_current.load(Ordering::Relaxed) {
                return Err("SKIP");
            }
            q = ctx
                .wake_condvar
                .wait_timeout(q, Duration::from_millis(200))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConflictPolicy {
    Ask,
    OverwriteAll,
    OverwriteOlder,
    SkipAll,
}

enum ConflictOutcome {
    WriteTo(PathBuf),
    Skip,
}

fn build_conflict_info(path: &Path) -> ConflictFileInfo {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let meta = fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = meta.and_then(|m| m.modified().ok());
    ConflictFileInfo {
        path: path.to_path_buf(),
        name,
        size,
        modified,
    }
}

fn is_source_newer(src: &Path, dst: &Path) -> bool {
    let src_m = fs::metadata(src).ok().and_then(|m| m.modified().ok());
    let dst_m = fs::metadata(dst).ok().and_then(|m| m.modified().ok());
    match (src_m, dst_m) {
        (Some(s), Some(d)) => s > d,
        (Some(_), None) => true,
        _ => false,
    }
}

fn resolve_file_conflict(
    task_id: u64,
    src: &Path,
    dst: &Path,
    policy: &mut ConflictPolicy,
    event_tx: &Sender<WorkerEvent>,
    ctx: &WorkerRuntimeContext,
) -> Result<ConflictOutcome, String> {
    let mut current_dst = dst.to_path_buf();

    while current_dst.exists() {
        check_pause_and_cancellation(ctx).map_err(|e| e.to_string())?;
        if is_same_file_or_dir(src, &current_dst) {
            return Ok(ConflictOutcome::Skip);
        }

        match *policy {
            ConflictPolicy::OverwriteAll => return Ok(ConflictOutcome::WriteTo(current_dst)),
            ConflictPolicy::SkipAll => return Ok(ConflictOutcome::Skip),
            ConflictPolicy::OverwriteOlder => {
                if is_source_newer(src, &current_dst) {
                    return Ok(ConflictOutcome::WriteTo(current_dst));
                } else {
                    return Ok(ConflictOutcome::Skip);
                }
            }
            ConflictPolicy::Ask => {
                let (reply_tx, reply_rx) = crossbeam_channel::bounded::<ConflictResolution>(1);
                let _ = event_tx.send(WorkerEvent::ConflictDetected {
                    task_id,
                    source: build_conflict_info(src),
                    destination: build_conflict_info(&current_dst),
                    reply_tx,
                });
                if let Ok(guard) = ctx.repaint_signal.lock() {
                    if let Some(ref cb) = *guard {
                        cb();
                    }
                }

                let resolution = loop {
                    if ctx.cancel_all.load(Ordering::Relaxed) {
                        return Err("CANCEL".to_string());
                    }
                    if ctx.skip_current.load(Ordering::Relaxed) {
                        return Err("SKIP".to_string());
                    }
                    match reply_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(res) => break res,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            return Err("CANCEL".to_string());
                        }
                    }
                };

                match resolution {
                    ConflictResolution::Overwrite => {
                        return Ok(ConflictOutcome::WriteTo(current_dst));
                    }
                    ConflictResolution::OverwriteAll => {
                        *policy = ConflictPolicy::OverwriteAll;
                        return Ok(ConflictOutcome::WriteTo(current_dst));
                    }
                    ConflictResolution::OverwriteOlder => {
                        *policy = ConflictPolicy::OverwriteOlder;
                        if is_source_newer(src, &current_dst) {
                            return Ok(ConflictOutcome::WriteTo(current_dst));
                        } else {
                            return Ok(ConflictOutcome::Skip);
                        }
                    }
                    ConflictResolution::Skip => return Ok(ConflictOutcome::Skip),
                    ConflictResolution::SkipAll => {
                        *policy = ConflictPolicy::SkipAll;
                        return Ok(ConflictOutcome::Skip);
                    }
                    ConflictResolution::Rename(new_name) => {
                        let clean = Path::new(new_name.trim())
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if clean.is_empty() || clean == "." || clean == ".." {
                            continue;
                        }
                        let parent = current_dst.parent().unwrap_or_else(|| Path::new(""));
                        current_dst = parent.join(clean);
                    }
                    ConflictResolution::Cancel => return Err("CANCEL".to_string()),
                }
            }
        }
    }

    Ok(ConflictOutcome::WriteTo(current_dst))
}

fn execute_task(
    task: &TaskItem,
    tracker: &mut QueueProgressTracker,
    event_tx: &Sender<WorkerEvent>,
    ctx: &WorkerRuntimeContext,
) -> Result<String, String> {
    let is_pl = ctx.lang == Language::Pl;
    let mut conflict_policy = ConflictPolicy::Ask;

    match &task.command {
        WorkerCommand::Copy {
            sources,
            target_dir,
        } => {
            let mut had_skipped = false;
            for src in sources {
                check_pause_and_cancellation(ctx)
                    .map_err(|e| e.to_string())?;
                let file_name = src
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| if is_pl { "plik" } else { "file" }.to_string());
                let mut dest = target_dir.join(&file_name);

                // Zabezpieczenie przed kopiowaniem katalogu do jego własnego podkatalogu
                if src.is_dir() {
                    let src_norm = normalize_path(src);
                    let target_norm = normalize_path(target_dir);
                    if target_norm.starts_with(&src_norm) {
                        return Err(if is_pl {
                            format!(
                                "Nie można skopiować folderu '{}' do jego własnego podkatalogu",
                                file_name
                            )
                        } else {
                            format!(
                                "Cannot copy folder '{}' into its own subfolder",
                                file_name
                            )
                        });
                    }
                }

                // Zabezpieczenie przed samonadpisaniem (src == dest)
                if is_same_file_or_dir(src, &dest) {
                    dest = generate_non_colliding_copy_name_for_lang(&dest, ctx.lang);
                }

                if let Err(e) = copy_recursive(
                    task.id,
                    src,
                    &dest,
                    tracker,
                    event_tx,
                    ctx,
                    &mut conflict_policy,
                    &mut had_skipped,
                ) {
                    if e == "SKIP" || e == "CANCEL" {
                        return Err(e);
                    }
                    return Err(if is_pl {
                        format!("Błąd kopiowania {}: {}", file_name, e)
                    } else {
                        format!("Error copying {}: {}", file_name, e)
                    });
                }
            }
            Ok(if is_pl {
                format!("Skopiowano pomyślnie {} elementów", tracker.task_items_done)
            } else {
                format!("Successfully copied {} item(s)", tracker.task_items_done)
            })
        }

        WorkerCommand::Move {
            sources,
            target_dir,
        } => {
            for src in sources {
                check_pause_and_cancellation(ctx)
                    .map_err(|e| e.to_string())?;
                let file_name = src
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| if is_pl { "plik" } else { "file" }.to_string());
                let dest = target_dir.join(&file_name);

                // Zabezpieczenie przed przenoszeniem na ten sam plik (no-op)
                if is_same_file_or_dir(src, &dest) {
                    tracker.task_items_done += 1;
                    continue;
                }

                // Zabezpieczenie przed przenoszeniem katalogu do jego własnego podkatalogu
                if src.is_dir() {
                    let src_norm = normalize_path(src);
                    let target_norm = normalize_path(target_dir);
                    if target_norm.starts_with(&src_norm) {
                        return Err(if is_pl {
                            format!(
                                "Nie można przenieść folderu '{}' do jego własnego podkatalogu",
                                file_name
                            )
                        } else {
                            format!(
                                "Cannot move folder '{}' into its own subfolder",
                                file_name
                            )
                        });
                    }
                }

                let mut had_skipped = false;
                let resolved_dest = if !src.is_dir() && dest.exists() {
                    match resolve_file_conflict(
                        task.id,
                        src,
                        &dest,
                        &mut conflict_policy,
                        event_tx,
                        ctx,
                    )? {
                        ConflictOutcome::WriteTo(p) => p,
                        ConflictOutcome::Skip => {
                            let size = src.metadata().map(|m| m.len()).unwrap_or(0);
                            tracker.task_items_done += 1;
                            tracker.task_bytes_done += size;
                            continue;
                        }
                    }
                } else {
                    dest
                };

                if !resolved_dest.exists() && fs::rename(src, &resolved_dest).is_ok() {
                    let size = resolved_dest.metadata().map(|m| m.len()).unwrap_or(0);
                    tracker.task_items_done += 1;
                    tracker.task_bytes_done += size;
                    tracker.update(&file_name, size, size, event_tx);
                    continue;
                }

                if src.is_dir() {
                    if let Err(e) = copy_recursive(
                        task.id,
                        src,
                        &resolved_dest,
                        tracker,
                        event_tx,
                        ctx,
                        &mut conflict_policy,
                        &mut had_skipped,
                    ) {
                        if e == "SKIP" || e == "CANCEL" {
                            return Err(e);
                        }
                        return Err(if is_pl {
                            format!("Błąd przenoszenia {}: {}", file_name, e)
                        } else {
                            format!("Error moving {}: {}", file_name, e)
                        });
                    }
                } else if let Err(e) = copy_single_file(src, &resolved_dest, tracker, event_tx, ctx) {
                    if e == "SKIP" || e == "CANCEL" {
                        return Err(e);
                    }
                    return Err(if is_pl {
                        format!("Błąd przenoszenia {}: {}", file_name, e)
                    } else {
                        format!("Error moving {}: {}", file_name, e)
                    });
                }

                if ctx.skip_current.load(Ordering::Relaxed) {
                    return Err("SKIP".to_string());
                }
                if ctx.cancel_all.load(Ordering::Relaxed) {
                    return Err("CANCEL".to_string());
                }

                if !had_skipped {
                    if src.is_dir() {
                        fs::remove_dir_all(src).map_err(|e| {
                            if is_pl {
                                format!("Błąd usuwania źródła {}: {}", file_name, e)
                            } else {
                                format!("Error removing source {}: {}", file_name, e)
                            }
                        })?;
                    } else {
                        fs::remove_file(src).map_err(|e| {
                            if is_pl {
                                format!("Błąd usuwania źródła {}: {}", file_name, e)
                            } else {
                                format!("Error removing source {}: {}", file_name, e)
                            }
                        })?;
                    }
                }
            }
            Ok(if is_pl {
                format!("Przeniesiono pomyślnie {} elementów", tracker.task_items_done)
            } else {
                format!("Successfully moved {} item(s)", tracker.task_items_done)
            })
        }

        WorkerCommand::Delete { targets, use_trash } => {
            for target in targets {
                check_pause_and_cancellation(ctx)
                    .map_err(|e| e.to_string())?;
                let file_name = target
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| if is_pl { "element" } else { "item" }.to_string());

                tracker.update(&file_name, 0, 0, event_tx);

                let res = if *use_trash {
                    trash::delete(target).map_err(|e| e.to_string())
                } else {
                    let sym_is_dir = fs::symlink_metadata(target)
                        .map(|m| m.is_dir())
                        .unwrap_or(false);
                    if sym_is_dir {
                        fs::remove_dir_all(target).map_err(|e| e.to_string())
                    } else {
                        fs::remove_file(target).map_err(|e| e.to_string())
                    }
                };

                if let Err(e) = res {
                    return Err(if is_pl {
                        format!("Błąd usuwania {}: {}", file_name, e)
                    } else {
                        format!("Error deleting {}: {}", file_name, e)
                    });
                }

                tracker.task_items_done += 1;
            }
            Ok(if is_pl {
                format!("Usunięto pomyślnie {} elementów", tracker.task_items_done)
            } else {
                format!("Successfully deleted {} item(s)", tracker.task_items_done)
            })
        }

        WorkerCommand::CreateDir { path } => {
            fs::create_dir_all(path).map_err(|e| {
                if is_pl {
                    format!("Błąd tworzenia katalogu: {}", e)
                } else {
                    format!("Error creating folder: {}", e)
                }
            })?;
            tracker.task_items_done += 1;
            Ok(if is_pl {
                "Katalog został utworzony".to_string()
            } else {
                "Folder created".to_string()
            })
        }

        WorkerCommand::ExtractZip {
            archive_path,
            target_dir,
            filter_prefix,
            selected_entries,
        } => {
            let cancel_arc = Arc::new(AtomicBool::new(false));
            let cancel_check = Arc::clone(&cancel_arc);
            let skip_ref = Arc::clone(ctx.skip_current);
            let cancel_ref = Arc::clone(ctx.cancel_all);

            let selected_opt = if selected_entries.is_empty() {
                None
            } else {
                Some(selected_entries.as_slice())
            };

            let res = crate::archive::extract_archive_entries(
                archive_path,
                target_dir,
                filter_prefix.as_deref(),
                selected_opt,
                &cancel_arc,
                |rel_name, written, total| {
                    if skip_ref.load(Ordering::Relaxed) || cancel_ref.load(Ordering::Relaxed) {
                        cancel_check.store(true, Ordering::Relaxed);
                    }
                    tracker.update(rel_name, written, total, event_tx);
                },
            );

            if ctx.skip_current.load(Ordering::Relaxed) {
                return Err("SKIP".to_string());
            }
            if ctx.cancel_all.load(Ordering::Relaxed) {
                return Err("CANCEL".to_string());
            }

            match res {
                Ok(count) => {
                    tracker.task_items_done += count;
                    Ok(if is_pl {
                        format!("Rozpakowano pomyślnie {} plików", count)
                    } else {
                        format!("Successfully extracted {} file(s)", count)
                    })
                }
                Err(e) => Err(e),
            }
        }

        WorkerCommand::Rename { source, new_name } => {
            let parent = source.parent().unwrap_or_else(|| Path::new(""));
            let dest = parent.join(new_name);

            if is_same_file_or_dir(source, &dest) {
                return Ok(if is_pl {
                    "Nazwa pliku nie uległa zmianie".to_string()
                } else {
                    "File name unchanged".to_string()
                });
            }

            if dest.exists() {
                return Err(if is_pl {
                    format!("Plik o nazwie '{}' już istnieje!", new_name)
                } else {
                    format!("A file named '{}' already exists!", new_name)
                });
            }

            fs::rename(source, &dest).map_err(|e| {
                if is_pl {
                    format!("Błąd zmiany nazwy: {}", e)
                } else {
                    format!("Error renaming: {}", e)
                }
            })?;
            tracker.task_items_done += 1;
            Ok(if is_pl {
                format!("Zmieniono nazwę na '{}'", new_name)
            } else {
                format!("Renamed to '{}'", new_name)
            })
        }

        WorkerCommand::BatchRename { pairs } => {
            let executed = crate::multi_rename::execute_batch_rename(pairs)?;
            tracker.task_items_done += executed.len();
            Ok(if is_pl {
                format!("Zmieniono nazwy {} elementów", executed.len())
            } else {
                format!("Renamed {} item(s)", executed.len())
            })
        }

        WorkerCommand::CreateZip {
            sources,
            archive_path,
        } => {
            let cancel_arc = Arc::new(AtomicBool::new(false));
            let cancel_check = Arc::clone(&cancel_arc);
            let skip_ref = Arc::clone(ctx.skip_current);
            let cancel_ref = Arc::clone(ctx.cancel_all);

            let res = crate::zip_support::create_zip_archive(
                sources,
                archive_path,
                &cancel_arc,
                |item_name, written, total| {
                    if skip_ref.load(Ordering::Relaxed) || cancel_ref.load(Ordering::Relaxed) {
                        cancel_check.store(true, Ordering::Relaxed);
                    }
                    tracker.update(item_name, written, total, event_tx);
                },
            );

            if ctx.skip_current.load(Ordering::Relaxed) {
                let _ = fs::remove_file(archive_path);
                return Err("SKIP".to_string());
            }
            if ctx.cancel_all.load(Ordering::Relaxed) {
                let _ = fs::remove_file(archive_path);
                return Err("CANCEL".to_string());
            }

            match res {
                Ok(count) => {
                    tracker.task_items_done += count;
                    Ok(if is_pl {
                        format!("Spakowano pomyślnie {} plików do ZIP", count)
                    } else {
                        format!("Successfully packed {} file(s) into ZIP", count)
                    })
                }
                Err(e) => {
                    let _ = fs::remove_file(archive_path);
                    Err(e)
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy_recursive(
    task_id: u64,
    src: &Path,
    dst: &Path,
    tracker: &mut QueueProgressTracker,
    event_tx: &Sender<WorkerEvent>,
    ctx: &WorkerRuntimeContext,
    conflict_policy: &mut ConflictPolicy,
    had_skipped: &mut bool,
) -> Result<(), String> {
    check_pause_and_cancellation(ctx).map_err(|e| e.to_string())?;

    let sym_meta = fs::symlink_metadata(src)
        .map_err(|e| format!("Błąd odczytu metadanych {}: {}", src.display(), e))?;
    if crate::fs_item::is_symlink_or_reparse(&sym_meta) && src.is_dir() {
        return Ok(());
    }

    if src.is_dir() {
        fs::create_dir_all(dst)
            .map_err(|e| format!("Błąd tworzenia katalogu {}: {}", dst.display(), e))?;
        for entry in fs::read_dir(src)
            .map_err(|e| format!("Błąd odczytu katalogu {}: {}", src.display(), e))?
        {
            check_pause_and_cancellation(ctx).map_err(|e| e.to_string())?;
            let entry = entry.map_err(|e| e.to_string())?;
            let child_src = entry.path();
            let child_dst = dst.join(entry.file_name());
            if child_src.is_dir() {
                let src_norm = normalize_path(&child_src);
                let dst_norm = normalize_path(&child_dst);
                if dst_norm.starts_with(&src_norm) {
                    continue;
                }
            }
            copy_recursive(
                task_id,
                &child_src,
                &child_dst,
                tracker,
                event_tx,
                ctx,
                conflict_policy,
                had_skipped,
            )?;
        }
    } else {
        if is_same_file_or_dir(src, dst) {
            return Ok(());
        }

        let final_dst = if dst.exists() {
            match resolve_file_conflict(task_id, src, dst, conflict_policy, event_tx, ctx)? {
                ConflictOutcome::WriteTo(p) => p,
                ConflictOutcome::Skip => {
                    *had_skipped = true;
                    let file_size = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
                    tracker.task_items_done += 1;
                    tracker.task_bytes_done += file_size;
                    return Ok(());
                }
            }
        } else {
            dst.to_path_buf()
        };

        copy_single_file(src, &final_dst, tracker, event_tx, ctx)?;
    }

    Ok(())
}

fn copy_single_file(
    src: &Path,
    dst: &Path,
    tracker: &mut QueueProgressTracker,
    event_tx: &Sender<WorkerEvent>,
    ctx: &WorkerRuntimeContext,
) -> Result<(), String> {
    if is_same_file_or_dir(src, dst) {
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Błąd tworzenia katalogu: {}", e))?;
    }

    let file_name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let mut reader = fs::File::open(src)
        .map_err(|e| format!("Błąd otwarcia pliku {}: {}", src.display(), e))?;
    let src_meta = reader.metadata().ok();
    let file_size = src_meta.as_ref().map(|m| m.len()).unwrap_or(0);

    let mut writer = fs::File::create(dst)
        .map_err(|e| format!("Błąd zapisu do pliku {}: {}", dst.display(), e))?;

    let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer
    let mut file_copied = 0u64;

    loop {
        if let Err(reason) = check_pause_and_cancellation(ctx) {
            drop(writer);
            let _ = fs::remove_file(dst);
            return Err(reason.to_string());
        }

        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Błąd odczytu {}: {}", src.display(), e))?;
        if bytes_read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..bytes_read])
            .map_err(|e| format!("Błąd zapisu {}: {}", dst.display(), e))?;
        file_copied += bytes_read as u64;
        tracker.task_bytes_done += bytes_read as u64;

        tracker.update(&file_name, file_copied, file_size, event_tx);
    }

    if let Some(ref meta) = src_meta {
        if let Ok(mtime) = meta.modified() {
            let _ = writer.set_modified(mtime);
        }
    }
    drop(writer);
    if let Some(meta) = src_meta {
        let _ = fs::set_permissions(dst, meta.permissions());
    }

    tracker.task_items_done += 1;
    tracker.update(&file_name, file_size, file_size, event_tx);

    Ok(())
}
