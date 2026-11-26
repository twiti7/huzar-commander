use std::path::Path;

use similar::{ChangeTag, DiffOp, TextDiff};

use crate::opener::detect_is_binary;

const MAX_DIFF_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Equal,
    Delete,
    Insert,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub left_line_no: Option<usize>,
    pub left_text: String,
    pub right_line_no: Option<usize>,
    pub right_text: String,
    pub kind: DiffLineKind,
}

#[derive(Debug, Clone, Default)]
pub struct DiffResult {
    pub rows: Vec<DiffRow>,
    pub diff_row_indices: Vec<usize>,
    pub added_count: usize,
    pub removed_count: usize,
    pub changed_count: usize,
}

pub fn compute_side_by_side_diff(
    left_text: &str,
    right_text: &str,
    ignore_whitespace: bool,
) -> DiffResult {
    let orig_left: Vec<&str> = left_text.lines().collect();
    let orig_right: Vec<&str> = right_text.lines().collect();

    let norm_left_owned: Vec<String>;
    let norm_right_owned: Vec<String>;
    let (cmp_left, cmp_right) = if ignore_whitespace {
        norm_left_owned = orig_left.iter().map(|l| l.trim().to_string()).collect();
        norm_right_owned = orig_right.iter().map(|l| l.trim().to_string()).collect();
        (norm_left_owned.join("\n"), norm_right_owned.join("\n"))
    } else {
        (left_text.to_string(), right_text.to_string())
    };

    let left_slices: Vec<&str> = cmp_left.lines().collect();
    let right_slices: Vec<&str> = cmp_right.lines().collect();
    let diff = TextDiff::from_slices(&left_slices, &right_slices);

    let mut result = DiffResult::default();

    for op in diff.ops() {
        match *op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for offset in 0..len {
                    let li = old_index + offset;
                    let ri = new_index + offset;
                    result.rows.push(DiffRow {
                        left_line_no: Some(li + 1),
                        left_text: orig_left.get(li).copied().unwrap_or("").to_string(),
                        right_line_no: Some(ri + 1),
                        right_text: orig_right.get(ri).copied().unwrap_or("").to_string(),
                        kind: DiffLineKind::Equal,
                    });
                }
            }
            DiffOp::Delete {
                old_index,
                old_len,
                ..
            } => {
                result.diff_row_indices.push(result.rows.len());
                for offset in 0..old_len {
                    let li = old_index + offset;
                    result.removed_count += 1;
                    result.rows.push(DiffRow {
                        left_line_no: Some(li + 1),
                        left_text: orig_left.get(li).copied().unwrap_or("").to_string(),
                        right_line_no: None,
                        right_text: String::new(),
                        kind: DiffLineKind::Delete,
                    });
                }
            }
            DiffOp::Insert {
                new_index,
                new_len,
                ..
            } => {
                result.diff_row_indices.push(result.rows.len());
                for offset in 0..new_len {
                    let ri = new_index + offset;
                    result.added_count += 1;
                    result.rows.push(DiffRow {
                        left_line_no: None,
                        left_text: String::new(),
                        right_line_no: Some(ri + 1),
                        right_text: orig_right.get(ri).copied().unwrap_or("").to_string(),
                        kind: DiffLineKind::Insert,
                    });
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let paired = old_len.min(new_len);
                // Sanity fallback for paired lines vs changes
                let _ = ChangeTag::Equal;
                result.diff_row_indices.push(result.rows.len());

                for offset in 0..paired {
                    let li = old_index + offset;
                    let ri = new_index + offset;
                    result.changed_count += 1;
                    result.rows.push(DiffRow {
                        left_line_no: Some(li + 1),
                        left_text: orig_left.get(li).copied().unwrap_or("").to_string(),
                        right_line_no: Some(ri + 1),
                        right_text: orig_right.get(ri).copied().unwrap_or("").to_string(),
                        kind: DiffLineKind::Replace,
                    });
                }
                for offset in paired..old_len {
                    let li = old_index + offset;
                    result.removed_count += 1;
                    result.rows.push(DiffRow {
                        left_line_no: Some(li + 1),
                        left_text: orig_left.get(li).copied().unwrap_or("").to_string(),
                        right_line_no: None,
                        right_text: String::new(),
                        kind: DiffLineKind::Delete,
                    });
                }
                for offset in paired..new_len {
                    let ri = new_index + offset;
                    result.added_count += 1;
                    result.rows.push(DiffRow {
                        left_line_no: None,
                        left_text: String::new(),
                        right_line_no: Some(ri + 1),
                        right_text: orig_right.get(ri).copied().unwrap_or("").to_string(),
                        kind: DiffLineKind::Insert,
                    });
                }
            }
        }
    }

    result
}

pub fn load_and_diff_files(
    left_path: &Path,
    right_path: &Path,
    ignore_whitespace: bool,
) -> Result<DiffResult, String> {
    let l_meta = std::fs::metadata(left_path)
        .map_err(|e| format!("Cannot read '{}': {}", left_path.display(), e))?;
    let r_meta = std::fs::metadata(right_path)
        .map_err(|e| format!("Cannot read '{}': {}", right_path.display(), e))?;

    if l_meta.len() > MAX_DIFF_BYTES || r_meta.len() > MAX_DIFF_BYTES {
        return Err("Files larger than 4 MB are not supported in side-by-side diff.".to_string());
    }

    let l_bytes = std::fs::read(left_path)
        .map_err(|e| format!("Error reading '{}': {}", left_path.display(), e))?;
    let r_bytes = std::fs::read(right_path)
        .map_err(|e| format!("Error reading '{}': {}", right_path.display(), e))?;

    if detect_is_binary(&l_bytes) || detect_is_binary(&r_bytes) {
        return Err("Binary files cannot be compared in text diff view.".to_string());
    }

    let left_str = String::from_utf8_lossy(&l_bytes);
    let right_str = String::from_utf8_lossy(&r_bytes);
    Ok(compute_side_by_side_diff(&left_str, &right_str, ignore_whitespace))
}
