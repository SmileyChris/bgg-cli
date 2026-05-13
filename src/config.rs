use crate::error::{Error, Result};
use crate::paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub username: Option<String>,
}

pub fn load() -> Result<Config> {
    let path = paths::config_file();
    if !path.exists() {
        return Ok(Config::default());
    }
    let s = std::fs::read_to_string(&path).map_err(|e| Error::Cache {
        path: path.clone(),
        source: e,
    })?;
    let mut cfg = Config::default();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("username") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let val = rest.trim_matches('"').to_string();
            cfg.username = Some(val);
        }
    }
    Ok(cfg)
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = paths::config_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Cache {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let mut out = String::new();
    if let Some(u) = &cfg.username {
        out.push_str(&format!("username = \"{u}\"\n"));
    }
    std::fs::write(&path, out).map_err(|e| Error::Cache { path, source: e })
}

pub fn require_username() -> Result<String> {
    load()?.username.ok_or(Error::NoUser)
}
