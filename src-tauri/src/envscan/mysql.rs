use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn scan_mysql(env: &HashMap<String, String>, roots: &ProbeRoots) -> EnvItem {
    let mut item = base("mysql", "MySQL", true, "mysql");
    item.port = "3306".into();
    let listening = winutil::tcp_open(3306);
    let service = winutil::service_exists(&[
        "MySQL80", "MySQL84", "MySQL90", "MySQL57", "MySQL", "MariaDB",
    ]);
    if let Some(s) = &service {
        item.service_name = s.clone();
    }
    let installs = discover_mysqls(env, roots);
    if let Some(hit) = installs
        .iter()
        .find(|i| i.active)
        .or_else(|| installs.first())
    {
        item.executable = hit.executable.clone();
        item.home_dir = hit.home_dir.clone();
        if !hit.version.is_empty() {
            item.version = hit.version.clone();
        }
    }
    match classify_listener(listening, service.is_some() || !installs.is_empty()) {
        "ok" => {
            item.status = "ok".into();
            item.summary = "3306 正在监听".into();
        }
        "not_running" => {
            item.status = "not_running".into();
            item.summary = "已安装但 3306 未监听，请按教程启动 MySQL 服务".into();
        }
        "port_busy" => {
            item.status = "port_busy".into();
            item.summary = "3306 已被占用，但未找到 MySQL 安装".into();
        }
        _ => {
            item.status = "not_installed".into();
            item.summary = "未检测到 MySQL".into();
        }
    }
    item.installs = installs;
    let svc = if item.service_name.is_empty() {
        "未检测到".to_string()
    } else {
        item.service_name.clone()
    };
    let listen = if listening {
        "127.0.0.1:3306 已打开"
    } else {
        "未监听"
    };
    let mut facts = vec![
        fact("版本", &item.version),
        fact("mysql 客户端", &item.executable),
        fact("安装 / basedir", &item.home_dir),
        fact("数据目录", &item.data_dir),
        fact("端口", "3306"),
        fact("Windows 服务", &svc),
        fact("监听状态", listen),
    ];
    if !item.installs.is_empty() {
        facts.push(fact("发现套数", &format!("{} 套", item.installs.len())));
    }
    item.facts = facts;
    item.configs = into_configs(cfgscan::mysql_configs(&item.home_dir, &item.data_dir));
    item
}

fn discover_mysqls(env: &HashMap<String, String>, roots: &ProbeRoots) -> Vec<EnvInstall> {
    let mut raw: Vec<(String, PathBuf)> = Vec::new();
    for exe in winutil::which_all("mysql", env) {
        raw.push(("PATH".into(), exe));
    }
    let vendor = roots.mysql_vendor_dirs.clone();
    for exe in winutil::walk_find_files(&vendor, "mysql.exe", 4, 8) {
        raw.push(("磁盘".into(), exe));
    }
    let mysql_names = probe::names(&roots.mysql_name_has);
    for home in winutil::child_homes(&roots.common, &mysql_names, r"bin\mysql.exe") {
        raw.push(("磁盘".into(), home.join("bin").join("mysql.exe")));
    }
    let mut installs = Vec::new();
    let active = winutil::which("mysql", env);
    for (source, exe) in raw {
        if !exe.is_file() {
            continue;
        }
        let exe_s = winutil::display_path(&exe);
        if installs
            .iter()
            .any(|i: &EnvInstall| i.executable.eq_ignore_ascii_case(&exe_s))
        {
            continue;
        }
        let home = exe
            .parent()
            .and_then(|p| p.parent())
            .map(winutil::display_path)
            .unwrap_or_else(|| exe_s.clone());
        let version = first_line(
            &winutil::run_capture(&exe, &["--version"], env, None)
                .map(|o| winutil::combined_output(&o))
                .unwrap_or_default(),
        );
        let active_flag = active
            .as_ref()
            .map(|a| winutil::same_path(a, &exe))
            .unwrap_or(false);
        installs.push(EnvInstall {
            source,
            home_dir: home,
            executable: exe_s,
            version,
            active: active_flag,
        });
    }
    installs
}
