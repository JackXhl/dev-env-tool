use crate::winutil;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigOut {
    pub kind: String,
    pub path: String,
    pub exists: bool,
    pub facts: Vec<(String, String)>,
    pub preview: String,
}

pub fn maven_configs(env: &HashMap<String, String>, maven_home: &str) -> Vec<ConfigOut> {
    let mut files = Vec::new();
    if Path::new(maven_home).is_dir() {
        files.push((
            "全局 settings.xml",
            PathBuf::from(maven_home).join("conf").join("settings.xml"),
        ));
    }
    files.push((
        "用户 settings.xml",
        winutil::dirs_home().join(".m2").join("settings.xml"),
    ));
    files
        .into_iter()
        .map(|(kind, path)| parse_maven_settings(kind, &path, env))
        .collect()
}

fn usable_dir(raw: &str) -> Option<PathBuf> {
    let p = PathBuf::from(raw);
    if raw.trim().is_empty() || p.as_os_str().is_empty() {
        return None;
    }
    Some(p)
}

pub fn mysql_configs(home_dir: &str, data_dir: &str) -> Vec<ConfigOut> {
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    if let Some(home) = usable_dir(home_dir) {
        let homes = [home.clone(), home.join("..")];
        for home in homes {
            for name in ["my.ini", "my.cnf"] {
                let p = home.join(name);
                if p.is_file() {
                    push_unique(&mut candidates, "安装目录配置", p);
                }
            }
        }
    }
    if let Some(data) = usable_dir(data_dir) {
        if data.is_dir() {
            for name in ["my.ini", "my.cnf"] {
                let p = data.join(name);
                if p.is_file() {
                    push_unique(&mut candidates, "数据目录配置", p);
                }
            }
        }
    }
    for p in winutil::walk_find_files(&[PathBuf::from(r"C:\ProgramData\MySQL")], "my.ini", 3, 6) {
        push_unique(&mut candidates, "ProgramData 配置", p);
    }
    let cnf = PathBuf::from(r"C:\my.cnf");
    if cnf.is_file() {
        push_unique(&mut candidates, "系统配置", cnf);
    }
    if candidates.is_empty() {
        let guess = usable_dir(home_dir)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData\MySQL"))
            .join("my.ini");
        return vec![missing_config("my.ini / my.cnf", &guess)];
    }
    candidates
        .into_iter()
        .map(|(kind, path)| parse_kv_config(&kind, &path, MYSQL_KEYS, MYSQL_SECRETS))
        .collect()
}

pub fn redis_configs(home_dir: &str) -> Vec<ConfigOut> {
    let Some(home) = usable_dir(home_dir) else {
        return vec![missing_config("redis.conf", Path::new("redis.conf"))];
    };
    let names = [
        "redis.windows.conf",
        "redis.windows-service.conf",
        "redis.conf",
    ];
    let mut files = Vec::new();
    for name in names {
        let p = home.join(name);
        if p.is_file() {
            files.push((format!("配置 {name}"), p));
        }
    }
    if files.is_empty() {
        for name in names {
            for p in winutil::walk_find_files(std::slice::from_ref(&home), name, 2, 3) {
                files.push((format!("配置 {name}"), p));
            }
        }
    }
    if files.is_empty() {
        return vec![missing_config(
            "redis.conf",
            &home.join("redis.windows.conf"),
        )];
    }
    files
        .into_iter()
        .map(|(kind, path)| parse_kv_config(&kind, &path, REDIS_KEYS, REDIS_SECRETS))
        .collect()
}

pub fn nginx_configs(home_dir: &str) -> Vec<ConfigOut> {
    let Some(home) = usable_dir(home_dir) else {
        return vec![missing_config("nginx.conf", Path::new("nginx.conf"))];
    };
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let main = home.join("conf").join("nginx.conf");
    if main.is_file() {
        files.push(("主配置 nginx.conf".into(), main));
    } else {
        for p in winutil::walk_find_files(std::slice::from_ref(&home), "nginx.conf", 2, 4) {
            files.push(("配置 nginx.conf".into(), p));
        }
    }
    if files.is_empty() {
        return vec![missing_config(
            "nginx.conf",
            &home.join("conf").join("nginx.conf"),
        )];
    }
    files
        .into_iter()
        .map(|(kind, path)| parse_kv_config(&kind, &path, NGINX_KEYS, &[]))
        .collect()
}

