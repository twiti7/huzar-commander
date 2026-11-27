use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::fs_item::FileItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipLocation {
    pub archive_path: PathBuf,
    pub internal_dir: String, // e.g. "" or "sub/folder/"
}

impl ZipLocation {
    pub fn new(archive_path: PathBuf) -> Self {
        Self {
            archive_path,
            internal_dir: String::new(),
        }
    }

    pub fn enter_subdir(&mut self, subdir: &str) {
        let clean = subdir.trim_matches('/');
        if self.internal_dir.is_empty() {
            self.internal_dir = format!("{}/", clean);
        } else {
            self.internal_dir = format!("{}{}/", self.internal_dir, clean);
        }
    }

    pub fn go_up(&mut self) -> bool {
        if self.internal_dir.is_empty() {
            // Already at the root of the archive -> should exit archive
            return false;
        }

        let trimmed = self.internal_dir.trim_end_matches('/');
        if let Some(pos) = trimmed.rfind('/') {
            self.internal_dir = format!("{}/", &trimmed[..=pos]);
        } else {
            self.internal_dir.clear();
        }
        true
    }

    pub fn display_path(&self) -> String {
        let tag = crate::archive::ArchiveFormat::from_path(&self.archive_path)
            .map(|f| f.tag())
            .unwrap_or("ZIP");
        let name = self
            .archive_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "archiwum.zip".to_string());

        if self.internal_dir.is_empty() {
            format!("{} [{}]", name, tag)
        } else {
            format!("{} [{}]/{}", name, tag, self.internal_dir)
        }
    }
}

pub fn is_zip_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .map(|e| e.to_string_lossy().eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
}

/// Validates and normalizes a relative path from a ZIP entry, rejecting traversal, absolute paths, drive prefixes, and NUL bytes.
pub fn sanitize_zip_rel_path(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }

    let normalized = raw.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }

    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return None;
    }

    let mut out = PathBuf::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            std::path::Component::Normal(part) => out.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return None,
        }
    }

    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

