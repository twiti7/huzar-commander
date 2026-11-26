use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortColumn {
    Name,
    Ext,
    Size,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FileItem {
    pub name: String,
    pub ext: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_parent: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub is_hidden: bool,
    pub is_readonly: bool,
    pub is_symlink: bool,
}

/// Returns `true` if metadata describes a symbolic link or a Windows NTFS reparse point (junction).
pub fn is_symlink_or_reparse(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x0000_0400 != 0 {
            return true;
        }
    }
    false
}

impl FileItem {
    pub fn new_parent(parent_path: PathBuf) -> Self {
        Self {
            name: "..".to_string(),
            ext: String::new(),
            path: parent_path,
            is_dir: true,
            is_parent: true,
            size: 0,
            modified: None,
            is_hidden: false,
            is_readonly: false,
            is_symlink: false,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let sym_meta = std::fs::symlink_metadata(path).ok()?;
        let is_symlink = is_symlink_or_reparse(&sym_meta);
        let target_meta = if is_symlink {
            std::fs::metadata(path).ok()
        } else {
            None
        };

        let is_dir = target_meta
            .as_ref()
            .map(|m| m.is_dir())
            .unwrap_or_else(|| sym_meta.is_dir());
        let name = path.file_name()?.to_string_lossy().to_string();

        let ext = if is_dir {
            String::new()
        } else {
            path.extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };

        let size = if is_dir {
            0
        } else {
            target_meta
                .as_ref()
                .map(|m| m.len())
                .unwrap_or_else(|| sym_meta.len())
        };
        let modified = target_meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .or_else(|| sym_meta.modified().ok());
        let is_readonly = target_meta
            .as_ref()
            .map(|m| m.permissions().readonly())
            .unwrap_or_else(|| sym_meta.permissions().readonly());
        #[cfg(windows)]
        let is_hidden = {
            use std::os::windows::fs::MetadataExt;
            name.starts_with('.') || (sym_meta.file_attributes() & 0x0000_0002 != 0)
        };
        #[cfg(not(windows))]
        let is_hidden = name.starts_with('.');

        Some(Self {
            name,
            ext,
            path: path.to_path_buf(),
            is_dir,
            is_parent: false,
            size,
            modified,
            is_hidden,
            is_readonly,
            is_symlink,
        })
    }
}
