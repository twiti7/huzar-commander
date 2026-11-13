fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let out_dir = match std::env::var("OUT_DIR") {
            Ok(val) => val,
            Err(_) => return,
        };
        let rc_path = format!("{}/app.rc", out_dir);
        let res_path = format!("{}/app.o", out_dir);

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let ico_path = format!("{}/assets/huzar_icon.ico", manifest_dir).replace('\\', "/");
        let rc_content = format!("1 ICON \"{}\"\n", ico_path);

        if std::fs::write(&rc_path, rc_content).is_ok() {
            let status = std::process::Command::new("windres")
                .args(["-i", &rc_path, "-O", "coff", "-o", &res_path])
                .status();

            if let Ok(s) = status {
                if s.success() {
                    println!("cargo:rustc-link-arg={}", res_path);
                }
            }
        }
        println!("cargo:rerun-if-changed=assets/huzar_icon.ico");
    }
}
