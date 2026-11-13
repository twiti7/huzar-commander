use std::path::{Path, PathBuf};

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut std::ffi::c_void,
        lp_operation: *const u16,
        lp_file: *const u16,
        lp_parameters: *const u16,
        lp_directory: *const u16,
        n_show_cmd: i32,
    ) -> isize;
}

/// Otwiera plik w systemowym skojarzonym programie (np. odtwarzacz wideo dla .mp4/.mkv,
/// przeglądarka PDF dla .pdf, domyślna przeglądarka obrazów dla .jpg, itp.).
pub fn open_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("Plik '{}' nie istnieje", path.display()));
    }

    #[cfg(windows)]
    {
        open_file_windows(path)
    }

    #[cfg(not(windows))]
    {
        open::that(path).map_err(|e| format!("Błąd otwierania pliku: {}", e))
    }
}

/// Rozdziela linię polecenia edytora na program i argumenty,
/// z pełną obsługą cudzysłowów oraz ścieżek ze spacjami.
pub fn split_editor_cmd(cmd_str: &str) -> (String, Vec<String>) {
    let trimmed = cmd_str.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }

    // 1. Jeśli cała ścieżka wskazuje na istniejący plik (np. C:\Program Files\Notepad++\notepad++.exe),
    // to traktujemy całość jako ścieżkę do programu bez argumentów.
    if Path::new(trimmed).is_file() {
        return (trimmed.to_string(), Vec::new());
    }

    // 2. Tokenizuj z uwzględnieniem cudzysłowów
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;

    for ch in trimmed.chars() {
        match in_quote {
            Some(quote_char) => {
                if ch == quote_char {
                    in_quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    in_quote = Some(ch);
                } else if ch.is_whitespace() {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.is_empty() {
        (String::new(), Vec::new())
    } else {
        let prog = tokens.remove(0);
        (prog, tokens)
    }
}

/// Rozwiązuje polecenie edytora (np. "code", "notepad++") do pełnej ścieżki pliku wykonywalnego.
pub fn resolve_editor_command(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.is_file() {
        return Some(p.to_path_buf());
    }

    #[cfg(windows)]
    {
        // 1. Popularne edytory: VS Code ("code")
        if program.eq_ignore_ascii_case("code") {
            if let Ok(prog_files) = std::env::var("ProgramFiles") {
                let p_exe = PathBuf::from(&prog_files).join(r"Microsoft VS Code\Code.exe");
                if p_exe.is_file() {
                    return Some(p_exe);
                }
            }
            if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
                let p_exe = PathBuf::from(&local_app).join(r"Programs\Microsoft VS Code\Code.exe");
                if p_exe.is_file() {
                    return Some(p_exe);
                }
            }
            if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
                let p_exe = PathBuf::from(&prog_files_x86).join(r"Microsoft VS Code\Code.exe");
                if p_exe.is_file() {
                    return Some(p_exe);
                }
            }
        }

        // 2. Popularne edytory: Notepad++ ("notepad++")
        if program.eq_ignore_ascii_case("notepad++") {
            if let Ok(prog_files) = std::env::var("ProgramFiles") {
                let p_exe = PathBuf::from(&prog_files).join(r"Notepad++\notepad++.exe");
                if p_exe.is_file() {
                    return Some(p_exe);
                }
            }
            if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
                let p_exe = PathBuf::from(&prog_files_x86).join(r"Notepad++\notepad++.exe");
                if p_exe.is_file() {
                    return Some(p_exe);
                }
            }
        }

        // 3. Sprawdź czy dodanie rozszerzenia (.exe, .cmd, .bat) pasuje w bieżącym katalogu
        for ext in &[".exe", ".cmd", ".bat"] {
            let with_ext = format!("{}{}", program, ext);
            let p_cand = PathBuf::from(&with_ext);
            if p_cand.is_file() {
                return Some(p_cand);
            }
        }

        // 4. Przeszukaj katalogi w zmiennej środowiskowej PATH
        let has_ext = p.extension().is_some();
        let exts: &[&str] = if has_ext {
            &[""]
        } else {
            &[".exe", ".cmd", ".bat", ""]
        };

        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                for ext in exts {
                    let file_name = if ext.is_empty() {
                        program.to_string()
                    } else {
                        format!("{}{}", program, ext)
                    };
                    let candidate = dir.join(&file_name);
                    if candidate.is_file() {
                        // Jeśli znaleziono code.cmd w podfolderze bin, sprawdź czy w katalogu nadrzędnym jest Code.exe
                        if file_name.eq_ignore_ascii_case("code.cmd") || file_name.eq_ignore_ascii_case("code.bat") {
                            if let Some(parent) = candidate.parent().and_then(|p| p.parent()) {
                                let native_code = parent.join("Code.exe");
                                if native_code.is_file() {
                                    return Some(native_code);
                                }
                            }
                        }
                        return Some(candidate);
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(program);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

#[cfg(windows)]
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
}

#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case, clippy::upper_case_acronyms)]
struct OPENFILENAMEW {
    lStructSize: u32,
    hwndOwner: *mut std::ffi::c_void,
    hInstance: *mut std::ffi::c_void,
    lpstrFilter: *const u16,
    lpstrCustomFilter: *mut u16,
    nMaxCustFilter: u32,
    nFilterIndex: u32,
    lpstrFile: *mut u16,
    nMaxFile: u32,
    lpstrFileTitle: *mut u16,
    nMaxFileTitle: u32,
    lpstrInitialDir: *const u16,
    lpstrTitle: *const u16,
    Flags: u32,
    nFileOffset: u16,
    nFileExtension: u16,
    lpstrDefExt: *const u16,
    lCustData: usize,
    lpfnHook: *mut std::ffi::c_void,
    lpTemplateName: *const u16,
    pvReserved: *mut std::ffi::c_void,
    dwReserved: u32,
    FlagsEx: u32,
}

/// Otwiera natywne okno eksploratora plików w celu wyboru programu edytora (*.exe, *.cmd, *.bat).
#[cfg(windows)]
pub fn pick_editor_executable() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut file_buf = [0u16; 1024];

    // Filter string zakończony podwójnym null:
    let filter_text = "Programy (*.exe;*.cmd;*.bat)\0*.exe;*.cmd;*.bat\0Wszystkie pliki (*.*)\0*.*\0\0";
    let filter: Vec<u16> = filter_text.encode_utf16().collect();

    let title_text = "Wybierz program edytora\0";
    let title: Vec<u16> = title_text.encode_utf16().collect();

    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = file_buf.as_mut_ptr();
    ofn.nMaxFile = file_buf.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    // OFN_EXPLORER (0x00080000) | OFN_FILEMUSTEXIST (0x00001000) | OFN_PATHMUSTEXIST (0x00000800) | OFN_NOCHANGEDIR (0x00000008)
    ofn.Flags = 0x00080000 | 0x00001000 | 0x00000800 | 0x00000008;

    let ret = unsafe { GetOpenFileNameW(&mut ofn) };
    if ret != 0 {
        let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
        if len > 0 {
            let os_str = OsString::from_wide(&file_buf[..len]);
            let p = PathBuf::from(os_str);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn pick_editor_executable() -> Option<PathBuf> {
    if let Ok(output) = std::process::Command::new("zenity")
        .args(["--file-selection", "--title=Wybierz edytor"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    if let Ok(output) = std::process::Command::new("kdialog")
        .args(["--getopenfilename", ".", "*|Wszystkie pliki"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

/// Uruchamia plik w wybranym edytorze lub poleceniu (np. "code", "notepad.exe", "notepad++").
/// Jeśli polecenie zawiera argumenty (np. "code -n"), zostaną one poprawnie rozdzielone.
pub fn open_with_editor(path: &Path, editor_cmd: &str) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("Plik '{}' nie istnieje", path.display()));
    }

    let trimmed = editor_cmd.trim();
    if trimmed.is_empty() {
        return open_file(path);
    }

    let (program, extra_args) = split_editor_cmd(trimmed);
    if program.is_empty() {
        return open_file(path);
    }

    // 1. Rozwiąż pełną ścieżkę do edytora (np. dla "code", "notepad++" lub z PATH)
    let resolved = resolve_editor_command(&program);
    let target_prog = resolved.as_deref().unwrap_or(Path::new(&program));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let ext = target_prog
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Jeśli to plik skryptu .cmd lub .bat, uruchom przez cmd.exe /c bez migania konsoli
        if ext == "cmd" || ext == "bat" {
            let mut cmd = std::process::Command::new("cmd.exe");
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.arg("/c");
            cmd.arg(target_prog);
            cmd.args(&extra_args);
            cmd.arg(path);
            if let Some(parent) = path.parent() {
                cmd.current_dir(parent);
            }
            return cmd.spawn().map(|_| ()).map_err(|e| {
                format!("Nie można uruchomić skryptu edytora '{}': {}", target_prog.display(), e)
            });
        }
    }

    let mut cmd = std::process::Command::new(target_prog);
    cmd.args(&extra_args);
    cmd.arg(path);

    if let Some(parent) = path.parent() {
        cmd.current_dir(parent);
    }

    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) => {
            #[cfg(windows)]
            {
                // Fallback na Windows: spróbuj przez cmd.exe /c
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;

                let mut cmd_fallback = std::process::Command::new("cmd.exe");
                cmd_fallback.creation_flags(CREATE_NO_WINDOW);
                cmd_fallback.arg("/c");
                cmd_fallback.arg(&program);
                cmd_fallback.args(&extra_args);
                cmd_fallback.arg(path);
                if let Some(parent) = path.parent() {
                    cmd_fallback.current_dir(parent);
                }
                if cmd_fallback.spawn().is_ok() {
                    return Ok(());
                }
            }

            Err(format!(
                "Nie można uruchomić edytora '{}': {}",
                target_prog.display(), e
            ))
        }
    }
}

#[cfg(windows)]
fn open_file_windows(path: &Path) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    let path_str = path.as_os_str();
    let wide_path: Vec<u16> = path_str.encode_wide().chain(std::iter::once(0)).collect();

    // Przekazanie katalogu nadrzędnego jest kluczowe dla odtwarzaczy wideo,
    // aby mogły automatycznie znaleźć napisy (.srt/.sub) w tym samym folderze.
    let parent_dir = path.parent();
    let wide_dir: Option<Vec<u16>> = parent_dir
        .map(|p| p.as_os_str().encode_wide().chain(std::iter::once(0)).collect());
    let dir_ptr = wide_dir.as_ref().map(|v| v.as_ptr()).unwrap_or(ptr::null());

    let op_open: Vec<u16> = OsStr::new("open").encode_wide().chain(std::iter::once(0)).collect();
    let op_openas: Vec<u16> = OsStr::new("openas").encode_wide().chain(std::iter::once(0)).collect();

    // 1. Domyślna akcja (NULL) z katalogiem roboczym ustawionym na folder pliku.
    // Działa natychmiastowo dla wszystkich aplikacji Win32 oraz UWP / Modern Windows Apps (Filmy i TV, VLC, itp.).
    let res = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            ptr::null(),
            wide_path.as_ptr(),
            ptr::null(),
            dir_ptr,
            1, // SW_SHOWNORMAL
        )
    };

    let code = res as isize;
    if code > 32 {
        return Ok(());
    }

    // 2. Jeśli NULL zwrócił błąd, spróbuj jawnego czasownika "open"
    let res = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            op_open.as_ptr(),
            wide_path.as_ptr(),
            ptr::null(),
            dir_ptr,
            1,
        )
    };
    let code = res as isize;
    if code > 32 {
        return Ok(());
    }

    // 3. SE_ERR_NOASSOC (31) lub SE_ERR_ASSOCINCOMPLETE (27):
    // Brak skojarzenia w rejestrze - wywołaj systemowe okno "Otwórz za pomocą..." (openas)
    if code == 31 || code == 27 {
        let res_as = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                op_openas.as_ptr(),
                wide_path.as_ptr(),
                ptr::null(),
                dir_ptr,
                1,
            )
        };
        if res_as as isize > 32 {
            return Ok(());
        }
    }

    // 4. Fallback na crate open
    if open::that(path).is_ok() {
        return Ok(());
    }

    Err(format_shell_error(code, path))
}

