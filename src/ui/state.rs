use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use crate::app::PanelSide;
use crate::opener::ViewerMode;
use crate::search::{SearchHandle, SearchResult};

#[derive(Default)]
pub struct SearchDialogState {
    pub is_open: bool,
    pub mask: String,
    pub dir: String,
    pub text: String,
    pub case_sensitive: bool,
    pub recursive: bool,
    pub results: Vec<SearchResult>,
    pub selected_idx: Option<usize>,
    pub handle: Option<SearchHandle>,
    pub status_text: String,
    pub scanned_count: usize,
}

impl SearchDialogState {
    pub fn stop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

fn build_utf8_lower_map(haystack: &[u8]) -> (String, Vec<usize>) {
    let mut lower = String::with_capacity(haystack.len());
    let mut offsets = Vec::with_capacity(haystack.len());
    let mut idx = 0usize;

    while idx < haystack.len() {
        match std::str::from_utf8(&haystack[idx..]) {
            Ok(valid) => {
                for (rel, ch) in valid.char_indices() {
                    let orig = idx + rel;
                    for lc in ch.to_lowercase() {
                        let start = lower.len();
                        lower.push(lc);
                        for _ in start..lower.len() {
                            offsets.push(orig);
                        }
                    }
                }
                break;
            }
            Err(e) => {
                let valid_len = e.valid_up_to();
                if valid_len > 0 {
                    if let Ok(valid) = std::str::from_utf8(&haystack[idx..idx + valid_len]) {
                        for (rel, ch) in valid.char_indices() {
                            let orig = idx + rel;
                            for lc in ch.to_lowercase() {
                                let start = lower.len();
                                lower.push(lc);
                                for _ in start..lower.len() {
                                    offsets.push(orig);
                                }
                            }
                        }
                    }
                }
                let bad_pos = idx + valid_len;
                if bad_pos < haystack.len() {
                    lower.push('\u{FFFD}');
                    for _ in 0..'\u{FFFD}'.len_utf8() {
                        offsets.push(bad_pos);
                    }
                }
                idx = bad_pos + 1;
            }
        }
    }

    (lower, offsets)
}

fn find_in_block_ci(
    haystack: &[u8],
    needle_lower: &str,
    encoding: crate::opener::TextEncoding,
    find_last: bool,
    max_start_rel: usize,
) -> Option<usize> {
    let (lower, offsets) = match encoding {
        crate::opener::TextEncoding::Utf8 => build_utf8_lower_map(haystack),
        crate::opener::TextEncoding::Windows1250 => {
            let decoded = crate::opener::decode_bytes_with_encoding(haystack, encoding);
            let mut lower = String::with_capacity(decoded.len());
            let mut offsets = Vec::with_capacity(decoded.len());
            for (orig_byte, ch) in decoded.chars().enumerate() {
                for lc in ch.to_lowercase() {
                    let start = lower.len();
                    lower.push(lc);
                    for _ in start..lower.len() {
                        offsets.push(orig_byte);
                    }
                }
            }
            (lower, offsets)
        }
        crate::opener::TextEncoding::Utf16Le | crate::opener::TextEncoding::Utf16Be => {
            let decoded = crate::opener::decode_bytes_with_encoding(haystack, encoding);
            let mut lower = String::with_capacity(decoded.len());
            let mut offsets = Vec::with_capacity(decoded.len());
            let mut byte_pos = 0usize;
            for ch in decoded.chars() {
                let u16_bytes = ch.len_utf16() * 2;
                for lc in ch.to_lowercase() {
                    let start = lower.len();
                    lower.push(lc);
                    for _ in start..lower.len() {
                        offsets.push(byte_pos);
                    }
                }
                byte_pos += u16_bytes;
            }
            (lower, offsets)
        }
    };

    if find_last {
        lower
            .match_indices(needle_lower)
            .filter_map(|(pos, _)| offsets.get(pos).copied())
            .filter(|&rel| rel < max_start_rel)
            .last()
    } else {
        lower
            .find(needle_lower)
            .and_then(|pos| offsets.get(pos).copied())
            .filter(|&rel| rel < max_start_rel)
    }
}

/// Unicode-aware forward and backward file search usable in both UI threads and background workers.
pub fn search_file_offset(
    path: &std::path::Path,
    query: &str,
    current_offset: u64,
    forward: bool,
    encoding: crate::opener::TextEncoding,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Option<u64>, String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::atomic::Ordering;

    let needle_lower = query.trim().to_lowercase();
    if needle_lower.is_empty() {
        return Ok(None);
    }

    let mut file = File::open(path).map_err(|e| format!("Błąd otwierania pliku: {}", e))?;
    let file_len = file
        .metadata()
        .map_err(|e| format!("Błąd odczytu rozmiaru: {}", e))?
        .len();
    if file_len == 0 {
        return Ok(None);
    }

    const BLOCK_SIZE: usize = 128 * 1024;
    let overlap_len: usize = (needle_lower.len() * 4).clamp(64, 4096);

    if forward {
        let mut pos = current_offset.saturating_add(1).min(file_len);
        if file.seek(SeekFrom::Start(pos)).is_err() {
            return Ok(None);
        }

        let mut buf = vec![0u8; BLOCK_SIZE];
        let mut overlap: Vec<u8> = Vec::new();

        while pos < file_len {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let to_read = (BLOCK_SIZE as u64).min(file_len - pos) as usize;
            if file.read_exact(&mut buf[..to_read]).is_err() {
                break;
            }

            let mut scan_buf = overlap.clone();
            scan_buf.extend_from_slice(&buf[..to_read]);
            let scan_base = pos.saturating_sub(overlap.len() as u64);

            if let Some(rel) = find_in_block_ci(&scan_buf, &needle_lower, encoding, false, scan_buf.len())
            {
                let found_offset = scan_base + rel as u64;
                if found_offset > current_offset {
                    return Ok(Some(found_offset));
                }
            }

            let keep = overlap_len.min(scan_buf.len());
            overlap = scan_buf[scan_buf.len() - keep..].to_vec();
            pos += to_read as u64;
        }
    } else {
        let mut end = current_offset.min(file_len);
        while end > 0 {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let block_start = end.saturating_sub(BLOCK_SIZE as u64);
            let read_end = (end + overlap_len as u64).min(file_len);
            let to_read = (read_end - block_start) as usize;

            if file.seek(SeekFrom::Start(block_start)).is_err() {
                break;
            }
            let mut scan_buf = vec![0u8; to_read];
            if file.read_exact(&mut scan_buf).is_err() {
                break;
            }

            let max_start_rel = (end - block_start) as usize;
            if let Some(rel) =
                find_in_block_ci(&scan_buf, &needle_lower, encoding, true, max_start_rel)
            {
                return Ok(Some(block_start + rel as u64));
            }

            end = block_start;
        }
    }

    Ok(None)
}

pub struct ViewerSearchHandle {
    pub rx: crossbeam_channel::Receiver<Result<Option<u64>, String>>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub struct ImageLoadHandle {
    pub rx: crossbeam_channel::Receiver<Result<(egui::ColorImage, u32, u32), String>>,
}

pub struct ViewerWindowState {
    pub is_open: bool,
    pub title: String,
    pub content: String,
    pub path: Option<PathBuf>,
    pub total_size: u64,
    pub offset: u64,
    pub mode: ViewerMode,
    pub encoding: crate::opener::TextEncoding,
    pub word_wrap: bool,
    pub tail_mode: bool,
    pub last_tail_check: Option<Instant>,
    pub chunk: Vec<u8>,
    pub search_query: String,
    pub is_search_open: bool,
    pub search_status: Option<String>,
    pub search_handle: Option<ViewerSearchHandle>,
    pub last_match_offset: Option<u64>,
    pub structured_subview: crate::ui::structured_viewer::StructuredSubView,
    pub image_texture: Option<egui::TextureHandle>,
    pub image_dims: Option<(u32, u32)>,
    pub image_zoom: f32,
    pub image_fit: bool,
    pub image_loader: Option<ImageLoadHandle>,
    pub image_error: Option<String>,
    pub scroll_ratio: f32,
    pub scroll_to_top: bool,
    pub scroll_to_bottom: bool,
}

impl Default for ViewerWindowState {
    fn default() -> Self {
        Self {
            is_open: false,
            title: String::new(),
            content: String::new(),
            path: None,
            total_size: 0,
            offset: 0,
            mode: ViewerMode::Text,
            encoding: crate::opener::TextEncoding::Utf8,
            word_wrap: true,
            tail_mode: false,
            last_tail_check: None,
            chunk: Vec::new(),
            search_query: String::new(),
            is_search_open: false,
            search_status: None,
            search_handle: None,
            last_match_offset: None,
            structured_subview: crate::ui::structured_viewer::StructuredSubView::Code,
            image_texture: None,
            image_dims: None,
            image_zoom: 1.0,
            image_fit: true,
            image_loader: None,
            image_error: None,
            scroll_ratio: 0.0,
            scroll_to_top: false,
            scroll_to_bottom: false,
        }
    }
}

impl ViewerWindowState {
    pub fn chunk_size(&self) -> usize {
        if self.mode == ViewerMode::Markdown
            || self.mode == ViewerMode::Json
            || self.mode == ViewerMode::Yaml
        {
            1024 * 1024
        } else {
            64 * 1024
        }
    }

