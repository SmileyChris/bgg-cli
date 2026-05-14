use crate::cache;
use crate::config;
use crate::error::{Error, Result};
use crate::model::CollectionItem;
use crate::paths;
use anstream::println;
use anstyle::{Effects, Style};
use std::cmp::Ordering;
use std::io::IsTerminal;

const MUTED: Style = Style::new().effects(Effects::DIMMED);

pub fn run(
    filter_arg: String,
    sort_arg: String,
    cols_arg: Option<String>,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;

    if json {
        let items: Vec<&CollectionItem> = cache.items.values().collect();
        let out =
            serde_json::to_string_pretty(&items).map_err(|e| Error::Parse(format!("json: {e}")))?;
        std::println!("{out}");
        return Ok(());
    }

    let filter = FilterSpec::parse(&filter_arg)?;
    let sort = SortSpec::parse(&sort_arg)?;
    let cols = resolve_columns(cols_arg.as_deref(), sort.field)?;

    let mut items: Vec<&CollectionItem> =
        cache.items.values().filter(|i| filter.matches(i)).collect();
    items.sort_by(|a, b| sort.compare(a, b));

    let total = items.len();
    if let Some(n) = limit {
        items.truncate(n);
    }
    render_table(&items, &cols);
    if std::io::stdout().is_terminal() {
        print_footer(items.len(), total);
    }
    Ok(())
}

fn print_footer(shown: usize, total: usize) {
    if total == 0 {
        println!("{MUTED}No items match the filter.{MUTED:#}");
    } else if shown < total {
        println!("{MUTED}Showing {shown} of {total} items.{MUTED:#}");
    } else {
        println!("{MUTED}{total} items.{MUTED:#}");
    }
}

// ---------- Filter ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    Owned,
    PrevOwned,
    Wishlist,
    WantToPlay,
    WantToBuy,
    Preordered,
    ForTrade,
    Expansion,
    Rated,
    Played,
    Solo,
    All,
}

const FILTER_VALUES: &str = "owned, prev-owned, wishlist, want-to-play, want-to-buy, preordered, for-trade, expansion, rated, played, solo, all (prefix with `not:` to invert)";

impl FilterKind {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "owned" => FilterKind::Owned,
            "prev-owned" => FilterKind::PrevOwned,
            "wishlist" => FilterKind::Wishlist,
            "want-to-play" => FilterKind::WantToPlay,
            "want-to-buy" => FilterKind::WantToBuy,
            "preordered" => FilterKind::Preordered,
            "for-trade" => FilterKind::ForTrade,
            "expansion" => FilterKind::Expansion,
            "rated" => FilterKind::Rated,
            "played" => FilterKind::Played,
            "solo" => FilterKind::Solo,
            "all" => FilterKind::All,
            other => {
                return Err(Error::BadArg(format!(
                    "unknown --filter value `{other}`. Valid: {FILTER_VALUES}"
                )));
            }
        })
    }

    fn item_matches(self, i: &CollectionItem) -> bool {
        match self {
            FilterKind::Owned => i.status.own,
            FilterKind::PrevOwned => i.status.prev_owned,
            FilterKind::Wishlist => i.status.wishlist,
            FilterKind::WantToPlay => i.status.want_to_play,
            FilterKind::WantToBuy => i.status.want_to_buy,
            FilterKind::Preordered => i.status.preordered,
            FilterKind::ForTrade => i.status.for_trade,
            FilterKind::Expansion => i.subtype == "boardgameexpansion",
            FilterKind::Rated => i.stats.as_ref().and_then(|s| s.user_rating).is_some(),
            FilterKind::Played => i.num_plays > 0,
            FilterKind::Solo => i.stats.as_ref().is_some_and(|s| s.supports_player_count(1)),
            FilterKind::All => true,
        }
    }
}

#[derive(Debug)]
struct FilterSpec {
    rules: Vec<(FilterKind, bool)>, // (kind, invert)
}

