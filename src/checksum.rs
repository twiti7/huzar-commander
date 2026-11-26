use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::Digest as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChecksums {
    pub sha256: String,
    pub md5: String,
    pub crc32: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    Sha256,
    Md5,
    Crc32,
}

impl ChecksumAlgorithm {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sha256 => "SHA-256",
            Self::Md5 => "MD5",
            Self::Crc32 => "CRC32",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumVerifyResult {
    Empty,
    Matched(ChecksumAlgorithm),
    Mismatch,
}

#[derive(Debug, Clone)]
pub enum ChecksumProgress {
    Progress { bytes_done: u64, total_bytes: u64 },
    Done(FileChecksums),
    Error(String),
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut out, "{:02x}", b);
    }
    out
}

/// Computes SHA-256, MD5, and CRC32 in a single 256 KB streaming pass over `path`.
pub fn compute_file_checksums(
    path: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<FileChecksums, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let mut file = File::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let total_bytes = file.metadata().map(|m| m.len()).unwrap_or(0);

    let mut sha256 = sha2::Sha256::new();
    let mut md5 = md5::Md5::new();
    let mut crc = crc32fast::Hasher::new();

    let mut buf = vec![0u8; 256 * 1024];
    let mut bytes_done = 0u64;
    let mut last_reported = 0u64;
    const REPORT_INTERVAL: u64 = 2 * 1024 * 1024;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }

        let chunk = &buf[..n];
        sha256.update(chunk);
        md5.update(chunk);
        crc.update(chunk);

        bytes_done = bytes_done.saturating_add(n as u64);
        if bytes_done.saturating_sub(last_reported) >= REPORT_INTERVAL {
            last_reported = bytes_done;
            on_progress(bytes_done, total_bytes);
        }
    }

    on_progress(bytes_done, total_bytes);

    Ok(FileChecksums {
        sha256: bytes_to_hex(&sha256.finalize()),
        md5: bytes_to_hex(&md5.finalize()),
        crc32: format!("{:08x}", crc.finalize()),
    })
}

/// Normalizes a pasted checksum line or hash (supports raw hex, `<hash>  file`, BSD `TAG (file) = <hash>`, `0x...`).
pub fn normalize_checksum_token(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let raw_token = if let Some((_, rhs)) = trimmed.rsplit_once('=') {
        rhs.split_whitespace().next().unwrap_or("")
    } else {
        trimmed.split_whitespace().next().unwrap_or("")
    };

    let without_prefix = raw_token
        .strip_prefix("0x")
        .or_else(|| raw_token.strip_prefix("0X"))
        .unwrap_or(raw_token);

    without_prefix
        .chars()
        .filter(|&c| c != ':' && c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Compares `expected` against computed SHA-256, MD5, and CRC32 checksums.
pub fn verify_checksum_input(expected: &str, hashes: &FileChecksums) -> ChecksumVerifyResult {
    let norm = normalize_checksum_token(expected);
    if norm.is_empty() {
        return ChecksumVerifyResult::Empty;
    }
    if norm == hashes.sha256 {
        ChecksumVerifyResult::Matched(ChecksumAlgorithm::Sha256)
    } else if norm == hashes.md5 {
        ChecksumVerifyResult::Matched(ChecksumAlgorithm::Md5)
    } else if norm == hashes.crc32 {
        ChecksumVerifyResult::Matched(ChecksumAlgorithm::Crc32)
    } else {
        ChecksumVerifyResult::Mismatch
    }
}
