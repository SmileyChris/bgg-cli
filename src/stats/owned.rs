use crate::model::CollectionItem;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub(crate) struct OwnedStats {
    pub(crate) count: usize,
    pub(crate) plays: PlaysStats,
    pub(crate) ratings: RatingsStats,
    pub(crate) year: YearStats,
    pub(crate) time: TimeStats,
    pub(crate) players: PlayersStats,
}

#[derive(Debug, Serialize)]
pub(crate) struct PlaysStats {
    pub(crate) total: u64,
    pub(crate) played_count: usize,
    pub(crate) unplayed_count: usize,
    pub(crate) avg_per_owned: f32,
    pub(crate) top: Vec<TopEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RatingsStats {
    pub(crate) rated_count: usize,
    pub(crate) your_average: Option<f32>,
    pub(crate) bgg_average: Option<f32>,
    pub(crate) your_distribution: [usize; 10],
    pub(crate) bgg_distribution: [usize; 10],
    pub(crate) top: Vec<TopEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct YearStats {
    pub(crate) oldest: Option<TopEntry>,
    pub(crate) newest: Option<TopEntry>,
    pub(crate) by_year: BTreeMap<i32, usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TimeStats {
    pub(crate) avg_minutes: Option<u32>,
    pub(crate) quick_under_30: usize,
    pub(crate) medium_30_to_89: usize,
    pub(crate) long_90_to_180: usize,
    pub(crate) epic_over_180: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct PlayersStats {
    pub(crate) solo_capable: usize,
    pub(crate) two_capable: usize,
    pub(crate) common_range: Option<(u32, u32)>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TopEntry {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) value: f64,
}

pub(crate) fn build(all: &[&CollectionItem]) -> OwnedStats {
    let owned: Vec<&CollectionItem> = all
        .iter()
        .copied()
        .filter(|i| i.status.own && i.subtype == "boardgame")
        .collect();
    OwnedStats {
        count: owned.len(),
        plays: plays_stats(&owned),
        ratings: ratings_stats(&owned),
        year: year_stats(&owned),
        time: time_stats(&owned),
        players: players_stats(&owned),
    }
}

fn plays_stats(owned: &[&CollectionItem]) -> PlaysStats {
    let total: u64 = owned.iter().map(|i| i.num_plays as u64).sum();
    let played_count = owned.iter().filter(|i| i.num_plays > 0).count();
    let unplayed_count = owned.len() - played_count;
    let avg = if owned.is_empty() {
        0.0
    } else {
        total as f32 / owned.len() as f32
    };
    let mut by_plays: Vec<&&CollectionItem> = owned.iter().filter(|i| i.num_plays > 0).collect();
    by_plays.sort_by(|a, b| b.num_plays.cmp(&a.num_plays).then(a.name.cmp(&b.name)));
    let top = by_plays
        .iter()
        .take(5)
        .map(|i| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: i.num_plays as f64,
        })
        .collect();
    PlaysStats {
        total,
        played_count,
        unplayed_count,
        avg_per_owned: avg,
        top,
    }
}

fn ratings_stats(owned: &[&CollectionItem]) -> RatingsStats {
    let rated: Vec<(&&CollectionItem, f32)> = owned
        .iter()
        .filter_map(|i| i.stats.as_ref().and_then(|s| s.user_rating).map(|r| (i, r)))
        .collect();
    let your_average = if rated.is_empty() {
        None
    } else {
        Some(rated.iter().map(|(_, r)| *r).sum::<f32>() / rated.len() as f32)
    };
    let bgg_vals: Vec<f32> = owned
        .iter()
        .filter_map(|i| i.stats.as_ref().and_then(|s| s.average))
        .collect();
    let bgg_average = if bgg_vals.is_empty() {
        None
    } else {
        Some(bgg_vals.iter().sum::<f32>() / bgg_vals.len() as f32)
    };
    let mut your_distribution = [0usize; 10];
    for (_, r) in &rated {
        if let Some(b) = bucket_1_to_10(*r) {
            your_distribution[b] += 1;
        }
    }
    let mut bgg_distribution = [0usize; 10];
    for v in &bgg_vals {
        if let Some(b) = bucket_1_to_10(*v) {
            bgg_distribution[b] += 1;
        }
    }
    let mut rated_sorted = rated.clone();
    rated_sorted.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.name.cmp(&b.0.name))
    });
    let top = rated_sorted
        .iter()
        .take(5)
        .map(|(i, r)| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: *r as f64,
        })
        .collect();
    RatingsStats {
        rated_count: rated.len(),
        your_average,
        bgg_average,
        your_distribution,
        bgg_distribution,
        top,
    }
}

