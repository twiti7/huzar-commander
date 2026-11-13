use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Ext,
    Size,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let metadata = path.metadata().ok()?;
        let is_dir = metadata.is_dir();
        let name = path.file_name()?.to_string_lossy().to_string();

        let ext = if is_dir {
            String::new()
        } else {
            path.extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };

        let size = if is_dir { 0 } else { metadata.len() };
        let modified = metadata.modified().ok();
        let is_readonly = metadata.permissions().readonly();
        #[cfg(windows)]
        let is_hidden = {
            use std::os::windows::fs::MetadataExt;
            name.starts_with('.') || (metadata.file_attributes() & 0x0000_0002 != 0)
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
        })
    }
}