    pub fn is_busy(&self) -> bool {
        self.search_handle.is_some() || self.image_loader.is_some() || self.tail_mode
    }

    /// Oblicza aktualny procent przeglądania całego pliku (uwzględniając offset chunka oraz przewinięcie w oknie).
    pub fn calculate_view_percentage(&self) -> f64 {
        if self.total_size == 0 || self.mode == ViewerMode::Image {
            return 100.0;
        }

        let chunk_step = self.chunk_size() as u64;
        let chunk_len = self.chunk.len() as u64;

        if self.total_size <= chunk_step {
            (self.scroll_ratio as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            let chunk_start = self.offset as f64;
            let chunk_span = chunk_len as f64;
            let current_pos = chunk_start + chunk_span * self.scroll_ratio as f64;
            ((current_pos / self.total_size as f64) * 100.0).clamp(0.0, 100.0)
        }
    }

    /// Oblicza bieżący offset bajtowy widoczny na samej górze ekranu
    pub fn current_visible_offset(&self) -> u64 {
        if self.total_size == 0 {
            return 0;
        }
        let chunk_len = self.chunk.len() as u64;
        if chunk_len == 0 {
            return self.offset;
        }

        let delta = (chunk_len as f64 * self.scroll_ratio as f64).round() as u64;
        let visible = self.offset + delta;
        visible.min(self.total_size)
    }

    pub fn stop_find(&mut self) {
        if let Some(h) = self.search_handle.take() {
            h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub fn open_file(&mut self, title: String, path: PathBuf) -> Result<(), String> {
        self.stop_find();
        self.title = title;
        self.path = Some(path.clone());
        self.offset = 0;
        self.scroll_ratio = 0.0;
        self.tail_mode = false;
        self.last_tail_check = None;
        self.last_match_offset = None;
        self.search_status = None;
        self.image_texture = None;
        self.image_dims = None;
        self.image_zoom = 1.0;
        self.image_fit = true;
        self.image_loader = None;
        self.image_error = None;

        if crate::opener::is_native_preview_image(&path) {
            let meta = std::fs::metadata(&path)
                .map_err(|e| format!("Nie można otworzyć pliku obrazu: {}", e))?;
            self.total_size = meta.len();
            self.mode = ViewerMode::Image;
            self.chunk = Vec::new();
            self.content = String::new();
            self.is_open = true;

            let (tx, rx) = crossbeam_channel::bounded(1);
            let img_path = path.clone();
            std::thread::spawn(move || {
                let res = image::open(&img_path)
                    .map(|dyn_img| {
                        let rgba = dyn_img.to_rgba8();
                        let w = rgba.width();
                        let h = rgba.height();
                        let color_img = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            rgba.as_raw(),
                        );
                        (color_img, w, h)
                    })
                    .map_err(|e| e.to_string());
                let _ = tx.send(res);
            });
            self.image_loader = Some(ImageLoadHandle { rx });
            return Ok(());
        }

        let is_md = crate::opener::is_markdown_file(&path);
        let is_json = crate::opener::is_json_file(&path);
        let is_yaml = crate::opener::is_yaml_file(&path);
        let is_code = crate::opener::is_code_file(&path);
        let initial_chunk_size = if is_md || is_json || is_yaml {
            1024 * 1024
        } else {
            64 * 1024
        };

        let (buf, total_size) = crate::opener::read_file_chunk(&path, 0, initial_chunk_size)?;
        self.total_size = total_size;
        self.encoding = crate::opener::detect_encoding(&buf);

        let is_binary = match self.encoding {
            crate::opener::TextEncoding::Utf16Le | crate::opener::TextEncoding::Utf16Be => false,
            _ => crate::opener::detect_is_binary(&buf),
        };
        self.mode = if is_binary {
            ViewerMode::Hex
        } else if is_md {
            ViewerMode::Markdown
        } else if is_json {
            ViewerMode::Json
        } else if is_yaml {
            ViewerMode::Yaml
        } else if is_code {
            ViewerMode::Code
        } else {
            ViewerMode::Text
        };
        self.structured_subview = crate::ui::structured_viewer::StructuredSubView::Code;
        self.chunk = buf;
        self.refresh_content();
        self.is_open = true;
        Ok(())
    }

    pub fn reload_chunk(&mut self) {
        if self.mode == ViewerMode::Image {
            if let Some(ref p) = self.path.clone() {
                if self.image_texture.is_none() && self.image_loader.is_none() {
                    let _ = self.open_file(self.title.clone(), p.clone());
                }
            }
            return;
        }
        if let Some(path) = self.path.clone() {
            let chunk_sz = self.chunk_size();
            let max_offset = self.total_size.saturating_sub(chunk_sz as u64);
            if self.offset > max_offset && self.total_size > chunk_sz as u64 {
                self.offset = max_offset;
            }
            match crate::opener::read_file_chunk(&path, self.offset, chunk_sz) {
                Ok((buf, total_size)) => {
                    self.total_size = total_size;
                    self.chunk = buf;
                    self.refresh_content();
                }
                Err(e) => {
                    self.content = format!("Błąd odczytu pliku: {}", e);
                }
            }
        }
    }

    pub fn refresh_content(&mut self) {
        self.content = match self.mode {
            ViewerMode::Hex => {
                crate::opener::format_hex_chunk(&self.chunk, self.offset, self.total_size)
            }
            ViewerMode::Text => crate::opener::format_text_chunk_with_encoding(
                &self.chunk,
                self.offset,
                self.total_size,
                self.encoding,
            ),
            ViewerMode::Code | ViewerMode::Markdown | ViewerMode::Json | ViewerMode::Yaml => {
                crate::opener::decode_bytes_with_encoding(&self.chunk, self.encoding)
            }
            ViewerMode::Image => String::new(),
        };
    }

    pub fn seek_next(&mut self) {
        if self.mode == ViewerMode::Image {
            return;
        }
        let chunk_sz = self.chunk_size() as u64;
        let max_offset = self.total_size.saturating_sub(chunk_sz);
        if self.offset < max_offset {
            self.offset = (self.offset + chunk_sz).min(max_offset);
            self.reload_chunk();
        }
    }

    pub fn seek_prev(&mut self) {
        if self.mode == ViewerMode::Image {
            return;
        }
        let chunk_sz = self.chunk_size() as u64;
        if self.offset > 0 {
            self.offset = self.offset.saturating_sub(chunk_sz);
            self.reload_chunk();
        }
    }

    pub fn start_find(
        &mut self,
        forward: bool,
        repaint_signal: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    ) {
        self.stop_find();
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };

        let start_cursor = if forward && self.offset == 0 && self.last_match_offset != Some(0) {
            // Allow matching at offset 0 on initial forward search
            None
        } else {
            Some(self.offset)
        };
        let encoding = self.encoding;
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_thr = std::sync::Arc::clone(&cancel);
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.search_status = None;

        std::thread::spawn(move || {
            let res = match start_cursor {
                None => {
                    let prefix_hit = std::fs::File::open(&path).ok().and_then(|mut f| {
                        use std::io::Read;
                        let mut b = vec![0u8; (query.len() * 4 + 32).min(4096)];
                        let n = f.read(&mut b).ok()?;
                        b.truncate(n);
                        find_in_block_ci(&b, &query.to_lowercase(), encoding, false, b.len())
                    });
                    if let Some(0) = prefix_hit {
                        Ok(Some(0))
                    } else {
                        search_file_offset(&path, &query, 0, true, encoding, &cancel_thr)
                    }
                }
                Some(off) => search_file_offset(&path, &query, off, forward, encoding, &cancel_thr),
            };
            if !cancel_thr.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = tx.send(res);
                if let Some(sig) = repaint_signal {
                    sig();
                }
            }
        });

        self.search_handle = Some(ViewerSearchHandle { rx, cancel });
    }

