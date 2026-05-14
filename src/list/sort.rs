use crate::error::{Error, Result};
use crate::model::CollectionItem;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SortField {
    Name,
    Year,
    Bggid,
    Plays,
    Rating,
    Time,
    Added,
    Geek,
    Players,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SortSpec {
    pub(crate) field: SortField,
    direction: Direction,
}

const SORT_VALUES: &str =
    "name, year, bggid, plays, rating, time, added, geek, players (append `:asc` or `:desc` to override the natural direction)";

impl SortField {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "name" => SortField::Name,
            "year" => SortField::Year,
            "bggid" => SortField::Bggid,
            "plays" => SortField::Plays,
            "rating" => SortField::Rating,
            "time" => SortField::Time,
            "added" => SortField::Added,
            "geek" => SortField::Geek,
            "players" => SortField::Players,
            other => {
                return Err(Error::BadArg(format!(
                    "unknown sort field `{other}`. Valid: {SORT_VALUES}"
                )));
            }
        })
    }

    fn natural_direction(self) -> Direction {
        match self {
            SortField::Plays | SortField::Rating | SortField::Added | SortField::Geek => {
                Direction::Desc
            }
            _ => Direction::Asc,
        }
    }
}

fn parse_direction(s: &str) -> Result<Direction> {
    match s {
        "asc" => Ok(Direction::Asc),
        "desc" => Ok(Direction::Desc),
        other => Err(Error::BadArg(format!(
            "unknown sort direction `{other}`. Valid: asc, desc"
        ))),
    }
}

impl SortSpec {
    pub(crate) fn parse(s: &str) -> Result<Self> {
        let (name, dir_override) = match s.split_once(':') {
            Some((n, d)) => (n.trim(), Some(parse_direction(d.trim())?)),
            None => (s, None),
        };
        let field = SortField::parse(name)?;
        let direction = dir_override.unwrap_or_else(|| field.natural_direction());
        Ok(SortSpec { field, direction })
    }

    pub(crate) fn compare(&self, a: &CollectionItem, b: &CollectionItem) -> Ordering {
        let primary = field_cmp(a, b, self.field, self.direction);
        primary.then_with(|| name_cmp(a, b))
    }
}

fn field_cmp(
    a: &CollectionItem,
    b: &CollectionItem,
    f: SortField,
    direction: Direction,
) -> Ordering {
    match f {
        SortField::Name => apply_direction(name_cmp(a, b), direction),
        SortField::Year => opt_cmp(a.year_published, b.year_published, direction),
        SortField::Bggid => apply_direction(a.id.cmp(&b.id), direction),
        SortField::Plays => apply_direction(a.num_plays.cmp(&b.num_plays), direction),
        SortField::Rating => opt_f_cmp(rating(a), rating(b), direction),
        SortField::Time => opt_cmp(time(a), time(b), direction),
        SortField::Added => opt_cmp(a.collid, b.collid, direction),
        SortField::Geek => opt_f_cmp(geek(a), geek(b), direction),
        SortField::Players => opt_cmp(min_players(a), min_players(b), direction)
            .then_with(|| opt_cmp(max_players(a), max_players(b), direction)),
    }
}

fn name_cmp(a: &CollectionItem, b: &CollectionItem) -> Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

fn apply_direction(ordering: Ordering, direction: Direction) -> Ordering {
    match direction {
        Direction::Asc => ordering,
        Direction::Desc => ordering.reverse(),
    }
}

fn opt_cmp<T: Ord>(a: Option<T>, b: Option<T>, direction: Direction) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => apply_direction(x.cmp(&y), direction),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn opt_f_cmp(a: Option<f32>, b: Option<f32>, direction: Direction) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            apply_direction(x.partial_cmp(&y).unwrap_or(Ordering::Equal), direction)
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(super) fn rating(i: &CollectionItem) -> Option<f32> {
    i.stats.as_ref().and_then(|s| s.user_rating)
}

pub(super) fn geek(i: &CollectionItem) -> Option<f32> {
    i.stats.as_ref().and_then(|s| s.bayes_average)
}

pub(super) fn time(i: &CollectionItem) -> Option<u32> {
    i.stats.as_ref().and_then(|s| s.playing_time)
}

pub(super) fn min_players(i: &CollectionItem) -> Option<u32> {
    i.stats.as_ref().and_then(|s| s.min_players)
}

pub(super) fn max_players(i: &CollectionItem) -> Option<u32> {
    i.stats.as_ref().and_then(|s| s.max_players)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> CollectionItem {
        use crate::model::Status;
        CollectionItem {
            id: 1,
            collid: Some(1),
            subtype: "boardgame".into(),
            name: name.into(),
            year_published: None,
            image: None,
            thumbnail: None,
            status: Status {
                own: true,
                ..Default::default()
            },
            num_plays: 0,
            stats: None,
        }
    }

    #[test]
    fn sort_parses_field_and_uses_natural_direction_by_default() {
        let a = SortSpec::parse("name").unwrap();
        assert_eq!(a.field, SortField::Name);
        assert_eq!(a.direction, Direction::Asc);

        let c = SortSpec::parse("plays").unwrap();
        assert_eq!(c.field, SortField::Plays);
        assert_eq!(c.direction, Direction::Desc);
    }

    #[test]
    fn sort_direction_postfix_overrides_natural() {
        assert_eq!(
            SortSpec::parse("name:desc").unwrap().direction,
            Direction::Desc
        );
        assert_eq!(
            SortSpec::parse("plays:asc").unwrap().direction,
            Direction::Asc
        );
        assert_eq!(
            SortSpec::parse("year:asc").unwrap().direction,
            Direction::Asc
        );
    }

    #[test]
    fn sort_rejects_unknown_field_or_direction() {
        assert!(SortSpec::parse("nope").is_err());
        assert!(SortSpec::parse("name:sideways").is_err());
    }

    #[test]
    fn sort_desc_keeps_missing_optional_values_last() {
        use crate::model::Stats;

        let unrated = item("Unrated");
        let mut low = item("Low");
        low.stats = Some(Stats {
            min_players: None,
            max_players: None,
            playing_time: None,
            user_rating: Some(4.0),
            average: None,
            bayes_average: None,
            users_rated: None,
        });
        let mut high = item("High");
        high.stats = Some(Stats {
            min_players: None,
            max_players: None,
            playing_time: None,
            user_rating: Some(9.0),
            average: None,
            bayes_average: None,
            users_rated: None,
        });

        let sort = SortSpec::parse("rating").unwrap();
        let mut rows = [&unrated, &low, &high];
        rows.sort_by(|a, b| sort.compare(a, b));

        let names: Vec<&str> = rows.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["High", "Low", "Unrated"]);
    }
}
