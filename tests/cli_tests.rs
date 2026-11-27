use std::path::{Path, PathBuf};

use huzar_commander::app::parse_cd_command;

#[test]
fn test_parse_cd_command_relative_and_parent() {
    let cwd = Path::new(r"C:\Workspace\commander");

    assert_eq!(parse_cd_command("cd ..", cwd), Some(PathBuf::from(r"C:\Workspace")));
    assert_eq!(parse_cd_command("cd ../other", cwd), Some(PathBuf::from(r"C:\Workspace\other")));
    assert_eq!(parse_cd_command("cd src", cwd), Some(PathBuf::from(r"C:\Workspace\commander\src")));
    assert_eq!(
        parse_cd_command(r#"cd "my folder""#, cwd),
        Some(PathBuf::from(r"C:\Workspace\commander\my folder"))
    );

    let unix_cwd = Path::new("/home/user/commander");
    assert_eq!(parse_cd_command("cd ..", unix_cwd), Some(PathBuf::from("/home/user")));
    assert_eq!(
        parse_cd_command("cd ../other", unix_cwd),
        Some(PathBuf::from("/home/user/other"))
    );
    assert_eq!(
        parse_cd_command("cd src", unix_cwd),
        Some(PathBuf::from("/home/user/commander/src"))
    );
}

#[test]
fn test_parse_cd_command_drive_root_and_flags() {
    let cwd = Path::new(r"C:\Workspace\commander");

    assert_eq!(parse_cd_command("D:", cwd), Some(PathBuf::from(r"D:\")));
    assert_eq!(parse_cd_command("d:", cwd), Some(PathBuf::from(r"D:\")));
    assert_eq!(parse_cd_command(r"cd /d D:\Projects", cwd), Some(PathBuf::from(r"D:\Projects")));
    assert_eq!(parse_cd_command(r"cd \", cwd), Some(PathBuf::from(r"C:\")));
    assert_eq!(parse_cd_command("cd /", Path::new("/home/user")), Some(PathBuf::from("/")));
}

#[test]
fn test_parse_cd_command_ignores_non_cd_commands() {
    let cwd = Path::new(r"C:\Workspace\commander");

    assert_eq!(parse_cd_command("dir", cwd), None);
    assert_eq!(parse_cd_command("echo hello", cwd), None);
    assert_eq!(parse_cd_command("cargo test", cwd), None);
    assert_eq!(parse_cd_command("cdrom", cwd), None);
    assert_eq!(parse_cd_command("   ", cwd), None);
}

#[test]
fn test_parse_cd_command_unc_paths() {
    let cwd = Path::new(r"C:\Workspace\commander");

    assert_eq!(
        parse_cd_command(r"cd \\TRUENAS", cwd),
        Some(PathBuf::from(r"\\TRUENAS"))
    );
    assert_eq!(
        parse_cd_command(r"cd \\TRUENAS\", cwd),
        Some(PathBuf::from(r"\\TRUENAS"))
    );
    assert_eq!(
        parse_cd_command(r"\\TRUENAS", cwd),
        Some(PathBuf::from(r"\\TRUENAS"))
    );
    assert_eq!(
        parse_cd_command(r"cd \\TRUENAS\media", cwd),
        Some(PathBuf::from(r"\\TRUENAS\media"))
    );
    assert_eq!(
        parse_cd_command("cd ..", Path::new(r"\\TRUENAS\media")),
        Some(PathBuf::from(r"\\TRUENAS"))
    );
    assert_eq!(
        parse_cd_command("cd media", Path::new(r"\\TRUENAS")),
        Some(PathBuf::from(r"\\TRUENAS\media"))
    );
    assert_eq!(
        parse_cd_command(r"cd \", Path::new(r"\\TRUENAS\media\films")),
        Some(PathBuf::from(r"\\TRUENAS\media"))
    );
}
