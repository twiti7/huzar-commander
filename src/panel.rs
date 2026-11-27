use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};

use crate::archive::{is_supported_archive, list_archive_entries};
use crate::fs_item::{FileItem, SortColumn, SortOrder};
use crate::zip_support::ZipLocation;

/// Parses a Windows UNC path (`\\server`, `\\server\share`, `\\server\share\sub...`)
/// into `(server, Option<share>, remaining_segments)`.
/// Device/extended namespaces (`\\?\...`, `\\.\...`) are excluded.
pub fn parse_unc_parts(path: &Path) -> Option<(String, Option<String>, Vec<String>)> {
    let raw = path.to_string_lossy();
    let trimmed = raw.trim();
    if trimmed.starts_with(r"\\?\")
        || trimmed.starts_with(r"\\.\")
        || (cfg!(windows) && (trimmed.starts_with("//?/") || trimmed.starts_with("//./")))
    {
        return None;
    }
    let has_unc_prefix = trimmed.starts_with(r"\\") || (cfg!(windows) && trimmed.starts_with("//"));
    if !has_unc_prefix {
        return None;
    }

    let rest = trimmed.trim_start_matches(['\\', '/']);
    let parts: Vec<&str> = rest
        .split(['\\', '/'])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }

    let server = parts[0].to_string();
    let share = parts.get(1).map(|s| (*s).to_string());
    let sub = parts.iter().skip(2).map(|s| (*s).to_string()).collect();
    Some((server, share, sub))
}

/// Returns `true` if `path` is a UNC path (`\\server...`).
pub fn is_unc_path(path: &Path) -> bool {
    parse_unc_parts(path).is_some()
}

/// Returns `Some(server)` iff `path` points to a UNC server root (`\\server` or `\\server\`).
pub fn unc_server_root(path: &Path) -> Option<String> {
    parse_unc_parts(path).and_then(|(server, share, _)| if share.is_none() { Some(server) } else { None })
}

/// Normalizes a UNC path string (resolving `.` and `..` across share/subfolders).
pub fn normalize_unc_path(raw: &str) -> Option<PathBuf> {
    let (server, share_opt, rest) = parse_unc_parts(Path::new(raw))?;
    let mut stack: Vec<String> = Vec::new();
    if let Some(share) = share_opt {
        stack.push(share);
    }
    for part in rest {
        if part == "." {
            continue;
        }
        if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }
    if stack.is_empty() {
        Some(PathBuf::from(format!(r"\\{}", server)))
    } else {
        Some(PathBuf::from(format!(r"\\{}\{}", server, stack.join(r"\"))))
    }
}

/// Returns the parent directory for upward (`..`) navigation, including stepping from
/// `\\server\share` up to `\\server`.
pub fn parent_dir_for_navigation(path: &Path) -> Option<PathBuf> {
    if let Some((server, share_opt, rest)) = parse_unc_parts(path) {
        let share = share_opt?;
        if rest.is_empty() {
            return Some(PathBuf::from(format!(r"\\{}", server)));
        }
        let mut out = format!(r"\\{}\{}", server, share);
        for seg in &rest[..rest.len() - 1] {
            out.push('\\');
            out.push_str(seg);
        }
        return Some(PathBuf::from(out));
    }
    path.parent().map(|p| p.to_path_buf())
}

/// Returns the directory segment name used to re-focus the cursor after stepping up (`..`).
pub fn dir_name_for_go_up(path: &Path) -> Option<String> {
    if let Some((_, share_opt, rest)) = parse_unc_parts(path) {
        return rest.last().cloned().or(share_opt);
    }
    path.file_name().map(|s| s.to_string_lossy().to_string())
}

#[cfg(windows)]
mod win_unc {
    use std::collections::HashSet;
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    use crate::fs_item::FileItem;

    use super::parse_unc_parts;

    const NERR_SUCCESS: u32 = 0;
    const NO_ERROR: u32 = 0;
    const ERROR_MORE_DATA: u32 = 234;
    const ERROR_NO_MORE_ITEMS: u32 = 259;
    const MAX_PREFERRED_LENGTH: u32 = u32::MAX;
    const STYPE_DISKTREE: u32 = 0;
    const STYPE_MASK: u32 = 0x0000_FFFF;
    const STYPE_SPECIAL: u32 = 0x8000_0000;

    const RESOURCE_GLOBALNET: u32 = 0x0000_0002;
    const RESOURCETYPE_DISK: u32 = 0x0000_0001;
    const RESOURCEDISPLAYTYPE_SERVER: u32 = 0x0000_0002;
    const RESOURCEUSAGE_CONTAINER: u32 = 0x0000_0002;

    #[repr(C)]
    struct ShareInfo1 {
        shi1_netname: *mut u16,
        shi1_type: u32,
        shi1_remark: *mut u16,
    }

    #[repr(C)]
    struct NetResourceW {
        dw_scope: u32,
        dw_type: u32,
        dw_display_type: u32,
        dw_usage: u32,
        lp_local_name: *mut u16,
        lp_remote_name: *mut u16,
        lp_comment: *mut u16,
        lp_provider: *mut u16,
    }

    #[link(name = "netapi32")]
    extern "system" {
        fn NetShareEnum(
            servername: *const u16,
            level: u32,
            bufptr: *mut *mut u8,
            prefmaxlen: u32,
            entriesread: *mut u32,
            totalentries: *mut u32,
            resume_handle: *mut u32,
        ) -> u32;

        fn NetApiBufferFree(buffer: *mut std::ffi::c_void) -> u32;
    }

    #[link(name = "mpr")]
    extern "system" {
        fn WNetOpenEnumW(
            dw_scope: u32,
            dw_type: u32,
            dw_usage: u32,
            lp_net_resource: *const NetResourceW,
            lph_enum: *mut *mut std::ffi::c_void,
        ) -> u32;

        fn WNetEnumResourceW(
            h_enum: *mut std::ffi::c_void,
            lpc_count: *mut u32,
            lp_buffer: *mut std::ffi::c_void,
            lp_buffer_size: *mut u32,
        ) -> u32;

        fn WNetCloseEnum(h_enum: *mut std::ffi::c_void) -> u32;
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        let mut wide: Vec<u16> = std::ffi::OsStr::new(s).encode_wide().collect();
        wide.push(0);
        wide
    }

    unsafe fn wide_ptr_to_string(ptr: *const u16) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let mut len = 0usize;
        while len < 4096 && *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        OsString::from_wide(slice).to_string_lossy().to_string()
    }

    fn make_share_item(server: &str, name: String, is_hidden: bool) -> FileItem {
        FileItem {
            name: name.clone(),
            ext: String::new(),
            path: PathBuf::from(format!(r"\\{}\{}", server, name)),
            is_dir: true,
            is_parent: false,
            size: 0,
            modified: None,
            is_hidden,
            is_readonly: false,
            is_symlink: false,
        }
    }

    fn enumerate_shares_netapi(server: &str) -> Result<Vec<FileItem>, String> {
        let server_wide = to_wide_null(&format!(r"\\{}", server));
        let mut resume_handle: u32 = 0;
        let mut items = Vec::new();
        let mut seen = HashSet::new();

        for _ in 0..64 {
            let mut buf_ptr: *mut u8 = std::ptr::null_mut();
            let mut entries_read: u32 = 0;
            let mut total_entries: u32 = 0;

            let status = unsafe {
                NetShareEnum(
                    server_wide.as_ptr(),
                    1,
                    &mut buf_ptr,
                    MAX_PREFERRED_LENGTH,
                    &mut entries_read,
                    &mut total_entries,
                    &mut resume_handle,
                )
            };

            if status == NERR_SUCCESS || status == ERROR_MORE_DATA {
                if !buf_ptr.is_null() && entries_read > 0 {
                    let entries = unsafe {
                        std::slice::from_raw_parts(
                            buf_ptr as *const ShareInfo1,
                            entries_read as usize,
                        )
                    };
                    for entry in entries {
                        if (entry.shi1_type & STYPE_MASK) != STYPE_DISKTREE {
                            continue;
                        }
                        let name = unsafe { wide_ptr_to_string(entry.shi1_netname) };
                        if name.is_empty() {
                            continue;
                        }
                        let is_hidden =
                            (entry.shi1_type & STYPE_SPECIAL) != 0 || name.ends_with('$');
                        if seen.insert(name.to_lowercase()) {
                            items.push(make_share_item(server, name, is_hidden));
                        }
                    }
                }
                if !buf_ptr.is_null() {
                    unsafe {
                        NetApiBufferFree(buf_ptr as *mut std::ffi::c_void);
                    }
                }
                if status == NERR_SUCCESS {
                    return Ok(items);
                }
            } else {
                if !buf_ptr.is_null() {
                    unsafe {
                        NetApiBufferFree(buf_ptr as *mut std::ffi::c_void);
                    }
                }
                return Err(format!(
                    "{} (os error {})",
                    std::io::Error::from_raw_os_error(status as i32),
                    status
                ));
            }
        }

        Ok(items)
    }

    fn enumerate_shares_wnet(server: &str) -> Result<Vec<FileItem>, String> {
        let mut wide_server = to_wide_null(&format!(r"\\{}", server));
        let root_nr = NetResourceW {
            dw_scope: RESOURCE_GLOBALNET,
            dw_type: RESOURCETYPE_DISK,
            dw_display_type: RESOURCEDISPLAYTYPE_SERVER,
            dw_usage: RESOURCEUSAGE_CONTAINER,
            lp_local_name: std::ptr::null_mut(),
            lp_remote_name: wide_server.as_mut_ptr(),
            lp_comment: std::ptr::null_mut(),
            lp_provider: std::ptr::null_mut(),
        };

        let mut handle: *mut std::ffi::c_void = std::ptr::null_mut();
        let open_status = unsafe {
            WNetOpenEnumW(
                RESOURCE_GLOBALNET,
                RESOURCETYPE_DISK,
                0,
                &root_nr,
                &mut handle,
            )
        };
        if open_status != NO_ERROR {
            return Err(format!(
                "{} (os error {})",
                std::io::Error::from_raw_os_error(open_status as i32),
                open_status
            ));
        }

        let mut items = Vec::new();
        let mut seen = HashSet::new();
        let mut buf: Vec<u64> = vec![0u64; 4096];

        for _ in 0..64 {
            let mut count: u32 = u32::MAX;
            let mut buf_size: u32 = (buf.len() * std::mem::size_of::<u64>()) as u32;
            let status = unsafe {
                WNetEnumResourceW(
                    handle,
                    &mut count,
                    buf.as_mut_ptr() as *mut std::ffi::c_void,
                    &mut buf_size,
                )
            };

            if status == ERROR_MORE_DATA {
                let needed_words = (buf_size as usize).div_ceil(std::mem::size_of::<u64>());
                let next_words = needed_words.max(buf.len().saturating_mul(2));
                buf.resize(next_words, 0);
                continue;
            }

            if status == ERROR_NO_MORE_ITEMS {
                break;
            }

            if status != NO_ERROR {
                unsafe {
                    WNetCloseEnum(handle);
                }
                return Err(format!(
                    "{} (os error {})",
                    std::io::Error::from_raw_os_error(status as i32),
                    status
                ));
            }

            let entries = unsafe {
                std::slice::from_raw_parts(buf.as_ptr() as *const NetResourceW, count as usize)
            };
            for entry in entries {
                let remote = unsafe { wide_ptr_to_string(entry.lp_remote_name) };
                if let Some((_, Some(share), _)) = parse_unc_parts(Path::new(&remote)) {
                    if share.is_empty()
                        || share.eq_ignore_ascii_case("IPC$")
                        || share.eq_ignore_ascii_case("print$")
                    {
                        continue;
                    }
                    let is_hidden = share.ends_with('$');
                    if seen.insert(share.to_lowercase()) {
                        items.push(make_share_item(server, share, is_hidden));
                    }
                }
            }
        }

        unsafe {
            WNetCloseEnum(handle);
        }
        Ok(items)
    }

    pub fn list_unc_shares(server: &str) -> Result<Vec<FileItem>, String> {
        let netapi_res = enumerate_shares_netapi(server);
        if let Ok(ref items) = netapi_res {
            if !items.is_empty() {
                return netapi_res;
            }
        }

        match enumerate_shares_wnet(server) {
            Ok(items) => Ok(items),
            Err(_) => match netapi_res {
                Ok(items) => Ok(items),
                Err(err) => Err(err),
            },
        }
    }
}

#[cfg(windows)]
fn list_unc_shares(server: &str) -> Result<Vec<FileItem>, String> {
    win_unc::list_unc_shares(server)
}

#[cfg(not(windows))]
fn list_unc_shares(server: &str) -> Result<Vec<FileItem>, String> {
    Err(format!(
        "UNC share enumeration for \\\\{} is only supported on Windows",
        server
    ))
}

fn read_directory_entries(
    path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<FileItem>, String> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Ok(Vec::new());
    }

    if let Some(server) = unc_server_root(path) {
        return list_unc_shares(&server);
    }

    let mut new_items = Vec::new();
    if let Some(parent) = parent_dir_for_navigation(path) {
        new_items.push(FileItem::new_parent(parent));
    }

    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                    return Ok(Vec::new());
                }
                if let Some(item) = FileItem::from_path(&entry.path()) {
                    new_items.push(item);
                }
            }
            Ok(new_items)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Clone)]
