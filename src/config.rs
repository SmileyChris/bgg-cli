use crate::error::{Error, Result};
use crate::paths;
use std::path::Path;
use toml_edit::DocumentMut;

#[derive(Debug, Default, PartialEq, Eq)]
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
    parse_config(&s, &path)
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = paths::config_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Cache {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let out = render_config(cfg)?;
    std::fs::write(&path, out).map_err(|e| Error::Cache { path, source: e })
}

pub fn require_username() -> Result<String> {
    load()?.username.ok_or(Error::NoUser)
}

fn parse_config(s: &str, path: &Path) -> Result<Config> {
    let doc = s
        .parse::<DocumentMut>()
        .map_err(|e| Error::Parse(format!("config {}: {e}", path.display())))?;
    let username = match doc.get("username") {
        Some(item) => Some(
            item.as_str()
                .ok_or_else(|| {
                    Error::Parse(format!(
                        "config {}: username must be a string",
                        path.display()
                    ))
                })?
                .to_string(),
        ),
        None => None,
    };
    Ok(Config { username })
}

fn render_config(cfg: &Config) -> Result<String> {
    let mut out = String::new();
    if let Some(u) = &cfg.username {
        let quoted =
            serde_json::to_string(u).map_err(|e| Error::Parse(format!("config serialize: {e}")))?;
        out.push_str("username = ");
        out.push_str(&quoted);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_path() -> PathBuf {
        PathBuf::from("config.toml")
    }

    #[test]
    fn parses_valid_toml_config() {
        let cfg = parse_config(
            r#"
                # comments are normal TOML now
                username = "alice"
                username_extra = "ignored"
            "#,
            &fake_path(),
        )
        .unwrap();
        assert_eq!(
            cfg,
            Config {
                username: Some("alice".into())
            }
        );
    }

    #[test]
    fn rejects_non_string_username() {
        let err = parse_config("username = 42", &fake_path()).unwrap_err();
        assert!(matches!(err, Error::Parse(_)));
    }

    #[test]
    fn render_escapes_username_as_toml_string() {
        let cfg = Config {
            username: Some(r#"ali"ce"#.into()),
        };
        let out = render_config(&cfg).unwrap();
        let parsed = parse_config(&out, &fake_path()).unwrap();
        assert_eq!(parsed, cfg);
    }
}