    pub fn poll_background(&mut self, ctx: &egui::Context, i18n: &crate::i18n::I18n) {
        if let Some(ref handle) = self.search_handle {
            match handle.rx.try_recv() {
                Ok(Ok(Some(found_offset))) => {
                    self.search_handle = None;
                    self.offset = found_offset;
                    self.last_match_offset = Some(found_offset);
                    self.reload_chunk();
                    let off_hex = format!("0x{:08X}", found_offset);
                    self.search_status =
                        Some(i18n.t_fmt("viewer.search_found_at", &[("offset", &off_hex)]));
                    ctx.request_repaint();
                }
                Ok(Ok(None)) => {
                    self.search_handle = None;
                    self.search_status = Some(i18n.t("viewer.search_not_found").to_string());
                    ctx.request_repaint();
                }
                Ok(Err(err)) => {
                    self.search_handle = None;
                    self.search_status = Some(err);
                    ctx.request_repaint();
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.search_handle = None;
                }
            }
        }

        if let Some(ref img_handle) = self.image_loader {
            match img_handle.rx.try_recv() {
                Ok(Ok((color_img, w, h))) => {
                    self.image_loader = None;
                    let tex = ctx.load_texture(
                        format!("viewer_img_{}", self.title),
                        color_img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.image_texture = Some(tex);
                    self.image_dims = Some((w, h));
                    ctx.request_repaint();
                }
                Ok(Err(err)) => {
                    self.image_loader = None;
                    self.image_error = Some(err);
                    ctx.request_repaint();
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.image_loader = None;
                }
            }
        }

        if self.tail_mode && self.mode != ViewerMode::Image {
            let due = self
                .last_tail_check
                .map(|t| t.elapsed() >= std::time::Duration::from_millis(500))
                .unwrap_or(true);
            if due {
                self.last_tail_check = Some(Instant::now());
                if let Some(ref p) = self.path {
                    if let Ok(meta) = std::fs::metadata(p) {
                        let new_len = meta.len();
                        if new_len != self.total_size {
                            self.total_size = new_len;
                            let step = self.chunk_size() as u64;
                            self.offset = new_len.saturating_sub(step);
                            self.reload_chunk();
                            self.scroll_to_bottom = true;
                            ctx.request_repaint();
                        }
                    }
                }
            }
        }
    }

    pub fn find_next(&mut self, forward: bool) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        match search_file_offset(
            &path,
            &self.search_query,
            self.offset,
            forward,
            self.encoding,
            &cancel,
        ) {
            Ok(Some(found_offset)) => {
                self.offset = found_offset;
                self.last_match_offset = Some(found_offset);
                self.reload_chunk();
                self.search_status = Some(format!("Trafienie na pozycji 0x{:08X}", found_offset));
            }
            Ok(None) => {
                self.search_status = Some("Koniec pliku - brak kolejnych wystąpień".to_string());
            }
            Err(err) => {
                self.search_status = Some(err);
            }
        }
    }
}

#[derive(Default)]
pub struct QuickViewState {
    pub enabled: bool,
    pub pending_target: Option<(PathBuf, String, bool)>,
    pub pending_since: Option<Instant>,
    pub loaded_path: Option<PathBuf>,
    pub is_dir_summary: bool,
    pub dir_name: String,
    pub dir_files: usize,
    pub dir_dirs: usize,
    pub dir_bytes: u64,
    pub dir_calc_rx: Option<crossbeam_channel::Receiver<(u64, usize, usize)>>,
    pub dir_calc_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub placeholder_msg: Option<String>,
    pub viewer: ViewerWindowState,
}

impl QuickViewState {
    pub fn cancel_dir_calc(&mut self) {
        if let Some(cancel) = self.dir_calc_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.dir_calc_rx = None;
    }

