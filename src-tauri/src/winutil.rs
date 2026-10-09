use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use winreg::enums::*;
use winreg::RegKey;

#[link(name = "version")]
extern "system" {
    fn GetFileVersionInfoSizeW(filename: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(filename: *const u16, handle: u32, len: u32, data: *mut u8) -> i32;
    fn VerQueryValueW(
        block: *const u8,
        sub_block: *const u16,
        buf: *mut *mut c_void,
        len: *mut u32,
    ) -> i32;
}

#[repr(C)]
struct VsFixedFileInfo {
    signature: u32,
    struc_version: u32,
    file_version_ms: u32,
    file_version_ls: u32,
    product_version_ms: u32,
    product_version_ls: u32,
    file_flags_mask: u32,
    file_flags: u32,
    file_os: u32,
    file_type: u32,
    file_subtype: u32,
    file_date_ms: u32,
    file_date_ls: u32,
}

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn overlay_registry_env() -> HashMap<String, String> {
    let mut map: HashMap<String, String> = std::env::vars().collect();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    overlay_from_key(
        &hklm,
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        &mut map,
    );
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    overlay_from_key(&hkcu, "Environment", &mut map);

    let machine_path = read_reg_path(
        &hklm,
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    );
    let user_path = read_reg_path(&hkcu, "Environment");
    let mut path = String::new();
    if !user_path.is_empty() {
        path.push_str(&user_path);
    }
    if !machine_path.is_empty() {
        if !path.is_empty() && !path.ends_with(';') {
            path.push(';');
        }
        path.push_str(&machine_path);
    }
    if !path.is_empty() {
        map.insert("Path".into(), path.clone());
        map.insert("PATH".into(), path);
    }

    let snapshot = map.clone();
    for (k, v) in snapshot {
        map.insert(k, expand_with(&v, &map));
    }
    map
}

fn overlay_from_key(root: &RegKey, sub: &str, map: &mut HashMap<String, String>) {
    let Ok(key) = root.open_subkey(sub) else {
        return;
    };
    for (name, _) in key.enum_values().filter_map(|x| x.ok()) {
        if name.eq_ignore_ascii_case("Path") {
            continue;
        }
        if let Ok(s) = key.get_value::<String, _>(&name) {
            map.insert(name, s);
        }
    }
}

fn read_reg_path(root: &RegKey, sub: &str) -> String {
    let Ok(key) = root.open_subkey(sub) else {
        return String::new();
    };
    key.get_value::<String, _>("Path")
        .or_else(|_| key.get_value::<String, _>("PATH"))
        .unwrap_or_default()
}

fn expand_with(raw: &str, map: &HashMap<String, String>) -> String {
    let mut out = raw.to_string();
    for _ in 0..8 {
        let mut next = String::new();
        let mut chars = out.chars().peekable();
        let mut changed = false;
        while let Some(c) = chars.next() {
            if c == '%' {
                let mut name = String::new();
                let mut closed = false;
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == '%' {
                        closed = true;
                        break;
                    }
                    name.push(n);
                }
                if closed {
                    if let Some(v) = map_get_ci(map, &name) {
                        next.push_str(v);
                        changed = true;
                        continue;
                    }
                    next.push('%');
                    next.push_str(&name);
                    next.push('%');
                } else {
                    next.push('%');
                    next.push_str(&name);
                }
            } else {
                next.push(c);
            }
        }
        out = next;
        if !changed {
            break;
        }
    }
    out
}

fn map_get_ci<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a String> {
    map.get(key).or_else(|| {
        map.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    })
}

pub fn env_get_ci<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a String> {
    map_get_ci(map, key)
}

pub fn which(name: &str, env: &HashMap<String, String>) -> Option<PathBuf> {
    which_all(name, env).into_iter().next()
}

