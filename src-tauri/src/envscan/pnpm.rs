use super::*;
use crate::cfgscan;
use crate::winutil;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn scan_pnpm(env: &HashMap<String, String>) -> EnvItem {
    let mut item = base("pnpm", "pnpm", true, "nvm");
    match winutil::which("pnpm", env) {
        None => {
            item.status = "not_installed".into();
            item.summary = "未找到 pnpm。推荐用 nvm 装好 Node 后执行 npm i -g pnpm".into();
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
            if item.version.is_empty() {
                item.status = "wrong_version".into();
                item.summary = "pnpm 已找到但无法读取版本".into();
            } else {
                item.status = "ok".into();
                item.summary = format!("已安装 {}", item.version);
            }
        }
    }
    let store = winutil::env_get_ci(env, "PNPM_STORE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            winutil::dirs_home()
                .join("AppData")
                .join("Local")
                .join("pnpm")
                .join("store")
        });
    item.data_dir = if store.exists() {
        winutil::display_path(&store)
    } else {
        String::new()
    };
    item.facts = vec![
        fact("版本", &item.version),
        fact("可执行文件", &item.executable),
        fact("安装目录", &item.home_dir),
        fact("store 目录", &item.data_dir),
        fact("说明", "推荐在 nvm 当前 Node 下安装：npm i -g pnpm"),
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