const NVM_SETTING_FIELDS: &[(&str, &str, &str)] = &[
    ("root", "安装根", ""),
    ("path", "当前 Node 链接", ""),
    ("arch", "架构", ""),
    ("proxy", "代理", "未设置"),
    (
        "node_mirror",
        "Node 镜像",
        "未配置（官方 https://nodejs.org/dist）",
    ),
    (
        "npm_mirror",
        "npm 镜像",
        "未配置（官方 https://registry.npmjs.org）",
    ),
];

pub fn nvm_configs(home_dir: &str) -> Vec<ConfigOut> {
    let Some(home) = usable_dir(home_dir) else {
        return vec![missing_config("settings.txt", Path::new("settings.txt"))];
    };
    let path = home.join("settings.txt");
    let mut cfg = empty_cfg("nvm settings.txt", &path, path.is_file());
    if !cfg.exists {
        cfg.facts.push(("状态".into(), "未找到配置文件".into()));
        return vec![cfg];
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        cfg.facts.push(("状态".into(), "无法读取".into()));
        return vec![cfg];
    };
    let pairs = parse_conf_pairs(&text);
    for (key, label, default) in NVM_SETTING_FIELDS {
        let value = pairs
            .iter()
            .rev()
            .find(|(k, v)| k.eq_ignore_ascii_case(key) && !v.is_empty())
            .map(|(_, v)| v.clone())
            .filter(|v| !v.eq_ignore_ascii_case("none"))
            .unwrap_or_else(|| (*default).into());
        if value.is_empty() {
            continue;
        }
        cfg.facts.push(((*label).into(), value));
    }
    if cfg.facts.is_empty() {
        cfg.facts
            .push(("状态".into(), "已找到文件，未解析到常用项".into()));
    }
    cfg.preview = pairs
        .iter()
        .take(24)
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");
    vec![cfg]
}

const NPMRC_KEYS: &[&str] = &[
    "registry",
    "prefix",
    "cache",
    "store-dir",
    "electron_mirror",
    "disturl",
    "sass_binary_site",
];

pub fn npmrc_configs() -> Vec<ConfigOut> {
    let path = winutil::dirs_home().join(".npmrc");
    vec![parse_kv_config("用户 .npmrc", &path, NPMRC_KEYS, &[])]
}

const MYSQL_KEYS: &[&str] = &[
    "port",
    "datadir",
    "basedir",
    "bind-address",
    "bind_address",
    "max_connections",
    "character-set-server",
    "collation-server",
    "innodb_buffer_pool_size",
    "default-storage-engine",
    "sql_mode",
    "tmpdir",
    "log-error",
    "mysqlx_port",
    "socket",
];
const MYSQL_SECRETS: &[&str] = &["password", "pwd"];
const REDIS_KEYS: &[&str] = &[
    "port",
    "bind",
    "dir",
    "dbfilename",
    "requirepass",
    "maxmemory",
    "logfile",
    "timeout",
    "protected-mode",
    "databases",
    "appendonly",
    "appendfilename",
    "daemonize",
    "pidfile",
];
const REDIS_SECRETS: &[&str] = &["requirepass", "masterauth"];
const NGINX_KEYS: &[&str] = &[
    "worker_processes",
    "error_log",
    "pid",
    "worker_connections",
    "listen",
    "server_name",
    "root",
    "index",
    "proxy_pass",
    "client_max_body_size",
    "gzip",
    "include",
    "access_log",
];

fn parse_maven_settings(kind: &str, path: &Path, _env: &HashMap<String, String>) -> ConfigOut {
    let mut cfg = empty_cfg(kind, path, path.is_file());
    if !cfg.exists {
        cfg.facts.push(("状态".into(), "文件不存在".into()));
        return cfg;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        cfg.facts.push(("状态".into(), "无法读取".into()));
        return cfg;
    };
    let stripped = winutil::strip_xml_comments(&text);
    if let Some(repo) = xml_tag(&stripped, "localRepository") {
        cfg.facts.push(("本地仓库".into(), repo.trim().into()));
    } else {
        cfg.facts.push((
            "本地仓库".into(),
            format!("未配置，默认 {}", winutil::default_m2_repo().display()),
        ));
    }
    let mirrors = xml_blocks(&stripped, "mirror");
    if mirrors.is_empty() {
        cfg.facts.push(("镜像源".into(), "未配置".into()));
    } else {
        cfg.facts
            .push(("镜像源".into(), format!("已配置 {} 个", mirrors.len())));
        for (i, block) in mirrors.iter().enumerate() {
            let id = xml_tag(block, "id").unwrap_or_else(|| format!("mirror-{}", i + 1));
            let of = xml_tag(block, "mirrorOf").unwrap_or_else(|| "*".into());
            let url = xml_tag(block, "url").unwrap_or_default();
            cfg.facts
                .push((format!("镜像 {id}"), format!("{of} → {url}")));
        }
    }
    if let Some(offline) = xml_tag(&stripped, "offline") {
        cfg.facts.push(("offline".into(), offline));
    }
    cfg.preview = maven_preview(&stripped);
    cfg
}

