use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use crossbeam_channel::{Receiver, Sender};

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub start_dir: PathBuf,
    pub mask: String,
    pub text: String,
    pub case_sensitive: bool,
    pub recursive: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub path: PathBuf,
    pub name: String,
    pub dir: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub match_line: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SearchMessage {
    Found(SearchResult),
    ScannedCount(usize),
    Finished(usize),
    Error(String),
}

pub struct SearchHandle {
    pub rx: Receiver<SearchMessage>,
    pub cancel: Arc<AtomicBool>,
}

pub fn start_search(params: SearchParams) -> SearchHandle {
    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = Arc::clone(&cancel);

    thread::spawn(move || {
        run_search_thread(params, tx, cancel_thread);
    });

    SearchHandle { rx, cancel }
}

fn run_search_thread(
    params: SearchParams,
    tx: Sender<SearchMessage>,
    cancel: Arc<AtomicBool>,
) {
    let start_dir = params.start_dir;
    let mask = params.mask.trim();
    let text = params.text.trim();
    let has_text = !text.is_empty();

    let mut scanned_count = 0;
    let mut found_count = 0;

    let walker = if params.recursive {
        walkdir::WalkDir::new(&start_dir).into_iter()
    } else {
        walkdir::WalkDir::new(&start_dir).max_depth(1).into_iter()
    };

    for entry in walker.flatten() {
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(SearchMessage::Finished(found_count));
            return;
        }

        let file_type = entry.file_type();
        if file_type.is_dir() {
            continue;
        }

        scanned_count += 1;
        if scanned_count % 100 == 0 {
            let _ = tx.send(SearchMessage::ScannedCount(scanned_count));
        }

        let file_name = entry.file_name().to_string_lossy();
        if !crate::panel::matches_filter(&file_name, mask) {
            continue;
        }

        let path = entry.path().to_path_buf();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = metadata.len();
        let modified = metadata.modified().ok();
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();

        let mut match_line = None;

        if has_text {
            // Skip huge files (> 50 MB) for text content search
            if size > 50 * 1024 * 1024 {
                continue;
            }

            match check_file_for_text(&path, text, params.case_sensitive) {
                Ok(Some(line)) => {
                    match_line = Some(line);
                }
                Ok(None) => continue, // Text not found
                Err(_) => continue,   // Error or binary file
            }
        }

        found_count += 1;
        let result = SearchResult {
            path,
            name: file_name.to_string(),
            dir,
            size,
            modified,
            match_line,
        };

        if tx.send(SearchMessage::Found(result)).is_err() {
            return;
        }
    }

    let _ = tx.send(SearchMessage::Finished(found_count));
}

/// Safely truncates a string to at most `max_chars` Unicode scalar values, appending `...` if truncated.
pub fn truncate_preview(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", prefix)
    } else {
        prefix
    }
}

fn check_file_for_text(
    path: &Path,
    needle: &str,
    case_sensitive: bool,
) -> Result<Option<String>, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let needle_cmp = if case_sensitive {
        needle.to_string()
    } else {
        needle.to_lowercase()
    };

    let mut byte_buf = Vec::new();
    let mut lines_checked = 0;

    // Check first chunk for null bytes (binary file heuristic)
    let buffer = reader.fill_buf()?;
    if buffer.iter().take(512).any(|&b| b == 0) {
        return Ok(None);
    }

    while reader.read_until(b'\n', &mut byte_buf)? > 0 {
        lines_checked += 1;
        if lines_checked > 50000 {
            break; // Guard against infinite or massive files
        }

        let line_cow = String::from_utf8_lossy(&byte_buf);
        let matches = if case_sensitive {
            line_cow.contains(&needle_cmp)
        } else {
            line_cow.to_lowercase().contains(&needle_cmp)
        };

        if matches {
            let preview = truncate_preview(line_cow.trim(), 120);
            return Ok(Some(preview));
        }

        byte_buf.clear();
    }

    Ok(None)
}
