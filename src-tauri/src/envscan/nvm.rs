use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) fn scan_nvm(env: &HashMap<String, String>, roots: &ProbeRoots) -> EnvItem {
    let mut item = base("nvm", "nvm（推荐）", false, "nvm");
    let exe = winutil::which("nvm", env).or_else(|| {
        let mut homes = vec![winutil::env_get_ci(env, "NVM_HOME").map(PathBuf::from)];
        homes.extend(roots.nvm_homes.iter().cloned().map(Some));
        homes.push(Some(
            winutil::dirs_home()
                .join("AppData")
                .join("Roaming")
                .join("nvm"),
        ));
        homes
            .into_iter()
            .flatten()
            .find_map(|dir| winutil::first_existing(&[dir.join("nvm.exe"), dir.join("nvm.cmd")]))
    });
    match exe {
        None => {
            item.status = "not_installed".into();
            item.summary = "未找到 nvm。推荐用它管理 Node.js 和 pnpm，见安装教程与下载".into();
        }
        Some(exe) => {
            item.executable = winutil::display_path(&exe);
            if let Some(parent) = exe.parent() {
                item.home_dir = winutil::display_path(parent);
            }
            if let Some(home) = winutil::env_get_ci(env, "NVM_HOME") {
                item.home_dir = home.clone();
            }
            item.configs = into_configs(cfgscan::nvm_configs(&item.home_dir));
            if let Some(link) = winutil::env_get_ci(env, "NVM_SYMLINK") {
                item.data_dir = link.clone();
            } else if let Some(path) = item
                .configs
                .iter()
                .flat_map(|c| c.facts.iter())
                .find(|f| f.label == "当前 Node 链接")
            {
                item.data_dir = path.value.clone();
            } else {
                item.data_dir = PathBuf::from(&item.home_dir)
                    .join("nodejs")
                    .display()
                    .to_string();
            }
            item.version = winutil::pe_file_version(&exe)
                .or_else(|| winutil::uninstall_display_version("nvm", Path::new(&item.home_dir)))
                .unwrap_or_default();
            item.installs = discover_nvm_nodes(&item.home_dir, &item.data_dir);
            let current = item
                .installs
                .iter()
                .find(|i| i.active)
                .map(|i| i.version.as_str())
                .unwrap_or("未切换");
            item.status = "ok".into();
            item.summary = if item.version.is_empty() {
                format!("已找到 nvm，当前 Node {current}")
            } else {
                format!("nvm {}，当前 Node {current}", item.version)
            };
        }
    }
    if item.configs.is_empty() {
        item.configs = into_configs(cfgscan::nvm_configs(&item.home_dir));
    }
    let node_mirror = config_fact(&item.configs, "Node 镜像");
    let npm_mirror = config_fact(&item.configs, "npm 镜像");
    let current = item
        .installs
        .iter()
        .find(|i| i.active)
        .map(|i| i.version.clone())
        .unwrap_or_default();
    item.facts = vec![
        fact("版本", &item.version),
        fact("可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact("当前 Node 链接", &item.data_dir),
        fact("当前 Node 版本", &current),
        fact("已安装 Node", &format!("{} 套", item.installs.len())),
        fact("Node 镜像", &node_mirror),
        fact("npm 镜像", &npm_mirror),
        fact(
            "NVM_HOME",
            winutil::env_get_ci(env, "NVM_HOME")
                .map(|s| s.as_str())
                .unwrap_or("未设置"),
        ),
    ];
    item
}

fn discover_nvm_nodes(home_dir: &str, symlink: &str) -> Vec<EnvInstall> {
    let home = PathBuf::from(home_dir);
    let Ok(entries) = std::fs::read_dir(&home) else {
        return Vec::new();
    };
    let active = PathBuf::from(symlink)
        .canonicalize()
        .ok()
        .map(|p| winutil::display_path(&p));
    let mut installs = Vec::new();
    for e in entries.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if name == "nodejs" || name == "nodejs.bak" || name == "symlink" {
            continue;
        }
        let exe = dir.join("node.exe");
        if !exe.is_file() {
            continue;
        }
        let home_s = winutil::display_path(&dir);
        let canon = dir.canonicalize().ok().map(|p| winutil::display_path(&p));
        let active_flag = match (&active, &canon) {
            (Some(a), Some(c)) => a.eq_ignore_ascii_case(c),
            _ => false,
        };
        let version = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .trim_start_matches('v')
            .to_string();
        installs.push(EnvInstall {
            source: "nvm".into(),
            home_dir: home_s,
            executable: winutil::display_path(&exe),
            version,
            active: active_flag,
        });
    }
    installs.sort_by(|a, b| b.active.cmp(&a.active).then(b.version.cmp(&a.version)));
    installs
}
