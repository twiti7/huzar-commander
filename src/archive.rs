use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::fs_item::FileItem;
use crate::zip_support::{
    extract_single_zip_entry, extract_zip_entries, list_zip_entries, sanitize_zip_rel_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
    SevenZ,
}

impl ArchiveFormat {
    /// Detects a supported archive format from its filename extension (`.zip`, `.tar`, `.tar.gz`, `.tgz`, `.7z`).
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if name.ends_with(".tar") {
            Some(Self::Tar)
        } else if name.ends_with(".7z") {
            Some(Self::SevenZ)
        } else if name.ends_with(".zip") {
            Some(Self::Zip)
        } else {
            None
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Self::Zip => "ZIP",
            Self::Tar => "TAR",
            Self::TarGz => "TAR.GZ",
            Self::SevenZ => "7Z",
        }
    }

    /// Strips `.tar.gz` cleanly or falls back to `file_stem` for single extensions.
    pub fn strip_archive_stem(path: &Path) -> String {
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".tar.gz") && name.len() > 7 {
                return name[..name.len() - 7].to_string();
            }
        }
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "archive".to_string())
    }
}

/// Returns `true` if `path` is an existing file in a supported browsable archive format.
pub fn is_supported_archive(path: &Path) -> bool {
    path.is_file() && ArchiveFormat::from_path(path).is_some()
}

struct RawArchiveEntry {
    raw_name: String,
    size: u64,
    is_dir: bool,
    modified: Option<SystemTime>,
}

fn normalize_archive_entry_name(raw: &str) -> String {
    let mut s = raw.replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    s.trim_start_matches('/').to_string()
}

fn project_virtual_dir(
    archive_path: &Path,
    internal_dir: &str,
    entries: impl IntoIterator<Item = RawArchiveEntry>,
) -> Vec<FileItem> {
    let prefix = if internal_dir.is_empty() {
        String::new()
    } else if internal_dir.ends_with('/') {
        normalize_archive_entry_name(internal_dir)
    } else {
        format!("{}/", normalize_archive_entry_name(internal_dir))
    };

    let mut dirs_seen: HashSet<String> = HashSet::new();
    let mut items: Vec<FileItem> = Vec::new();

    for entry in entries {
        let normalized = normalize_archive_entry_name(&entry.raw_name);
        if normalized.is_empty() || normalized == "." {
            continue;
        }
        let check_rel = normalized.trim_end_matches('/');
        if sanitize_zip_rel_path(check_rel).is_none() {
            continue;
        }

        if !normalized.starts_with(&prefix) {
            continue;
        }

        let rel = &normalized[prefix.len()..];
        if rel.is_empty() {
            continue;
        }

        if let Some(slash_pos) = rel.find('/') {
            let dir_name = &rel[..slash_pos];
            if !dir_name.is_empty() && !dirs_seen.contains(dir_name) {
                dirs_seen.insert(dir_name.to_string());
                let full_dir_path = if prefix.is_empty() {
                    format!("{}/", dir_name)
                } else {
                    format!("{}{}/", prefix, dir_name)
                };
                items.push(FileItem {
                    name: dir_name.to_string(),
                    ext: String::new(),
                    path: archive_path.join(full_dir_path),
                    is_dir: true,
                    is_parent: false,
                    size: 0,
                    modified: entry.modified,
                    is_hidden: false,
                    is_readonly: false,
                    is_symlink: false,
                });
            }
        } else if entry.is_dir {
            let dir_name = rel.trim_end_matches('/');
            if !dir_name.is_empty() && !dirs_seen.contains(dir_name) {
                dirs_seen.insert(dir_name.to_string());
                items.push(FileItem {
                    name: dir_name.to_string(),
                    ext: String::new(),
                    path: archive_path.join(&normalized),
                    is_dir: true,
                    is_parent: false,
                    size: 0,
                    modified: entry.modified,
                    is_hidden: false,
                    is_readonly: false,
                    is_symlink: false,
                });
            }
        } else {
            let ext = Path::new(rel)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            items.push(FileItem {
                name: rel.to_string(),
                ext,
                path: archive_path.join(&normalized),
                is_dir: false,
                is_parent: false,
                size: entry.size,
                modified: entry.modified,
                is_hidden: false,
                is_readonly: false,
                is_symlink: false,
            });
        }
    }

    items
}