fn parse_kv_config(kind: &str, path: &Path, keys: &[&str], secrets: &[&str]) -> ConfigOut {
    let mut cfg = empty_cfg(kind, path, path.is_file());
    if !cfg.exists {
        cfg.facts.push(("状态".into(), "文件不存在".into()));
        return cfg;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        cfg.facts.push(("状态".into(), "无法读取".into()));
        return cfg;
    };
    let pairs = parse_conf_pairs(&text);
    for key in keys {
        if let Some(v) = pairs
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
        {
            let shown = if secrets.iter().any(|s| s.eq_ignore_ascii_case(key)) {
                if v.is_empty() {
                    "未设置".into()
                } else {
                    "已配置".into()
                }
            } else {
                v
            };
            cfg.facts.push(((*key).into(), shown));
        }
    }
    if cfg.facts.is_empty() {
        cfg.facts
            .push(("状态".into(), "已找到文件，未解析到常用项".into()));
    }
    let mut lines = Vec::new();
    for (k, v) in &pairs {
        if secrets.iter().any(|s| s.eq_ignore_ascii_case(k)) {
            lines.push(format!("{k} 已配置"));
        } else {
            lines.push(format!("{k} {v}"));
        }
        if lines.len() >= 24 {
            break;
        }
    }
    cfg.preview = lines.join("\n");
    cfg
}

fn parse_conf_pairs(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(';')
            || line.starts_with('[')
            || line == "{"
            || line == "}"
        {
            continue;
        }
        let (k, v) = if let Some((a, b)) = line.split_once('=') {
            (a.trim(), b.trim().trim_matches('"').trim_matches('\''))
        } else if let Some((a, b)) = line.split_once(':') {
            let a = a.trim();
            if !a.is_empty()
                && a.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                (a, b.trim().trim_matches('"').trim_matches('\''))
            } else {
                let mut parts = line.splitn(2, char::is_whitespace);
                let Some(a) = parts.next() else { continue };
                (a, parts.next().unwrap_or("").trim())
            }
        } else {
            let mut parts = line.splitn(2, char::is_whitespace);
            let Some(a) = parts.next() else { continue };
            (a, parts.next().unwrap_or("").trim())
        };
        let k = k.trim_end_matches('{').trim_end_matches(':').trim();
        let v = v.trim().trim_end_matches(';').trim().trim_matches('"');
        if k.is_empty() || k == "{" || k == "}" {
            continue;
        }
        out.push((k.to_string(), v.to_string()));
    }
    out
}

