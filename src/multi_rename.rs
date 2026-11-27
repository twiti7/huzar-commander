use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use regex::RegexBuilder;

use crate::fs_item::FileItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaseTransform {
    #[default]
    Unchanged,
    Lower,
    Upper,
    Title,
    FirstUpper,
}

#[derive(Debug, Clone)]
pub struct MultiRenameConfig {
    pub name_mask: String,
    pub ext_mask: String,
    pub search: String,
    pub replace: String,
    pub use_regex: bool,
    pub case_sensitive: bool,
    pub counter_start: usize,
    pub counter_step: usize,
    pub counter_digits: usize,
    pub case_transform: CaseTransform,
}

impl Default for MultiRenameConfig {
    fn default() -> Self {
        Self {
            name_mask: "[N]".to_string(),
            ext_mask: "[E]".to_string(),
            search: String::new(),
            replace: String::new(),
            use_regex: false,
            case_sensitive: false,
            counter_start: 1,
            counter_step: 1,
            counter_digits: 1,
            case_transform: CaseTransform::Unchanged,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenamePreviewRow {
    pub source: PathBuf,
    pub old_name: String,
    pub new_name: String,
    pub target: PathBuf,
    pub changed: bool,
    pub conflict: Option<String>,
}

fn split_stem_and_ext(name: &str, is_dir: bool) -> (&str, &str) {
    if is_dir {
        return (name, "");
    }
    if let Some(dot) = name.rfind('.') {
        if dot > 0 {
            return (&name[..dot], &name[dot + 1..]);
        }
    }
    (name, "")
}

struct PlaceholderValues<'a> {
    stem: &'a str,
    ext: &'a str,
    counter: &'a str,
    ymd: &'a str,
    y: &'a str,
    m: &'a str,
    d: &'a str,
}

fn apply_placeholders(template: &str, vals: &PlaceholderValues<'_>) -> String {
    template
        .replace("[N]", vals.stem)
        .replace("[E]", vals.ext)
        .replace("[C]", vals.counter)
        .replace("[YMD]", vals.ymd)
        .replace("[Y]", vals.y)
        .replace("[M]", vals.m)
        .replace("[D]", vals.d)
}

fn replace_literal_ci(input: &str, search: &str, replace: &str) -> String {
    let input_lower = input.to_lowercase();
    let search_lower = search.to_lowercase();
    if search_lower.is_empty() || input_lower.len() != input.len() {
        return input.replace(search, replace);
    }

    let mut out = String::with_capacity(input.len());
    let mut pos = 0usize;
    while let Some(rel) = input_lower[pos..].find(&search_lower) {
        let start = pos + rel;
        let end = start + search_lower.len();
        out.push_str(&input[pos..start]);
        out.push_str(replace);
        pos = end;
    }
    out.push_str(&input[pos..]);
    out
}

fn to_title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut new_word = true;
    for ch in s.chars() {
        if !ch.is_alphanumeric() {
            out.push(ch);
            new_word = true;
        } else if new_word {
            for uc in ch.to_uppercase() {
                out.push(uc);
            }
            new_word = false;
        } else {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
        }
    }
    out
}

fn to_first_upper(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut done_first = false;
    for ch in s.chars() {
        if !done_first && ch.is_alphabetic() {
            for uc in ch.to_uppercase() {
                out.push(uc);
            }
            done_first = true;
        } else {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
        }
    }
    out
}

fn apply_case_transform(name: &str, transform: CaseTransform) -> String {
    match transform {
        CaseTransform::Unchanged => name.to_string(),
        CaseTransform::Lower => name.to_lowercase(),
        CaseTransform::Upper => name.to_uppercase(),
        CaseTransform::Title => to_title_case(name),
        CaseTransform::FirstUpper => to_first_upper(name),
    }
}

fn path_collision_key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

fn invalid_filename_reason(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Some("Invalid empty or '.'/'..' name".to_string());
    }
    #[cfg(windows)]
    const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    #[cfg(not(windows))]
    const FORBIDDEN: &[char] = &['/'];

    if name.chars().any(|c| FORBIDDEN.contains(&c) || c == '\0') {
        return Some("Contains forbidden path characters".to_string());
    }
    None
}