impl FilterSpec {
    fn parse(s: &str) -> Result<Self> {
        let mut rules = Vec::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (name, invert) = match part.strip_prefix("not:") {
                Some(rest) => (rest.trim(), true),
                None => (part, false),
            };
            rules.push((FilterKind::parse(name)?, invert));
        }
        if rules.is_empty() {
            return Err(Error::BadArg("--filter is empty".into()));
        }
        Ok(FilterSpec { rules })
    }

    fn matches(&self, i: &CollectionItem) -> bool {
        self.rules.iter().all(|(kind, invert)| {
            let m = kind.item_matches(i);
            if *invert {
                !m
            } else {
                m
            }
        })
    }
}

// ---------- Sort ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
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
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug)]
struct SortSpec {
    field: SortField,
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
    fn parse(s: &str) -> Result<Self> {
        let (name, dir_override) = match s.split_once(':') {
            Some((n, d)) => (n.trim(), Some(parse_direction(d.trim())?)),
            None => (s, None),
        };
        let field = SortField::parse(name)?;
        let direction = dir_override.unwrap_or_else(|| field.natural_direction());
        Ok(SortSpec { field, direction })
    }

    fn compare(&self, a: &CollectionItem, b: &CollectionItem) -> Ordering {
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

// Nulls last regardless of direction.
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

fn rating(i: &CollectionItem) -> Option<f32> {
    i.stats.as_ref().and_then(|s| s.user_rating)
}
fn geek(i: &CollectionItem) -> Option<f32> {
    i.stats.as_ref().and_then(|s| s.bayes_average)
}
fn time(i: &CollectionItem) -> Option<u32> {
    i.stats.as_ref().and_then(|s| s.playing_time)
}
fn min_players(i: &CollectionItem) -> Option<u32> {
    i.stats.as_ref().and_then(|s| s.min_players)
}
fn max_players(i: &CollectionItem) -> Option<u32> {
    i.stats.as_ref().and_then(|s| s.max_players)
}

// ---------- Columns ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Col {
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

    fn all() -> Vec<Col> {
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

    fn header(self) -> &'static str {
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

    fn cell(self, item: &CollectionItem, linkify: bool) -> String {
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

    /// Width includes a trailing space; Name is rendered last and unpadded.
    fn is_last_friendly(self) -> bool {
        matches!(self, Col::Name)
    }
}

/// Column auto-add: if the user sorted by a field that has a column, append it.
fn column_for_sort(field: SortField) -> Option<Col> {
    match field {
        SortField::Name => None,
        SortField::Year => None,
        SortField::Bggid => Some(Col::Bggid),
        SortField::Plays => Some(Col::Plays),
        SortField::Rating => Some(Col::Rating),
        SortField::Time => Some(Col::Time),
        SortField::Added => None, // collid is opaque, not worth displaying
        SortField::Geek => Some(Col::Geek),
        SortField::Players => Some(Col::Players),
    }
}

fn resolve_columns(cols_arg: Option<&str>, sort_field: SortField) -> Result<Vec<Col>> {
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
    // Default: year, name. Append the sort column if applicable and not already there.
    let mut out = vec![Col::Year, Col::Name];
    if let Some(extra) = column_for_sort(sort_field) {
        if !out.contains(&extra) {
            // Insert before Name so Name stays as the last (free-width) column.
            let name_pos = out
                .iter()
                .position(|c| *c == Col::Name)
                .unwrap_or(out.len());
            out.insert(name_pos, extra);
        }
    }
    Ok(out)
}

// ---------- Rendering ----------

fn render_table(items: &[&CollectionItem], cols: &[Col]) {
    let linkify = std::io::stdout().is_terminal();

    // Precompute each cell so column widths reflect actual content.
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(items.len());
    for it in items {
        rows.push(cols.iter().map(|c| c.cell(it, linkify)).collect());
    }

    // Width per column = max of header and cells. Skip width-padding the final
    // column if it's name (variable, often long, and last).
    let last_idx = cols.len().saturating_sub(1);
    let mut widths: Vec<usize> = cols.iter().map(|c| c.header().chars().count()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            let w = display_width(cell);
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }

    // Header
    let mut header = String::new();
    for (i, c) in cols.iter().enumerate() {
        if i == last_idx && c.is_last_friendly() {
            header.push_str(c.header());
        } else {
            header.push_str(&pad_right(c.header(), widths[i]));
            header.push_str("  ");
        }
    }
    println!("{header}");

    // Rows
    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i == last_idx && cols[i].is_last_friendly() {
                line.push_str(cell);
            } else {
                line.push_str(&pad_right(cell, widths[i]));
                line.push_str("  ");
            }
        }
        println!("{line}");
    }
}

fn pad_right(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        s.to_string()
    } else {
        let mut out = s.to_string();
        out.push_str(&" ".repeat(width - w));
        out
    }
}

