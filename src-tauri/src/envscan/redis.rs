use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn scan_redis(env: &HashMap<String, String>, roots: &ProbeRoots) -> EnvItem {
    let mut item = base("redis", "Redis", true, "redisWindows");
    item.port = "6379".into();
    let listening = winutil::tcp_open(6379);
    let service = winutil::service_exists(&["Redis", "Redis64", "Redis-x64-5.0", "Memurai"]);
    if let Some(s) = &service {
        item.service_name = s.clone();
    }
    let installs = discover_redis(env, roots);
    if let Some(hit) = installs
        .iter()
        .find(|i| i.active)
        .or_else(|| installs.first())
    {
        item.executable = hit.executable.clone();
        item.home_dir = hit.home_dir.clone();
        item.version = hit.version.clone();
        let parent = PathBuf::from(&hit.home_dir);
        let data_candidates = [
            parent.join("data").join("redis"),
            parent.join("data"),
            parent.join("dump.rdb"),
        ];
        if let Some(d) = data_candidates.iter().find(|c| c.exists()) {
            item.data_dir = if d.is_file() {
                parent.display().to_string()
            } else {
                d.display().to_string()
            };
        } else {
            item.data_dir = hit.home_dir.clone();
        }
    }
    match classify_listener(listening, service.is_some() || !installs.is_empty()) {
        "ok" => {
            item.status = "ok".into();
            item.summary = "6379 正在监听".into();
            if let Some(cli) = winutil::which("redis-cli", env) {
                if let Ok(out) = winutil::run_capture(&cli, &["INFO", "server"], env, None) {
                    let text = winutil::combined_output(&out);
                    if let Some(line) = text.lines().find(|l| l.starts_with("redis_version:")) {
                        item.version = line.trim_start_matches("redis_version:").trim().to_string();
                    }
                }
            }
        }
        "not_running" => {
            item.status = "not_running".into();
            item.summary = "已安装但 6379 未监听，请按教程启动 Redis 服务".into();
        }
        "port_busy" => {
            item.status = "port_busy".into();
            item.summary = "6379 已被占用，但未找到 Redis 安装".into();
        }
        _ => {
            item.status = "not_installed".into();
            item.summary = "未检测到 Redis".into();
        }
    }
    item.installs = installs;
    item.facts = vec![
        fact("版本", &item.version),
        fact("可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact("数据目录", &item.data_dir),
        fact("端口", "6379"),
        fact(
            "Windows 服务",
            if item.service_name.is_empty() {
                "未检测到"
            } else {
                &item.service_name
            },
        ),
        fact(
            "监听状态",
            if listening {
                "127.0.0.1:6379 已打开"
            } else {
                "未监听"
            },
        ),
        fact("发现套数", &format!("{} 套", item.installs.len())),
    ];
    item.configs = into_configs(cfgscan::redis_configs(&item.home_dir));
    item
}

fn discover_redis(env: &HashMap<String, String>, roots: &ProbeRoots) -> Vec<EnvInstall> {
    let mut raw: Vec<(String, PathBuf)> = Vec::new();
    for name in ["redis-server", "redis-cli"] {
        for exe in winutil::which_all(name, env) {
            raw.push(("PATH".into(), exe));
        }
    }
    for p in roots.redis_exes.clone() {
        if p.is_file() {
            raw.push(("磁盘".into(), p));
        }
    }
    for exe in winutil::walk_find_files(&roots.redis_walk_dirs, "redis-server.exe", 2, 4) {
        raw.push(("磁盘".into(), exe));
    }
    let redis_names = probe::names(&roots.redis_name_has);
    for home in winutil::child_homes(&roots.common, &redis_names, "redis-server.exe") {
        raw.push(("磁盘".into(), home.join("redis-server.exe")));
    }
    let mut installs = Vec::new();
    let active = winutil::which("redis-server", env).or_else(|| winutil::which("redis-cli", env));
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
