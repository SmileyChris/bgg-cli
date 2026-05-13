use crate::cache;
use crate::config;
use crate::error::{Error, Result};
use crate::model::CollectionItem;
use crate::paths;
use std::cmp::Ordering;
use std::io::IsTerminal;

pub fn run(sort_arg: String, cols_arg: Option<String>, json: bool) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;

    if json {
        let items: Vec<&CollectionItem> = cache.items.values().collect();
        let out =
            serde_json::to_string_pretty(&items).map_err(|e| Error::Parse(format!("json: {e}")))?;
        println!("{out}");
        return Ok(());
    }

    let sort = SortSpec::parse(&sort_arg)?;
    let cols = resolve_columns(cols_arg.as_deref(), sort.field)?;

    let mut items: Vec<&CollectionItem> = cache
        .items
        .values()
        .filter(|i| i.status.own && i.subtype == "boardgame")
        .collect();
    items.sort_by(|a, b| sort.compare(a, b));

    render_table(&items, &cols);
    Ok(())
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
            other => return Err(Error::Parse(format!("unknown sort field `{other}`"))),
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

impl SortSpec {
    fn parse(s: &str) -> Result<Self> {
        let (name, invert) = match s.strip_prefix('^') {
            Some(rest) => (rest, true),
            None => (s, false),
        };
        let field = SortField::parse(name)?;
        let direction = match (field.natural_direction(), invert) {
            (d, false) => d,
            (Direction::Asc, true) => Direction::Desc,
            (Direction::Desc, true) => Direction::Asc,
        };
        Ok(SortSpec { field, direction })
    }

    fn compare(&self, a: &CollectionItem, b: &CollectionItem) -> Ordering {
        let primary = field_cmp(a, b, self.field);
        let primary = match self.direction {
            Direction::Asc => primary,
            Direction::Desc => primary.reverse(),
        };
        primary.then_with(|| name_cmp(a, b))
    }
}

fn field_cmp(a: &CollectionItem, b: &CollectionItem, f: SortField) -> Ordering {
    match f {
        SortField::Name => name_cmp(a, b),
        SortField::Year => opt_cmp(a.year_published, b.year_published),
        SortField::Bggid => a.id.cmp(&b.id),
        SortField::Plays => a.num_plays.cmp(&b.num_plays),
        SortField::Rating => opt_f_cmp(rating(a), rating(b)),
        SortField::Time => opt_cmp(time(a), time(b)),
        SortField::Added => opt_cmp(a.collid, b.collid),
        SortField::Geek => opt_f_cmp(geek(a), geek(b)),
        SortField::Players => opt_cmp(min_players(a), min_players(b))
            .then_with(|| opt_cmp(max_players(a), max_players(b))),
    }
}

fn name_cmp(a: &CollectionItem, b: &CollectionItem) -> Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

// Nulls last regardless of direction.
fn opt_cmp<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn opt_f_cmp(a: Option<f32>, b: Option<f32>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
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
            other => return Err(Error::Parse(format!("unknown column `{other}`"))),
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
            return Err(Error::Parse("--cols is empty".into()));
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
    fn sort_parses_field_and_inverts_with_caret_prefix() {
        let a = SortSpec::parse("name").unwrap();
        assert_eq!(a.field, SortField::Name);
        assert_eq!(a.direction, Direction::Asc);

        let b = SortSpec::parse("^name").unwrap();
        assert_eq!(b.direction, Direction::Desc);

        let c = SortSpec::parse("plays").unwrap();
        assert_eq!(c.field, SortField::Plays);
        assert_eq!(c.direction, Direction::Desc); // natural

        let d = SortSpec::parse("^plays").unwrap();
        assert_eq!(d.direction, Direction::Asc); // inverted from natural
    }

    #[test]
    fn sort_rejects_unknown_field() {
        assert!(SortSpec::parse("nope").is_err());
        assert!(SortSpec::parse("^nope").is_err());
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
}