fn tar_entries_from_reader<R: Read>(reader: R) -> Result<Vec<RawArchiveEntry>, String> {
    let mut archive = tar::Archive::new(reader);
    let mut out = Vec::new();
    let entries = archive
        .entries()
        .map_err(|e| format!("Błąd odczytu TAR: {}", e))?;

    for entry_res in entries {
        let entry = entry_res.map_err(|e| format!("Błąd wpisu TAR: {}", e))?;
        let path_cow = entry
            .path()
            .map_err(|e| format!("Nieprawidłowa ścieżka TAR: {}", e))?;
        let raw_name = path_cow.to_string_lossy().to_string();
        let header = entry.header();
        let entry_type = header.entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            continue;
        }
        let is_dir = entry_type.is_dir();
        let size = if is_dir { 0 } else { entry.size() };
        let modified = header
            .mtime()
            .ok()
            .filter(|&t| t > 0)
            .map(|secs| UNIX_EPOCH + Duration::from_secs(secs));
        out.push(RawArchiveEntry {
            raw_name,
            size,
            is_dir,
            modified,
        });
    }

    Ok(out)
}

/// Lists entries inside `.zip`, `.tar`, `.tar.gz`/`.tgz`, or `.7z` for virtual panel browsing.
pub fn list_archive_entries(archive_path: &Path, internal_dir: &str) -> Result<Vec<FileItem>, String> {
    let format = ArchiveFormat::from_path(archive_path)
        .ok_or_else(|| format!("Nieobsługiwany format archiwum: {}", archive_path.display()))?;

    match format {
        ArchiveFormat::Zip => list_zip_entries(archive_path, internal_dir),
        ArchiveFormat::Tar => {
            let file = File::open(archive_path)
                .map_err(|e| format!("Błąd otwarcia TAR: {}", e))?;
            let entries = tar_entries_from_reader(file)?;
            Ok(project_virtual_dir(archive_path, internal_dir, entries))
        }
        ArchiveFormat::TarGz => {
            let file = File::open(archive_path)
                .map_err(|e| format!("Błąd otwarcia TAR.GZ: {}", e))?;
            let gz = flate2::read::GzDecoder::new(file);
            let entries = tar_entries_from_reader(gz)?;
            Ok(project_virtual_dir(archive_path, internal_dir, entries))
        }
        ArchiveFormat::SevenZ => {
            let reader = sevenz_rust::SevenZReader::open(
                archive_path,
                sevenz_rust::Password::empty(),
            )
            .map_err(|e| format!("Błąd otwarcia 7Z: {}", e))?;
            let entries: Vec<RawArchiveEntry> = reader
                .archive()
                .files
                .iter()
                .map(|entry| RawArchiveEntry {
                    raw_name: entry.name().to_string(),
                    size: if entry.is_directory() { 0 } else { entry.size() },
                    is_dir: entry.is_directory(),
                    modified: None,
                })
                .collect();
            Ok(project_virtual_dir(archive_path, internal_dir, entries))
        }
    }
}

fn extract_single_from_tar<R: Read>(
    reader: R,
    internal_name: &str,
    target_file: &Path,
) -> Result<u64, String> {
    let target_norm = normalize_archive_entry_name(internal_name);
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| format!("Błąd odczytu TAR: {}", e))?;

    for entry_res in entries {
        let mut entry = entry_res.map_err(|e| format!("Błąd wpisu TAR: {}", e))?;
        let Ok(path_cow) = entry.path() else {
            continue;
        };
        let entry_norm = normalize_archive_entry_name(&path_cow.to_string_lossy());
        if entry_norm == target_norm && !entry.header().entry_type().is_dir() {
            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Błąd tworzenia katalogu tymczasowego: {}", e))?;
            }
            let mut out = File::create(target_file)
                .map_err(|e| format!("Błąd tworzenia pliku wyjściowego: {}", e))?;
            let bytes = std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Błąd wypakowywania z TAR: {}", e))?;
            return Ok(bytes);
        }
    }

    Err(format!(
        "Nie znaleziono wpisu '{}' w archiwum TAR",
        internal_name
    ))
}

