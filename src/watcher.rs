use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::panel::is_unc_path;

fn canonical_compare_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    let trimmed = if cfg!(windows) {
        let t = s.trim_end_matches(['\\', '/']);
        if t.len() == 2 && t.as_bytes()[1] == b':' {
            format!("{}\\", t)
        } else if t.is_empty() {
            s.to_string()
        } else {
            t.to_string()
        }
    } else {
        let t = s.trim_end_matches('/');
        if t.is_empty() && !s.is_empty() {
            "/".to_string()
        } else {
            t.to_string()
        }
    };

    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed
    }
}

pub fn paths_match(a: &Path, b: &Path) -> bool {
    canonical_compare_key(a) == canonical_compare_key(b)
}

pub fn is_watchable_dir(path: &Path) -> bool {
    if path.as_os_str().is_empty() || is_unc_path(path) {
        return false;
    }
    path.is_dir()
}

pub struct DirWatcher {
    watcher: Option<RecommendedWatcher>,
    watched_paths: Vec<PathBuf>,
    watched_shared: Arc<Mutex<Vec<PathBuf>>>,
    dirty_paths: Arc<Mutex<HashMap<PathBuf, Instant>>>,
    debounce: Duration,
}

impl DirWatcher {
    pub fn new(debounce: Duration, repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        let watched_shared: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
        let dirty_paths: Arc<Mutex<HashMap<PathBuf, Instant>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let watched_cb = Arc::clone(&watched_shared);
        let dirty_cb = Arc::clone(&dirty_paths);

        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else {
                return;
            };
            if matches!(event.kind, EventKind::Access(_) | EventKind::Other) {
                return;
            }

            let watched_snapshot = watched_cb
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if watched_snapshot.is_empty() {
                return;
            }

            let mut matched_dirs: Vec<PathBuf> = Vec::new();
            for ev_path in &event.paths {
                for watched in &watched_snapshot {
                    let is_direct = paths_match(ev_path, watched);
                    let is_child = ev_path
                        .parent()
                        .is_some_and(|parent| paths_match(parent, watched));
                    if (is_direct || is_child)
                        && !matched_dirs.iter().any(|d| paths_match(d, watched))
                    {
                        matched_dirs.push(watched.clone());
                    }
                }
            }

            if !matched_dirs.is_empty() {
                let now = Instant::now();
                let mut dirty = dirty_cb.lock().unwrap_or_else(|e| e.into_inner());
                for dir in matched_dirs {
                    dirty.insert(dir, now);
                }
                if let Some(ref sig) = repaint_signal {
                    sig();
                }
            }
        })
        .ok();

        Self {
            watcher,
            watched_paths: Vec::new(),
            watched_shared,
            dirty_paths,
            debounce,
        }
    }

    pub fn sync_watched_dirs(&mut self, desired: &[PathBuf]) {
        let mut normalized: Vec<PathBuf> = Vec::new();
        for p in desired {
            if is_watchable_dir(p) && !normalized.iter().any(|existing| paths_match(existing, p)) {
                normalized.push(p.clone());
            }
        }

        let same = normalized.len() == self.watched_paths.len()
            && normalized
                .iter()
                .all(|n| self.watched_paths.iter().any(|w| paths_match(n, w)));
        if same {
            return;
        }

        if let Some(ref mut w) = self.watcher {
            for old in &self.watched_paths {
                if !normalized.iter().any(|n| paths_match(n, old)) {
                    let _ = w.unwatch(old);
                }
            }
            for new_p in &normalized {
                if !self.watched_paths.iter().any(|w| paths_match(w, new_p)) {
                    let _ = w.watch(new_p, RecursiveMode::NonRecursive);
                }
            }
        }

        self.watched_paths = normalized.clone();
        *self
            .watched_shared
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = normalized;
    }

    pub fn poll_ready_dirs(&mut self) -> Vec<PathBuf> {
        let mut dirty = self.dirty_paths.lock().unwrap_or_else(|e| e.into_inner());
        if dirty.is_empty() {
            return Vec::new();
        }
        let debounce = self.debounce;
        let ready: Vec<PathBuf> = dirty
            .iter()
            .filter(|(_, ts)| ts.elapsed() >= debounce)
            .map(|(p, _)| p.clone())
            .collect();
        for p in &ready {
            dirty.remove(p);
        }
        ready
    }

    pub fn has_pending(&self) -> bool {
        !self
            .dirty_paths
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }
}
