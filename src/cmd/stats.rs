use crate::cache;
use crate::config;
use crate::error::{Error, Result};
use crate::model::{CacheFile, CollectionItem};
use crate::paths;
use anstream::println;
use anstyle::{AnsiColor, Effects, Style};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sparkline::{select_sparkline, SparkTheme, SparkThemeName};
use std::collections::BTreeMap;

pub fn run(json: bool) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;
    let report = build(&cache);
    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Parse(format!("json: {e}")))?;
        std::println!("{out}");
    } else {
        print_text(&report);
    }
    Ok(())
}

/// One-screen summary printed when `bgg` is run with no subcommand. Acts as the
/// default landing view: header (user, item count, last sync), four counts /
/// averages, and a footer pointing at `bgg stats` / `bgg list`.
pub fn run_summary() -> Result<()> {
    let Some(username) = config::load()?.username else {
        println!("No logged-in user. Run `bgg auth`.");
        return Ok(());
    };
    let cache = match cache::load(&paths::cache_file(&username), &username) {
        Ok(c) => c,
        Err(Error::NoCache(_)) => {
            println!("Logged in as {username}. Run `bgg sync` to fetch your collection.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    print_summary(&build(&cache));
    Ok(())
}

const SECTION: Style = Style::new()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan)))
    .effects(Effects::BOLD);
const LABEL: Style = Style::new().effects(Effects::DIMMED);
const STRONG: Style = Style::new().effects(Effects::BOLD);
const ACCENT: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));
const MUTED: Style = Style::new().effects(Effects::DIMMED);

// ---------- Report shape ----------

#[derive(Debug, Serialize)]
struct Report {
    username: String,
    last_sync: Option<DateTime<Utc>>,
    items: ItemsBlock,
    statuses: Statuses,
    owned: OwnedStats,
}

#[derive(Debug, Serialize)]
struct ItemsBlock {
    total: usize,
    boardgames: usize,
    expansions: usize,
    other: usize,
}

#[derive(Debug, Serialize)]
struct Statuses {
    own: usize,
    own_boardgames: usize,
    own_expansions: usize,
    prev_owned: usize,
    for_trade: usize,
    want: usize,
    want_to_play: usize,
    want_to_buy: usize,
    wishlist: usize,
    preordered: usize,
    wishlist_by_priority: BTreeMap<u8, usize>,
}

#[derive(Debug, Serialize)]
struct OwnedStats {
    count: usize,
    plays: PlaysStats,
    ratings: RatingsStats,
    year: YearStats,
    time: TimeStats,
    players: PlayersStats,
}

#[derive(Debug, Serialize)]
struct PlaysStats {
    total: u64,
    played_count: usize,
    unplayed_count: usize,
    avg_per_owned: f32,
    top: Vec<TopEntry>,
}

#[derive(Debug, Serialize)]
struct RatingsStats {
    rated_count: usize,
    your_average: Option<f32>,
    bgg_average: Option<f32>,
    /// Counts of your-ratings in integer buckets 1..=10 (rounded). Index 0 = "1".
    your_distribution: [usize; 10],
    /// Counts of BGG averages in integer buckets 1..=10 (rounded), one per owned game.
    bgg_distribution: [usize; 10],
    top: Vec<TopEntry>,
}

#[derive(Debug, Serialize)]
struct YearStats {
    oldest: Option<TopEntry>,
    newest: Option<TopEntry>,
    by_year: BTreeMap<i32, usize>,
}

#[derive(Debug, Serialize)]
struct TimeStats {
    avg_minutes: Option<u32>,
    quick_under_30: usize,
    medium_30_to_89: usize,
    long_90_to_180: usize,
    epic_over_180: usize,
}

#[derive(Debug, Serialize)]
struct PlayersStats {
    solo_capable: usize,
    two_capable: usize,
    common_range: Option<(u32, u32)>,
}