/// Extracts a single file from `.zip`, `.tar`, `.tar.gz`/`.tgz`, or `.7z` to `target_file`.
pub fn extract_single_archive_entry(
    archive_path: &Path,
    internal_name: &str,
    target_file: &Path,
) -> Result<u64, String> {
    let format = ArchiveFormat::from_path(archive_path)
        .ok_or_else(|| format!("Nieobsługiwany format archiwum: {}", archive_path.display()))?;

    match format {
        ArchiveFormat::Zip => extract_single_zip_entry(archive_path, internal_name, target_file),
        ArchiveFormat::Tar => {
            let file = File::open(archive_path)
                .map_err(|e| format!("Błąd otwarcia TAR: {}", e))?;
            extract_single_from_tar(file, internal_name, target_file)
        }
        ArchiveFormat::TarGz => {
            let file = File::open(archive_path)
                .map_err(|e| format!("Błąd otwarcia TAR.GZ: {}", e))?;
            let gz = flate2::read::GzDecoder::new(file);
            extract_single_from_tar(gz, internal_name, target_file)
        }
        ArchiveFormat::SevenZ => {
            let target_norm = normalize_archive_entry_name(internal_name);
            let mut reader = sevenz_rust::SevenZReader::open(
                archive_path,
                sevenz_rust::Password::empty(),
            )
            .map_err(|e| format!("Błąd otwarcia 7Z: {}", e))?;
            let mut extracted_bytes: Option<u64> = None;

            reader
                .for_each_entries(|entry, stream| {
                    let entry_norm = normalize_archive_entry_name(entry.name());
                    if entry_norm == target_norm && !entry.is_directory() {
                        if let Some(parent) = target_file.parent() {
                            fs::create_dir_all(parent).map_err(|e| {
                                sevenz_rust::Error::other(format!(
                                    "Błąd tworzenia katalogu tymczasowego: {}",
                                    e
                                ))
                            })?;
                        }
                        let mut out = File::create(target_file).map_err(|e| {
                            sevenz_rust::Error::other(format!(
                                "Błąd tworzenia pliku wyjściowego: {}",
                                e
                            ))
                        })?;
                        let bytes = std::io::copy(stream, &mut out).map_err(|e| {
                            sevenz_rust::Error::other(format!(
                                "Błąd wypakowywania z 7Z: {}",
                                e
                            ))
                        })?;
                        extracted_bytes = Some(bytes);
                        Ok(false)
                    } else {
                        Ok(true)
                    }
                })
                .map_err(|e| format!("Błąd wypakowywania 7Z: {}", e))?;

            extracted_bytes.ok_or_else(|| {
                format!("Nie znaleziono wpisu '{}' w archiwum 7Z", internal_name)
            })
        }
    }
}

fn should_extract_entry(
    raw_name: &str,
    filter_prefix: Option<&str>,
    normalized_selected: &[String],
) -> Option<(PathBuf, String)> {
    let normalized = normalize_archive_entry_name(raw_name);
    let check_rel = normalized.trim_end_matches('/');
    sanitize_zip_rel_path(check_rel)?;

    if let Some(prefix) = filter_prefix {
        let p_norm = normalize_archive_entry_name(prefix);
        if !p_norm.is_empty() && !normalized.starts_with(&p_norm) {
            return None;
        }
    }

    if !normalized_selected.is_empty() {
        let trimmed_raw = normalized.trim_start_matches('/');
        let matches_selection = normalized_selected.iter().any(|sel| {
            let sel_dir = sel.trim_end_matches('/');
            trimmed_raw == sel.as_str() || trimmed_raw.starts_with(&format!("{}/", sel_dir))
        });
        if !matches_selection {
            return None;
        }
    }

    let rel_after_prefix = if let Some(prefix) = filter_prefix {
        let p_norm = normalize_archive_entry_name(prefix);
        normalized
            .strip_prefix(&p_norm)
            .unwrap_or(&normalized)
            .to_string()
    } else {
        normalized
    };

    let rel_clean = rel_after_prefix.trim_matches('/');
    if rel_clean.is_empty() {
        return None;
    }

    let safe_rel = sanitize_zip_rel_path(rel_clean)?;
    Some((safe_rel, rel_clean.to_string()))
}

fn normalize_selected_list(selected_entries: Option<&[String]>) -> Vec<String> {
    selected_entries
        .unwrap_or(&[])
        .iter()
        .map(|s| normalize_archive_entry_name(s))
        .filter(|s| !s.is_empty())
        .collect()
}

fn extract_tar_reader<R: Read>(
    reader: R,
    target_dir: &Path,
    filter_prefix: Option<&str>,
    normalized_selected: &[String],
    cancel: &AtomicBool,
    mut progress_cb: impl FnMut(&str, u64, u64),
) -> Result<usize, String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Błąd tworzenia katalogu docelowego: {}", e))?;

    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| format!("Błąd odczytu TAR: {}", e))?;
    let mut extracted_count = 0usize;

    for entry_res in entries {
        if cancel.load(Ordering::Relaxed) {
            return Err("Operacja anulowana przez użytkownika".to_string());
        }

        let mut entry = entry_res.map_err(|e| format!("Błąd wpisu TAR: {}", e))?;
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            continue;
        }

        let Ok(path_cow) = entry.path() else {
            continue;
        };
        let raw_name = path_cow.to_string_lossy().to_string();
        let Some((safe_rel, rel_display)) =
            should_extract_entry(&raw_name, filter_prefix, normalized_selected)
        else {
            continue;
        };

        let out_path = target_dir.join(&safe_rel);
        if entry_type.is_dir() {
            let _ = fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut outfile = File::create(&out_path)
                .map_err(|e| format!("Błąd zapisu {}: {}", out_path.display(), e))?;
            let mut buf = [0u8; 64 * 1024];
            let mut file_written = 0u64;
            let file_size = entry.size();
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Operacja anulowana".to_string());
                }
                let n = entry
                    .read(&mut buf)
                    .map_err(|e| format!("Błąd odczytu z TAR: {}", e))?;
                if n == 0 {
                    break;
                }
                outfile
                    .write_all(&buf[..n])
                    .map_err(|e| format!("Błąd zapisu pliku: {}", e))?;
                file_written += n as u64;
                progress_cb(&rel_display, file_written, file_size);
            }
            extracted_count += 1;
        }
    }

    Ok(extracted_count)
}

