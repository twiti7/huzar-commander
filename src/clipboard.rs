use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

/// Koduje listę ścieżek do bufora UTF-16 zakończonego podwójnym znakiem NUL (format `DROPFILES` / `CF_HDROP`).
pub fn encode_dropfiles_wide(paths: &[PathBuf]) -> Vec<u16> {
    let mut out = Vec::new();
    for p in paths {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            out.extend(p.as_os_str().encode_wide());
        }
        #[cfg(not(windows))]
        {
            out.extend(p.to_string_lossy().encode_utf16());
        }
        out.push(0);
    }
    if out.is_empty() {
        out.push(0);
    }
    out.push(0);
    out
}

/// Dekoduje listę ścieżek z bufora UTF-16 zakończonego podwójnym znakiem NUL.
pub fn decode_dropfiles_wide(wide: &[u16]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < wide.len() {
        let rel_end = wide[start..].iter().position(|&c| c == 0);
        let end = match rel_end {
            Some(len) => start + len,
            None => wide.len(),
        };
        if end == start {
            break;
        }
        let slice = &wide[start..end];
        #[cfg(windows)]
        {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;
            result.push(PathBuf::from(OsString::from_wide(slice)));
        }
        #[cfg(not(windows))]
        {
            result.push(PathBuf::from(String::from_utf16_lossy(slice)));
        }
        start = end + 1;
    }
    result
}

#[derive(Debug, Default, Clone)]
pub struct FileClipboard {
    internal: Option<(Vec<PathBuf>, ClipboardOp)>,
}

impl FileClipboard {
    pub fn set_files_memory(&mut self, paths: Vec<PathBuf>, op: ClipboardOp) {
        if paths.is_empty() {
            self.internal = None;
        } else {
            self.internal = Some((paths, op));
        }
    }

    pub fn get_files_memory(&self) -> Option<(Vec<PathBuf>, ClipboardOp)> {
        self.internal.clone()
    }

    pub fn clear_memory(&mut self) {
        self.internal = None;
    }

    pub fn cut_paths(&self) -> Option<&[PathBuf]> {
        match &self.internal {
            Some((paths, ClipboardOp::Cut)) if !paths.is_empty() => Some(paths.as_slice()),
            _ => None,
        }
    }

    pub fn set_files(&mut self, paths: Vec<PathBuf>, op: ClipboardOp) -> Result<(), String> {
        if paths.is_empty() {
            self.internal = None;
            return Ok(());
        }

        #[cfg(windows)]
        {
            let res = win32::set_clipboard_files(&paths, op);
            self.internal = Some((paths, op));
            res
        }

        #[cfg(not(windows))]
        {
            self.internal = Some((paths, op));
            Ok(())
        }
    }

    pub fn get_files(&self) -> Option<(Vec<PathBuf>, ClipboardOp)> {
        #[cfg(windows)]
        {
            match win32::get_clipboard_files() {
                win32::ClipboardReadResult::Files(paths, op) => Some((paths, op)),
                win32::ClipboardReadResult::Empty => None,
                win32::ClipboardReadResult::OpenFailed => self.internal.clone(),
            }
        }

        #[cfg(not(windows))]
        {
            self.internal.clone()
        }
    }

    pub fn has_files(&self) -> bool {
        #[cfg(windows)]
        {
            win32::has_clipboard_files()
        }

        #[cfg(not(windows))]
        {
            self.internal
                .as_ref()
                .is_some_and(|(p, _)| !p.is_empty())
        }
    }

    pub fn clear(&mut self) {
        self.internal = None;
        #[cfg(windows)]
        {
            win32::clear_clipboard_files();
        }
    }
}

#[cfg(windows)]
mod win32 {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::ptr;

    use super::{encode_dropfiles_wide, ClipboardOp};

    const CF_HDROP: u32 = 15;
    const GMEM_MOVEABLE: u32 = 0x0002;
    const GMEM_ZEROINIT: u32 = 0x0040;
    const GHND: u32 = GMEM_MOVEABLE | GMEM_ZEROINIT;
    const DROPEFFECT_COPY: u32 = 1;
    const DROPEFFECT_MOVE: u32 = 2;