fn xml_tag(text: &str, tag: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r"(?s)<{tag}>\s*([^<]+?)\s*</{tag}>")).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn xml_blocks(text: &str, tag: &str) -> Vec<String> {
    let Ok(re) = regex::Regex::new(&format!(r"(?s)<{tag}>([\s\S]*?)</{tag}>")) else {
        return Vec::new();
    };
    re.captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn maven_preview(stripped: &str) -> String {
    let mut lines = Vec::new();
    if let Some(repo) = xml_tag(stripped, "localRepository") {
        lines.push(format!("localRepository: {repo}"));
    }
    for block in xml_blocks(stripped, "mirror") {
        let id = xml_tag(&block, "id").unwrap_or_default();
        let url = xml_tag(&block, "url").unwrap_or_default();
        let of = xml_tag(&block, "mirrorOf").unwrap_or_default();
        lines.push(format!("mirror {id} ({of}): {url}"));
    }
    lines.join("\n")
}

fn missing_config(kind: &str, guess: &Path) -> ConfigOut {
    let mut cfg = empty_cfg(kind, guess, false);
    cfg.facts.push(("状态".into(), "未找到配置文件".into()));
    cfg
}

fn empty_cfg(kind: &str, path: &Path, exists: bool) -> ConfigOut {
    ConfigOut {
        kind: kind.into(),
        path: winutil::display_path(path),
        exists,
        facts: Vec::new(),
        preview: String::new(),
    }
}

fn push_unique(list: &mut Vec<(String, PathBuf)>, kind: &str, path: PathBuf) {
    let s = winutil::display_path(&path);
    if list
        .iter()
        .any(|(_, p)| winutil::display_path(p).eq_ignore_ascii_case(&s))
    {
        return;
    }
    list.push((kind.into(), path));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_reads_repo_and_mirrors() {
        let xml = r#"
        <settings>
          <!-- <localRepository>/tmp/old</localRepository> -->
          <localRepository>D:\Maven_Repo</localRepository>
          <mirrors>
            <mirror>
              <id>aliyun</id>
              <mirrorOf>*</mirrorOf>
              <url>https://maven.aliyun.com/repository/public</url>
            </mirror>
          </mirrors>
        </settings>
        "#;
        let stripped = winutil::strip_xml_comments(xml);
        assert_eq!(
            xml_tag(&stripped, "localRepository").as_deref(),
            Some(r"D:\Maven_Repo")
        );
        assert_eq!(xml_blocks(&stripped, "mirror").len(), 1);
        assert!(xml_tag(&xml_blocks(&stripped, "mirror")[0], "id")
            .unwrap()
            .contains("aliyun"));
    }

    #[test]
    fn empty_home_does_not_scan_cwd() {
        let tmp = std::env::temp_dir().join(format!("cfgscan-empty-home-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("nginx.conf"), "listen 80;\n").unwrap();
        std::fs::write(tmp.join("redis.conf"), "port 6379\n").unwrap();
        std::fs::write(tmp.join("my.ini"), "port=3306\n").unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(&tmp).unwrap();
        let nginx = nginx_configs("");
        let redis = redis_configs("");
        let mysql = mysql_configs("", "");
        std::env::set_current_dir(&old).unwrap();
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(nginx.iter().all(|c| !c.exists), "{nginx:?}");
        assert!(redis.iter().all(|c| !c.exists), "{redis:?}");
        assert!(
            !mysql
                .iter()
                .any(|c| c.exists && c.path.to_ascii_lowercase().contains("cfgscan-empty-home")),
            "{mysql:?}"
        );
    }

    #[test]
    fn redis_and_mysql_pairs() {
        let redis = "port 6379\nbind 127.0.0.1\n# comment\nrequirepass secret\n";
        let pairs = parse_conf_pairs(redis);
        assert_eq!(pairs[0], ("port".into(), "6379".into()));
        let mysql = "[mysqld]\nport=3306\ndatadir=\"C:\\\\data\"\n";
        let mp = parse_conf_pairs(mysql);
        assert!(mp.iter().any(|(k, v)| k == "port" && v == "3306"));
        assert!(mp.iter().any(|(k, v)| k == "datadir" && v.contains("data")));
        let nginx = "worker_processes  1;\nlisten       80;\nserver_name  localhost;\n";
        let np = parse_conf_pairs(nginx);
        assert!(np.iter().any(|(k, v)| k == "listen" && v == "80"));
        assert!(np.iter().any(|(k, v)| k == "worker_processes" && v == "1"));
        let nvm = "root: D:\\tools\\nvm\npath: D:\\tools\\nvm\\nodejs\nnode_mirror: https://npmmirror.com/mirrors/node/\n";
        let nv = parse_conf_pairs(nvm);
        assert!(nv
            .iter()
            .any(|(k, v)| k == "root" && v.contains(r"D:\tools\nvm")));
        assert!(nv
            .iter()
            .any(|(k, v)| k == "node_mirror" && v.contains("npmmirror")));
    }

    #[test]
    fn nvm_settings_lists_mirrors() {
        let dir = std::env::temp_dir().join(format!("nvm-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.txt"),
            "root: D:\\tools\\nvm\r\npath: D:\\tools\\nvm\\nodejs\r\nnode_mirror: https://npmmirror.com/mirrors/node/\r\n",
        )
        .unwrap();
        let cfgs = nvm_configs(&dir.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cfgs.len(), 1);
        assert!(cfgs[0].exists);
        assert!(cfgs[0]
            .facts
            .iter()
            .any(|(k, v)| k == "Node 镜像" && v.contains("npmmirror")));
        assert!(cfgs[0]
            .facts
            .iter()
            .any(|(k, v)| k == "npm 镜像" && v.contains("官方")));
        assert!(cfgs[0]
            .facts
            .iter()
            .any(|(k, v)| k == "安装根" && v.contains(r"D:\tools\nvm")));
    }
}
