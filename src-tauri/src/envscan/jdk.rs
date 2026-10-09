use super::*;
use crate::winutil;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) fn scan_jdk(env: &HashMap<String, String>, roots: &ProbeRoots) -> EnvItem {
    let mut item = base("jdk", "JDK 21", true, "jdk21");
    let active_exe = winutil::which("java", env);
    let installs = discover_jdks(env, active_exe.as_deref(), roots);

    if let Some(exe) = &active_exe {
        apply_active_java(&mut item, exe, env);
    }

    let has_21 = installs
        .iter()
        .any(|i| java_major_of(&i.version) == Some(21))
        || java_major_of(&item.version) == Some(21);

    if active_exe.is_none() {
        match (has_21, installs.is_empty()) {
            (true, _) => {
                item.status = "wrong_version".into();
                item.summary = "本机已找到 JDK 21，但当前 PATH 未指向它".into();
                if let Some(hit) = installs
                    .iter()
                    .find(|i| java_major_of(&i.version) == Some(21))
                {
                    item.home_dir = hit.home_dir.clone();
                    item.executable = hit.executable.clone();
                    item.version = hit.version.clone();
                }
            }
            (false, false) => {
                item.status = "wrong_version".into();
                item.summary = "已找到 Java，但当前 PATH 未指向可用的 JDK 21".into();
                item.executable = installs[0].executable.clone();
                item.home_dir = installs[0].home_dir.clone();
                item.version = installs[0].version.clone();
            }
            _ => {
                item.status = "not_installed".into();
                item.summary = "未找到 JDK，需要 21".into();
            }
        }
    } else if item.status == "wrong_version" && has_21 && java_major_of(&item.version) != Some(21) {
        item.summary = format!("PATH 未指向 JDK 21。{}", item.summary);
    }

    item.installs = installs;
    item.facts = jdk_facts(&item, env);
    item
}

fn apply_active_java(item: &mut EnvItem, exe: &Path, env: &HashMap<String, String>) {
    item.executable = winutil::display_path(exe);
    if let Some(home) = winutil::env_get_ci(env, "JAVA_HOME") {
        item.home_dir = home.clone();
    } else if let Some(parent) = exe.parent().and_then(|p| p.parent()) {
        item.home_dir = winutil::display_path(parent);
    }
    let text = java_version_text(exe, env);
    item.version = first_line(&text);
    match java_major_of(&item.version).or_else(|| winutil::parse_java_major(&text)) {
        Some(21) => {
            item.status = "ok".into();
            item.summary = format!("当前 PATH 使用 {}", item.version);
        }
        Some(n) => {
            item.status = "wrong_version".into();
            item.summary = format!("当前主版本是 {n}，需要 21");
        }
        None => {
            item.status = "wrong_version".into();
            item.summary = "无法解析 Java 版本".into();
        }
    }
}

fn jdk_facts(item: &EnvItem, env: &HashMap<String, String>) -> Vec<EnvFact> {
    let mut facts = vec![
        fact("当前版本", &item.version),
        fact("java 可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact(
            "JAVA_HOME",
            winutil::env_get_ci(env, "JAVA_HOME")
                .map(|s| s.as_str())
                .unwrap_or("未设置"),
        ),
    ];
    if !item.installs.is_empty() {
        facts.push(fact("发现套数", &format!("{} 套", item.installs.len())));
    }
    facts
}

fn discover_jdks(
    env: &HashMap<String, String>,
    active: Option<&Path>,
    roots: &ProbeRoots,
) -> Vec<EnvInstall> {
    let mut raw: Vec<(String, PathBuf)> = Vec::new();
    if let Some(home) = winutil::env_get_ci(env, "JAVA_HOME") {
        raw.push(("JAVA_HOME".into(), PathBuf::from(home)));
    }
    for exe in winutil::which_all("java", env) {
        if let Some(home) = exe.parent().and_then(|p| p.parent()) {
            raw.push(("PATH".into(), home.to_path_buf()));
        }
    }
    for home in winutil::collect_reg_string_values(
        "JavaHome",
        &[
            r"SOFTWARE\JavaSoft\JDK",
            r"SOFTWARE\JavaSoft\Java Development Kit",
            r"SOFTWARE\JavaSoft\Java Runtime Environment",
            r"SOFTWARE\WOW6432Node\JavaSoft\JDK",
            r"SOFTWARE\WOW6432Node\JavaSoft\Java Development Kit",
            r"SOFTWARE\Eclipse Adoptium",
            r"SOFTWARE\Semeru",
            r"SOFTWARE\Azul Systems\Zulu",
            r"SOFTWARE\BellSoft",
            r"SOFTWARE\Amazon Corretto",
        ],
    ) {
        raw.push(("注册表".into(), PathBuf::from(home)));
    }
    let mut disk_roots = roots.jdk_vendor_dirs.clone();
    disk_roots.push(winutil::dirs_home().join(".jdks"));
    for exe in winutil::walk_find_files(&disk_roots, "java.exe", 3, 12) {
        if let Some(home) = exe.parent().and_then(|p| p.parent()) {
            raw.push(("磁盘".into(), home.to_path_buf()));
        }
    }
    let jdk_parents = roots.with_common(&roots.jdk_parent_dirs);
    let jdk_names = probe::names(&roots.jdk_name_has);
    for home in winutil::child_homes(&jdk_parents, &jdk_names, r"bin\java.exe") {
        raw.push(("磁盘".into(), home));
    }

    let mut installs = Vec::new();
    for (source, home) in raw {
        let exe = home.join("bin").join("java.exe");
        if !exe.is_file() {
            continue;
        }
        let home_s = winutil::display_path(&home);
        if installs
            .iter()
            .any(|i: &EnvInstall| i.home_dir.eq_ignore_ascii_case(&home_s))
        {
            continue;
        }
        let text = java_version_text(&exe, env);
        let version = first_line(&text);
        let active_flag = active.map(|a| winutil::same_path(a, &exe)).unwrap_or(false);
        installs.push(EnvInstall {
            source,
            home_dir: home_s,
            executable: winutil::display_path(&exe),
            version,
            active: active_flag,
        });
    }
    installs.sort_by(|a, b| b.active.cmp(&a.active).then(a.home_dir.cmp(&b.home_dir)));
    installs
}