    pub fn clear(&mut self) {
        self.cancel_dir_calc();
        self.viewer.stop_find();
        self.pending_target = None;
        self.pending_since = None;
        self.loaded_path = None;
        self.is_dir_summary = false;
        self.dir_name.clear();
        self.dir_files = 0;
        self.dir_dirs = 0;
        self.dir_bytes = 0;
        self.placeholder_msg = None;
        self.viewer.is_open = false;
        self.viewer.path = None;
        self.viewer.chunk.clear();
        self.viewer.content.clear();
        self.viewer.image_texture = None;
        self.viewer.image_loader = None;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.clear();
        }
    }

    pub fn request_preview(&mut self, target: Option<(PathBuf, String, bool, bool)>) {
        let Some((path, name, is_dir, is_parent)) = target else {
            if self.loaded_path.is_some() || self.pending_target.is_some() {
                self.clear();
            }
            return;
        };

        if is_parent {
            if self.loaded_path.as_ref() != Some(&path) {
                self.clear();
                self.loaded_path = Some(path);
                self.placeholder_msg = Some("..".to_string());
            }
            return;
        }

        let already_loaded = self.loaded_path.as_ref() == Some(&path);
        let already_pending = self
            .pending_target
            .as_ref()
            .is_some_and(|(p, _, _)| p == &path);

        if !already_loaded && !already_pending {
            self.pending_target = Some((path, name, is_dir));
            self.pending_since = Some(Instant::now());
        }
    }