#[cfg(windows)]
fn format_shell_error(code: isize, path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("plik");
    match code {
        2 => format!("Nie znaleziono pliku '{}'", name),
        3 => format!("Nie znaleziono ścieżki do pliku '{}'", name),
        5 => format!("Brak uprawnień do uruchomienia pliku '{}'", name),
        8 => "Brak pamięci systemowej do uruchomienia programu".to_string(),
        11 => format!("Plik '{}' nie jest poprawną aplikacją wykonywalną", name),
        26 => "Błąd komunikacji DDE".to_string(),
        27 => format!("Niekompletne skojarzenie pliku dla '{}'", name),
        29 => format!("Plik '{}' jest zablokowany przez inny proces", name),
        30 => "System jest zajęty obsługą transakcji DDE".to_string(),
        31 => format!("Brak skojarzonego programu dla pliku '{}'. Skojarz program w Windows.", name),
        32 => "Nie znaleziono wymaganej biblioteki DLL".to_string(),
        _ => format!("Nie udało się uruchomić '{}' (kod błędu: {})", name, code),
    }
}

/// Wypakowuje wybrany plik z archiwum ZIP do katalogu tymczasowego (%TEMP%/cmdr_preview/...)
/// i uruchamia go w skojarzonym programie.
pub fn open_zip_entry(archive_path: &Path, internal_name: &str) -> Result<PathBuf, String> {
    let clean_name = internal_name.replace('\\', "/");
    let file_name = clean_name.rsplit('/').next().unwrap_or(&clean_name);

    let archive_stem = archive_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("archive");

    let temp_dir = std::env::temp_dir()
        .join("cmdr_preview")
        .join(archive_stem);

    let target_file = temp_dir.join(file_name);

    crate::zip_support::extract_single_zip_entry(archive_path, internal_name, &target_file)?;

    open_file(&target_file)?;

    Ok(target_file)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerMode {
    Text,
    Hex,
    Markdown,
    Json,
    Yaml,
}

/// Sprawdza, czy plik posiada rozszerzenie wskazujące na dokument Markdown (.md, .markdown)
pub fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

/// Sprawdza, czy plik posiada rozszerzenie wskazujące na format JSON (.json)
pub fn is_json_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

/// Sprawdza, czy plik posiada rozszerzenie wskazujące na format YAML (.yaml, .yml)
pub fn is_yaml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
}