pub struct DirLoadHandle {
    pub target_path: PathBuf,
    pub old_dir_name: Option<String>,
    pub zip_location: Option<ZipLocation>,
    pub preserve_selection: bool,
    pub record_history: bool,
    pub rx: Receiver<Result<Vec<FileItem>, String>>,
    pub cancel: Arc<AtomicBool>,
    pub started_at: Instant,
}

impl std::fmt::Debug for DirLoadHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirLoadHandle")
            .field("target_path", &self.target_path)
            .field("old_dir_name", &self.old_dir_name)
            .field("started_at", &self.started_at)
            .finish()
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PanelState {
    pub current_path: PathBuf,
    pub all_items: Vec<FileItem>,
    pub items: Vec<FileItem>,
    pub selected_paths: HashSet<PathBuf>,
    pub cursor_idx: usize,
    pub scroll_to_cursor: bool,
    pub sort_column: SortColumn,
    pub sort_order: SortOrder,
    pub filter: String,
    pub is_filter_open: bool,
    pub show_hidden: bool,
    pub custom_title: Option<String>,
    pub zip_location: Option<ZipLocation>,
    pub total_bytes: u64,
    pub total_files: usize,
    pub total_dirs: usize,
    pub selected_bytes: u64,
    pub selected_count: usize,
    pub error_message: Option<String>,
    pub loading_handle: Option<DirLoadHandle>,
    pub calculating_dir_sizes: HashSet<PathBuf>,
    pub calculated_dirs: HashSet<PathBuf>,
    pub dir_size_tx: Sender<(PathBuf, u64, usize)>,
    pub dir_size_rx: Receiver<(PathBuf, u64, usize)>,
    pub history_back: Vec<PathBuf>,
    pub history_forward: Vec<PathBuf>,
    pub is_editing_path: bool,
    pub path_edit_text: String,
    pub focus_path_edit: bool,
    pub quick_search: String,
    pub quick_search_time: Option<Instant>,
}