    pub fn poll(
        &mut self,
        ctx: &egui::Context,
        i18n: &crate::i18n::I18n,
        repaint_signal: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    ) {
        if !self.enabled {
            return;
        }

        if let (Some(since), Some(_)) = (self.pending_since, self.pending_target.as_ref()) {
            if since.elapsed() >= std::time::Duration::from_millis(100) {
                self.pending_since = None;
                if let Some((path, name, is_dir)) = self.pending_target.take() {
                    self.cancel_dir_calc();
                    self.viewer.stop_find();
                    self.loaded_path = Some(path.clone());
                    self.placeholder_msg = None;

                    if is_dir {
                        self.is_dir_summary = true;
                        self.dir_name = name;
                        self.dir_files = 0;
                        self.dir_dirs = 0;
                        self.dir_bytes = 0;
                        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let cancel_thr = std::sync::Arc::clone(&cancel);
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        std::thread::spawn(move || {
                            let mut total_size = 0u64;
                            let mut files = 0usize;
                            let mut dirs = 0usize;
                            for entry in walkdir::WalkDir::new(&path)
                                .min_depth(1)
                                .follow_links(false)
                                .into_iter()
                                .flatten()
                            {
                                if cancel_thr.load(std::sync::atomic::Ordering::Relaxed) {
                                    return;
                                }
                                if entry.file_type().is_dir() {
                                    dirs += 1;
                                } else if entry.file_type().is_file() {
                                    files += 1;
                                    if let Ok(m) = entry.metadata() {
                                        total_size = total_size.saturating_add(m.len());
                                    }
                                }
                            }
                            if !cancel_thr.load(std::sync::atomic::Ordering::Relaxed) {
                                let _ = tx.send((total_size, files, dirs));
                                if let Some(sig) = repaint_signal {
                                    sig();
                                }
                            }
                        });
                        self.dir_calc_rx = Some(rx);
                        self.dir_calc_cancel = Some(cancel);
                    } else {
                        self.is_dir_summary = false;
                        if let Err(e) = self.viewer.open_file(name, path) {
                            self.placeholder_msg = Some(e);
                        }
                    }
                }
            }
        }

        if let Some(ref rx) = self.dir_calc_rx {
            match rx.try_recv() {
                Ok((bytes, files, dirs)) => {
                    self.dir_bytes = bytes;
                    self.dir_files = files;
                    self.dir_dirs = dirs;
                    self.dir_calc_rx = None;
                    self.dir_calc_cancel = None;
                    ctx.request_repaint();
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.dir_calc_rx = None;
                    self.dir_calc_cancel = None;
                }
            }
        }

        if !self.is_dir_summary && self.placeholder_msg.is_none() {
            self.viewer.poll_background(ctx, i18n);
        }
    }
}

#[derive(Default)]
pub struct FileOpsState {
    pub show_mkdir: bool,
    pub mkdir_input: String,

