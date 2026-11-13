use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DriveInfo {
    pub letter: char,
    pub path: PathBuf,
    pub label: String,
    pub volume_label: String,
    pub total_space: u64,
    pub free_space: u64,
}

#[cfg(target_os = "windows")]
extern "system" {
    fn GetDiskFreeSpaceExW(
        lpDirectoryName: *const u16,
        lpFreeBytesAvailableToCaller: *mut u64,
        lpTotalNumberOfBytes: *mut u64,
        lpTotalNumberOfFreeBytes: *mut u64,
    ) -> i32;

    fn GetVolumeInformationW(
        lpRootPathName: *const u16,
        lpVolumeNameBuffer: *mut u16,
        nVolumeNameSize: u32,
        lpVolumeSerialNumber: *mut u32,
        lpMaximumComponentLength: *mut u32,
        lpFileSystemFlags: *mut u32,
        lpFileSystemNameBuffer: *mut u16,
        nFileSystemNameSize: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
fn get_volume_label(path_str: &str) -> String {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut wide: Vec<u16> = OsStr::new(path_str).encode_wide().collect();
    wide.push(0);

    let mut name_buf = [0u16; 260];
    let ret = unsafe {
        GetVolumeInformationW(
            wide.as_ptr(),
            name_buf.as_mut_ptr(),
            name_buf.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };

    if ret != 0 {
        let len = name_buf.iter().position(|&c| c == 0).unwrap_or(name_buf.len());
        let label = String::from_utf16_lossy(&name_buf[..len]).trim().to_string();
        if !label.is_empty() {
            return label;
        }
    }

    "Dysk lokalny".to_string()
}

#[cfg(target_os = "windows")]
fn get_drive_space(path_str: &str) -> (u64, u64) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut wide: Vec<u16> = OsStr::new(path_str).encode_wide().collect();
    wide.push(0);

    let mut free_available: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free: u64 = 0;

    unsafe {
        let ret = GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_available,
            &mut total_bytes,
            &mut total_free,
        );
        if ret != 0 {
            (total_bytes, free_available)
        } else {
            (0, 0)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_unix_space(path_str: &str) -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        if let Ok(c_path) = CString::new(path_str) {
            unsafe {
                let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
                if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) == 0 {
                    let stat = stat.assume_init();
                    let block_size = stat.f_frsize as u64;
                    let total = (stat.f_blocks as u64).saturating_mul(block_size);
                    let free = (stat.f_bavail as u64).saturating_mul(block_size);
                    return (total, free);
                }
            }
        }
    }
    (0, 0)
}

#[cfg(not(target_os = "windows"))]
fn scan_unix_mounts(drives: &mut Vec<DriveInfo>) {
    let mut seen_paths = std::collections::HashSet::new();

    // 1. Root "/"
    let root_path = PathBuf::from("/");
    let (root_total, root_free) = get_unix_space("/");
    drives.push(DriveInfo {
        letter: 'R',
        path: root_path.clone(),
        label: "[-R-]".to_string(),
        volume_label: "Root (/)".to_string(),
        total_space: root_total,
        free_space: root_free,
    });
    seen_paths.insert(root_path);

    // 2. Home directory "~"
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        if home_path.exists() && !seen_paths.contains(&home_path) {
            let (home_total, home_free) = get_unix_space(&home);
            drives.push(DriveInfo {
                letter: 'H',
                path: home_path.clone(),
                label: "[-H-]".to_string(),
                volume_label: "Home (~)".to_string(),
                total_space: home_total,
                free_space: home_free,
            });
            seen_paths.insert(home_path);
        }
    }

    // 3. Dynamic letter assigner
    let mut next_letter = b'A';
    let mut get_letter = || -> char {
        while next_letter == b'H' || next_letter == b'R' {
            next_letter += 1;
        }
        let ch = next_letter as char;
        if next_letter < b'Z' {
            next_letter += 1;
        }
        ch
    };

    // 4. Mounts from /proc/mounts (Linux)
    if let Ok(mounts_content) = std::fs::read_to_string("/proc/mounts") {
        for line in mounts_content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let dev = parts[0];
            let mnt_str = parts[1];
            let fstype = parts[2];

            // Filter out pseudo/virtual filesystems
            match fstype {
                "sysfs" | "proc" | "devtmpfs" | "devpts" | "tmpfs" | "securityfs" | "cgroup"
                | "cgroup2" | "pstore" | "bpf" | "autofs" | "mqueue" | "hugetlbfs" | "debugfs"
                | "tracefs" | "fusectl" | "configfs" | "ramfs" | "overlay" | "nsfs"
                | "binfmt_misc" => continue,
                _ => {}
            }

            if mnt_str.starts_with("/proc")
                || mnt_str.starts_with("/sys")
                || mnt_str.starts_with("/dev")
                || mnt_str.starts_with("/run/user")
                || mnt_str.starts_with("/var/lib/docker")
                || mnt_str.starts_with("/var/lib/containerd")
                || mnt_str.starts_with("/snap")
            {
                continue;
            }

            let mnt_path = PathBuf::from(mnt_str);
            if !mnt_path.exists() || seen_paths.contains(&mnt_path) {
                continue;
            }

            let (total, free) = get_unix_space(mnt_str);
            if total == 0 {
                continue;
            }

            let letter = get_letter();
            let vol_name = mnt_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| dev.to_string());

            drives.push(DriveInfo {
                letter,
                path: mnt_path.clone(),
                label: format!("[-{}-]", letter),
                volume_label: format!("{} ({})", vol_name, mnt_str),
                total_space: total,
                free_space: free,
            });
            seen_paths.insert(mnt_path);
        }
    } else {
        // Fallback: check /mnt and /media
        for base in &["/mnt", "/media"] {
            let base_path = PathBuf::from(base);
            if let Ok(entries) = std::fs::read_dir(&base_path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && !seen_paths.contains(&p) {
                        let (total, free) = get_unix_space(&p.to_string_lossy());
                        let letter = get_letter();
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        drives.push(DriveInfo {
                            letter,
                            path: p.clone(),
                            label: format!("[-{}-]", letter),
                            volume_label: format!("{} ({})", name, p.to_string_lossy()),
                            total_space: total,
                            free_space: free,
                        });
                        seen_paths.insert(p);
                    }
                }
            }
        }
    }
}

pub fn scan_drives() -> Vec<DriveInfo> {
    let mut drives = Vec::new();

    #[cfg(target_os = "windows")]
    {
        for c in b'A'..=b'Z' {
            let letter = c as char;
            let drive_str = format!("{}:\\", letter);
            let drive_path = PathBuf::from(&drive_str);
            if drive_path.exists() {
                let (total, free) = get_drive_space(&drive_str);
                let vol_label = get_volume_label(&drive_str);
                drives.push(DriveInfo {
                    letter,
                    path: drive_path,
                    label: format!("[-{}-]", letter),
                    volume_label: vol_label,
                    total_space: total,
                    free_space: free,
                });
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        scan_unix_mounts(&mut drives);
    }

    drives
}
