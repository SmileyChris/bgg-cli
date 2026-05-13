use crate::error::{Error, Result};
use crate::model::{CacheFile, CollectionItem};
use chrono::Utc;
use std::path::Path;

pub fn load(path: &Path, username: &str) -> Result<CacheFile> {
    if !path.exists() {
        return Err(Error::NoCache(username.to_string()));
    }
    let bytes = std::fs::read(path).map_err(|e| Error::Cache {
        path: path.to_path_buf(),
        source: e,
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::Parse(format!("cache {}: {e}", path.display())))
}

pub fn save(path: &Path, cache: &CacheFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Cache {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(cache)
        .map_err(|e| Error::Parse(format!("cache serialize: {e}")))?;
    std::fs::write(path, bytes).map_err(|e| Error::Cache {
        path: path.to_path_buf(),
        source: e,
    })
}

#[derive(Debug, Default, PartialEq)]
pub struct MergeReport {
    pub new: u32,
    pub updated: u32,
    pub unchanged: u32,
    /// Items removed from the cache because they were not in `incoming`.
    /// Only set when `merge` is called with `prune = true` (i.e. full sync).
    pub removed: u32,
}

/// Cache key for a collection item. Prefer `collid` (BGG's per-user-collection
/// id, unique even when the same game appears multiple times) and fall back to
/// the BGG object id only when collid is missing.
pub fn item_key(item: &CollectionItem) -> String {
    item.collid
        .map(|c| c.to_string())
        .unwrap_or_else(|| item.id.to_string())
}

/// Merge `incoming` into `cache`.
///
/// When `prune` is true, any cache entries whose key is not in `incoming` are
/// removed. This is what `bgg sync --full` needs to detect deletions; it is
/// **not** safe in incremental mode, where `incoming` only contains items
/// modified since the last sync.
pub fn merge(cache: &mut CacheFile, incoming: Vec<CollectionItem>, prune: bool) -> MergeReport {
    let mut report = MergeReport::default();
    let mut seen: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(incoming.len());
    for item in incoming {
        let key = item_key(&item);
        seen.insert(key.clone());
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
    if prune {
        let to_remove: Vec<String> = cache
            .items
            .keys()
            .filter(|k| !seen.contains(k.as_str()))
            .cloned()
            .collect();
        report.removed = to_remove.len() as u32;
        for k in to_remove {
            cache.items.remove(&k);
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

    fn item_with_collid(id: u32, collid: u64, name: &str) -> CollectionItem {
        let mut it = item(id, name);
        it.collid = Some(collid);
        it
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
        let report = merge(&mut cache, incoming, false);
        assert_eq!(
            report,
            MergeReport {
                new: 1,
                updated: 1,
                unchanged: 1,
                removed: 0,
            }
        );
        assert_eq!(cache.items["2"].name, "Catan: Cities");
        assert!(cache.last_sync.is_some());
    }

    #[test]
    fn merge_incremental_does_not_prune_missing_entries() {
        let mut cache = CacheFile::empty("alice");
        cache.items.insert("1".into(), item(1, "Azul"));
        cache.items.insert("2".into(), item(2, "Catan"));

        // Incremental sync: BGG only returns items modified since last sync.
        // The cache MUST keep "Catan" even though it's not in incoming.
        let incoming = vec![item(1, "Azul: Master Chocolatier")];
        let report = merge(&mut cache, incoming, false);
        assert_eq!(report.removed, 0);
        assert_eq!(cache.items.len(), 2);
        assert!(cache.items.contains_key("2"));
    }

    #[test]
    fn merge_full_prunes_entries_not_in_incoming() {
        let mut cache = CacheFile::empty("alice");
        cache.items.insert("1".into(), item(1, "Azul"));
        cache.items.insert("2".into(), item(2, "Catan"));
        cache.items.insert("3".into(), item(3, "Wingspan"));

        // Full sync: incoming is the complete current collection.
        // Items not in incoming were removed from the user's BGG collection.
        let incoming = vec![item(1, "Azul"), item(3, "Wingspan")];
        let report = merge(&mut cache, incoming, true);
        assert_eq!(report.removed, 1);
        assert_eq!(report.unchanged, 2);
        assert!(!cache.items.contains_key("2"));
    }

    #[test]
    fn merge_keys_by_collid_so_duplicate_object_ids_coexist() {
        let mut cache = CacheFile::empty("alice");
        // Same BGG object id (174430 = Gloomhaven) appearing twice with
        // different collid — e.g. a user who owns two printings.
        let incoming = vec![
            item_with_collid(174430, 1001, "Gloomhaven (first printing)"),
            item_with_collid(174430, 1002, "Gloomhaven (anniversary)"),
        ];
        let report = merge(&mut cache, incoming, false);
        assert_eq!(report.new, 2);
        assert_eq!(cache.items.len(), 2);
        assert_eq!(cache.items["1001"].name, "Gloomhaven (first printing)");
        assert_eq!(cache.items["1002"].name, "Gloomhaven (anniversary)");
    }
}