impl PanelState {
    pub fn new(initial_path: PathBuf) -> Self {
        let (dir_size_tx, dir_size_rx) = crossbeam_channel::unbounded();
        let mut panel = Self {
            current_path: initial_path,
            all_items: Vec::new(),
            items: Vec::new(),
            selected_paths: HashSet::new(),
            cursor_idx: 0,
            scroll_to_cursor: true,
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
            history_back: Vec::new(),
            history_forward: Vec::new(),
            is_editing_path: false,
            path_edit_text: String::new(),
            focus_path_edit: false,
            quick_search: String::new(),
            quick_search_time: None,
        };
        panel.refresh();
        panel
    }

    /// Constructs a `PanelState` without an initial synchronous directory scan.
    pub fn new_deferred(initial_path: PathBuf, show_hidden: bool) -> Self {
        let (dir_size_tx, dir_size_rx) = crossbeam_channel::unbounded();
        Self {
            current_path: initial_path,
            all_items: Vec::new(),
            items: Vec::new(),
            selected_paths: HashSet::new(),
            cursor_idx: 0,
            scroll_to_cursor: true,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            filter: String::new(),
            is_filter_open: false,
            show_hidden,
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
            history_back: Vec::new(),
            history_forward: Vec::new(),
            is_editing_path: false,
            path_edit_text: String::new(),
            focus_path_edit: false,
            quick_search: String::new(),
            quick_search_time: None,
        }
    }