fn bucket_1_to_10(r: f32) -> Option<usize> {
    let b = r.round() as i32;
    (1..=10).contains(&b).then_some((b - 1) as usize)
}

fn year_stats(owned: &[&CollectionItem]) -> YearStats {
    let with_year: Vec<(&&CollectionItem, i32)> = owned
        .iter()
        .filter_map(|i| i.year_published.filter(|y| *y > 0).map(|y| (i, y)))
        .collect();
    let oldest = with_year
        .iter()
        .min_by_key(|(_, y)| *y)
        .map(|(i, y)| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: *y as f64,
        });
    let newest = with_year
        .iter()
        .max_by_key(|(_, y)| *y)
        .map(|(i, y)| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: *y as f64,
        });
    let mut by_year: BTreeMap<i32, usize> = BTreeMap::new();
    for (_, y) in &with_year {
        *by_year.entry(*y).or_insert(0) += 1;
    }
    YearStats {
        oldest,
        newest,
        by_year,
    }
}

pub(super) fn trim_outlier_years(
    by_year: &BTreeMap<i32, usize>,
    drop_frac: f64,
) -> Option<(i32, i32)> {
    let total: usize = by_year.values().sum();
    if total == 0 {
        return None;
    }
    let cutoff = ((total as f64) * drop_frac / 2.0).ceil() as usize;
    let cutoff = cutoff.max(1);
    let mut cum = 0;
    let lead = by_year.iter().find_map(|(y, n)| {
        cum += n;
        (cum >= cutoff).then_some(*y)
    })?;
    let mut cum = 0;
    let trail = by_year.iter().rev().find_map(|(y, n)| {
        cum += n;
        (cum >= cutoff).then_some(*y)
    })?;
    Some((lead, trail))
}

fn time_stats(owned: &[&CollectionItem]) -> TimeStats {
    let times: Vec<u32> = owned
        .iter()
        .filter_map(|i| {
            i.stats
                .as_ref()
                .and_then(|s| s.playing_time)
                .filter(|t| *t > 0)
        })
        .collect();
    let avg_minutes = if times.is_empty() {
        None
    } else {
        Some((times.iter().sum::<u32>() as f64 / times.len() as f64).round() as u32)
    };
    let mut quick = 0;
    let mut medium = 0;
    let mut long = 0;
    let mut epic = 0;
    for t in &times {
        match *t {
            0..=29 => quick += 1,
            30..=89 => medium += 1,
            90..=180 => long += 1,
            _ => epic += 1,
        }
    }
    TimeStats {
        avg_minutes,
        quick_under_30: quick,
        medium_30_to_89: medium,
        long_90_to_180: long,
        epic_over_180: epic,
    }
}