/// Sprawdza, czy w próbce danych występują bajty zerowe (oznaczające plik binarny)
pub fn detect_is_binary(buf: &[u8]) -> bool {
    buf.iter().take(4096).any(|&b| b == 0)
}

/// Odczytuje fragment pliku o maksymalnym rozmiarze `max_bytes` z dowolnej pozycji `offset`.
/// Pozwala na swobodne przewijanie całego pliku o dowolnym rozmiarze bez wczytywania całości do pamięci RAM.
pub fn read_file_chunk(
    path: &Path,
    offset: u64,
    max_bytes: usize,
) -> Result<(Vec<u8>, u64), String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(path).map_err(|e| format!("Nie można otworzyć pliku: {}", e))?;
    let total_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    if offset >= total_size {
        return Ok((Vec::new(), total_size));
    }

    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Błąd pozycjonowania (seek) w pliku: {}", e))?;

    let to_read = (max_bytes as u64).min(total_size - offset) as usize;
    let mut buf = vec![0u8; to_read];
    file.read_exact(&mut buf)
        .map_err(|e| format!("Błąd odczytu fragmentu pliku: {}", e))?;

    Ok((buf, total_size))
}

/// Formatuje fragment danych w widoku szesnastkowym (Hex view) z podanym offsetem bazowym
pub fn format_hex_chunk(buf: &[u8], base_offset: u64, total_size: u64) -> String {
    let mut out = String::with_capacity(buf.len() * 5 + 256);
    let end_offset = base_offset + buf.len() as u64;
    out.push_str(&format!(
        "--- [PODGLĄD HEX | Zakres: 0x{:08X} - 0x{:08X} | Rozmiar całkowity: {}] ---\n",
        base_offset,
        end_offset,
        humansize::format_size(total_size, humansize::DECIMAL)
    ));
    out.push_str("Offset(h)  00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F  Tekst ASCII\n");
    out.push_str("--------------------------------------------------------------------------\n");

    for (chunk_idx, line) in buf.chunks(16).enumerate() {
        let line_offset = base_offset + (chunk_idx * 16) as u64;
        out.push_str(&format!("{:08X}   ", line_offset));

        for i in 0..16 {
            if i == 8 {
                out.push(' ');
            }
            if let Some(&b) = line.get(i) {
                out.push_str(&format!("{:02X} ", b));
            } else {
                out.push_str("   ");
            }
        }
        out.push_str(" |");

        for &b in line {
            if (32..=126).contains(&b) {
                out.push(b as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }

    out
}

/// Formatuje fragment danych jako czytelny tekst UTF-8 / ASCII z nagłówkiem informującym o pozycji
pub fn format_text_chunk(buf: &[u8], base_offset: u64, total_size: u64) -> String {
    let text = String::from_utf8_lossy(buf);
    let mut out = String::with_capacity(text.len() + 128);
    let end_offset = base_offset + buf.len() as u64;
    if base_offset > 0 || end_offset < total_size {
        let pct = if total_size > 0 {
            (base_offset as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };
        out.push_str(&format!(
            "--- [PODGLĄD TEKSTOWY | Zakres: 0x{:08X} - 0x{:08X} | Rozmiar: {} ({:.1}%)] ---\n\n",
            base_offset,
            end_offset,
            humansize::format_size(total_size, humansize::DECIMAL),
            pct
        ));
    }
    out.push_str(&text);
    out
}

/// Sprawdza, czy plik jest obrazem/grafiką na podstawie rozszerzenia
pub fn is_image_file(name_or_path: &str) -> bool {
    let ext = name_or_path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "webp"
            | "ico"
            | "svg"
            | "svgz"
            | "tiff"
            | "tif"
            | "jfif"
            | "avif"
            | "heic"
            | "heif"
            | "psd"
            | "raw"
            | "cr2"
            | "nef"
            | "arw"
            | "dng"
    )
}

/// Sprawdza, czy plik jest plikiem wideo lub audio na podstawie rozszerzenia
pub fn is_media_file(name_or_path: &str) -> bool {
    let ext = name_or_path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "mp4"
            | "mkv"
            | "avi"
            | "mov"
            | "wmv"
            | "flv"
            | "webm"
            | "m4v"
            | "mpg"
            | "mpeg"
            | "ts"
            | "vob"
            | "3gp"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "aac"
            | "wma"
            | "m4a"
            | "opus"
    )
}

/// Formatuje podgląd szesnastkowy (Hex dump) dla plików binarnych
#[allow(dead_code)]
pub fn format_hex_dump(buf: &[u8], total_size: u64, is_truncated: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "--- [PLIK BINARNY | Rozmiar całkowity: {}] ---\n",
        humansize::format_size(total_size, humansize::DECIMAL)
    ));
    out.push_str("Offset(h)  00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F  Tekst ASCII\n");
    out.push_str("--------------------------------------------------------------------------\n");

    for (chunk_idx, chunk) in buf.chunks(16).enumerate() {
        if chunk_idx >= 2048 {
            out.push_str(&format!(
                "\n... [Ucięto wyświetlanie po 32 KB ze względów wydajnościowych (wczytano {})] ...\n",
                humansize::format_size(buf.len(), humansize::DECIMAL)
            ));
            break;
        }
        let offset = chunk_idx * 16;
        out.push_str(&format!("{:08X}   ", offset));

        // Bajty szesnastkowe
        for i in 0..16 {
            if i == 8 {
                out.push(' ');
            }
            if let Some(&b) = chunk.get(i) {
                out.push_str(&format!("{:02X} ", b));
            } else {
                out.push_str("   ");
            }
        }
        out.push_str(" |");

        // Reprezentacja ASCII
        for &b in chunk {
            if (32..=126).contains(&b) {
                out.push(b as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }

    if is_truncated {
        out.push_str(&format!(
            "\n--- [Koniec podglądu strumieniowego (rozmiar całego pliku: {})] ---",
            humansize::format_size(total_size, humansize::DECIMAL)
        ));
    }

    out
}

/// Odczytuje fragment pliku w trybie strumieniowym bez ładowania całego pliku do pamięci RAM.
/// Zwraca (sformatowana treść tekstowa lub hex, rozmiar całkowity pliku, czy jest binarny).
#[allow(dead_code)]
pub fn read_file_preview_stream(
    path: &Path,
    max_bytes: usize,
) -> Result<(String, u64, bool), String> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path).map_err(|e| format!("Nie można otworzyć pliku: {}", e))?;
    let total_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    let mut buf = Vec::with_capacity(max_bytes.min(65536));
    let mut take = (&mut file).take(max_bytes as u64);
    take.read_to_end(&mut buf)
        .map_err(|e| format!("Błąd strumieniowego odczytu pliku: {}", e))?;

    let is_truncated = total_size > buf.len() as u64;
    // Wykrywanie pliku binarnego (bajt zerowy w próbce)
    let is_binary = buf.iter().take(4096).any(|&b| b == 0);

    let content = if is_binary {
        format_hex_dump(&buf, total_size, is_truncated)
    } else {
        let text = String::from_utf8_lossy(&buf).to_string();
        if is_truncated {
            format!(
                "{}\n\n--- [Podgląd strumieniowy ucięto: wczytano {} z {}] ---",
                text,
                humansize::format_size(buf.len(), humansize::DECIMAL),
                humansize::format_size(total_size, humansize::DECIMAL)
            )
        } else {
            text
        }
    };

    Ok((content, total_size, is_binary))
}