pub fn which_all(name: &str, env: &HashMap<String, String>) -> Vec<PathBuf> {
    let Some(path) = env_get_ci(env, "PATH").cloned() else {
        return Vec::new();
    };
    let pathext = env_get_ci(env, "PATHEXT")
        .cloned()
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    let has_ext = Path::new(name).extension().is_some();
    let mut names = Vec::new();
    if has_ext {
        names.push(name.to_string());
    }
    for ext in pathext.split(';').filter(|s| !s.is_empty()) {
        if name
            .to_ascii_uppercase()
            .ends_with(&ext.to_ascii_uppercase())
        {
            continue;
        }
        names.push(format!("{name}{ext}"));
    }
    if !has_ext {
        names.push(name.to_string());
    }
    let mut found = Vec::new();
    for dir in path.split(';').filter(|s| !s.is_empty()) {
        for n in &names {
            let p = Path::new(dir).join(n);
            if p.is_file() && !found.iter().any(|x: &PathBuf| same_path(x, &p)) {
                found.push(p);
            }
        }
    }
    found
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    display_path(a).eq_ignore_ascii_case(&display_path(b))
}

pub fn display_path(p: &Path) -> String {
    p.to_string_lossy().trim_start_matches(r"\\?\").to_string()
}

pub fn child_homes(parents: &[PathBuf], name_has: &[&str], rel_exe: &str) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    for parent in parents {
        let Ok(entries) = std::fs::read_dir(parent) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !name_has.iter().any(|h| name.contains(h)) {
                continue;
            }
            let exe = p.join(rel_exe);
            if exe.is_file() {
                homes.push(p);
            }
        }
    }
    homes
}

pub fn collect_reg_string_values(value_name: &str, roots: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for sub in roots {
        if let Ok(key) = hklm.open_subkey(sub) {
            walk_reg_string(&key, value_name, 0, 6, &mut out);
        }
    }
    out
}

fn walk_reg_string(
    key: &RegKey,
    value_name: &str,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<String>,
) {
    if depth > max_depth {
        return;
    }
    if let Ok(v) = key.get_value::<String, _>(value_name) {
        if !v.trim().is_empty() {
            out.push(v);
        }
    }
    for name in key.enum_keys().filter_map(|x| x.ok()) {
        if let Ok(sub) = key.open_subkey(&name) {
            walk_reg_string(&sub, value_name, depth + 1, max_depth, out);
        }
    }
}

pub fn apply_env(cmd: &mut Command, env: &HashMap<String, String>) {
    cmd.env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.creation_flags(CREATE_NO_WINDOW);
}

pub fn command_for(
    program: &Path,
    args: &[&str],
    env: &HashMap<String, String>,
    cwd: Option<&Path>,
) -> Command {
    let ext = program
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut cmd = if ext == "cmd" || ext == "bat" {
        let mut c = Command::new("cmd");
        c.arg("/C");
        c.arg(program);
        c.args(args);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    apply_env(&mut cmd, env);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd
}

pub fn run_capture(
    program: &Path,
    args: &[&str],
    env: &HashMap<String, String>,
    cwd: Option<&Path>,
) -> std::io::Result<Output> {
    let mut cmd = command_for(program, args, env, cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());
    let mut child = cmd.spawn()?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stdout.take() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stderr.take() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        match child.try_wait()? {
            Some(st) => break st,
            None if Instant::now() >= deadline => {
                kill_process_tree(child.id());
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_h.join();
                let _ = err_h.join();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("{} 超时", program.display()),
                ));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let stdout = out_h.join().unwrap_or_default();
    let stderr = err_h.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub fn pe_file_version(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut dummy = 0u32;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut dummy);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr()) == 0 {
            return None;
        }
        let mut len = 0u32;
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let root: Vec<u16> = "\\".encode_utf16().chain(Some(0)).collect();
        if VerQueryValueW(buf.as_ptr(), root.as_ptr(), &mut ptr, &mut len) == 0 || ptr.is_null() {
            return None;
        }
        let info = &*(ptr as *const VsFixedFileInfo);
        if info.signature != 0xFEEF_04BD {
            return None;
        }
        let major = info.product_version_ms >> 16;
        let minor = info.product_version_ms & 0xFFFF;
        let patch = info.product_version_ls >> 16;
        Some(format!("{major}.{minor}.{patch}"))
    }
}

