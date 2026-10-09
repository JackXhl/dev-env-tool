use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRoots {
    pub common: Vec<PathBuf>,
    #[serde(default)]
    pub jdk_vendor_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub jdk_parent_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub jdk_name_has: Vec<String>,
    #[serde(default)]
    pub maven_vendor_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub maven_name_has: Vec<String>,
    #[serde(default)]
    pub nvm_homes: Vec<PathBuf>,
    #[serde(default)]
    pub mysql_vendor_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub mysql_name_has: Vec<String>,
    #[serde(default)]
    pub nginx_exes: Vec<PathBuf>,
    #[serde(default)]
    pub nginx_walk_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub nginx_name_has: Vec<String>,
    #[serde(default)]
    pub redis_exes: Vec<PathBuf>,
    #[serde(default)]
    pub redis_walk_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub redis_name_has: Vec<String>,
}

impl ProbeRoots {
    pub fn load() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("probe-roots.json");
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(Self::fallback)
    }

    pub fn fallback() -> Self {
        serde_json::from_str(include_str!("../../resources/probe-roots.json"))
            .expect("bundled probe-roots.json")
    }

    pub fn with_common(&self, extra: &[PathBuf]) -> Vec<PathBuf> {
        let mut v = extra.to_vec();
        v.extend(self.common.iter().cloned());
        v
    }
}

pub fn names(list: &[String]) -> Vec<&str> {
    list.iter().map(|s| s.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_probe_roots_parse() {
        let roots = ProbeRoots::fallback();
        assert!(roots.common.iter().any(|p| p.ends_with("tools")));
        assert!(!roots.jdk_vendor_dirs.is_empty());
        assert!(!roots.nvm_homes.is_empty());
        assert!(!roots.nginx_exes.is_empty());
    }
}
