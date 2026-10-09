pub mod probe;

mod jdk;
mod maven;
mod mysql;
mod nginx;
mod node;
mod nvm;
mod pnpm;
mod redis;

use crate::cfgscan;
use crate::winutil;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub use probe::ProbeRoots;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvFact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvInstall {
    pub source: String,
    pub home_dir: String,
    pub executable: String,
    pub version: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvItem {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub status: String,
    pub summary: String,
    pub version: String,
    pub executable: String,
    pub home_dir: String,
    pub data_dir: String,
    pub port: String,
    pub service_name: String,
    pub link_key: String,
    pub facts: Vec<EnvFact>,
    pub installs: Vec<EnvInstall>,
    pub configs: Vec<EnvConfig>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvConfig {
    pub kind: String,
    pub path: String,
    pub exists: bool,
    pub facts: Vec<EnvFact>,
    pub preview: String,
}

pub fn scan(emit: impl Fn(&EnvItem)) -> Vec<EnvItem> {
    let env = winutil::overlay_registry_env();
    let roots = ProbeRoots::load();
    let mut items = Vec::new();

    for mut item in [
        jdk::scan_jdk(&env, &roots),
        maven::scan_maven(&env, &roots),
        mysql::scan_mysql(&env, &roots),
        redis::scan_redis(&env, &roots),
        nginx::scan_nginx(&env, &roots),
        nvm::scan_nvm(&env, &roots),
        node::scan_node(&env),
        pnpm::scan_pnpm(&env),
    ] {
        fill_unknown(&mut item);
        emit(&item);
        items.push(item);
    }
    items
}

fn fill_unknown(item: &mut EnvItem) {
    if item.version.trim().is_empty() {
        item.version = "未知".into();
    }
    if item.executable.trim().is_empty() {
        item.executable = "未知".into();
    }
    if item.home_dir.trim().is_empty() {
        item.home_dir = "未知".into();
    }
    if item.data_dir.trim().is_empty() {
        item.data_dir = "未知".into();
    }
    if item.port.trim().is_empty() {
        item.port = "—".into();
    }
    if item.service_name.trim().is_empty() {
        item.service_name = "—".into();
    }
    for f in &mut item.facts {
        if f.value.trim().is_empty() {
            f.value = "未知".into();
        }
    }
}

pub(crate) fn base(id: &str, name: &str, required: bool, link_key: &str) -> EnvItem {
    EnvItem {
        id: id.into(),
        name: name.into(),
        required,
        status: "not_installed".into(),
        summary: String::new(),
        version: String::new(),
        executable: String::new(),
        home_dir: String::new(),
        data_dir: String::new(),
        port: String::new(),
        service_name: String::new(),
        link_key: link_key.into(),
        facts: Vec::new(),
        installs: Vec::new(),
        configs: Vec::new(),
    }
}

pub(crate) fn into_configs(list: Vec<cfgscan::ConfigOut>) -> Vec<EnvConfig> {
    list.into_iter()
        .map(|c| EnvConfig {
            kind: c.kind,
            path: c.path,
            exists: c.exists,
            facts: c
                .facts
                .into_iter()
                .map(|(label, value)| EnvFact { label, value })
                .collect(),
            preview: c.preview,
        })
        .collect()
}

pub(crate) fn fact(label: &str, value: &str) -> EnvFact {
    EnvFact {
        label: label.into(),
        value: value.into(),
    }
}

pub(crate) fn first_line(text: &str) -> String {
    text.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

pub(crate) fn config_fact(configs: &[EnvConfig], label: &str) -> String {
    configs
        .iter()
        .flat_map(|c| c.facts.iter())
        .find(|f| f.label == label)
        .map(|f| f.value.clone())
        .unwrap_or_default()
}

pub(crate) fn java_version_text(exe: &Path, env: &HashMap<String, String>) -> String {
    winutil::run_capture(exe, &["-version"], env, None)
        .map(|o| winutil::combined_output(&o))
        .unwrap_or_default()
}

pub(crate) fn maven_version_text(exe: &Path, env: &HashMap<String, String>) -> String {
    winutil::run_capture(exe, &["-version"], env, None)
        .map(|o| winutil::combined_output(&o))
        .unwrap_or_default()
}

pub(crate) fn java_major_of(version: &str) -> Option<u32> {
    winutil::parse_java_major(version).or_else(|| {
        let re = regex::Regex::new(r"\b(1\.)?(\d{1,2})\b").ok()?;
        re.captures(version)
            .and_then(|c| c.get(2))
            .and_then(|m| m.as_str().parse().ok())
    })
}

pub(crate) fn maven_ok(version: &str) -> bool {
    winutil::parse_maven_major(version)
        .map(|n| n >= 3)
        .unwrap_or(false)
}

pub(crate) fn nginx_version_from_path(home: &str) -> String {
    let name = Path::new(home)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(home);
    let re = regex::Regex::new(r"(?i)nginx[-_ ]?(\d+(?:\.\d+){1,3})").ok();
    if let Some(v) = re.and_then(|r| r.captures(name)).and_then(|c| c.get(1)) {
        return format!("nginx/{}", v.as_str());
    }
    String::new()
}

pub(crate) fn classify_listener(listening: bool, installed: bool) -> &'static str {
    match (listening, installed) {
        (true, true) => "ok",
        (false, true) => "not_running",
        (true, false) => "port_busy",
        (false, false) => "not_installed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_classes() {
        assert_eq!(classify_listener(true, true), "ok");
        assert_eq!(classify_listener(true, false), "port_busy");
        assert_eq!(classify_listener(false, true), "not_running");
        assert_eq!(classify_listener(false, false), "not_installed");
    }

    #[test]
    fn first_line_skips_blanks() {
        assert_eq!(first_line("\n  Apache Maven 3.9.9\n"), "Apache Maven 3.9.9");
        assert_eq!(first_line(""), "");
    }

    #[test]
    fn nginx_version_from_folder_name() {
        assert_eq!(
            nginx_version_from_path(r"D:\tools\nginx-1.24.0"),
            "nginx/1.24.0"
        );
        assert_eq!(nginx_version_from_path(r"C:\nginx"), "");
    }

    #[test]
    #[ignore = "本机全量探查，CI 不跑"]
    fn scan_skeleton_lists_paths() {
        let items = scan(|_| {});
        let ids: Vec<_> = items.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"jdk"));
        assert!(ids.contains(&"maven"));
        assert!(ids.contains(&"node"));
        assert!(ids.contains(&"nvm"));
        assert!(ids.contains(&"pnpm"));
        assert!(ids.contains(&"mysql"));
        assert!(ids.contains(&"redis"));
        assert!(ids.contains(&"nginx"));
        assert!(!ids.contains(&"wechat"));
        for it in &items {
            assert!(!it.executable.is_empty());
            assert!(!it.home_dir.is_empty());
            assert!(!it.data_dir.is_empty());
            assert!(!it.facts.is_empty());
        }
        let maven = items.iter().find(|i| i.id == "maven").unwrap();
        if maven.executable.to_ascii_lowercase().contains("mvn.cmd") {
            assert_eq!(maven.status, "ok", "{}", maven.summary);
            assert!(!maven.configs.is_empty());
        }
        let mysql = items.iter().find(|i| i.id == "mysql").unwrap();
        assert!(!mysql.configs.is_empty());
        let redis = items.iter().find(|i| i.id == "redis").unwrap();
        assert!(!redis.configs.is_empty());
        let nginx = items.iter().find(|i| i.id == "nginx").unwrap();
        assert!(!nginx.configs.is_empty());
        let nvm = items.iter().find(|i| i.id == "nvm").unwrap();
        assert!(!nvm.configs.is_empty());
        if nvm.status == "ok" {
            assert!(nvm.facts.iter().any(|f| f.label == "Node 镜像"));
            assert!(nvm.facts.iter().any(|f| f.label == "npm 镜像"));
            assert_ne!(nvm.version, "未知", "{}", nvm.summary);
            assert!(
                nvm.version.chars().next().unwrap().is_ascii_digit(),
                "nvm version {}",
                nvm.version
            );
        }
        let pnpm = items.iter().find(|i| i.id == "pnpm").unwrap();
        if pnpm.executable.to_ascii_lowercase().contains("pnpm.cmd")
            || pnpm.executable.to_ascii_lowercase().contains("pnpm.exe")
        {
            assert_eq!(pnpm.status, "ok", "{}", pnpm.summary);
        }
        if redis.status == "not_running" {
            assert!(redis.summary.contains("未监听"));
        }
    }
}
