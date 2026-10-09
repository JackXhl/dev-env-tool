use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;

pub(crate) fn scan_node(env: &HashMap<String, String>) -> EnvItem {
    let mut item = base("node", "Node.js 18+", true, "nvm");
    match winutil::which("node", env) {
        None => {
            item.status = "not_installed".into();
            item.summary = "未找到 node。请先按教程安装 nvm，再用 nvm 安装 18+".into();
        }
        Some(exe) => {
            item.executable = winutil::display_path(&exe);
            if let Some(parent) = exe.parent() {
                item.home_dir = winutil::display_path(parent);
            }
            let out = winutil::run_capture(&exe, &["-v"], env, None).ok();
            let text = out
                .as_ref()
                .map(winutil::combined_output)
                .unwrap_or_default();
            item.version = first_line(&text);
            let major = item
                .version
                .trim()
                .trim_start_matches('v')
                .split('.')
                .next()
                .and_then(|s| s.parse::<u32>().ok());
            match major {
                Some(n) if n >= 18 => {
                    item.status = "ok".into();
                    item.summary = format!("已安装 {}", item.version);
                }
                Some(n) => {
                    item.status = "wrong_version".into();
                    item.summary = format!("当前主版本是 {n}，需要 18+");
                }
                None => {
                    item.status = "wrong_version".into();
                    item.summary = "无法解析 Node 版本".into();
                }
            }
        }
    }
    let via_nvm = item.executable.to_ascii_lowercase().contains("nvm")
        || winutil::env_get_ci(env, "NVM_SYMLINK")
            .map(|s| {
                item.executable
                    .to_ascii_lowercase()
                    .contains(&s.to_ascii_lowercase())
            })
            .unwrap_or(false);
    item.facts = vec![
        fact("版本", &item.version),
        fact("可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact(
            "管理方式",
            if via_nvm {
                "nvm"
            } else {
                "建议改用 nvm 管理"
            },
        ),
    ];
    item.configs = into_configs(cfgscan::npmrc_configs());
    if let Some(reg) = item
        .configs
        .iter()
        .flat_map(|c| c.facts.iter())
        .find(|f| f.label.eq_ignore_ascii_case("registry"))
    {
        item.facts.push(fact("npm 镜像", &reg.value));
    }
    item
}
