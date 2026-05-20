use crate::model::{CacheFile, CollectionItem, Stats};
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn owned_boardgames<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<&'a CollectionItem> {
    items
        .into_iter()
        .filter(|i| i.status.own && i.subtype == "boardgame")
        .collect()
}

pub(crate) fn owned_boardgames_from_cache(cache: &CacheFile) -> Vec<&CollectionItem> {
    owned_boardgames(cache.items.values())
}

pub(crate) fn user_ratings<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<(&'a CollectionItem, f32)> {
    items
        .into_iter()
        .filter_map(|i| i.stats.as_ref().and_then(|s| s.user_rating).map(|r| (i, r)))
        .collect()
}

pub(crate) fn bgg_averages<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<(&'a CollectionItem, f32)> {
    items
        .into_iter()
        .filter_map(|i| i.stats.as_ref().and_then(|s| s.average).map(|r| (i, r)))
        .collect()
}

pub(crate) fn user_and_bgg_ratings<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<(&'a CollectionItem, f32, f32)> {
    items
        .into_iter()
        .filter_map(|i| {
            let stats = i.stats.as_ref()?;
            Some((i, stats.user_rating?, stats.average?))
        })
        .collect()
}

pub(crate) fn published_years<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<(&'a CollectionItem, i32)> {
    items
        .into_iter()
        .filter_map(|i| i.year_published.filter(|y| *y > 0).map(|y| (i, y)))
        .collect()
}

pub(crate) fn playing_times<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<(&'a CollectionItem, u32)> {
    items
        .into_iter()
        .filter_map(|i| {
            i.stats
                .as_ref()
                .and_then(|s| s.playing_time)
                .filter(|t| *t > 0)
                .map(|t| (i, t))
        })
        .collect()
}

pub(crate) fn player_ranges<'a>(
    items: impl IntoIterator<Item = &'a CollectionItem>,
) -> Vec<(&'a CollectionItem, (u32, u32))> {
    items
        .into_iter()
        .filter_map(|i| i.stats.as_ref().and_then(player_range).map(|r| (i, r)))
        .collect()
}

pub(crate) fn player_range(stats: &Stats) -> Option<(u32, u32)> {
    let (Some(min), Some(max)) = (stats.min_players, stats.max_players) else {
        return None;
    };
    (min > 0 && max > 0).then_some((min, max))
}

pub(crate) fn bucket_1_to_10(r: f32) -> Option<usize> {
    let b = r.round() as i32;
    (1..=10).contains(&b).then_some((b - 1) as usize)
}

pub(crate) fn random_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

pub(crate) fn random_index(seed: u64, label: &str, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    label.hash(&mut hasher);
    len.hash(&mut hasher);
    Some((hasher.finish() as usize) % len)
}
