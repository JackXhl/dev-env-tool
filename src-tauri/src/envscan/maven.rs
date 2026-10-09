use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) fn scan_maven(env: &HashMap<String, String>, roots: &ProbeRoots) -> EnvItem {
    let mut item = base("maven", "Maven 3.x+", true, "maven");
    let active_exe = winutil::which("mvn", env);
    let installs = discover_mavens(env, active_exe.as_deref(), roots);

    if let Some(exe) = &active_exe {
        item.executable = winutil::display_path(exe);
        if let Some(home) =
            winutil::env_get_ci(env, "MAVEN_HOME").or_else(|| winutil::env_get_ci(env, "M2_HOME"))
        {
            item.home_dir = home.clone();
        } else if let Some(parent) = exe.parent().and_then(|p| p.parent()) {
            item.home_dir = winutil::display_path(parent);
        }
        let text = maven_version_text(exe, env);
        item.version = first_line(&text);
        match winutil::parse_maven_major(&text) {
            Some(n) if n >= 3 => {
                item.status = "ok".into();
                item.summary = format!("当前 PATH 使用 {}", item.version);
            }
            Some(n) => {
                item.status = "wrong_version".into();
                item.summary = format!("当前主版本是 {n}，需要 3.x+");
            }
            None => {
                item.status = "wrong_version".into();
                item.summary = "无法解析 Maven 版本".into();
            }
        }
    } else if let Some(hit) = installs.first() {
        item.executable = hit.executable.clone();
        item.home_dir = hit.home_dir.clone();
        item.version = hit.version.clone();
        if maven_ok(&hit.version) {
            item.status = "wrong_version".into();
            item.summary = "本机已找到 Maven，但当前 PATH 未指向它".into();
        } else {
            item.status = "not_installed".into();
            item.summary = "未找到可用的 Maven 3".into();
        }
    } else {
        item.status = "not_installed".into();
        item.summary = "未找到 mvn".into();
    }

    let home_ref = Path::new(&item.home_dir);
    item.data_dir = winutil::read_maven_local_repo(env, home_ref.is_dir().then_some(home_ref))
        .display()
        .to_string();
    item.installs = installs;
    item.facts = vec![
        fact("当前版本", &item.version),
        fact("mvn 可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact(
            "MAVEN_HOME",
            winutil::env_get_ci(env, "MAVEN_HOME")
                .or_else(|| winutil::env_get_ci(env, "M2_HOME"))
                .map(|s| s.as_str())
                .unwrap_or("未设置"),
        ),
        fact("本地仓库", &item.data_dir),
        fact("发现套数", &format!("{} 套", item.installs.len())),
    ];
    item.configs = into_configs(cfgscan::maven_configs(env, &item.home_dir));
    if let Some(cfg) = item.configs.iter().find(|c| c.exists) {
        if let Some(mirrors) = cfg.facts.iter().find(|f| f.label == "镜像源") {
            item.facts.push(fact("settings 镜像", &mirrors.value));
        }
    }
    item
}

fn discover_mavens(
    env: &HashMap<String, String>,
    active: Option<&Path>,
    roots: &ProbeRoots,
) -> Vec<EnvInstall> {
    let mut raw: Vec<(String, PathBuf)> = Vec::new();
    if let Some(home) =
        winutil::env_get_ci(env, "MAVEN_HOME").or_else(|| winutil::env_get_ci(env, "M2_HOME"))
    {
        raw.push(("MAVEN_HOME".into(), PathBuf::from(home)));
    }
    for exe in winutil::which_all("mvn", env) {
        if let Some(home) = exe.parent().and_then(|p| p.parent()) {
            raw.push(("PATH".into(), home.to_path_buf()));
        }
    }
    let disk_roots = roots.maven_vendor_dirs.clone();
    for exe in winutil::walk_find_files(&disk_roots, "mvn.cmd", 3, 6) {
        if let Some(home) = exe.parent().and_then(|p| p.parent()) {
            raw.push(("磁盘".into(), home.to_path_buf()));
        }
    }
    let maven_names = probe::names(&roots.maven_name_has);
    for home in winutil::child_homes(&roots.common, &maven_names, r"bin\mvn.cmd") {
        raw.push(("磁盘".into(), home));
    }
    let mut installs = Vec::new();
    for (source, home) in raw {
        let exe = ["mvn.cmd", "mvn.bat", "mvn"]
            .iter()
            .map(|n| home.join("bin").join(n))
            .find(|p| p.is_file());
        let Some(exe) = exe else {
            continue;
        };
        let home_s = winutil::display_path(&home);
        if installs
            .iter()
            .any(|i: &EnvInstall| i.home_dir.eq_ignore_ascii_case(&home_s))
        {
            continue;
        }
        let version = first_line(&maven_version_text(&exe, env));
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