/// Visual width of `s`, treating OSC 8 hyperlinks as zero-width and the link
/// label as its char count.
fn display_width(s: &str) -> usize {
    // Strip OSC 8 sequences: ESC ] 8 ; ... BEL or ESC \
    // Format we emit: "\x1b]8;;<url>\x1b\\<text>\x1b]8;;\x1b\\"
    let mut visible = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&']') {
            // Skip until ESC \
            while let Some(c2) = chars.next() {
                if c2 == '\x1b' && chars.next() == Some('\\') {
                    break;
                }
            }
        } else {
            visible.push(c);
        }
    }
    visible.chars().count()
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

// Small helper so the Time cell can append "min" without scattered logic.
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
    fn sort_parses_field_and_uses_natural_direction_by_default() {
        let a = SortSpec::parse("name").unwrap();
        assert_eq!(a.field, SortField::Name);
        assert_eq!(a.direction, Direction::Asc);

        let c = SortSpec::parse("plays").unwrap();
        assert_eq!(c.field, SortField::Plays);
        assert_eq!(c.direction, Direction::Desc); // natural
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
        // Explicit direction that matches natural is fine, not an error.
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

        let mut unrated = item(true, "boardgame", 0);
        unrated.name = "Unrated".into();
        let mut low = item(true, "boardgame", 0);
        low.name = "Low".into();
        low.stats = Some(Stats {
            min_players: None,
            max_players: None,
            playing_time: None,
            user_rating: Some(4.0),
            average: None,
            bayes_average: None,
            users_rated: None,
        });
        let mut high = item(true, "boardgame", 0);
        high.name = "High".into();
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
        // No implicit Plays column because user supplied --cols explicitly.
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

    fn item(owned: bool, subtype: &str, plays: u32) -> CollectionItem {
        use crate::model::Status;
        CollectionItem {
            id: 1,
            collid: Some(1),
            subtype: subtype.into(),
            name: "x".into(),
            year_published: None,
            image: None,
            thumbnail: None,
            status: Status {
                own: owned,
                ..Default::default()
            },
            num_plays: plays,
            stats: None,
        }
    }

    #[test]
    fn filter_default_keeps_owned_boardgames_and_drops_expansions() {
        let spec = FilterSpec::parse("owned,not:expansion").unwrap();
        let bg = item(true, "boardgame", 0);
        let exp = item(true, "boardgameexpansion", 0);
        let unowned = item(false, "boardgame", 0);
        assert!(spec.matches(&bg));
        assert!(!spec.matches(&exp));
        assert!(!spec.matches(&unowned));
    }

    #[test]
    fn filter_played_matches_when_num_plays_gt_zero() {
        let spec = FilterSpec::parse("played").unwrap();
        assert!(spec.matches(&item(false, "boardgame", 3)));
        assert!(!spec.matches(&item(false, "boardgame", 0)));
    }

    #[test]
    fn filter_solo_requires_min_players_le_one() {
        use crate::model::Stats;
        let mut i = item(true, "boardgame", 0);
        i.stats = Some(Stats {
            min_players: Some(1),
            max_players: Some(4),
            playing_time: None,
            user_rating: None,
            average: None,
            bayes_average: None,
            users_rated: None,
        });
        assert!(FilterSpec::parse("solo").unwrap().matches(&i));
        i.stats.as_mut().unwrap().min_players = Some(2);
        assert!(!FilterSpec::parse("solo").unwrap().matches(&i));
        i.stats.as_mut().unwrap().min_players = Some(0);
        assert!(!FilterSpec::parse("solo").unwrap().matches(&i));
    }

    #[test]
    fn filter_rejects_unknown_value() {
        assert!(FilterSpec::parse("owned,bogus").is_err());
    }

    #[test]
    fn filter_rejects_empty_value() {
        assert!(FilterSpec::parse("").is_err());
        assert!(FilterSpec::parse("  ,  ").is_err());
    }
}
