mod common;
mod owned;
pub(crate) mod players;
pub(crate) mod plays;
pub(crate) mod ratings;
mod render;
mod report;
mod spark;
pub(crate) mod time;
pub(crate) mod year;

pub(crate) use render::{print_summary, print_text};
pub(crate) use report::build;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CacheFile, CollectionItem, Stats, Status};

    fn stats(
        min_players: u32,
        max_players: u32,
        playing_time: u32,
        user_rating: Option<f32>,
        average: Option<f32>,
    ) -> Stats {
        Stats {
            min_players: Some(min_players),
            max_players: Some(max_players),
            playing_time: Some(playing_time),
            user_rating,
            average,
            bayes_average: average,
            users_rated: Some(1000),
        }
    }

    fn item(
        id: u32,
        name: &str,
        subtype: &str,
        owned: bool,
        plays: u32,
        year: Option<i32>,
        stats: Option<Stats>,
    ) -> CollectionItem {
        CollectionItem {
            id,
            collid: Some(id as u64),
            subtype: subtype.into(),
            name: name.into(),
            year_published: year,
            image: None,
            thumbnail: None,
            status: Status {
                own: owned,
                ..Default::default()
            },
            num_plays: plays,
            stats,
        }
    }

    fn cache_with(items: Vec<CollectionItem>) -> CacheFile {
        let mut cache = CacheFile::empty("tester");
        for item in items {
            cache.items.insert(item.collid.unwrap().to_string(), item);
        }
        cache
    }

    fn assert_f32_eq(left: Option<f32>, right: Option<f32>) {
        match (left, right) {
            (Some(left), Some(right)) => assert!((left - right).abs() < f32::EPSILON),
            (None, None) => {}
            other => panic!("float options differ: {other:?}"),
        }
    }

    fn top_tuple(entry: Option<&owned::TopEntry>) -> Option<(u32, &str, i32)> {
        entry.map(|t| (t.id, t.name.as_str(), t.value as i32))
    }

    #[test]
    fn deep_dive_headlines_match_owned_overview() {
        let cache = cache_with(vec![
            item(
                1,
                "User rated only",
                "boardgame",
                true,
                10,
                Some(2000),
                Some(stats(1, 4, 25, Some(9.0), None)),
            ),
            item(
                2,
                "Rated with BGG",
                "boardgame",
                true,
                0,
                Some(1990),
                Some(stats(2, 4, 60, Some(7.0), Some(7.5))),
            ),
            item(
                3,
                "BGG only",
                "boardgame",
                true,
                5,
                Some(2020),
                Some(stats(2, 4, 120, None, Some(8.0))),
            ),
            item(4, "Missing stats", "boardgame", true, 0, None, None),
            item(
                5,
                "Expansion",
                "boardgameexpansion",
                true,
                99,
                Some(1800),
                Some(stats(1, 1, 10, Some(10.0), Some(10.0))),
            ),
            item(
                6,
                "Not owned",
                "boardgame",
                false,
                99,
                Some(1800),
                Some(stats(1, 1, 10, Some(10.0), Some(10.0))),
            ),
        ]);
        let all: Vec<&CollectionItem> = cache.items.values().collect();
        let overview = owned::build(&all);

        let plays = plays::build(&cache);
        assert_eq!(overview.count, plays.played_count + plays.unplayed_count);
        assert_eq!(overview.plays.total, plays.total);
        assert_eq!(overview.plays.played_count, plays.played_count);
        assert_eq!(overview.plays.unplayed_count, plays.unplayed_count);
        assert!((overview.plays.avg_per_owned - plays.avg_per_owned).abs() < f32::EPSILON);

        let ratings = ratings::build(&cache);
        assert_eq!(overview.count, ratings.total_owned);
        assert_eq!(overview.ratings.rated_count, ratings.rated_count);
        assert_f32_eq(overview.ratings.your_average, ratings.your_average);
        assert_f32_eq(overview.ratings.bgg_average, ratings.bgg_average);
        assert_eq!(
            overview.ratings.your_distribution,
            ratings.your_distribution
        );
        assert_eq!(overview.ratings.bgg_distribution, ratings.bgg_distribution);

        let year = year::build(&cache);
        assert_eq!(
            top_tuple(overview.year.oldest.as_ref()),
            top_tuple(year.oldest.as_ref())
        );
        assert_eq!(
            top_tuple(overview.year.newest.as_ref()),
            top_tuple(year.newest.as_ref())
        );
        assert_eq!(overview.year.by_year, year.by_year);

        let time = time::build(&cache);
        assert_eq!(overview.time.avg_minutes, time.avg_minutes);
        assert_eq!(overview.time.quick_under_30, time.quick_under_30.len());
        assert_eq!(overview.time.medium_30_to_89, time.medium_30_to_89.len());
        assert_eq!(overview.time.long_90_to_180, time.long_90_to_180.len());
        assert_eq!(overview.time.epic_over_180, time.epic_over_180.len());

        let players = players::build(&cache);
        assert_eq!(overview.players.solo_capable, players.solo_capable);
        assert_eq!(overview.players.two_capable, players.two_capable);
        assert_eq!(overview.players.common_range, players.common_range);
    }
}
