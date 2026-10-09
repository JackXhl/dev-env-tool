mod cfgscan;
mod envscan;
mod winutil;

use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub fn scan_cli() -> Vec<envscan::EnvItem> {
    envscan::scan(|_| {})
}

fn resource_candidates(app: Option<&AppHandle>, name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app) = app {
        if let Ok(dir) = app.path().resource_dir() {
            candidates.push(dir.join(name));
            candidates.push(dir.join("resources").join(name));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(name));
            candidates.push(parent.join("resources").join(name));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("resources").join(name));
    candidates
}

fn load_resource_json(
    app: Option<&AppHandle>,
    name: &str,
) -> Result<(PathBuf, serde_json::Value), String> {
    let p = resource_candidates(app, name)
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("读不到资源文件 {name}"))?;
    let text = std::fs::read_to_string(&p).map_err(|e| format!("读不到 {}: {e}", p.display()))?;
    let val = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok((p, val))
}

fn merge_object(base: &mut serde_json::Value, overlay: serde_json::Value) {
    let Some(base_map) = base.as_object_mut() else {
        *base = overlay;
        return;
    };
    let Some(over_map) = overlay.as_object() else {
        return;
    };
    for (k, v) in over_map {
        match (base_map.get_mut(k), v) {
            (Some(existing), overlay_val) if existing.is_object() && overlay_val.is_object() => {
                merge_object(existing, overlay_val.clone());
            }
            _ => {
                base_map.insert(k.clone(), v.clone());
            }
        }
    }
}

fn is_http_url(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.contains(char::is_control) || url.contains(char::is_whitespace) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !lower.starts_with("https:///")
        && !lower.starts_with("http:///")
}

#[tauri::command]
fn load_brand(app: AppHandle) -> Result<serde_json::Value, String> {
    load_resource_json(Some(&app), "brand.json").map(|(_, v)| v)
}

#[tauri::command]
fn load_links(app: AppHandle) -> Result<serde_json::Value, String> {
    let (path, mut links) = load_resource_json(Some(&app), "links.json")?;
    let mut local_candidates = vec![path.with_file_name("links.local.json")];
    local_candidates.extend(resource_candidates(Some(&app), "links.local.json"));
    if let Some(local_path) = local_candidates.into_iter().find(|p| p.is_file()) {
        let text = std::fs::read_to_string(&local_path)
            .map_err(|e| format!("读不到 {}: {e}", local_path.display()))?;
        let overlay: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        merge_object(&mut links, overlay);
    }
    Ok(links)
}

#[tauri::command]
async fn scan_env(app: AppHandle) -> Result<Vec<envscan::EnvItem>, String> {
    let emit = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        envscan::scan(|item| {
            let _ = emit.emit("scan-progress", item);
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !is_http_url(&url) {
        return Err("只打开 http(s) 链接".into());
    }
    app.opener()
        .open_url(url.trim(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_overlay_keeps_tutorial() {
        let mut links = serde_json::json!({
            "jdk21": {
                "download": "https://adoptium.net/temurin/releases/?version=21",
                "tutorial": "https://blog.csdn.net/lf5566fl/article/details/166794914"
            }
        });
        merge_object(
            &mut links,
            serde_json::json!({
                "jdk21": { "download": "https://pan.baidu.com/s/example?pwd=abcd" }
            }),
        );
        assert_eq!(
            links["jdk21"]["download"],
            "https://pan.baidu.com/s/example?pwd=abcd"
        );
        assert_eq!(
            links["jdk21"]["tutorial"],
            "https://blog.csdn.net/lf5566fl/article/details/166794914"
        );
    }

    #[test]
    fn merge_rejects_non_object_overlay() {
        let mut links = serde_json::json!({ "jdk21": { "download": "https://a.example" } });
        merge_object(&mut links, serde_json::json!("oops"));
        assert_eq!(links["jdk21"]["download"], "https://a.example");
    }

    #[test]
    fn http_url_rejects_schemes_and_control_chars() {
        assert!(is_http_url("https://nginx.org/en/download.html"));
        assert!(is_http_url("HTTP://example.com/path"));
        assert!(is_http_url(" https://example.com"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("file:///C:/Windows"));
        assert!(!is_http_url("https://example.com\nfile:///etc/passwd"));
        assert!(!is_http_url("https://example.com/foo bar"));
    }
}

#[tauri::command]
fn open_local_path(path: String, mode: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    match mode.as_str() {
        "file" => winutil::open_file(p),
        _ => winutil::open_folder(p),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_brand,
            load_links,
            scan_env,
            open_url,
            open_local_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
