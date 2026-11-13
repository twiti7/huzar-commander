use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    },
    Rename {
        source: PathBuf,
        new_name: String,
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

        let is_paused_thread = Arc::clone(&is_paused);
        let skip_current_thread = Arc::clone(&skip_current_flag);
        let cancel_all_thread = Arc::clone(&cancel_all_flag);
        let state_thread = Arc::clone(&state);
        let condvar_thread = Arc::clone(&wake_condvar);
        let event_tx_thread = event_tx.clone();
        let repaint_sig_thread = Arc::clone(&repaint_signal);

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
                let current_task = {
                    let mut q = state_thread.lock().unwrap();
                    while q.pending_queue.is_empty() {
                        q.active_task = None;
                        if cancel_all_thread.load(Ordering::Relaxed) {
                            cancel_all_thread.store(false, Ordering::Relaxed);
                        }
                        q = condvar_thread.wait(q).unwrap();
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

                    q.active_task = Some(task.clone());
                    skip_current_thread.store(false, Ordering::SeqCst);

                    let snapshot = q.snapshot(is_paused_thread.load(Ordering::Relaxed));
                    send_event(WorkerEvent::QueueUpdated(snapshot));
                    task
                };

                // 2. Notify task started
                send_event(WorkerEvent::TaskStarted {
                    task_id: current_task.id,
                    title: current_task.title.clone(),
                    total_items: current_task.total_items,
                    total_bytes: current_task.total_bytes,
                });

                // 3. Progress tracker referencing shared state
                let mut tracker = QueueProgressTracker::new(
                    current_task.id,
                    current_task.total_items,
                    current_task.total_bytes,
                    Arc::clone(&state_thread),
                );

                // 4. Execute the task
                let runtime_ctx = WorkerRuntimeContext {
                    is_paused: &is_paused_thread,
                    skip_current: &skip_current_thread,
                    cancel_all: &cancel_all_thread,
                    wake_condvar: &condvar_thread,
                    state: &state_thread,
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

                // 5. Update state counters
                let queue_is_empty = {
                    let mut q = state_thread.lock().unwrap();
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

                // 6. Notify completion / skip / cancel
                if is_cancel {
                    send_event(WorkerEvent::QueueFinished {
                        message: "Operacje anulowane przez użytkownika".to_string(),
                    });
                } else if is_skip {
                    send_event(WorkerEvent::TaskFinished {
                        task_id: current_task.id,
                        message: format!("Pominięto zadanie: {}", current_task.title),
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
                        message: "Wszystkie zadania w kolejce zostały zakończone".to_string(),
                    });
                    let q = state_thread.lock().unwrap();
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
        let (items, bytes) = calculate_command_totals(&cmd);
        let (title, detail) = describe_command(&cmd);

        // Reset cancellation flags if a new task is added
        self.cancel_all_flag.store(false, Ordering::SeqCst);
        self.skip_current_flag.store(false, Ordering::SeqCst);

        let snapshot = {
            let mut q = self.state.lock().unwrap();
            if q.task_id_gen == 0 {
                q.task_id_gen = 1;
            }
            let task = TaskItem {
                id: q.task_id_gen,
                title,
                detail,
                command: cmd,
                total_items: items,
                total_bytes: bytes,
            };
            q.task_id_gen += 1;
            q.total_tasks_counter += 1;
            q.queue_total_items += items;
            q.queue_total_bytes += bytes;
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

    pub fn is_busy(&self) -> bool {
        let q = self.state.lock().unwrap();
        q.active_task.is_some() || !q.pending_queue.is_empty()
    }

    pub fn set_repaint_signal<F: Fn() + Send + Sync + 'static>(&self, callback: F) {
        let mut sig = self.repaint_signal.lock().unwrap();
        *sig = Some(Arc::new(callback));
    }

    #[allow(dead_code)]
    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
        let snapshot = {
            let q = self.state.lock().unwrap();
            q.snapshot(true)
        };
        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
    }

    #[allow(dead_code)]
    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
        let snapshot = {
            let q = self.state.lock().unwrap();
            q.snapshot(false)
        };
        let _ = self.event_tx.send(WorkerEvent::QueueUpdated(snapshot));
        self.wake_condvar.notify_all();
    }

    pub fn toggle_pause(&self) {
        let old = self.is_paused.fetch_xor(true, Ordering::SeqCst);
        let new_val = !old;
        let snapshot = {
            let q = self.state.lock().unwrap();
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
            let mut q = self.state.lock().unwrap();
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

fn describe_command(cmd: &WorkerCommand) -> (String, String) {
    match cmd {
        WorkerCommand::Copy {
            sources,
            target_dir,
        } => {
            let title = if sources.len() == 1 {
                let name = sources[0]
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "plik".to_string());
                format!("Kopiowanie '{}'", name)
            } else {
                format!("Kopiowanie {} elementów", sources.len())
            };
            let detail = format!("do {}", target_dir.display());
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
                    .unwrap_or_else(|| "plik".to_string());
                format!("Przenoszenie '{}'", name)
            } else {
                format!("Przenoszenie {} elementów", sources.len())
            };
            let detail = format!("do {}", target_dir.display());
            (title, detail)
        }
        WorkerCommand::Delete { targets, use_trash } => {
            let title = if *use_trash {
                format!("Kosz: {} el.", targets.len())
            } else {
                format!("Usuwanie: {} el.", targets.len())
            };
            let detail = if targets.len() == 1 {
                targets[0].to_string_lossy().to_string()
            } else {
                format!("{} wybranych ścieżek", targets.len())
            };
            (title, detail)
        }
        WorkerCommand::CreateDir { path } => {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "katalog".to_string());
            (format!("Tworzenie katalogu '{}'", name), path.display().to_string())
        }
        WorkerCommand::ExtractZip {
            archive_path,
            target_dir,
            ..
        } => {
            let name = archive_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "archiwum.zip".to_string());
            (format!("Rozpakowywanie '{}'", name), format!("do {}", target_dir.display()))
        }
        WorkerCommand::Rename { source, new_name } => {
            let old_name = source
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "plik".to_string());
            (format!("Zmiana nazwy '{}'", old_name), format!("na '{}'", new_name))
        }
        WorkerCommand::CreateZip {
            sources,
            archive_path,
        } => {
            let name = archive_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "archiwum.zip".to_string());
            let title = if sources.len() == 1 {
                format!("Pakowanie '{}' do ZIP", name)
            } else {
                format!("Pakowanie {} elementów do ZIP", sources.len())
            };
            (title, format!("do {}", archive_path.display()))
        }
    }
}

