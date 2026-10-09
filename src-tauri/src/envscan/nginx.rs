use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn scan_nginx(env: &HashMap<String, String>, roots: &ProbeRoots) -> EnvItem {
    let mut item = base("nginx", "Nginx", true, "nginx");
    item.port = "80".into();
    let listening = winutil::tcp_open(80);
    let service = winutil::service_exists(&["nginx", "Nginx"]);
    if let Some(s) = &service {
        item.service_name = s.clone();
    }
    let installs = discover_nginx(env, roots);
    if let Some(hit) = installs
        .iter()
        .find(|i| i.active)
        .or_else(|| installs.first())
    {
        item.executable = hit.executable.clone();
        item.home_dir = hit.home_dir.clone();
        item.version = hit.version.clone();
        let conf_dir = PathBuf::from(&hit.home_dir).join("conf");
        let logs_dir = PathBuf::from(&hit.home_dir).join("logs");
        if conf_dir.is_dir() {
            item.data_dir = conf_dir.display().to_string();
        } else if logs_dir.is_dir() {
            item.data_dir = logs_dir.display().to_string();
        } else {
            item.data_dir = hit.home_dir.clone();
        }
    }
    match classify_listener(listening, service.is_some() || !installs.is_empty()) {
        "ok" => {
            item.status = "ok".into();
            item.summary = "80 正在监听".into();
        }
        "not_running" => {
            item.status = "not_running".into();
            item.summary = "已安装但 80 未监听，请按教程启动 Nginx".into();
        }
        "port_busy" => {
            item.status = "port_busy".into();
            item.summary = "80 已被占用，但未找到 Nginx 安装".into();
        }
        _ => {
            item.status = "not_installed".into();
            item.summary = "未检测到 Nginx".into();
        }
    }
    item.installs = installs;
    let svc = if item.service_name.is_empty() {
        "未检测到".to_string()
    } else {
        item.service_name.clone()
    };
    item.facts = vec![
        fact("版本", &item.version),
        fact("可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact("配置目录", &item.data_dir),
        fact("端口", "80"),
        fact("Windows 服务", &svc),
        fact(
            "监听状态",
            if listening {
                "127.0.0.1:80 已打开"
            } else {
                "未监听"
            },
        ),
        fact("发现套数", &format!("{} 套", item.installs.len())),
    ];
    item.configs = into_configs(cfgscan::nginx_configs(&item.home_dir));
    item
}

fn discover_nginx(env: &HashMap<String, String>, roots: &ProbeRoots) -> Vec<EnvInstall> {
    let mut raw: Vec<(String, PathBuf)> = Vec::new();
    for exe in winutil::which_all("nginx", env) {
        raw.push(("PATH".into(), exe));
    }
    for p in roots.nginx_exes.clone() {
        if p.is_file() {
            raw.push(("磁盘".into(), p));
        }
    }
    for exe in winutil::walk_find_files(&roots.nginx_walk_dirs, "nginx.exe", 2, 4) {
        raw.push(("磁盘".into(), exe));
    }
    let nginx_names = probe::names(&roots.nginx_name_has);
    for home in winutil::child_homes(&roots.common, &nginx_names, "nginx.exe") {
        raw.push(("磁盘".into(), home.join("nginx.exe")));
    }
    let mut installs = Vec::new();
    let active = winutil::which("nginx", env);
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
            .map(winutil::display_path)
            .unwrap_or_else(|| exe_s.clone());
        let version = nginx_version_from_path(&home);
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