    #[repr(C)]
    struct DropFiles {
        p_files: u32,
        pt_x: i32,
        pt_y: i32,
        f_nc: i32,
        f_wide: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(h_wnd_new_owner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(
            u_format: u32,
            h_mem: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn GetClipboardData(u_format: u32) -> *mut std::ffi::c_void;
        fn IsClipboardFormatAvailable(format: u32) -> i32;
        fn RegisterClipboardFormatW(lpsz_format: *const u16) -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(u_flags: u32, dw_bytes: usize) -> *mut std::ffi::c_void;
        fn GlobalLock(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(h_mem: *mut std::ffi::c_void) -> i32;
        fn GlobalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn DragQueryFileW(
            h_drop: *mut std::ffi::c_void,
            i_file: u32,
            lpsz_file: *mut u16,
            cch: u32,
        ) -> u32;
    }

    struct ClipboardGuard;

    impl ClipboardGuard {
        fn open() -> Option<Self> {
            if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
                Some(Self)
            } else {
                None
            }
        }
    }

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    pub enum ClipboardReadResult {
        Files(Vec<PathBuf>, ClipboardOp),
        Empty,
        OpenFailed,
    }

    fn preferred_drop_effect_format() -> u32 {
        let name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
        unsafe { RegisterClipboardFormatW(name.as_ptr()) }
    }

    fn alloc_global_bytes(data: &[u8]) -> Result<*mut std::ffi::c_void, String> {
        let h_mem = unsafe { GlobalAlloc(GHND, data.len()) };
        if h_mem.is_null() {
            return Err("GlobalAlloc nie powiodło się".to_string());
        }
        let locked = unsafe { GlobalLock(h_mem) };
        if locked.is_null() {
            unsafe {
                GlobalFree(h_mem);
            }
            return Err("GlobalLock nie powiodło się".to_string());
        }
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), locked as *mut u8, data.len());
            GlobalUnlock(h_mem);
        }
        Ok(h_mem)
    }

    pub fn set_clipboard_files(paths: &[PathBuf], op: ClipboardOp) -> Result<(), String> {
        let _guard = ClipboardGuard::open()
            .ok_or_else(|| "Nie można otworzyć schowka systemowego".to_string())?;

        if unsafe { EmptyClipboard() } == 0 {
            return Err("EmptyClipboard nie powiodło się".to_string());
        }

        let wide = encode_dropfiles_wide(paths);
        let header = DropFiles {
            p_files: std::mem::size_of::<DropFiles>() as u32,
            pt_x: 0,
            pt_y: 0,
            f_nc: 0,
            f_wide: 1,
        };

        let mut payload = Vec::with_capacity(std::mem::size_of::<DropFiles>() + wide.len() * 2);
        payload.extend_from_slice(&header.p_files.to_ne_bytes());
        payload.extend_from_slice(&header.pt_x.to_ne_bytes());
        payload.extend_from_slice(&header.pt_y.to_ne_bytes());
        payload.extend_from_slice(&header.f_nc.to_ne_bytes());
        payload.extend_from_slice(&header.f_wide.to_ne_bytes());
        for w in wide {
            payload.extend_from_slice(&w.to_ne_bytes());
        }

        let h_drop = alloc_global_bytes(&payload)?;
        let res_drop = unsafe { SetClipboardData(CF_HDROP, h_drop) };
        if res_drop.is_null() {
            unsafe {
                GlobalFree(h_drop);
            }
            return Err("SetClipboardData (CF_HDROP) nie powiodło się".to_string());
        }

        let drop_effect_fmt = preferred_drop_effect_format();
        if drop_effect_fmt != 0 {
            let effect = match op {
                ClipboardOp::Copy => DROPEFFECT_COPY,
                ClipboardOp::Cut => DROPEFFECT_MOVE,
            };
            if let Ok(h_effect) = alloc_global_bytes(&effect.to_ne_bytes()) {
                let res_eff = unsafe { SetClipboardData(drop_effect_fmt, h_effect) };
                if res_eff.is_null() {
                    unsafe {
                        GlobalFree(h_effect);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn has_clipboard_files() -> bool {
        unsafe { IsClipboardFormatAvailable(CF_HDROP) != 0 }
    }

    pub fn get_clipboard_files() -> ClipboardReadResult {
        if !has_clipboard_files() {
            return ClipboardReadResult::Empty;
        }

        let _guard = match ClipboardGuard::open() {
            Some(g) => g,
            None => return ClipboardReadResult::OpenFailed,
        };

        let h_drop = unsafe { GetClipboardData(CF_HDROP) };
        if h_drop.is_null() {
            return ClipboardReadResult::Empty;
        }

        let count = unsafe { DragQueryFileW(h_drop, 0xFFFF_FFFF, ptr::null_mut(), 0) };
        if count == 0 {
            return ClipboardReadResult::Empty;
        }

        let mut paths = Vec::with_capacity(count as usize);
        for idx in 0..count {
            let len = unsafe { DragQueryFileW(h_drop, idx, ptr::null_mut(), 0) };
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied = unsafe { DragQueryFileW(h_drop, idx, buf.as_mut_ptr(), buf.len() as u32) };
            if copied > 0 {
                let os = OsString::from_wide(&buf[..copied as usize]);
                paths.push(PathBuf::from(os));
            }
        }

        if paths.is_empty() {
            return ClipboardReadResult::Empty;
        }

        let mut op = ClipboardOp::Copy;
        let drop_effect_fmt = preferred_drop_effect_format();
        if drop_effect_fmt != 0 && unsafe { IsClipboardFormatAvailable(drop_effect_fmt) } != 0 {
            let h_effect = unsafe { GetClipboardData(drop_effect_fmt) };
            if !h_effect.is_null() {
                let ptr = unsafe { GlobalLock(h_effect) } as *const u32;
                if !ptr.is_null() {
                    let effect = unsafe { *ptr };
                    unsafe {
                        GlobalUnlock(h_effect);
                    }
                    if (effect & DROPEFFECT_MOVE) != 0 {
                        op = ClipboardOp::Cut;
                    }
                }
            }
        }

        ClipboardReadResult::Files(paths, op)
    }

    pub fn clear_clipboard_files() {
        if has_clipboard_files() {
            if let Some(_guard) = ClipboardGuard::open() {
                unsafe {
                    EmptyClipboard();
                }
            }
        }
    }
}