    /// Clones this panel for a new duplicated tab with fresh channels and reset transient state.
    pub fn clone_for_new_tab(&self) -> Self {
        let (dir_size_tx, dir_size_rx) = crossbeam_channel::unbounded();
        let cursor_idx = if self.items.is_empty() {
            0
        } else {
            self.cursor_idx.min(self.items.len() - 1)
        };
        Self {
            current_path: self.current_path.clone(),
            all_items: self.all_items.clone(),
            items: self.items.clone(),
            selected_paths: HashSet::new(),
            cursor_idx,
            scroll_to_cursor: true,
            sort_column: self.sort_column,
            sort_order: self.sort_order,
            filter: String::new(),
            is_filter_open: false,
            show_hidden: self.show_hidden,
            custom_title: None,
            zip_location: self.zip_location.clone(),
            total_bytes: self.total_bytes,
            total_files: self.total_files,
            total_dirs: self.total_dirs,
            selected_bytes: 0,
            selected_count: 0,
            error_message: None,
            loading_handle: None,
            calculating_dir_sizes: HashSet::new(),
            calculated_dirs: HashSet::new(),
            dir_size_tx,
            dir_size_rx,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            is_editing_path: false,
            path_edit_text: String::new(),
            focus_path_edit: false,
            quick_search: String::new(),
            quick_search_time: None,
        }
    }

    pub fn display_path(&self) -> String {
        if let Some(ref title) = self.custom_title {
            format!("[{}]", title)
        } else if let Some(ref loc) = self.zip_location {
            loc.display_path()
        } else {
            self.current_path.to_string_lossy().to_string()
        }
    }

    pub fn is_in_zip(&self) -> bool {
        self.zip_location.is_some()
    }

    pub fn enter_zip(&mut self, zip_path: PathBuf) {
        if is_supported_archive(&zip_path) {
            self.zip_location = Some(ZipLocation::new(zip_path.clone()));
            self.current_path = zip_path;
            self.selected_paths.clear();
            self.cursor_idx = 0;
            self.scroll_to_cursor = true;
            self.refresh();
        }
    }

    pub fn enter_zip_dir(&mut self, dir_name: &str) {
        if let Some(ref mut loc) = self.zip_location {
            loc.enter_subdir(dir_name);
            self.selected_paths.clear();
            self.cursor_idx = 0;
            self.scroll_to_cursor = true;
            self.refresh();
        }
    }

    pub fn exit_zip(&mut self) {
        if let Some(loc) = self.zip_location.take() {
            let parent = loc
                .archive_path
                .parent()
                .unwrap_or(&loc.archive_path)
                .to_path_buf();
            let archive_name = loc
                .archive_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
            self.current_path = parent;
            self.selected_paths.clear();
            self.refresh();
            if let Some(name) = archive_name {
                if let Some(idx) = self.items.iter().position(|it| it.name == name) {
                    self.cursor_idx = idx;
                    self.scroll_to_cursor = true;
                }
            }
        }
    }

    pub fn refresh(&mut self) {
        self.error_message = None;
        self.calculating_dir_sizes.clear();
        self.calculated_dirs.clear();
        let mut new_items = Vec::new();

        if let Some(ref loc) = self.zip_location {
            // ZIP archive browsing
            let parent_path = if loc.internal_dir.is_empty() {
                loc.archive_path
                    .parent()
                    .unwrap_or(&loc.archive_path)
                    .to_path_buf()
            } else {
                loc.archive_path.clone()
            };
            new_items.push(FileItem::new_parent(parent_path));

            match list_archive_entries(&loc.archive_path, &loc.internal_dir) {
                Ok(entries) => {
                    new_items.extend(entries);
                }
                Err(e) => {
                    self.error_message = Some(e);
                }
            }
        } else {
            // Normal file system or UNC share root directory browsing
            match read_directory_entries(&self.current_path, None) {
                Ok(entries) => {
                    new_items = entries;
                }
                Err(e) => {
                    self.error_message = Some(format!("Błąd odczytu: {}", e));
                }
            }
        }

        self.all_items = new_items;
        self.sort_items();
        self.apply_filter();
    }