#[derive(Debug, Serialize)]
struct TopEntry {
    id: u32,
    name: String,
    value: f64,
}

// ---------- Build ----------

fn build(cache: &CacheFile) -> Report {
    let all: Vec<&CollectionItem> = cache.items.values().collect();
    let items = items_block(&all);
    let statuses = statuses(&all);
    let owned: Vec<&CollectionItem> = all
        .iter()
        .copied()
        .filter(|i| i.status.own && i.subtype == "boardgame")
        .collect();
    let owned_stats = OwnedStats {
        count: owned.len(),
        plays: plays_stats(&owned),
        ratings: ratings_stats(&owned),
        year: year_stats(&owned),
        time: time_stats(&owned),
        players: players_stats(&owned),
    };
    Report {
        username: cache.username.clone(),
        last_sync: cache.last_sync,
        items,
        statuses,
        owned: owned_stats,
    }
}

fn items_block(all: &[&CollectionItem]) -> ItemsBlock {
    let mut boardgames = 0;
    let mut expansions = 0;
    let mut other = 0;
    for i in all {
        match i.subtype.as_str() {
            "boardgame" => boardgames += 1,
            "boardgameexpansion" => expansions += 1,
            _ => other += 1,
        }
    }
    ItemsBlock {
        total: all.len(),
        boardgames,
        expansions,
        other,
    }
}