fn calculate_command_totals(cmd: &WorkerCommand) -> (usize, u64) {
    match cmd {
        WorkerCommand::Copy { sources, .. } | WorkerCommand::Move { sources, .. } => {
            calculate_sources_totals(sources)
        }
        WorkerCommand::Delete { targets, .. } => (targets.len(), 0),
        WorkerCommand::CreateDir { .. } => (1, 0),
        WorkerCommand::ExtractZip { archive_path, .. } => {
            let size = archive_path.metadata().map(|m| m.len()).unwrap_or(0);
            (1, size)
        }
        WorkerCommand::Rename { .. } => (1, 0),
        WorkerCommand::CreateZip { sources, .. } => calculate_sources_totals(sources),
    }
}

fn calculate_sources_totals(sources: &[PathBuf]) -> (usize, u64) {
    let mut total_files = 0;
    let mut total_bytes = 0;

    for src in sources {
        if src.is_file() {
            total_files += 1;
            total_bytes += src.metadata().map(|m| m.len()).unwrap_or(0);
        } else if src.is_dir() {
            for entry in walkdir::WalkDir::new(src).into_iter().flatten() {
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
            let q = state.lock().unwrap();
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
            let q = self.state.lock().unwrap();
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
    if !dst.exists() {
        return dst.to_path_buf();
    }
    let parent = dst.parent().unwrap_or_else(|| Path::new(""));
    let stem = dst.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = dst.extension().map(|e| e.to_string_lossy().to_string());

    let mut counter = 1;
    loop {
        let new_file_name = if counter == 1 {
            match &ext {
                Some(e) if !e.is_empty() => format!("{} - Kopia.{}", stem, e),
                _ => format!("{} - Kopia", stem),
            }
        } else {
            match &ext {
                Some(e) if !e.is_empty() => format!("{} - Kopia ({}).{}", stem, counter, e),
                _ => format!("{} - Kopia ({})", stem, counter),
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
        let mut q = ctx.state.lock().unwrap();
        while ctx.is_paused.load(Ordering::Relaxed) {
            if ctx.cancel_all.load(Ordering::Relaxed) {
                return Err("CANCEL");
            }
            if ctx.skip_current.load(Ordering::Relaxed) {
                return Err("SKIP");
            }
            q = ctx.wake_condvar.wait_timeout(q, Duration::from_millis(200)).unwrap().0;
        }
    }
    Ok(())
}

fn execute_task(
    task: &TaskItem,
    tracker: &mut QueueProgressTracker,
    event_tx: &Sender<WorkerEvent>,
    ctx: &WorkerRuntimeContext,
) -> Result<String, String> {
    match &task.command {
        WorkerCommand::Copy {
            sources,
            target_dir,
        } => {
            for src in sources {
                check_pause_and_cancellation(ctx)
                    .map_err(|e| e.to_string())?;
                let file_name = src
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "plik".to_string());
                let mut dest = target_dir.join(&file_name);

                // Zabezpieczenie przed kopiowaniem katalogu do jego własnego podkatalogu
                if src.is_dir() {
                    let src_norm = normalize_path(src);
                    let target_norm = normalize_path(target_dir);
                    if target_norm.starts_with(&src_norm) {
                        return Err(format!(
                            "Nie można skopiować folderu '{}' do jego własnego podkatalogu",
                            file_name
                        ));
                    }
                }

                // Zabezpieczenie przed samonadpisaniem (src == dest)
                if is_same_file_or_dir(src, &dest) {
                    dest = generate_non_colliding_copy_name(&dest);
                }

                if let Err(e) = copy_recursive(
                    src,
                    &dest,
                    tracker,
                    event_tx,
                    ctx,
                ) {
                    if e == "SKIP" || e == "CANCEL" {
                        return Err(e);
                    }
                    return Err(format!("Błąd kopiowania {}: {}", file_name, e));
                }
            }
            Ok(format!("Skopiowano pomyślnie {} elementów", tracker.task_items_done))
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
                    .unwrap_or_else(|| "plik".to_string());
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
                        return Err(format!(
                            "Nie można przenieść folderu '{}' do jego własnego podkatalogu",
                            file_name
                        ));
                    }
                }

                if fs::rename(src, &dest).is_ok() {
                    let size = dest.metadata().map(|m| m.len()).unwrap_or(0);
                    tracker.task_items_done += 1;
                    tracker.task_bytes_done += size;
                    tracker.update(&file_name, size, size, event_tx);
                    continue;
                }

                if let Err(e) = copy_recursive(
                    src,
                    &dest,
                    tracker,
                    event_tx,
                    ctx,
                ) {
                    if e == "SKIP" || e == "CANCEL" {
                        return Err(e);
                    }
                    return Err(format!("Błąd przenoszenia {}: {}", file_name, e));
                }

                if ctx.skip_current.load(Ordering::Relaxed) {
                    return Err("SKIP".to_string());
                }
                if ctx.cancel_all.load(Ordering::Relaxed) {
                    return Err("CANCEL".to_string());
                }

                if src.is_dir() {
                    let _ = fs::remove_dir_all(src);
                } else {
                    let _ = fs::remove_file(src);
                }
            }
            Ok(format!("Przeniesiono pomyślnie {} elementów", tracker.task_items_done))
        }

        WorkerCommand::Delete { targets, use_trash } => {
            for target in targets {
                check_pause_and_cancellation(ctx)
                    .map_err(|e| e.to_string())?;
                let file_name = target
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "element".to_string());

                tracker.update(&file_name, 0, 0, event_tx);

                let res = if *use_trash {
                    trash::delete(target).map_err(|e| e.to_string())
                } else if target.is_dir() {
                    fs::remove_dir_all(target).map_err(|e| e.to_string())
                } else {
                    fs::remove_file(target).map_err(|e| e.to_string())
                };

                if let Err(e) = res {
                    return Err(format!("Błąd usuwania {}: {}", file_name, e));
                }

                tracker.task_items_done += 1;
            }
            Ok(format!("Usunięto pomyślnie {} elementów", tracker.task_items_done))
        }

        WorkerCommand::CreateDir { path } => {
            fs::create_dir_all(path).map_err(|e| format!("Błąd tworzenia katalogu: {}", e))?;
            tracker.task_items_done += 1;
            Ok("Katalog został utworzony".to_string())
        }

        WorkerCommand::ExtractZip {
            archive_path,
            target_dir,
            filter_prefix,
        } => {
            let cancel_arc = Arc::new(AtomicBool::new(false));
            let cancel_check = Arc::clone(&cancel_arc);
            let skip_ref = Arc::clone(ctx.skip_current);
            let cancel_ref = Arc::clone(ctx.cancel_all);

            let res = crate::zip_support::extract_zip(
                archive_path,
                target_dir,
                filter_prefix.as_deref(),
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
                    Ok(format!("Rozpakowano pomyślnie {} plików", count))
                }
                Err(e) => Err(e),
            }
        }

        WorkerCommand::Rename { source, new_name } => {
            let parent = source.parent().unwrap_or_else(|| Path::new(""));
            let dest = parent.join(new_name);

            if is_same_file_or_dir(source, &dest) {
                return Ok("Nazwa pliku nie uległa zmianie".to_string());
            }

            if dest.exists() {
                return Err(format!("Plik o nazwie '{}' już istnieje!", new_name));
            }

            fs::rename(source, &dest).map_err(|e| format!("Błąd zmiany nazwy: {}", e))?;
            tracker.task_items_done += 1;
            Ok(format!("Zmieniono nazwę na '{}'", new_name))
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
                    Ok(format!("Spakowano pomyślnie {} plików do ZIP", count))
                }
                Err(e) => {
                    let _ = fs::remove_file(archive_path);
                    Err(e)
                }
            }
        }
    }
}

fn copy_recursive(
    src: &Path,
    dst: &Path,
    tracker: &mut QueueProgressTracker,
    event_tx: &Sender<WorkerEvent>,
    ctx: &WorkerRuntimeContext,
) -> Result<(), String> {
    check_pause_and_cancellation(ctx).map_err(|e| e.to_string())?;

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
                &child_src,
                &child_dst,
                tracker,
                event_tx,
                ctx,
            )?;
        }
    } else {
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
            .unwrap_or_else(|| "plik".to_string());

        let mut reader = fs::File::open(src)
            .map_err(|e| format!("Błąd otwarcia pliku {}: {}", src.display(), e))?;
        let mut writer = fs::File::create(dst)
            .map_err(|e| format!("Błąd zapisu do pliku {}: {}", dst.display(), e))?;
        let file_size = reader.metadata().map(|m| m.len()).unwrap_or(0);

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

        tracker.task_items_done += 1;
        tracker.update(&file_name, file_size, file_size, event_tx);
    }

    Ok(())
}