pub fn list_zip_entries(
    archive_path: &Path,
    internal_dir: &str,
) -> Result<Vec<FileItem>, String> {
    let file = File::open(archive_path).map_err(|e| format!("Błąd otwarcia ZIP: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Nieprawidłowe archiwum ZIP: {}", e))?;

    let norm_prefix = if internal_dir.is_empty() {
        String::new()
    } else if !internal_dir.ends_with('/') {
        format!("{}/", internal_dir)
    } else {
        internal_dir.to_string()
    };

    let mut dirs_seen = HashSet::new();
    let mut items = Vec::new();

    for i in 0..archive.len() {
        let Ok(file_entry) = archive.by_index(i) else {
            continue;
        };

        let Ok(name_cow) = file_entry.name() else {
            continue;
        };
        let raw_name = name_cow.replace('\\', "/");

        // Check if inside current directory
        if !norm_prefix.is_empty() && !raw_name.starts_with(&norm_prefix) {
            continue;
        }

        let rel_name = if norm_prefix.is_empty() {
            &raw_name[..]
        } else {
            &raw_name[norm_prefix.len()..]
        };

        if rel_name.is_empty() {
            continue;
        }

        let is_dir_entry = file_entry.is_dir() || rel_name.ends_with('/');
        let trimmed_rel = rel_name.trim_matches('/');

        if trimmed_rel.is_empty() {
            continue;
        }

        if let Some(slash_idx) = trimmed_rel.find('/') {
            // It is an entry in a sub-subdirectory -> add top-level subfolder if not seen
            let dir_name = &trimmed_rel[..slash_idx];
            if dirs_seen.insert(dir_name.to_string()) {
                let virtual_path = archive_path.join(&norm_prefix).join(dir_name);
                items.push(FileItem {
                    name: dir_name.to_string(),
                    ext: String::new(),
                    path: virtual_path,
                    is_dir: true,
                    is_parent: false,
                    size: 0,
                    modified: None,
                    is_hidden: false,
                    is_readonly: false,
                    is_symlink: false,
                });
            }
        } else if is_dir_entry {
            let dir_name = trimmed_rel;
            if dirs_seen.insert(dir_name.to_string()) {
                let virtual_path = archive_path.join(&norm_prefix).join(dir_name);
                items.push(FileItem {
                    name: dir_name.to_string(),
                    ext: String::new(),
                    path: virtual_path,
                    is_dir: true,
                    is_parent: false,
                    size: 0,
                    modified: None,
                    is_hidden: false,
                    is_readonly: false,
                    is_symlink: false,
                });
            }
        } else {
            // It is a file in the current directory
            let file_name = trimmed_rel.to_string();
            let ext = Path::new(&file_name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let size = file_entry.size();
            let virtual_path = archive_path.join(&norm_prefix).join(&file_name);

            items.push(FileItem {
                name: file_name,
                ext,
                path: virtual_path,
                is_dir: false,
                is_parent: false,
                size,
                modified: None,
                is_hidden: false,
                is_readonly: false,
                is_symlink: false,
            });
        }
    }

    Ok(items)
}

pub fn read_zip_stream(
    archive_path: &Path,
    internal_name: &str,
    max_bytes: usize,
) -> Result<String, String> {
    let file = File::open(archive_path).map_err(|e| format!("Błąd otwarcia ZIP: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Nieprawidłowe archiwum ZIP: {}", e))?;

    let clean_target = internal_name.replace('\\', "/");
    let clean_trimmed = clean_target.trim_start_matches('/');

    let entry_idx = (0..archive.len()).find(|&i| {
        if let Ok(e) = archive.by_index(i) {
            if let Ok(name_cow) = e.name() {
                let en = name_cow.replace('\\', "/");
                return en == clean_target || en == clean_trimmed;
            }
        }
        false
    });

    let mut entry = match entry_idx {
        Some(idx) => archive
            .by_index(idx)
            .map_err(|e| format!("Błąd odczytu wpisu ZIP: {}", e))?,
        None => return Err(format!("Nie znaleziono wpisu '{}' w archiwum ZIP", internal_name)),
    };

    let total_size = entry.size();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    while buf.len() < max_bytes {
        let bytes_to_read = (max_bytes - buf.len()).min(chunk.len());
        let n = entry
            .read(&mut chunk[..bytes_to_read])
            .map_err(|e| format!("Błąd odczytu z ZIP: {}", e))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    let is_truncated = total_size > buf.len() as u64;
    let is_binary = buf.iter().take(4096).any(|&b| b == 0);

    if is_binary {
        Ok(crate::opener::format_hex_dump(&buf, total_size, is_truncated))
    } else {
        let text = String::from_utf8_lossy(&buf).to_string();
        if is_truncated {
            Ok(format!(
                "{}\n\n--- [Podgląd ZIP ucięto: wczytano {} z {}] ---",
                text,
                humansize::format_size(buf.len(), humansize::DECIMAL),
                humansize::format_size(total_size, humansize::DECIMAL)
            ))
        } else {
            Ok(text)
        }
    }
}

#[allow(dead_code)]
pub fn read_zip_text(
    archive_path: &Path,
    internal_name: &str,
    max_bytes: usize,
) -> Result<String, String> {
    read_zip_stream(archive_path, internal_name, max_bytes)
}

pub fn extract_single_zip_entry(
    archive_path: &Path,
    internal_name: &str,
    target_file: &Path,
) -> Result<u64, String> {
    let file = File::open(archive_path).map_err(|e| format!("Błąd otwarcia ZIP: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Nieprawidłowe archiwum ZIP: {}", e))?;

    let clean_target = internal_name.replace('\\', "/");
    let clean_trimmed = clean_target.trim_start_matches('/');

    let entry_idx = (0..archive.len()).find(|&i| {
        if let Ok(e) = archive.by_index(i) {
            if let Ok(name_cow) = e.name() {
                let en = name_cow.replace('\\', "/");
                return en == clean_target || en == clean_trimmed;
            }
        }
        false
    });

    let mut entry = match entry_idx {
        Some(idx) => archive
            .by_index(idx)
            .map_err(|e| format!("Błąd odczytu wpisu ZIP: {}", e))?,
        None => return Err(format!("Nie znaleziono wpisu '{}' w archiwum ZIP", internal_name)),
    };

    if let Some(parent) = target_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Błąd tworzenia katalogu tymczasowego: {}", e))?;
    }

    let mut out = File::create(target_file)
        .map_err(|e| format!("Błąd tworzenia pliku wyjściowego: {}", e))?;
    let bytes = std::io::copy(&mut entry, &mut out)
        .map_err(|e| format!("Błąd wypakowywania z archiwum ZIP: {}", e))?;
    Ok(bytes)
}

pub fn extract_zip(
    archive_path: &Path,
    target_dir: &Path,
    filter_prefix: Option<&str>,
    cancel: &Arc<AtomicBool>,
    progress_cb: impl FnMut(&str, u64, u64),
) -> Result<usize, String> {
    extract_zip_entries(archive_path, target_dir, filter_prefix, None, cancel, progress_cb)
}

pub fn extract_zip_entries(
    archive_path: &Path,
    target_dir: &Path,
    filter_prefix: Option<&str>,
    selected_entries: Option<&[String]>,
    cancel: &Arc<AtomicBool>,
    mut progress_cb: impl FnMut(&str, u64, u64),
) -> Result<usize, String> {
    let file = File::open(archive_path).map_err(|e| format!("Błąd otwarcia ZIP: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Nieprawidłowe archiwum ZIP: {}", e))?;

    let mut extracted_count = 0;
    let total_entries = archive.len();
    let normalized_selected: Vec<String> = selected_entries
        .unwrap_or(&[])
        .iter()
        .map(|s| s.replace('\\', "/").trim_start_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Błąd tworzenia katalogu docelowego: {}", e))?;

    for i in 0..total_entries {
        if cancel.load(Ordering::Relaxed) {
            return Err("Operacja anulowana przez użytkownika".to_string());
        }

        let mut file_entry = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let Ok(name_cow) = file_entry.name() else {
            continue;
        };
        let raw_name = name_cow.replace('\\', "/");

        if sanitize_zip_rel_path(&raw_name).is_none() {
            continue;
        }

        if let Some(prefix) = filter_prefix {
            if !prefix.is_empty() && !raw_name.starts_with(prefix) {
                continue;
            }
        }

        if !normalized_selected.is_empty() {
            let trimmed_raw = raw_name.trim_start_matches('/');
            let matches_selection = normalized_selected.iter().any(|sel| {
                let sel_dir = sel.trim_end_matches('/');
                trimmed_raw == sel.as_str()
                    || trimmed_raw.starts_with(&format!("{}/", sel_dir))
            });
            if !matches_selection {
                continue;
            }
        }

        let rel_path = if let Some(prefix) = filter_prefix {
            raw_name.strip_prefix(prefix).unwrap_or(&raw_name)
        } else {
            &raw_name
        };

        let rel_path = rel_path.trim_start_matches('/');
        if rel_path.is_empty() {
            continue;
        }

        // Prevent path traversal vulnerability (Zip Slip)
        let Some(safe_rel) = sanitize_zip_rel_path(rel_path) else {
            continue;
        };
        let out_path = target_dir.join(&safe_rel);

        if file_entry.is_dir() || raw_name.ends_with('/') {
            let _ = fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let mut outfile = File::create(&out_path)
                .map_err(|e| format!("Błąd zapisu {}: {}", out_path.display(), e))?;

            let mut buf = [0u8; 64 * 1024];
            let mut file_written = 0u64;
            let file_size = file_entry.size();

            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Operacja anulowana".to_string());
                }

                let n = file_entry
                    .read(&mut buf)
                    .map_err(|e| format!("Błąd odczytu z archiwum: {}", e))?;
                if n == 0 {
                    break;
                }
                outfile
                    .write_all(&buf[..n])
                    .map_err(|e| format!("Błąd zapisu pliku: {}", e))?;
                file_written += n as u64;
                progress_cb(rel_path, file_written, file_size);
            }
            extracted_count += 1;
        }
    }

    Ok(extracted_count)
}