fn players_stats(owned: &[&CollectionItem]) -> PlayersStats {
    let mut solo = 0;
    let mut two = 0;
    let mut ranges: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for i in owned {
        let Some(stats) = i.stats.as_ref() else {
            continue;
        };
        let (Some(min), Some(max)) = (stats.min_players, stats.max_players) else {
            continue;
        };
        if min == 0 || max == 0 {
            continue;
        }
        if stats.supports_player_count(1) {
            solo += 1;
        }
        if stats.supports_player_count(2) {
            two += 1;
        }
        *ranges.entry((min, max)).or_insert(0) += 1;
    }
    let common_range = ranges
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(range, _)| range);
    PlayersStats {
        solo_capable: solo,
        two_capable: two,
        common_range,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CollectionItem, Stats, Status};

    fn item(id: u32, name: &str, owned: bool, plays: u32) -> CollectionItem {
        CollectionItem {
            id,
            collid: Some(id as u64),
            subtype: "boardgame".into(),
            name: name.into(),
            year_published: Some(2020),
            image: None,
            thumbnail: None,
            status: Status {
                own: owned,
                ..Default::default()
            },
            num_plays: plays,
            stats: Some(Stats {
                min_players: Some(2),
                max_players: Some(4),
                playing_time: Some(60),
                user_rating: Some(8.0),
                average: Some(7.5),
                bayes_average: Some(7.0),
                users_rated: Some(1000),
            }),
        }
    }

    fn owned_with(items: &[CollectionItem]) -> OwnedStats {
        let all: Vec<&CollectionItem> = items.iter().collect();
        build(&all)
    }

    #[test]
    fn owned_stats_restrict_to_boardgame_subtype() {
        let mut exp = item(1, "Expansion", true, 99);
        exp.subtype = "boardgameexpansion".into();
        let game = item(2, "Game", true, 3);
        let r = owned_with(&[exp, game]);
        assert_eq!(r.count, 1);
        assert_eq!(r.plays.total, 3);
        assert_eq!(r.plays.top.first().unwrap().name, "Game");
    }

    #[test]
    fn plays_top_excludes_unplayed_and_sorts_desc() {
        let items = vec![
            item(1, "Zero", true, 0),
            item(2, "Two", true, 2),
            item(3, "Ten", true, 10),
            item(4, "Five", true, 5),
        ];
        let r = owned_with(&items);
        let names: Vec<String> = r.plays.top.iter().map(|t| t.name.clone()).collect();
        assert_eq!(names, vec!["Ten", "Five", "Two"]);
        assert_eq!(r.plays.played_count, 3);
        assert_eq!(r.plays.unplayed_count, 1);
    }

    #[test]
    fn year_buckets_per_year_and_extremes() {
        let mk = |id, name, y| {
            let mut i = item(id, name, true, 0);
            i.year_published = Some(y);
            i
        };
        let items = vec![
            mk(1, "1985", 1985),
            mk(2, "1992", 1992),
            mk(3, "2020", 2020),
            mk(4, "2021", 2021),
        ];
        let r = owned_with(&items);
        assert_eq!(r.year.by_year.get(&1985), Some(&1));
        assert_eq!(r.year.by_year.get(&1992), Some(&1));
        assert_eq!(r.year.by_year.get(&2020), Some(&1));
        assert_eq!(r.year.by_year.get(&2021), Some(&1));
        assert_eq!(r.year.oldest.as_ref().unwrap().value as i32, 1985);
        assert_eq!(r.year.newest.as_ref().unwrap().value as i32, 2021);
    }

    #[test]
    fn trim_outlier_years_drops_thin_leading_and_trailing_tails() {
        let mut by = BTreeMap::new();
        by.insert(1500, 1);
        by.insert(1600, 1);
        for y in 2000..=2009 {
            by.insert(y, 10);
        }
        by.insert(2100, 1);
        by.insert(2200, 1);
        let (lo, hi) = trim_outlier_years(&by, 0.04).unwrap();
        assert_eq!(lo, 2000);
        assert_eq!(hi, 2009);
    }

    #[test]
    fn trim_outlier_years_returns_none_for_empty_input() {
        let by: BTreeMap<i32, usize> = BTreeMap::new();
        assert!(trim_outlier_years(&by, 0.02).is_none());
    }

    #[test]
    fn time_buckets_split_at_30_90_180() {
        let mk = |id, t| {
            let mut i = item(id, "x", true, 0);
            i.stats.as_mut().unwrap().playing_time = Some(t);
            i
        };
        let items = vec![
            mk(1, 15),
            mk(2, 30),
            mk(3, 89),
            mk(4, 90),
            mk(5, 180),
            mk(6, 181),
        ];
        let r = owned_with(&items);
        assert_eq!(r.time.quick_under_30, 1);
        assert_eq!(r.time.medium_30_to_89, 2);
        assert_eq!(r.time.long_90_to_180, 2);
        assert_eq!(r.time.epic_over_180, 1);
    }

    #[test]
    fn ratings_distribution_buckets_rounded_to_integers() {
        let mk = |id, r| {
            let mut i = item(id, "x", true, 0);
            i.stats.as_mut().unwrap().user_rating = Some(r);
            i
        };
        let items = vec![mk(1, 7.4), mk(2, 7.6), mk(3, 10.0), mk(4, 11.0)];
        let r = owned_with(&items);
        assert_eq!(r.ratings.your_distribution[6], 1, "bucket 7");
        assert_eq!(r.ratings.your_distribution[7], 1, "bucket 8");
        assert_eq!(r.ratings.your_distribution[9], 1, "bucket 10");
        let total: usize = r.ratings.your_distribution.iter().sum();
        assert_eq!(total, 3, "11.0 is dropped");
    }

    #[test]
    fn players_solo_two_and_common_range() {
        let mk = |id, min, max| {
            let mut i = item(id, "x", true, 0);
            let s = i.stats.as_mut().unwrap();
            s.min_players = Some(min);
            s.max_players = Some(max);
            i
        };
        let items = vec![mk(1, 1, 4), mk(2, 2, 4), mk(3, 2, 4), mk(4, 3, 6)];
        let r = owned_with(&items);
        assert_eq!(r.players.solo_capable, 1);
        assert_eq!(r.players.two_capable, 3);
        assert_eq!(r.players.common_range, Some((2, 4)));
    }
}