    pub show_new_file: bool,
    pub new_file_input: String,

    pub show_delete: bool,
    pub delete_targets: Vec<PathBuf>,
    pub delete_use_trash: bool,

    pub show_copy: bool,
    pub copy_sources: Vec<PathBuf>,
    pub copy_dest_path: String,

    pub show_move: bool,
    pub move_sources: Vec<PathBuf>,
    pub move_dest_path: String,

    pub show_rename: bool,
    pub rename_source: Option<PathBuf>,
    pub rename_input: String,

    pub show_pack_zip: bool,
    pub pack_zip_sources: Vec<PathBuf>,
    pub pack_zip_dest: String,

    pub show_mask_select: bool,
    pub mask_is_select: bool,
    pub mask_input: String,

    pub show_exit_confirm: bool,
}

#[derive(Default)]
pub struct ConflictDialogState {
    pub is_open: bool,
    pub source: Option<crate::worker::ConflictFileInfo>,
    pub destination: Option<crate::worker::ConflictFileInfo>,
    pub reply_tx: Option<crossbeam_channel::Sender<crate::worker::ConflictResolution>>,
    pub rename_input: String,
    pub show_rename_input: bool,
}

#[derive(Clone, Default)]
pub struct PropertiesDialogState {
    pub is_open: bool,
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub files_count: usize,
    pub dirs_count: usize,
    pub modified: Option<String>,
    pub readonly: bool,
    pub hidden: bool,
    pub is_calculating: bool,
    pub rx: Option<crossbeam_channel::Receiver<(u64, usize, usize)>>,
    pub checksums: Option<crate::checksum::FileChecksums>,
    pub checksum_calculating: bool,
    pub checksum_bytes_done: u64,
    pub checksum_bytes_total: u64,
    pub checksum_error: Option<String>,
    pub checksum_rx: Option<crossbeam_channel::Receiver<crate::checksum::ChecksumProgress>>,
    pub checksum_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub checksum_verify_input: String,
    pub checksum_copied_notice: Option<(crate::checksum::ChecksumAlgorithm, std::time::Instant)>,
}

impl PropertiesDialogState {
    pub fn cancel_checksum(&mut self) {
        if let Some(ref flag) = self.checksum_cancel {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.checksum_rx = None;
        self.checksum_cancel = None;
        self.checksum_calculating = false;
    }

    pub fn start_checksum_calculation(
        &mut self,
        repaint_signal: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    ) {
        self.cancel_checksum();
        self.checksums = None;
        self.checksum_error = None;
        self.checksum_bytes_done = 0;
        self.checksum_bytes_total = self.size;
        self.checksum_calculating = true;

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_thr = std::sync::Arc::clone(&cancel);
        let (tx, rx) = crossbeam_channel::unbounded();
        let path = self.path.clone();

        std::thread::spawn(move || {
            let tx_prog = tx.clone();
            let sig_prog = repaint_signal.clone();
            let res = crate::checksum::compute_file_checksums(&path, &cancel_thr, |done, total| {
                let _ = tx_prog.send(crate::checksum::ChecksumProgress::Progress {
                    bytes_done: done,
                    total_bytes: total,
                });
                if let Some(ref sig) = sig_prog {
                    sig();
                }
            });

            if cancel_thr.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            match res {
                Ok(hashes) => {
                    let _ = tx.send(crate::checksum::ChecksumProgress::Done(hashes));
                }
                Err(e) => {
                    let _ = tx.send(crate::checksum::ChecksumProgress::Error(e));
                }
            }
            if let Some(ref sig) = repaint_signal {
                sig();
            }
        });

        self.checksum_rx = Some(rx);
        self.checksum_cancel = Some(cancel);
    }

    pub fn poll_checksum(&mut self) -> bool {
        let mut updated = false;
        let mut finished = false;
        if let Some(ref rx) = self.checksum_rx {
            loop {
                match rx.try_recv() {
                    Ok(crate::checksum::ChecksumProgress::Progress {
                        bytes_done,
                        total_bytes,
                    }) => {
                        self.checksum_bytes_done = bytes_done;
                        if total_bytes > 0 {
                            self.checksum_bytes_total = total_bytes;
                        }
                        updated = true;
                    }
                    Ok(crate::checksum::ChecksumProgress::Done(hashes)) => {
                        self.checksums = Some(hashes);
                        self.checksum_error = None;
                        self.checksum_calculating = false;
                        finished = true;
                        updated = true;
                        break;
                    }
                    Ok(crate::checksum::ChecksumProgress::Error(e)) => {
                        self.checksum_error = Some(e);
                        self.checksum_calculating = false;
                        finished = true;
                        updated = true;
                        break;
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        self.checksum_calculating = false;
                        finished = true;
                        updated = true;
                        break;
                    }
                }
            }
        }
        if finished {
            self.checksum_rx = None;
            self.checksum_cancel = None;
        }
        updated
    }
}

#[derive(Debug, Clone)]
pub struct InternalDragPayload {
    pub source_side: crate::app::PanelSide,
    pub sources: Vec<PathBuf>,
    pub origin_pos: egui::Pos2,
    pub is_dragging: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DragDropState {
    pub active_drag: Option<InternalDragPayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigTab {
    #[default]
    Editors,
    Appearance,
    Behavior,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigDialogState {
    pub is_open: bool,
    pub active_tab: ConfigTab,
    pub language: Option<String>,
    pub default_editor: String,
    pub associations: Vec<(String, String)>,
    pub new_ext: String,
    pub new_editor: String,
    pub appearance: crate::config::AppearanceConfig,
    pub behavior: crate::config::BehaviorConfig,
    pub original_appearance: crate::config::AppearanceConfig,
}

impl ConfigDialogState {
    pub fn open_with_config(&mut self, config: &crate::config::AppConfig) {
        self.is_open = true;
        self.active_tab = ConfigTab::Editors;
        self.language = config.language.clone();
        self.default_editor = config.default_editor.clone();
        let mut list: Vec<(String, String)> = config
            .associations
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        self.associations = list;
        self.new_ext.clear();
        self.new_editor.clear();
        self.appearance = config.appearance.clone();
        self.behavior = config.behavior.clone();
        self.original_appearance = config.appearance.clone();
    }
}

pub struct ProgressUiState {
    pub is_open: bool,
    pub in_background: bool,
    pub title: String,
    pub current_item: String,
    pub items_done: usize,
    pub items_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub file_bytes_done: u64,
    pub file_bytes_total: u64,
    pub speed_bps: f32,
    pub eta_secs: Option<u64>,
    pub speed_history: VecDeque<f32>,
    pub start_time: Option<Instant>,

    // Queue UI
    pub show_queue_dialog: bool,
    pub queue_bytes_done: u64,
    pub queue_total_bytes: u64,
}

impl Default for ProgressUiState {
    fn default() -> Self {
        Self {
            is_open: false,
            in_background: false,
            title: String::new(),
            current_item: String::new(),
            items_done: 0,
            items_total: 0,
            bytes_done: 0,
            bytes_total: 0,
            file_bytes_done: 0,
            file_bytes_total: 0,
            speed_bps: 0.0,
            eta_secs: None,
            speed_history: VecDeque::with_capacity(30),
            start_time: None,
            show_queue_dialog: false,
            queue_bytes_done: 0,
            queue_total_bytes: 0,
        }
    }
}

#[derive(Default)]
pub struct DriveMenuState {
    pub side: Option<PanelSide>,
    pub cursor_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AboutTab {
    #[default]
    Overview,
    License,
    Libraries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LicenseViewKind {
    #[default]
    Mit,
    Apache2,
}

pub struct AboutDialogState {
    pub is_open: bool,
    pub current_tab: AboutTab,
    pub selected_license: LicenseViewKind,
    pub copy_notice_timer: Option<Instant>,
}

impl Default for AboutDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            current_tab: AboutTab::Overview,
            selected_license: LicenseViewKind::Mit,
            copy_notice_timer: None,
        }
    }
}

impl AboutDialogState {
    pub fn open_overview(&mut self) {
        self.is_open = true;
        self.current_tab = AboutTab::Overview;
    }

    pub fn open_license(&mut self) {
        self.is_open = true;
        self.current_tab = AboutTab::License;
    }

    pub fn open_libraries(&mut self) {
        self.is_open = true;
        self.current_tab = AboutTab::Libraries;
    }
}

#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Item {
        index: usize,
        item: crate::fs_item::FileItem,
        selected_count: usize,
    },
    Background {
        dir_path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextMenuSubmenu {
    #[default]
    None,
    SortBy,
    NewItem,
}

pub struct ContextMenuState {
    pub is_open: bool,
    pub pos: egui::Pos2,
    pub side: PanelSide,
    pub target: Option<ContextMenuTarget>,
    pub in_zip: bool,
    pub can_paste: bool,
    pub has_selection: bool,
    pub show_hidden: bool,
    pub sort_column: crate::fs_item::SortColumn,
    pub sort_order: crate::fs_item::SortOrder,
    pub open_submenu: ContextMenuSubmenu,
    pub submenu_anchor_y: f32,
    pub menu_rect: Option<egui::Rect>,
    pub submenu_rect: Option<egui::Rect>,
    pub last_menu_size: egui::Vec2,
    pub opened_this_frame: bool,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self {
            is_open: false,
            pos: egui::Pos2::ZERO,
            side: PanelSide::Left,
            target: None,
            in_zip: false,
            can_paste: false,
            has_selection: false,
            show_hidden: true,
            sort_column: crate::fs_item::SortColumn::Name,
            sort_order: crate::fs_item::SortOrder::Ascending,
            open_submenu: ContextMenuSubmenu::None,
            submenu_anchor_y: 0.0,
            menu_rect: None,
            submenu_rect: None,
            last_menu_size: egui::vec2(250.0, 320.0),
            opened_this_frame: false,
        }
    }
}

impl ContextMenuState {
    #[allow(clippy::too_many_arguments)]
    pub fn open_for_item(
        &mut self,
        side: PanelSide,
        pos: egui::Pos2,
        index: usize,
        item: crate::fs_item::FileItem,
        selected_count: usize,
        in_zip: bool,
        can_paste: bool,
        show_hidden: bool,
        sort_column: crate::fs_item::SortColumn,
        sort_order: crate::fs_item::SortOrder,
    ) {
        self.is_open = true;
        self.pos = pos;
        self.side = side;
        self.target = Some(ContextMenuTarget::Item {
            index,
            item,
            selected_count,
        });
        self.in_zip = in_zip;
        self.can_paste = can_paste;
        self.has_selection = selected_count > 0;
        self.show_hidden = show_hidden;
        self.sort_column = sort_column;
        self.sort_order = sort_order;
        self.open_submenu = ContextMenuSubmenu::None;
        self.submenu_anchor_y = pos.y;
        self.menu_rect = None;
        self.submenu_rect = None;
        self.opened_this_frame = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_for_background(
        &mut self,
        side: PanelSide,
        pos: egui::Pos2,
        dir_path: PathBuf,
        in_zip: bool,
        can_paste: bool,
        has_selection: bool,
        show_hidden: bool,
        sort_column: crate::fs_item::SortColumn,
        sort_order: crate::fs_item::SortOrder,
    ) {
        self.is_open = true;
        self.pos = pos;
        self.side = side;
        self.target = Some(ContextMenuTarget::Background { dir_path });
        self.in_zip = in_zip;
        self.can_paste = can_paste;
        self.has_selection = has_selection;
        self.show_hidden = show_hidden;
        self.sort_column = sort_column;
        self.sort_order = sort_order;
        self.open_submenu = ContextMenuSubmenu::None;
        self.submenu_anchor_y = pos.y;
        self.menu_rect = None;
        self.submenu_rect = None;
        self.opened_this_frame = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.target = None;
        self.open_submenu = ContextMenuSubmenu::None;
        self.menu_rect = None;
        self.submenu_rect = None;
        self.opened_this_frame = false;
    }

    pub fn contains_pos(&self, pos: egui::Pos2) -> bool {
        if !self.is_open {
            return false;
        }
        let in_main = self
            .menu_rect
            .map(|r| r.expand(2.0).contains(pos))
            .unwrap_or(false);
        let in_sub = self
            .submenu_rect
            .map(|r| r.expand(2.0).contains(pos))
            .unwrap_or(false);
        in_main || in_sub
    }
}

#[derive(Debug, Default)]
pub struct BookmarksDialogState {
    pub is_open: bool,
    pub cursor_idx: usize,
    pub new_name: String,
    pub new_path: String,
    pub editing_idx: Option<usize>,
}

impl BookmarksDialogState {
    pub fn open_for_dir(&mut self, active_dir: &std::path::Path) {
        self.is_open = true;
        self.editing_idx = None;
        self.new_path = active_dir.to_string_lossy().to_string();
        let default_name = active_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.new_path.clone());
        self.new_name = default_name;
    }
}

#[derive(Debug, Default)]
pub struct MultiRenameDialogState {
    pub is_open: bool,
    pub items: Vec<crate::fs_item::FileItem>,
    pub config: crate::multi_rename::MultiRenameConfig,
    pub last_batch: Vec<(PathBuf, PathBuf)>,
}

#[derive(Debug, Default)]
pub struct DiffDialogState {
    pub is_open: bool,
    pub left_path: Option<PathBuf>,
    pub right_path: Option<PathBuf>,
    pub ignore_whitespace: bool,
    pub result: Option<crate::diff::DiffResult>,
    pub error: Option<String>,
    pub loading_rx: Option<crossbeam_channel::Receiver<Result<crate::diff::DiffResult, String>>>,
    pub current_diff_idx: usize,
    pub scroll_to_row: Option<usize>,
}

impl DiffDialogState {
    pub fn poll(&mut self) -> bool {
        if let Some(ref rx) = self.loading_rx {
            match rx.try_recv() {
                Ok(Ok(res)) => {
                    self.loading_rx = None;
                    self.error = None;
                    self.current_diff_idx = 0;
                    self.scroll_to_row = res.diff_row_indices.first().copied();
                    self.result = Some(res);
                    return true;
                }
                Ok(Err(err)) => {
                    self.loading_rx = None;
                    self.result = None;
                    self.error = Some(err);
                    return true;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.loading_rx = None;
                    return true;
                }
            }
        }
        false
    }
}
