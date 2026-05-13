use crate::error::{Error, Result};
use crate::model::{CacheFile, CollectionItem};
use chrono::Utc;
use std::path::Path;

pub fn load(path: &Path, username: &str) -> Result<CacheFile> {
    if !path.exists() {
        return Err(Error::NoCache(username.to_string()));
    }
    let bytes = std::fs::read(path).map_err(|e| Error::Cache { path: path.to_path_buf(), source: e })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::Parse(format!("cache {}: {e}", path.display())))
}

pub fn save(path: &Path, cache: &CacheFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Cache { path: parent.to_path_buf(), source: e })?;
    }
    let bytes = serde_json::to_vec_pretty(cache)
        .map_err(|e| Error::Parse(format!("cache serialize: {e}")))?;
    std::fs::write(path, bytes)
        .map_err(|e| Error::Cache { path: path.to_path_buf(), source: e })
}

#[derive(Debug, Default, PartialEq)]
pub struct MergeReport {
    pub new: u32,
    pub updated: u32,
    pub unchanged: u32,
}

pub fn merge(cache: &mut CacheFile, incoming: Vec<CollectionItem>) -> MergeReport {
    let mut report = MergeReport::default();
    for item in incoming {
        let key = item.id.to_string();
        match cache.items.get(&key) {
            None => {
                report.new += 1;
                cache.items.insert(key, item);
            }
            Some(existing) if existing == &item => {
                report.unchanged += 1;
            }
            Some(_) => {
                report.updated += 1;
                cache.items.insert(key, item);
            }
        }
    }
    cache.last_sync = Some(Utc::now());
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use tempfile::tempdir;

    fn item(id: u32, name: &str) -> CollectionItem {
        CollectionItem {
            id,
            collid: None,
            subtype: "boardgame".into(),
            name: name.into(),
            year_published: None,
            image: None,
            thumbnail: None,
            status: Status::default(),
            num_plays: 0,
            stats: None,
        }
    }

    #[test]
    fn load_missing_returns_no_cache() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("missing.json");
        let err = load(&p, "alice").unwrap_err();
        assert!(matches!(err, Error::NoCache(u) if u == "alice"));
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.json");
        let mut cache = CacheFile::empty("alice");
        cache.items.insert("1".into(), item(1, "Azul"));
        save(&p, &cache).unwrap();
        let loaded = load(&p, "alice").unwrap();
        assert_eq!(loaded.username, "alice");
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items["1"].name, "Azul");
    }

    #[test]
    fn merge_classifies_new_updated_unchanged() {
        let mut cache = CacheFile::empty("alice");
        cache.items.insert("1".into(), item(1, "Azul"));
        cache.items.insert("2".into(), item(2, "Catan"));

        let incoming = vec![
            item(1, "Azul"),
            item(2, "Catan: Cities"),
            item(3, "Wingspan"),
        ];
        let report = merge(&mut cache, incoming);
        assert_eq!(report, MergeReport { new: 1, updated: 1, unchanged: 1 });
        assert_eq!(cache.items["2"].name, "Catan: Cities");
        assert!(cache.last_sync.is_some());
    }
}