pub fn uninstall_display_version(name_has: &str, home: &Path) -> Option<String> {
    let roots = [
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];
    let ver_re = regex::Regex::new(r"(\d+\.\d+\.\d+(?:\.\d+)?)").ok();
    let mut fallback = None;
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let root_key = RegKey::predef(hive);
        for root in roots {
            let Ok(key) = root_key.open_subkey(root) else {
                continue;
            };
            for name in key.enum_keys().flatten() {
                let Ok(sub) = key.open_subkey(&name) else {
                    continue;
                };
                let display: String = sub.get_value("DisplayName").unwrap_or_default();
                if !display
                    .to_ascii_lowercase()
                    .contains(&name_has.to_ascii_lowercase())
                {
                    continue;
                }
                let loc: String = sub.get_value("InstallLocation").unwrap_or_default();
                let ver = sub
                    .get_value::<String, _>("DisplayVersion")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        ver_re.as_ref().and_then(|re| {
                            re.captures(&display)
                                .and_then(|c| c.get(1))
                                .map(|m| m.as_str().to_string())
                        })
                    });
                let Some(ver) = ver else {
                    continue;
                };
                if !loc.is_empty() && same_path(Path::new(loc.trim_end_matches(['\\', '/'])), home)
                {
                    return Some(ver);
                }
                if fallback.is_none() {
                    fallback = Some(ver);
                }
            }
        }
    }
    fallback
}

fn kill_process_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn combined_output(out: &Output) -> String {
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.is_empty() {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&err);
    }
    s
}

pub fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

pub fn parse_java_major(text: &str) -> Option<u32> {
    let re = regex::Regex::new(r##"version\s+"?(\d+)"##).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

pub fn parse_maven_major(text: &str) -> Option<u32> {
    let re = regex::Regex::new(r"Apache Maven\s+(\d+)").ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

pub fn default_m2_repo() -> PathBuf {
    dirs_home().join(".m2").join("repository")
}

pub fn dirs_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Default"))
}

pub fn read_maven_local_repo(env: &HashMap<String, String>, maven_home: Option<&Path>) -> PathBuf {
    let mut repo = default_m2_repo();
    let mut files = Vec::new();
    if let Some(home) = maven_home.map(|p| p.to_path_buf()).or_else(|| {
        env_get_ci(env, "MAVEN_HOME")
            .or_else(|| env_get_ci(env, "M2_HOME"))
            .map(PathBuf::from)
    }) {
        files.push(home.join("conf").join("settings.xml"));
    }
    files.push(dirs_home().join(".m2").join("settings.xml"));
    for f in files {
        if let Some(p) = local_repo_from_settings(&f, env) {
            repo = p;
        }
    }
    repo
}

fn local_repo_from_settings(path: &Path, env: &HashMap<String, String>) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let stripped = strip_xml_comments(&text);
    let re = regex::Regex::new(r"<localRepository>\s*([^<]+)\s*</localRepository>").ok()?;
    let raw = re.captures(&stripped)?.get(1)?.as_str().trim();
    if raw.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_with(raw, env)))
}

pub fn strip_xml_comments(text: &str) -> String {
    regex::Regex::new(r"(?s)<!--.*?-->")
        .map(|re| re.replace_all(text, "").into_owned())
        .unwrap_or_else(|_| text.to_string())
}

pub fn open_folder(path: &Path) -> Result<(), String> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| "找不到所在文件夹".to_string())?
            .to_path_buf()
    };
    if !dir.exists() {
        return Err(format!("文件夹不存在：{}", dir.display()));
    }
    Command::new("explorer")
        .arg(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn open_file(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    Command::new("explorer")
        .arg(format!("/select,{}", display_path(path)))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn tcp_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(400),
    )
    .is_ok()
}