pub fn compute_rename_preview(
    items: &[FileItem],
    cfg: &MultiRenameConfig,
) -> Result<Vec<RenamePreviewRow>, String> {
    let compiled_regex = if cfg.use_regex && !cfg.search.is_empty() {
        Some(
            RegexBuilder::new(&cfg.search)
                .case_insensitive(!cfg.case_sensitive)
                .build()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let digits = cfg.counter_digits.clamp(1, 8);
    let mut rows = Vec::with_capacity(items.len());
    let mut source_keys: HashSet<String> = HashSet::with_capacity(items.len());

    for item in items {
        source_keys.insert(path_collision_key(&item.path));
    }

    for (idx, item) in items.iter().enumerate() {
        let (stem, ext) = split_stem_and_ext(&item.name, item.is_dir);
        let dt: DateTime<Local> = item.modified.map(DateTime::from).unwrap_or_else(Local::now);
        let ymd = dt.format("%Y%m%d").to_string();
        let y = dt.format("%Y").to_string();
        let m = dt.format("%m").to_string();
        let d = dt.format("%d").to_string();

        let counter_val = cfg
            .counter_start
            .saturating_add(idx.saturating_mul(cfg.counter_step));
        let counter_str = format!("{:0width$}", counter_val, width = digits);

        let vals = PlaceholderValues {
            stem,
            ext,
            counter: &counter_str,
            ymd: &ymd,
            y: &y,
            m: &m,
            d: &d,
        };
        let rendered_name = apply_placeholders(&cfg.name_mask, &vals);
        let rendered_ext = apply_placeholders(&cfg.ext_mask, &vals);

        let combined = if rendered_ext.trim().is_empty() {
            rendered_name
        } else {
            format!("{}.{}", rendered_name, rendered_ext)
        };

        let replaced = if cfg.search.is_empty() {
            combined
        } else if let Some(ref re) = compiled_regex {
            re.replace_all(&combined, cfg.replace.as_str()).into_owned()
        } else if cfg.case_sensitive {
            combined.replace(&cfg.search, &cfg.replace)
        } else {
            replace_literal_ci(&combined, &cfg.search, &cfg.replace)
        };

        let final_name = apply_case_transform(&replaced, cfg.case_transform);
        let parent = item.path.parent().unwrap_or_else(|| Path::new(""));
        let target = parent.join(&final_name);
        let changed = final_name != item.name;
        let conflict = invalid_filename_reason(&final_name);

        rows.push(RenamePreviewRow {
            source: item.path.clone(),
            old_name: item.name.clone(),
            new_name: final_name,
            target,
            changed,
            conflict,
        });
    }

    // Detect duplicate target names within the batch
    let mut target_counts: HashMap<String, usize> = HashMap::new();
    for row in &rows {
        let key = path_collision_key(&row.target);
        *target_counts.entry(key).or_insert(0) += 1;
    }

    for row in &mut rows {
        if row.conflict.is_some() {
            continue;
        }
        let target_key = path_collision_key(&row.target);
        if target_counts.get(&target_key).copied().unwrap_or(0) > 1 {
            row.conflict = Some(format!("Duplicate target '{}'", row.new_name));
            continue;
        }

        if row.changed {
            let source_key = path_collision_key(&row.source);
            let same_ci_target = cfg!(windows) && target_key == source_key;
            if !same_ci_target && row.target.exists() && !source_keys.contains(&target_key) {
                row.conflict = Some(format!("Already exists on disk '{}'", row.new_name));
            }
        }
    }

    Ok(rows)
}

pub fn execute_batch_rename(pairs: &[(PathBuf, PathBuf)]) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let active_pairs: Vec<(PathBuf, PathBuf)> = pairs
        .iter()
        .filter(|(s, d)| s != d)
        .cloned()
        .collect();
    if active_pairs.is_empty() {
        return Ok(Vec::new());
    }

    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Phase 1: Move sources to collision-free sibling temp names
    let mut staged: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::with_capacity(active_pairs.len());
    for (idx, (src, dst)) in active_pairs.iter().enumerate() {
        let parent = src.parent().unwrap_or_else(|| Path::new(""));
        let mut tmp_path = parent.join(format!(".huzar_tmp_rename_{}_{}_{}", pid, idx, nonce));
        let mut attempt = 0u32;
        while tmp_path.exists() {
            attempt += 1;
            tmp_path = parent.join(format!(
                ".huzar_tmp_rename_{}_{}_{}_{}",
                pid, idx, nonce, attempt
            ));
        }

        if let Err(e) = std::fs::rename(src, &tmp_path) {
            // Rollback phase 1
            for (orig_src, staged_tmp, _) in staged.iter().rev() {
                let _ = std::fs::rename(staged_tmp, orig_src);
            }
            return Err(format!(
                "Failed to rename '{}': {}",
                src.display(),
                e
            ));
        }
        staged.push((src.clone(), tmp_path, dst.clone()));
    }

    // Phase 2: Move temp files to final target paths
    let mut completed: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::with_capacity(staged.len());
    for (idx, (orig_src, tmp_path, dst)) in staged.iter().enumerate() {
        if let Err(e) = std::fs::rename(tmp_path, dst) {
            // Rollback completed back to orig_src and remaining staged tmp back to orig_src
            for (c_src, _, c_dst) in completed.iter().rev() {
                let _ = std::fs::rename(c_dst, c_src);
            }
            for (r_src, r_tmp, _) in staged[idx..].iter().rev() {
                let _ = std::fs::rename(r_tmp, r_src);
            }
            return Err(format!(
                "Failed to finalize rename to '{}': {}",
                dst.display(),
                e
            ));
        }
        completed.push((orig_src.clone(), tmp_path.clone(), dst.clone()));
    }

    Ok(active_pairs)
}
