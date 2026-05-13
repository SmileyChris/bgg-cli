use directories::ProjectDirs;
use std::path::PathBuf;

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("", "", "bgg-cli")
        .expect("could not determine project directories for current OS")
}

pub fn state_dir() -> PathBuf {
    let dirs = project_dirs();
    dirs.state_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dirs.data_dir().to_path_buf())
}

pub fn config_dir() -> PathBuf {
    project_dirs().config_dir().to_path_buf()
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn cache_file(username: &str) -> PathBuf {
    state_dir().join(format!("collection-{username}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_uses_username_in_name() {
        let p = cache_file("alice");
        assert!(p.to_string_lossy().ends_with("collection-alice.json"));
    }

    #[test]
    fn config_file_is_under_config_dir() {
        assert!(config_file().starts_with(config_dir()));
    }
}
