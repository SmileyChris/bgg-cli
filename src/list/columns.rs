use crate::error::{Error, Result};
use crate::list::sort::{geek, max_players, min_players, rating, time, SortField};
use crate::model::CollectionItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Col {
    Year,
    Name,
    Bggid,
    Plays,
    Rating,
    Time,
    Players,
    Geek,
}

const COL_VALUES: &str = "year, name, bggid, plays, rating, time, players, geek (or `all`)";

impl Col {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "year" => Col::Year,
            "name" => Col::Name,
            "bggid" => Col::Bggid,
            "plays" => Col::Plays,
            "rating" => Col::Rating,
            "time" => Col::Time,
            "players" => Col::Players,
            "geek" => Col::Geek,
            other => {
                return Err(Error::BadArg(format!(
                    "unknown column `{other}`. Valid: {COL_VALUES}"
                )));
            }
        })
    }

    pub(crate) fn all() -> Vec<Col> {
        vec![
            Col::Year,
            Col::Bggid,
            Col::Plays,
            Col::Rating,
            Col::Geek,
            Col::Time,
            Col::Players,
            Col::Name,
        ]
    }

    pub(crate) fn header(self) -> &'static str {
        match self {
            Col::Year => "YEAR",
            Col::Name => "NAME",
            Col::Bggid => "BGGID",
            Col::Plays => "PLAYS",
            Col::Rating => "RATING",
            Col::Time => "TIME",
            Col::Players => "PLAYERS",
            Col::Geek => "GEEK",
        }
    }

    pub(crate) fn cell(self, item: &CollectionItem, linkify: bool) -> String {
        match self {
            Col::Year => opt_string(item.year_published),
            Col::Name => {
                if linkify {
                    hyperlink(&bgg_url(item), &item.name)
                } else {
                    item.name.clone()
                }
            }
            Col::Bggid => item.id.to_string(),
            Col::Plays => item.num_plays.to_string(),
            Col::Rating => opt_f_string(rating(item), 1),
            Col::Time => opt_string(time(item)).replace_if_some("min"),
            Col::Players => match (min_players(item), max_players(item)) {
                (Some(a), Some(b)) if a == b => format!("{a}"),
                (Some(a), Some(b)) => format!("{a}-{b}"),
                (Some(a), None) => format!("{a}+"),
                (None, Some(b)) => format!("≤{b}"),
                (None, None) => "-".into(),
            },
            Col::Geek => opt_f_string(geek(item), 2),
        }
    }

    pub(crate) fn is_last_friendly(self) -> bool {
        matches!(self, Col::Name)
    }
}

fn column_for_sort(field: SortField) -> Option<Col> {
    match field {
        SortField::Name => None,
        SortField::Year => None,
        SortField::Bggid => Some(Col::Bggid),
        SortField::Plays => Some(Col::Plays),
        SortField::Rating => Some(Col::Rating),
        SortField::Time => Some(Col::Time),
        SortField::Added => None,
        SortField::Geek => Some(Col::Geek),
        SortField::Players => Some(Col::Players),
    }
}

pub(crate) fn resolve_columns(cols_arg: Option<&str>, sort_field: SortField) -> Result<Vec<Col>> {
    if let Some(s) = cols_arg {
        let trimmed = s.trim();
        if trimmed == "all" {
            return Ok(Col::all());
        }
        let mut out = Vec::new();
        for part in trimmed.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let col = Col::parse(part)?;
            if !out.contains(&col) {
                out.push(col);
            }
        }
        if out.is_empty() {
            return Err(Error::BadArg("--cols is empty".into()));
        }
        return Ok(out);
    }

    let mut out = vec![Col::Year, Col::Name];
    if let Some(extra) = column_for_sort(sort_field) {
        if !out.contains(&extra) {
            let name_pos = out
                .iter()
                .position(|c| *c == Col::Name)
                .unwrap_or(out.len());
            out.insert(name_pos, extra);
        }
    }
    Ok(out)
}

fn opt_string<T: std::fmt::Display>(o: Option<T>) -> String {
    o.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
}

fn opt_f_string(o: Option<f32>, precision: usize) -> String {
    o.map(|v| format!("{v:.precision$}"))
        .unwrap_or_else(|| "-".into())
}

fn bgg_url(item: &CollectionItem) -> String {
    format!("https://boardgamegeek.com/boardgame/{}", item.id)
}

fn hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

trait ReplaceIfSome {
    fn replace_if_some(self, suffix: &str) -> String;
}

impl ReplaceIfSome for String {
    fn replace_if_some(self, suffix: &str) -> String {
        if self == "-" {
            self
        } else {
            format!("{self} {suffix}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_columns_defaults_to_year_name() {
        let cols = resolve_columns(None, SortField::Name).unwrap();
        assert_eq!(cols, vec![Col::Year, Col::Name]);
    }

    #[test]
    fn resolve_columns_implicitly_adds_sort_column_before_name() {
        let cols = resolve_columns(None, SortField::Plays).unwrap();
        assert_eq!(cols, vec![Col::Year, Col::Plays, Col::Name]);
    }

    #[test]
    fn resolve_columns_explicit_overrides_implicit() {
        let cols = resolve_columns(Some("name,year"), SortField::Plays).unwrap();
        assert_eq!(cols, vec![Col::Name, Col::Year]);
    }

    #[test]
    fn resolve_columns_all_expands() {
        let cols = resolve_columns(Some("all"), SortField::Name).unwrap();
        assert_eq!(cols, Col::all());
    }

    #[test]
    fn resolve_columns_rejects_unknown() {
        assert!(resolve_columns(Some("year,bogus"), SortField::Name).is_err());
    }
}