/// Extracts all or selected entries from `.zip`, `.tar`, `.tar.gz`/`.tgz`, or `.7z` with Zip-Slip protection.
pub fn extract_archive_entries(
    archive_path: &Path,
    target_dir: &Path,
    filter_prefix: Option<&str>,
    selected_entries: Option<&[String]>,
    cancel: &Arc<AtomicBool>,
    mut progress_cb: impl FnMut(&str, u64, u64),
) -> Result<usize, String> {
    let format = ArchiveFormat::from_path(archive_path)
        .ok_or_else(|| format!("Nieobsługiwany format archiwum: {}", archive_path.display()))?;

    match format {
        ArchiveFormat::Zip => extract_zip_entries(
            archive_path,
            target_dir,
            filter_prefix,
            selected_entries,
            cancel,
            progress_cb,
        ),
        ArchiveFormat::Tar => {
            let file = File::open(archive_path)
                .map_err(|e| format!("Błąd otwarcia TAR: {}", e))?;
            let normalized_selected = normalize_selected_list(selected_entries);
            extract_tar_reader(
                file,
                target_dir,
                filter_prefix,
                &normalized_selected,
                cancel,
                progress_cb,
            )
        }
        ArchiveFormat::TarGz => {
            let file = File::open(archive_path)
                .map_err(|e| format!("Błąd otwarcia TAR.GZ: {}", e))?;
            let gz = flate2::read::GzDecoder::new(file);
            let normalized_selected = normalize_selected_list(selected_entries);
            extract_tar_reader(
                gz,
                target_dir,
                filter_prefix,
                &normalized_selected,
                cancel,
                progress_cb,
            )
        }
        ArchiveFormat::SevenZ => {
            fs::create_dir_all(target_dir)
                .map_err(|e| format!("Błąd tworzenia katalogu docelowego: {}", e))?;
            let normalized_selected = normalize_selected_list(selected_entries);
            let mut reader = sevenz_rust::SevenZReader::open(
                archive_path,
                sevenz_rust::Password::empty(),
            )
            .map_err(|e| format!("Błąd otwarcia 7Z: {}", e))?;
            let mut extracted_count = 0usize;
            let mut cancelled = false;

            reader
                .for_each_entries(|entry, stream| {
                    if cancel.load(Ordering::Relaxed) {
                        cancelled = true;
                        return Ok(false);
                    }

                    let Some((safe_rel, rel_display)) =
                        should_extract_entry(entry.name(), filter_prefix, &normalized_selected)
                    else {
                        return Ok(true);
                    };

                    let out_path = target_dir.join(&safe_rel);
                    if entry.is_directory() {
                        let _ = fs::create_dir_all(&out_path);
                    } else {
                        if let Some(parent) = out_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let mut outfile = File::create(&out_path).map_err(|e| {
                            sevenz_rust::Error::other(format!(
                                "Błąd zapisu {}: {}",
                                out_path.display(),
                                e
                            ))
                        })?;
                        let mut buf = [0u8; 64 * 1024];
                        let mut file_written = 0u64;
                        let file_size = entry.size();
                        loop {
                            if cancel.load(Ordering::Relaxed) {
                                cancelled = true;
                                return Ok(false);
                            }
                            let n = stream.read(&mut buf).map_err(|e| {
                                sevenz_rust::Error::other(format!("Błąd odczytu z 7Z: {}", e))
                            })?;
                            if n == 0 {
                                break;
                            }
                            outfile.write_all(&buf[..n]).map_err(|e| {
                                sevenz_rust::Error::other(format!("Błąd zapisu pliku: {}", e))
                            })?;
                            file_written += n as u64;
                            progress_cb(&rel_display, file_written, file_size);
                        }
                        extracted_count += 1;
                    }
                    Ok(true)
                })
                .map_err(|e| format!("Błąd wypakowywania 7Z: {}", e))?;

            if cancelled || cancel.load(Ordering::Relaxed) {
                return Err("Operacja anulowana przez użytkownika".to_string());
            }

            Ok(extracted_count)
        }
    }
}