    pub fn change_dir(&mut self, new_path: PathBuf) {
        if self.zip_location.is_some() {
            self.zip_location = None;
        }

        if new_path.is_dir() || unc_server_root(&new_path).is_some() {
            if new_path != self.current_path && !self.current_path.as_os_str().is_empty() {
                let prev = self.current_path.clone();
                self.push_history(prev);
            }
            self.custom_title = None;
            self.quick_search_clear();
            self.current_path = new_path;
            self.selected_paths.clear();
            self.cursor_idx = 0;
            self.scroll_to_cursor = true;
            self.refresh();
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.history_back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.history_forward.is_empty()
    }

    pub fn push_history(&mut self, prev: PathBuf) {
        if self.history_back.last() == Some(&prev) {
            return;
        }
        self.history_back.push(prev);
        if self.history_back.len() > 50 {
            self.history_back.remove(0);
        }
        self.history_forward.clear();
    }

    pub fn navigate_back_async(
        &mut self,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> bool {
        if let Some(target) = self.history_back.pop() {
            if self.zip_location.is_none()
                && self.custom_title.is_none()
                && !self.current_path.as_os_str().is_empty()
                && self.history_forward.last() != Some(&self.current_path)
            {
                self.history_forward.push(self.current_path.clone());
            }
            self.start_loading_dir_internal(target, None, false, repaint_signal);
            true
        } else {
            false
        }
    }

    pub fn navigate_forward_async(
        &mut self,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> bool {
        if let Some(target) = self.history_forward.pop() {
            if self.zip_location.is_none()
                && self.custom_title.is_none()
                && !self.current_path.as_os_str().is_empty()
                && self.history_back.last() != Some(&self.current_path)
            {
                self.history_back.push(self.current_path.clone());
            }
            self.start_loading_dir_internal(target, None, false, repaint_signal);
            true
        } else {
            false
        }
    }

    pub fn start_path_edit(&mut self) {
        self.is_editing_path = true;
        self.path_edit_text = self.current_path.to_string_lossy().to_string();
        self.focus_path_edit = true;
    }

    pub fn cancel_path_edit(&mut self) {
        self.is_editing_path = false;
        self.focus_path_edit = false;
    }

    pub fn quick_search_push(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let expired = self
            .quick_search_time
            .map(|t| t.elapsed() > std::time::Duration::from_millis(1500))
            .unwrap_or(true);
        if expired {
            self.quick_search.clear();
        }
        self.quick_search.push_str(text);
        self.quick_search_time = Some(Instant::now());

        let needle = self.quick_search.to_lowercase();
        let found_idx = self
            .items
            .iter()
            .position(|it| !it.is_parent && it.name.to_lowercase().starts_with(&needle))
            .or_else(|| {
                self.items
                    .iter()
                    .position(|it| !it.is_parent && it.name.to_lowercase().contains(&needle))
            });

        if let Some(idx) = found_idx {
            self.cursor_idx = idx;
            self.scroll_to_cursor = true;
            true
        } else {
            false
        }
    }

    pub fn quick_search_clear(&mut self) {
        self.quick_search.clear();
        self.quick_search_time = None;
    }

    pub fn quick_search_active(&self) -> Option<&str> {
        if self.quick_search.is_empty() {
            return None;
        }
        let fresh = self
            .quick_search_time
            .map(|t| t.elapsed() <= std::time::Duration::from_millis(1500))
            .unwrap_or(false);
        if fresh {
            Some(self.quick_search.as_str())
        } else {
            None
        }
    }

    pub fn go_up(&mut self) {
        if let Some(ref mut loc) = self.zip_location {
            if loc.go_up() {
                self.selected_paths.clear();
                self.cursor_idx = 0;
                self.scroll_to_cursor = true;
                self.refresh();
            } else {
                self.exit_zip();
            }
            return;
        }

        if let Some(parent_buf) = parent_dir_for_navigation(&self.current_path) {
            let old_dir_name = dir_name_for_go_up(&self.current_path);
            self.change_dir(parent_buf);

            // Put cursor on the directory we just came out from
            if let Some(name) = old_dir_name {
                if let Some(idx) = self.items.iter().position(|it| it.name == name) {
                    self.cursor_idx = idx;
                    self.scroll_to_cursor = true;
                }
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        self.loading_handle.is_some()
    }

    pub fn loading_target(&self) -> Option<&PathBuf> {
        self.loading_handle.as_ref().map(|h| &h.target_path)
    }

    pub fn loading_elapsed(&self) -> std::time::Duration {
        self.loading_handle
            .as_ref()
            .map(|h| h.started_at.elapsed())
            .unwrap_or_default()
    }

    pub fn cancel_loading(&mut self) -> bool {
        if let Some(handle) = self.loading_handle.take() {
            handle.cancel.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn start_loading_dir(
        &mut self,
        new_path: PathBuf,
        old_dir_name: Option<String>,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        self.start_loading_dir_internal(new_path, old_dir_name, true, repaint_signal);
    }

    fn start_loading_dir_internal(
        &mut self,
        new_path: PathBuf,
        old_dir_name: Option<String>,
        record_history: bool,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        // Przerwij ewentualne poprzednie wczytywanie w toku
        self.cancel_loading();

        if self.zip_location.is_some() {
            self.zip_location = None;
        }

        self.custom_title = None;
        self.error_message = None;
        self.calculating_dir_sizes.clear();
        self.calculated_dirs.clear();

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_thread = Arc::clone(&cancel);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let path_to_scan = new_path.clone();

        std::thread::spawn(move || {
            let result = read_directory_entries(&path_to_scan, Some(cancel_thread.as_ref()));
            if !cancel_thread.load(Ordering::Relaxed) {
                let _ = tx.send(result);
                if let Some(sig) = repaint_signal {
                    sig();
                }
            }
        });

        self.loading_handle = Some(DirLoadHandle {
            target_path: new_path,
            old_dir_name,
            zip_location: None,
            preserve_selection: false,
            record_history,
            rx,
            cancel,
            started_at: Instant::now(),
        });
    }

    pub fn refresh_async(
        &mut self,
        focus_item: Option<String>,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        if self.custom_title.is_some() {
            return;
        }
        let desired_focus = focus_item.or_else(|| {
            self.items
                .get(self.cursor_idx)
                .filter(|it| !it.is_parent)
                .map(|it| it.name.clone())
        });

        if let Some(loc) = self.zip_location.clone() {
            self.start_loading_zip_location(loc, desired_focus, true, repaint_signal);
            return;
        }

        let target_path = self.current_path.clone();
        self.cancel_loading();
        self.error_message = None;
        self.calculating_dir_sizes.clear();
        self.calculated_dirs.clear();

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_thread = Arc::clone(&cancel);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let path_to_scan = target_path.clone();

        std::thread::spawn(move || {
            let result = read_directory_entries(&path_to_scan, Some(cancel_thread.as_ref()));
            if !cancel_thread.load(Ordering::Relaxed) {
                let _ = tx.send(result);
                if let Some(sig) = repaint_signal {
                    sig();
                }
            }
        });

        self.loading_handle = Some(DirLoadHandle {
            target_path,
            old_dir_name: desired_focus,
            zip_location: None,
            preserve_selection: true,
            record_history: false,
            rx,
            cancel,
            started_at: Instant::now(),
        });
    }

    pub fn enter_zip_async(
        &mut self,
        zip_path: PathBuf,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        if is_supported_archive(&zip_path) {
            let loc = ZipLocation::new(zip_path);
            self.start_loading_zip_location(loc, None, false, repaint_signal);
        }
    }

    pub fn enter_zip_dir_async(
        &mut self,
        dir_name: &str,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        if let Some(mut loc) = self.zip_location.clone() {
            loc.enter_subdir(dir_name);
            self.start_loading_zip_location(loc, None, false, repaint_signal);
        }
    }

    fn start_loading_zip_location(
        &mut self,
        loc: ZipLocation,
        old_dir_name: Option<String>,
        preserve_selection: bool,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        self.cancel_loading();
        self.custom_title = None;
        self.error_message = None;
        self.calculating_dir_sizes.clear();
        self.calculated_dirs.clear();

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_thread = Arc::clone(&cancel);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let loc_thread = loc.clone();
        let target_path = loc.archive_path.clone();

        std::thread::spawn(move || {
            if cancel_thread.load(Ordering::Relaxed) {
                return;
            }
            let parent_path = if loc_thread.internal_dir.is_empty() {
                loc_thread
                    .archive_path
                    .parent()
                    .unwrap_or(&loc_thread.archive_path)
                    .to_path_buf()
            } else {
                loc_thread.archive_path.clone()
            };
            let mut new_items = vec![FileItem::new_parent(parent_path)];
            match list_archive_entries(&loc_thread.archive_path, &loc_thread.internal_dir) {
                Ok(entries) => {
                    new_items.extend(entries);
                    if !cancel_thread.load(Ordering::Relaxed) {
                        let _ = tx.send(Ok(new_items));
                        if let Some(sig) = repaint_signal {
                            sig();
                        }
                    }
                }
                Err(e) => {
                    if !cancel_thread.load(Ordering::Relaxed) {
                        let _ = tx.send(Err(e));
                        if let Some(sig) = repaint_signal {
                            sig();
                        }
                    }
                }
            }
        });

        self.loading_handle = Some(DirLoadHandle {
            target_path,
            old_dir_name,
            zip_location: Some(loc),
            preserve_selection,
            record_history: false,
            rx,
            cancel,
            started_at: Instant::now(),
        });
    }

    pub fn poll_loading(&mut self) -> Option<Result<PathBuf, String>> {
        let recv_res = self.loading_handle.as_ref()?.rx.try_recv();
        match recv_res {
            Ok(Ok(new_items)) => {
                let handle = self.loading_handle.take()?;
                let target_path = handle.target_path;
                let old_dir_name = handle.old_dir_name;

                if handle.record_history
                    && handle.zip_location.is_none()
                    && self.zip_location.is_none()
                    && self.custom_title.is_none()
                    && target_path != self.current_path
                    && !self.current_path.as_os_str().is_empty()
                {
                    let prev = self.current_path.clone();
                    self.push_history(prev);
                }
                if target_path != self.current_path {
                    self.quick_search_clear();
                }

                self.zip_location = handle.zip_location;
                self.current_path = target_path.clone();
                self.all_items = new_items;
                if handle.preserve_selection {
                    let valid_paths: HashSet<PathBuf> =
                        self.all_items.iter().map(|it| it.path.clone()).collect();
                    self.selected_paths.retain(|p| valid_paths.contains(p));
                } else {
                    self.selected_paths.clear();
                }
                self.cursor_idx = 0;
                self.scroll_to_cursor = true;
                self.sort_items();
                self.apply_filter();

                if let Some(name) = old_dir_name {
                    if let Some(idx) = self.items.iter().position(|it| it.name == name) {
                        self.cursor_idx = idx;
                        self.scroll_to_cursor = true;
                    }
                }

                Some(Ok(target_path))
            }
            Ok(Err(err)) => {
                self.loading_handle = None;
                self.error_message = Some(format!("Błąd odczytu: {}", err));
                Some(Err(err))
            }
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                self.loading_handle = None;
                Some(Err("Wątek przerwany".to_string()))
            }
        }
    }

    pub fn go_up_async(&mut self, repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>) {
        if let Some(mut loc) = self.zip_location.clone() {
            if loc.go_up() {
                self.start_loading_zip_location(loc, None, false, repaint_signal);
            } else {
                let parent = loc
                    .archive_path
                    .parent()
                    .unwrap_or(&loc.archive_path)
                    .to_path_buf();
                let archive_name = loc
                    .archive_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string());
                self.start_loading_dir(parent, archive_name, repaint_signal);
            }
            return;
        }

        if let Some(parent_buf) = parent_dir_for_navigation(&self.current_path) {
            let old_dir_name = dir_name_for_go_up(&self.current_path);
            self.start_loading_dir(parent_buf, old_dir_name, repaint_signal);
        }
    }

    pub fn apply_filter(&mut self) {
        let active_path = self.items.get(self.cursor_idx).map(|it| it.path.clone());

        let show_hidden = self.show_hidden;
        let is_empty_filter = self.filter.trim().is_empty();

        self.items = self
            .all_items
            .iter()
            .filter(|it| {
                if it.is_parent {
                    return true;
                }
                if !show_hidden && it.is_hidden {
                    return false;
                }
                if is_empty_filter {
                    true
                } else {
                    matches_filter(&it.name, &self.filter)
                }
            })
            .cloned()
            .collect();

        self.recalculate_totals();

        if let Some(path) = active_path {
            if let Some(idx) = self.items.iter().position(|it| it.path == path) {
                self.cursor_idx = idx;
            } else if !self.items.is_empty() {
                self.cursor_idx = 0;
            }
        } else if self.cursor_idx >= self.items.len() && !self.items.is_empty() {
            self.cursor_idx = self.items.len() - 1;
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.apply_filter();
    }

    #[allow(dead_code)]
    pub fn set_filter(&mut self, new_filter: String) {
        self.filter = new_filter;
        self.apply_filter();
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.apply_filter();
    }

    pub fn selected_or_cursor_items(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            self.selected_paths.iter().cloned().collect()
        } else if let Some(item) = self.items.get(self.cursor_idx) {
            if !item.is_parent {
                vec![item.path.clone()]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    pub fn toggle_select(&mut self, idx: usize) {
        if let Some(item) = self.items.get(idx) {
            if item.is_parent {
                return;
            }
            if self.selected_paths.contains(&item.path) {
                self.selected_paths.remove(&item.path);
            } else {
                self.selected_paths.insert(item.path.clone());
            }
            self.recalculate_totals();
        }
    }

    pub fn select_range(&mut self, from: usize, to: usize) {
        let start = from.min(to);
        let end = from.max(to);
        for idx in start..=end {
            if let Some(item) = self.items.get(idx) {
                if !item.is_parent {
                    self.selected_paths.insert(item.path.clone());
                }
            }
        }
        self.recalculate_totals();
    }

    pub fn select_all(&mut self) {
        for item in &self.items {
            if !item.is_parent {
                self.selected_paths.insert(item.path.clone());
            }
        }
        self.recalculate_totals();
    }

    pub fn deselect_all(&mut self) {
        self.selected_paths.clear();
        self.recalculate_totals();
    }

    pub fn invert_selection(&mut self) {
        for item in &self.items {
            if !item.is_parent {
                if self.selected_paths.contains(&item.path) {
                    self.selected_paths.remove(&item.path);
                } else {
                    self.selected_paths.insert(item.path.clone());
                }
            }
        }
        self.recalculate_totals();
    }

    pub fn select_by_pattern(&mut self, pattern: &str) {
        for item in &self.items {
            if !item.is_parent && matches_filter(&item.name, pattern) {
                self.selected_paths.insert(item.path.clone());
            }
        }
        self.recalculate_totals();
    }

    pub fn deselect_by_pattern(&mut self, pattern: &str) {
        for item in &self.items {
            if !item.is_parent && matches_filter(&item.name, pattern) {
                self.selected_paths.remove(&item.path);
            }
        }
        self.recalculate_totals();
    }

    pub fn is_calculating_dir(&self, path: &Path) -> bool {
        self.calculating_dir_sizes.contains(path)
    }

    pub fn is_calculated_dir(&self, path: &Path) -> bool {
        self.calculated_dirs.contains(path)
    }

    pub fn has_calculating_dirs(&self) -> bool {
        !self.calculating_dir_sizes.is_empty()
    }

    pub fn start_calculate_dir_size(
        &mut self,
        dir_path: PathBuf,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        if self.calculating_dir_sizes.contains(&dir_path) {
            return;
        }

        self.calculating_dir_sizes.insert(dir_path.clone());
        let tx = self.dir_size_tx.clone();
        let target_path = dir_path.clone();

        std::thread::spawn(move || {
            let mut total_size: u64 = 0;
            let mut file_count: usize = 0;

            for entry in walkdir::WalkDir::new(&target_path)
                .min_depth(1)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if entry.file_type().is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total_size = total_size.saturating_add(meta.len());
                        file_count += 1;
                    }
                }
            }

            let _ = tx.send((target_path, total_size, file_count));
            if let Some(sig) = repaint_signal {
                sig();
            }
        });
    }

    pub fn poll_dir_sizes(&mut self) -> Vec<(PathBuf, u64, usize)> {
        let mut finished = Vec::new();
        while let Ok((path, size, file_count)) = self.dir_size_rx.try_recv() {
            self.calculating_dir_sizes.remove(&path);
            self.calculated_dirs.insert(path.clone());

            let mut matched = false;
            if let Some(item) = self.items.iter_mut().find(|it| it.path == path) {
                item.size = size;
                matched = true;
            }
            if let Some(item) = self.all_items.iter_mut().find(|it| it.path == path) {
                item.size = size;
                matched = true;
            }

            if matched {
                self.recalculate_totals();
                finished.push((path, size, file_count));
            }
        }
        finished
    }

    pub fn space_select_and_calculate(
        &mut self,
        repaint_signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Option<PathBuf> {
        let idx = self.cursor_idx;
        let item = self.items.get(idx)?.clone();
        if item.is_parent {
            return None;
        }

        // 1. Zaznacz / odznacz bieżący element
        self.toggle_select(idx);

        // 2. Jeśli to katalog, uruchom asynchroniczne zliczanie rozmiaru w tle
        let calc_path = if item.is_dir {
            self.start_calculate_dir_size(item.path.clone(), repaint_signal);
            Some(item.path)
        } else {
            None
        };

        // 3. Przesuń kursor w dół o 1 wiersz (jak klawisz Insert), aby natychmiast móc zaznaczać kolejne elementy
        if idx + 1 < self.items.len() {
            self.cursor_idx += 1;
            self.scroll_to_cursor = true;
        }

        calc_path
    }

    pub fn load_search_results(&mut self, results: &[crate::search::SearchResult]) {
        self.zip_location = None;
        self.custom_title = Some("Wyniki wyszukiwania".to_string());
        let parent_item = FileItem::new_parent(self.current_path.clone());
        let mut new_items = vec![parent_item];

        for res in results {
            let item = FileItem {
                name: res.name.clone(),
                ext: res
                    .path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default(),
                path: res.path.clone(),
                is_dir: false,
                is_parent: false,
                size: res.size,
                modified: res.modified,
                is_hidden: false,
                is_readonly: false,
                is_symlink: false,
            };
            new_items.push(item);
        }

        self.all_items = new_items;
        self.selected_paths.clear();
        self.cursor_idx = 0;
        self.scroll_to_cursor = true;
        self.sort_items();
        self.apply_filter();
    }

    pub fn sort_items(&mut self) {
        let col = self.sort_column;
        let ord = self.sort_order;

        self.all_items.sort_by(|a, b| {
            // ".." is always at the very top
            if a.is_parent {
                return std::cmp::Ordering::Less;
            }
            if b.is_parent {
                return std::cmp::Ordering::Greater;
            }

            // Directories always come before files
            if a.is_dir != b.is_dir {
                return if a.is_dir {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }

            let cmp = match col {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Ext => a.ext.cmp(&b.ext).then_with(|| a.name.cmp(&b.name)),
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::Date => a.modified.cmp(&b.modified),
            };

            match ord {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });

        self.apply_filter();
    }

    pub fn recalculate_totals(&mut self) {
        let mut total_bytes = 0;
        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut sel_bytes = 0;
        let mut sel_count = 0;

        for item in &self.items {
            if item.is_parent {
                continue;
            }
            if item.is_dir {
                total_dirs += 1;
            } else {
                total_files += 1;
            }
            total_bytes += item.size;

            if self.selected_paths.contains(&item.path) {
                sel_count += 1;
                sel_bytes += item.size;
            }
        }

        self.total_bytes = total_bytes;
        self.total_files = total_files;
        self.total_dirs = total_dirs;
        self.selected_bytes = sel_bytes;
        self.selected_count = sel_count;
    }
}

/// Compares non-directory files across `left` and `right` panels by name, size, and modified time,
/// selecting missing, size-mismatched, or newer (`> 2s`) files in each panel.
pub fn compare_directories(left: &mut PanelState, right: &mut PanelState) -> (usize, usize) {
    use std::collections::HashMap;
    use std::time::Duration;

    left.selected_paths.clear();
    right.selected_paths.clear();

    let key_for = |name: &str| -> String {
        if cfg!(windows) {
            name.to_lowercase()
        } else {
            name.to_string()
        }
    };

    let left_map: HashMap<String, &FileItem> = left
        .items
        .iter()
        .filter(|it| !it.is_parent && !it.is_dir)
        .map(|it| (key_for(&it.name), it))
        .collect();
    let right_map: HashMap<String, &FileItem> = right
        .items
        .iter()
        .filter(|it| !it.is_parent && !it.is_dir)
        .map(|it| (key_for(&it.name), it))
        .collect();

    let tol = Duration::from_secs(2);

    for (k, l_item) in &left_map {
        let should_select = match right_map.get(k) {
            None => true,
            Some(r_item) => {
                if l_item.size != r_item.size {
                    true
                } else {
                    match (l_item.modified, r_item.modified) {
                        (Some(lm), Some(rm)) => lm > rm + tol,
                        _ => false,
                    }
                }
            }
        };
        if should_select {
            left.selected_paths.insert(l_item.path.clone());
        }
    }

    for (k, r_item) in &right_map {
        let should_select = match left_map.get(k) {
            None => true,
            Some(l_item) => {
                if r_item.size != l_item.size {
                    true
                } else {
                    match (r_item.modified, l_item.modified) {
                        (Some(rm), Some(lm)) => rm > lm + tol,
                        _ => false,
                    }
                }
            }
        };
        if should_select {
            right.selected_paths.insert(r_item.path.clone());
        }
    }

    left.recalculate_totals();
    right.recalculate_totals();
    (left.selected_paths.len(), right.selected_paths.len())
}

pub fn matches_filter(name: &str, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() || filter == "*" || filter == "*.*" {
        return true;
    }

    let name_lower = name.to_lowercase();
    let filter_lower = filter.to_lowercase();

    if filter_lower.contains(';') || filter_lower.contains(',') {
        return filter_lower
            .split([';', ','])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .any(|pat| matches_single_pattern(&name_lower, pat));
    }

    matches_single_pattern(&name_lower, &filter_lower)
}

fn matches_single_pattern(name: &str, pattern: &str) -> bool {
    if pattern.contains('*') || pattern.contains('?') {
        glob_match(pattern, name)
    } else {
        name.contains(pattern)
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p_bytes = pattern.as_bytes();
    let t_bytes = text.as_bytes();
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (None, 0);

    while ti < t_bytes.len() {
        if pi < p_bytes.len() && (p_bytes[pi] == b'?' || p_bytes[pi] == t_bytes[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p_bytes.len() && p_bytes[pi] == b'*' {
            star_pi = Some(pi);
            pi += 1;
            star_ti = ti;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < p_bytes.len() && p_bytes[pi] == b'*' {
        pi += 1;
    }

    pi == p_bytes.len()
}