pub fn service_exists(names: &[&str]) -> Option<String> {
    for name in names {
        let out = Command::new("sc")
            .args(["query", name])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(o) = out {
            let text = combined_output(&o);
            if (text.contains("SERVICE_NAME") || text.contains(name))
                && !text.contains("1060")
                && !text.to_ascii_lowercase().contains("does not exist")
            {
                return Some((*name).to_string());
            }
        }
    }
    None
}

pub fn walk_find_files(
    roots: &[PathBuf],
    file_name: &str,
    max_depth: usize,
    cap: usize,
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        if root.as_os_str().is_empty() {
            continue;
        }
        walk_all(root, file_name, 0, max_depth, cap, &mut found);
        if found.len() >= cap {
            break;
        }
    }
    found
}

fn walk_all(
    dir: &Path,
    file_name: &str,
    depth: usize,
    max_depth: usize,
    cap: usize,
    found: &mut Vec<PathBuf>,
) {
    if depth > max_depth || found.len() >= cap {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.file_name().and_then(|n| n.to_str()) == Some(file_name) {
            if !found.iter().any(|x| same_path(x, &p)) {
                found.push(p);
            }
            if found.len() >= cap {
                return;
            }
        } else if p.is_dir() {
            dirs.push(p);
        }
    }
    for d in dirs {
        walk_all(&d, file_name, depth + 1, max_depth, cap, found);
        if found.len() >= cap {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pe_version_reads_system_exe() {
        let cmd = PathBuf::from(r"C:\Windows\System32\cmd.exe");
        let v = pe_file_version(&cmd).expect("cmd.exe version");
        assert!(v.chars().next().unwrap().is_ascii_digit(), "{v}");
    }

    #[test]
    fn nvm_uninstall_version_if_present() {
        let home = PathBuf::from(r"D:\tools\nvm");
        if !home.is_dir() {
            return;
        }
        let v = uninstall_display_version("nvm", &home);
        if let Some(v) = v {
            assert!(v.chars().next().unwrap().is_ascii_digit(), "{v}");
        }
    }

    #[test]
    fn java_and_maven_majors() {
        assert_eq!(
            parse_java_major(r#"openjdk version "21.0.8" 2025-07-15 LTS"#),
            Some(21)
        );
        assert_eq!(parse_java_major(r#"java version "17.0.2""#), Some(17));
        assert_eq!(parse_maven_major("Apache Maven 3.9.9"), Some(3));
    }

    #[test]
    fn maven_repo_skips_commented_and_reads_global() {
        let xml = r#"
        <!-- localRepository
        <localRepository>/path/to/local/repo</localRepository>
        -->
        <localRepository>D:\Maven_Repo</localRepository>
        "#;
        let stripped = strip_xml_comments(xml);
        assert!(!stripped.contains("/path/to/local/repo"));
        assert!(stripped.contains(r"D:\Maven_Repo"));
        let re = regex::Regex::new(r"<localRepository>\s*([^<]+)\s*</localRepository>").unwrap();
        let raw = re
            .captures(&stripped)
            .unwrap()
            .get(1)
            .unwrap()
            .as_str()
            .trim();
        assert_eq!(raw, r"D:\Maven_Repo");
    }

    #[test]
    fn run_capture_kills_cmd_grandchild() {
        let env: HashMap<_, _> = std::env::vars().collect();
        let dir = std::env::temp_dir().join(format!("run-capture-hang-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bat = dir.join("hang.cmd");
        std::fs::write(&bat, "@echo off\r\nping -n 40 127.0.0.1 >nul\r\n").unwrap();
        let start = Instant::now();
        let err = run_capture(&bat, &[], &env, None).expect_err("should time out");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            start.elapsed() < Duration::from_secs(14),
            "elapsed {:?}",
            start.elapsed()
        );
    }
}