pub fn create_zip_archive(
    sources: &[PathBuf],
    archive_path: &Path,
    cancel: &Arc<AtomicBool>,
    mut progress_cb: impl FnMut(&str, u64, u64),
) -> Result<usize, String> {
    if let Some(parent) = archive_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = File::create(archive_path)
        .map_err(|e| format!("Błąd tworzenia archiwum ZIP {}: {}", archive_path.display(), e))?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut total_added = 0;

    for src in sources {
        if cancel.load(Ordering::Relaxed) {
            return Err("Operacja pakowania anulowana".to_string());
        }

        let base_name = src
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "plik".to_string());

        if src.is_file() {
            let mut infile = File::open(src)
                .map_err(|e| format!("Błąd otwarcia {}: {}", src.display(), e))?;
            let file_size = infile.metadata().map(|m| m.len()).unwrap_or(0);
            zip_writer
                .start_file(&base_name, options)
                .map_err(|e| format!("Błąd zapisu do ZIP: {}", e))?;

            let mut buf = [0u8; 64 * 1024];
            let mut written = 0u64;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Operacja pakowania anulowana".to_string());
                }
                let n = infile.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                zip_writer
                    .write_all(&buf[..n])
                    .map_err(|e| e.to_string())?;
                written += n as u64;
                progress_cb(&base_name, written, file_size);
            }
            total_added += 1;
        } else if src.is_dir() {
            let walker = walkdir::WalkDir::new(src).follow_links(false);
            for entry in walker.into_iter().flatten() {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Operacja pakowania anulowana".to_string());
                }
                let rel = match entry.path().strip_prefix(src) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let entry_path_in_zip = if rel_str.is_empty() {
                    format!("{}/", base_name)
                } else {
                    format!("{}/{}", base_name, rel_str)
                };

                if entry.file_type().is_dir() {
                    let dir_zip_name = if entry_path_in_zip.ends_with('/') {
                        entry_path_in_zip
                    } else {
                        format!("{}/", entry_path_in_zip)
                    };
                    let _ = zip_writer.add_directory(dir_zip_name, options);
                } else if entry.file_type().is_file() {
                    let mut infile = match File::open(entry.path()) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    let file_size = infile.metadata().map(|m| m.len()).unwrap_or(0);
                    if zip_writer.start_file(&entry_path_in_zip, options).is_err() {
                        continue;
                    }
                    let mut buf = [0u8; 64 * 1024];
                    let mut written = 0u64;
                    loop {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("Operacja pakowania anulowana".to_string());
                        }
                        let n = match infile.read(&mut buf) {
                            Ok(n) if n > 0 => n,
                            _ => break,
                        };
                        if zip_writer.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        written += n as u64;
                        progress_cb(&entry_path_in_zip, written, file_size);
                    }
                    total_added += 1;
                }
            }
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Błąd finalizacji ZIP: {}", e))?;
    Ok(total_added)
}