fn statuses(all: &[&CollectionItem]) -> Statuses {
    let mut s = Statuses {
        own: 0,
        own_boardgames: 0,
        own_expansions: 0,
        prev_owned: 0,
        for_trade: 0,
        want: 0,
        want_to_play: 0,
        want_to_buy: 0,
        wishlist: 0,
        preordered: 0,
        wishlist_by_priority: BTreeMap::new(),
    };
    for i in all {
        let st = &i.status;
        if st.own {
            s.own += 1;
            match i.subtype.as_str() {
                "boardgame" => s.own_boardgames += 1,
                "boardgameexpansion" => s.own_expansions += 1,
                _ => {}
            }
        }
        if st.prev_owned {
            s.prev_owned += 1;
        }
        if st.for_trade {
            s.for_trade += 1;
        }
        if st.want {
            s.want += 1;
        }
        if st.want_to_play {
            s.want_to_play += 1;
        }
        if st.want_to_buy {
            s.want_to_buy += 1;
        }
        if st.wishlist {
            s.wishlist += 1;
            if let Some(p) = st.wishlist_priority {
                *s.wishlist_by_priority.entry(p).or_insert(0) += 1;
            }
        }
        if st.preordered {
            s.preordered += 1;
        }
    }
    s
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

/// Trim leading and trailing years that look like outliers — i.e. the sparse
/// tails on either end whose cumulative share is below `drop_frac/2` of the
/// total per side. Returns the (min_year, max_year) of the kept range, or
/// None if there's no data.
fn trim_outlier_years(by_year: &BTreeMap<i32, usize>, drop_frac: f64) -> Option<(i32, i32)> {
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

// ---------- Text render ----------

const LABEL_W: usize = 19;

fn print_text(r: &Report) {
    println!(
        "{LABEL}Collection stats for{LABEL:#} {STRONG}{}{STRONG:#}",
        r.username
    );
    match r.last_sync {
        Some(t) => println!(
            "{LABEL}Last sync:{LABEL:#} {} {MUTED}({}){MUTED:#}",
            human_since(t, Utc::now()),
            t.to_rfc3339(),
        ),
        None => println!("{LABEL}Last sync:{LABEL:#} never"),
    }
    println!();

    println!("{SECTION}Items{SECTION:#}");
    let other_suffix = if r.items.other > 0 {
        format!(", other {}", r.items.other)
    } else {
        String::new()
    };
    print_line(
        "Total",
        format_args!(
            "{STRONG}{}{STRONG:#}  {MUTED}(boardgames {}, expansions {}{}){MUTED:#}",
            r.items.total, r.items.boardgames, r.items.expansions, other_suffix
        ),
    );
    let s = &r.statuses;
    print_line(
        "Owned",
        format_args!(
            "{STRONG}{}{STRONG:#}  {MUTED}(boardgames {}, expansions {}){MUTED:#}",
            s.own, s.own_boardgames, s.own_expansions
        ),
    );
    print_count("Previously owned", s.prev_owned);
    let wishlist_suffix = if s.wishlist_by_priority.is_empty() {
        String::new()
    } else {
        let parts: Vec<String> = s
            .wishlist_by_priority
            .iter()
            .map(|(p, n)| format!("P{p}: {n}"))
            .collect();
        format!("   {MUTED}({}){MUTED:#}", parts.join("  "))
    };
    print_line(
        "Wishlist",
        format_args!("{STRONG}{}{STRONG:#}{}", s.wishlist, wishlist_suffix),
    );
    print_count("Want to play", s.want_to_play);
    print_count("Want to buy", s.want_to_buy);
    print_count("Want", s.want);
    print_count("Preordered", s.preordered);
    print_count("For trade", s.for_trade);

    let owned = &r.owned;
    if owned.count == 0 {
        println!();
        println!("No owned boardgames cached — `bgg sync` first.");
        return;
    }

    let scope = format!("{MUTED}({} owned boardgames){MUTED:#}", owned.count);

    println!();
    println!("{SECTION}Plays{SECTION:#} {scope}");
    print_count("Total plays", owned.plays.total);
    print_line(
        "Played",
        format_args!(
            "{STRONG}{}{STRONG:#}   {LABEL}unplayed{LABEL:#}   {STRONG}{}{STRONG:#}",
            owned.plays.played_count, owned.plays.unplayed_count
        ),
    );
    print_line(
        "Average plays/owned",
        format_args!("{STRONG}{:.1}{STRONG:#}", owned.plays.avg_per_owned),
    );
    if !owned.plays.top.is_empty() {
        println!("  {LABEL}Most played:{LABEL:#}");
        for t in &owned.plays.top {
            println!(
                "    {:<36} {ACCENT}{}{ACCENT:#}",
                truncate(&t.name, 36),
                t.value as u64
            );
        }
    }

    println!();
    println!("{SECTION}Ratings{SECTION:#} {scope}");
    print_line(
        "BGG average",
        format_args!(
            "{}{}",
            fmt_avg(owned.ratings.bgg_average),
            inline_spark(&owned.ratings.bgg_distribution),
        ),
    );
    print_line(
        "You rated",
        format_args!(
            "{STRONG}{}{STRONG:#} {MUTED}/ {}{MUTED:#}",
            owned.ratings.rated_count, owned.count
        ),
    );
    print_line(
        "Your average",
        format_args!(
            "{}{}",
            fmt_avg(owned.ratings.your_average),
            inline_spark(&owned.ratings.your_distribution),
        ),
    );
    if !owned.ratings.top.is_empty() {
        println!("  {LABEL}Your top:{LABEL:#}");
        for t in &owned.ratings.top {
            println!(
                "    {:<36} {ACCENT}{:.1}{ACCENT:#}",
                truncate(&t.name, 36),
                t.value
            );
        }
    }

    println!();
    println!("{SECTION}Year{SECTION:#} {scope}");
    if let Some(o) = &owned.year.oldest {
        print_line(
            "Oldest",
            format_args!("{ACCENT}{}{ACCENT:#}  {}", o.value as i32, o.name),
        );
    }
    if let Some(n) = &owned.year.newest {
        print_line(
            "Newest",
            format_args!("{ACCENT}{}{ACCENT:#}  {}", n.value as i32, n.name),
        );
    }
    if let Some((min_y, max_y)) = trim_outlier_years(&owned.year.by_year, 0.02) {
        let dense: Vec<usize> = (min_y..=max_y)
            .map(|y| owned.year.by_year.get(&y).copied().unwrap_or(0))
            .collect();
        let spark = sparkline_row(&dense);
        print_line(
            "By year",
            format_args!(
                "{LABEL}{min_y}{LABEL:#} {ACCENT}{spark}{ACCENT:#} {LABEL}{max_y}{LABEL:#}"
            ),
        );
    }

    println!();
    println!("{SECTION}Time{SECTION:#} {scope}");
    let avg = owned
        .time
        .avg_minutes
        .map(|m| format!("{STRONG}{m} min{STRONG:#}"))
        .unwrap_or_else(|| format!("{MUTED}-{MUTED:#}"));
    print_line("Average", format_args!("{avg}"));
    print_count("Quick   (<30m)", owned.time.quick_under_30);
    print_count("Medium  (30-89m)", owned.time.medium_30_to_89);
    print_count("Long    (90-180m)", owned.time.long_90_to_180);
    print_count("Epic    (>180m)", owned.time.epic_over_180);

    println!();
    println!("{SECTION}Players{SECTION:#} {scope}");
    print_count("Solo capable", owned.players.solo_capable);
    print_count("Plays at 2", owned.players.two_capable);
    let range = owned
        .players
        .common_range
        .map(|(a, b)| {
            if a == b {
                format!("{a}")
            } else {
                format!("{a}-{b}")
            }
        })
        .unwrap_or_else(|| "-".into());
    print_line("Common range", format_args!("{ACCENT}{range}{ACCENT:#}"));
}

fn print_summary(r: &Report) {
    let when = match r.last_sync {
        Some(t) => format!("last sync {}", human_since(t, Utc::now())),
        None => "never synced".into(),
    };
    println!(
        "{LABEL}Synced as{LABEL:#} {STRONG}{}{STRONG:#} {MUTED}·{MUTED:#} {STRONG}{}{STRONG:#} items {MUTED}·{MUTED:#} {when}",
        r.username, r.items.total,
    );
    println!();
    print_line(
        "Total",
        format_args!(
            "{STRONG}{}{STRONG:#}  {MUTED}(boardgames {}, expansions {}){MUTED:#}",
            r.items.total, r.items.boardgames, r.items.expansions,
        ),
    );
    let s = &r.statuses;
    print_line(
        "Owned",
        format_args!(
            "{STRONG}{}{STRONG:#}  {MUTED}(boardgames {}, expansions {}){MUTED:#}",
            s.own, s.own_boardgames, s.own_expansions,
        ),
    );
    let owned = &r.owned;
    if owned.count > 0 {
        print_line(
            "Plays",
            format_args!(
                "{STRONG}{}{STRONG:#}  {MUTED}across {} of {} owned{MUTED:#}",
                owned.plays.total, owned.plays.played_count, owned.count,
            ),
        );
        let your = fmt_avg_compact(owned.ratings.your_average);
        let bgg = fmt_avg_compact(owned.ratings.bgg_average);
        print_line(
            "Ratings",
            format_args!(
                "{your} {MUTED}your avg{MUTED:#} {MUTED}·{MUTED:#} {bgg} {MUTED}BGG avg{MUTED:#}"
            ),
        );
    }
    println!();
    println!("{MUTED}`bgg stats` for full breakdown · `bgg list` for the table{MUTED:#}");
}

fn human_since(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let s = now.signed_duration_since(then).num_seconds().max(0);
    if s < 60 {
        return "just now".into();
    }
    let m = s / 60;
    if m < 60 {
        return format!("{m}m ago");
    }
    let h = m / 60;
    if h < 24 {
        return format!("{h}h ago");
    }
    let d = h / 24;
    if d < 30 {
        return format!("{d}d ago");
    }
    let mo = d / 30;
    if mo < 12 {
        return format!("{mo}mo ago");
    }
    format!("{}y ago", d / 365)
}

fn print_line(label: &str, value: std::fmt::Arguments<'_>) {
    let pad = LABEL_W.saturating_sub(label.chars().count());
    let spaces = " ".repeat(pad);
    println!("  {LABEL}{label}{LABEL:#}{spaces} {value}");
}

fn print_count<N: std::fmt::Display>(label: &str, value: N) {
    print_line(label, format_args!("{STRONG}{value}{STRONG:#}"));
}

/// Fixed-width rating average for inline alignment with a trailing sparkline.
/// Width 5 covers up to "10.00".
fn fmt_avg(o: Option<f32>) -> String {
    match o {
        Some(v) => format!("{STRONG}{v:<5.2}{STRONG:#}"),
        None => format!("{MUTED}{:<5}{MUTED:#}", "-"),
    }
}

/// Unpadded variant for inline summary text where alignment isn't useful.
fn fmt_avg_compact(o: Option<f32>) -> String {
    match o {
        Some(v) => format!("{STRONG}{v:.2}{STRONG:#}"),
        None => format!("{MUTED}-{MUTED:#}"),
    }
}

/// Spark with `1 … 10` end-labels for inline display after an average.
/// Returns empty if the distribution has no data.
fn inline_spark(dist: &[usize; 10]) -> String {
    if dist.iter().sum::<usize>() == 0 {
        return String::new();
    }
    let s = sparkline_row(dist);
    format!("   {LABEL}1{LABEL:#} {ACCENT}{s}{ACCENT:#} {LABEL}10{LABEL:#}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Render a slice of counts as a one-line sparkline using min=0 and max=max(values).
/// Zero values render as a space so gaps in the data show through.
fn sparkline_row(values: &[usize]) -> String {
    let theme = select_sparkline(SparkThemeName::Classic);
    let max = *values.iter().max().unwrap_or(&0);
    if max == 0 {
        return " ".repeat(values.len());
    }
    values
        .iter()
        .map(|v| {
            if *v == 0 {
                " ".to_string()
            } else {
                spark_char(&theme, *v as f64, 0.0, max as f64)
            }
        })
        .collect()
}

fn spark_char(theme: &SparkTheme, value: f64, min: f64, max: f64) -> String {
    if max <= min {
        return " ".into();
    }
    theme.spark(min, max, value).clone()
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

    fn cache_with(items: Vec<CollectionItem>) -> CacheFile {
        let mut c = CacheFile::empty("tester");
        for i in items {
            c.items.insert(i.collid.unwrap().to_string(), i);
        }
        c
    }

    #[test]
    fn counts_status_flags_across_all_subtypes() {
        let mut a = item(1, "A", true, 0);
        let mut b = item(2, "B", false, 0);
        b.status.wishlist = true;
        b.status.wishlist_priority = Some(2);
        a.subtype = "boardgameexpansion".into();
        let r = build(&cache_with(vec![a, b]));
        assert_eq!(r.items.boardgames, 1);
        assert_eq!(r.items.expansions, 1);
        assert_eq!(r.statuses.own, 1);
        assert_eq!(r.statuses.wishlist, 1);
        assert_eq!(r.statuses.wishlist_by_priority.get(&2), Some(&1));
    }

    #[test]
    fn owned_stats_restrict_to_boardgame_subtype() {
        let mut exp = item(1, "Expansion", true, 99);
        exp.subtype = "boardgameexpansion".into();
        let game = item(2, "Game", true, 3);
        let r = build(&cache_with(vec![exp, game]));
        assert_eq!(r.owned.count, 1);
        assert_eq!(r.owned.plays.total, 3);
        assert_eq!(r.owned.plays.top.first().unwrap().name, "Game");
    }

    #[test]
    fn plays_top_excludes_unplayed_and_sorts_desc() {
        let items = vec![
            item(1, "Zero", true, 0),
            item(2, "Two", true, 2),
            item(3, "Ten", true, 10),
            item(4, "Five", true, 5),
        ];
        let r = build(&cache_with(items));
        let names: Vec<String> = r.owned.plays.top.iter().map(|t| t.name.clone()).collect();
        assert_eq!(names, vec!["Ten", "Five", "Two"]);
        assert_eq!(r.owned.plays.played_count, 3);
        assert_eq!(r.owned.plays.unplayed_count, 1);
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
        let r = build(&cache_with(items));
        assert_eq!(r.owned.year.by_year.get(&1985), Some(&1));
        assert_eq!(r.owned.year.by_year.get(&1992), Some(&1));
        assert_eq!(r.owned.year.by_year.get(&2020), Some(&1));
        assert_eq!(r.owned.year.by_year.get(&2021), Some(&1));
        assert_eq!(r.owned.year.oldest.as_ref().unwrap().value as i32, 1985);
        assert_eq!(r.owned.year.newest.as_ref().unwrap().value as i32, 2021);
    }

    #[test]
    fn trim_outlier_years_drops_thin_leading_and_trailing_tails() {
        // 2 + 100 + 2 = 104 games, with sparse outliers at both ends.
        let mut by = BTreeMap::new();
        by.insert(1500, 1);
        by.insert(1600, 1);
        for y in 2000..=2009 {
            by.insert(y, 10);
        }
        by.insert(2100, 1);
        by.insert(2200, 1);
        // drop_frac = 0.04, cutoff per side = ceil(104 * 0.02) = 3.
        // Leading cum: 1, 2, 12 → lead = 2000.
        // Trailing cum: 1, 2, 12 → trail = 2009.
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
        let r = build(&cache_with(items));
        assert_eq!(r.owned.time.quick_under_30, 1);
        assert_eq!(r.owned.time.medium_30_to_89, 2);
        assert_eq!(r.owned.time.long_90_to_180, 2);
        assert_eq!(r.owned.time.epic_over_180, 1);
    }

    #[test]
    fn ratings_distribution_buckets_rounded_to_integers() {
        let mk = |id, r| {
            let mut i = item(id, "x", true, 0);
            i.stats.as_mut().unwrap().user_rating = Some(r);
            i
        };
        // 7.4 → 7, 7.6 → 8, 10.0 → 10. 11.0 is out of [1,10] and dropped.
        let items = vec![mk(1, 7.4), mk(2, 7.6), mk(3, 10.0), mk(4, 11.0)];
        let r = build(&cache_with(items));
        assert_eq!(r.owned.ratings.your_distribution[6], 1, "bucket 7");
        assert_eq!(r.owned.ratings.your_distribution[7], 1, "bucket 8");
        assert_eq!(r.owned.ratings.your_distribution[9], 1, "bucket 10");
        let total: usize = r.owned.ratings.your_distribution.iter().sum();
        assert_eq!(total, 3, "11.0 is dropped");
    }

    #[test]
    fn sparkline_row_uses_eight_block_chars_and_handles_zero_max() {
        // All zero → blank padded to length.
        assert_eq!(sparkline_row(&[0, 0, 0]), "   ");
        // Non-empty values render to one block-char each.
        let out = sparkline_row(&[1, 2, 4, 8]);
        assert_eq!(out.chars().count(), 4);
        // Top bucket must be the full block.
        assert!(out.ends_with('█'));
    }

    #[test]
    fn sparkline_row_renders_zero_buckets_as_spaces() {
        // Gaps in the data become visible spaces.
        let out = sparkline_row(&[5, 0, 0, 5]);
        let chars: Vec<char> = out.chars().collect();
        assert_eq!(chars.len(), 4);
        assert_eq!(chars[1], ' ');
        assert_eq!(chars[2], ' ');
        assert_ne!(chars[0], ' ');
        assert_ne!(chars[3], ' ');
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
        let r = build(&cache_with(items));
        assert_eq!(r.owned.players.solo_capable, 1);
        assert_eq!(r.owned.players.two_capable, 3);
        assert_eq!(r.owned.players.common_range, Some((2, 4)));
    }
}
