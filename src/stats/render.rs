use crate::stats::owned::trim_outlier_years;
use crate::stats::report::Report;
use crate::stats::spark::sparkline_row;
use anstream::println;
use anstyle::{AnsiColor, Effects, Style};
use chrono::{DateTime, Utc};

const SECTION: Style = Style::new()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan)))
    .effects(Effects::BOLD);
const LABEL: Style = Style::new().effects(Effects::DIMMED);
const STRONG: Style = Style::new().effects(Effects::BOLD);
const ACCENT: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));
const MUTED: Style = Style::new().effects(Effects::DIMMED);
const LABEL_W: usize = 19;

pub(crate) fn print_text(r: &Report) {
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

pub(crate) fn print_summary(r: &Report) {
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

fn fmt_avg(o: Option<f32>) -> String {
    match o {
        Some(v) => format!("{STRONG}{v:<5.2}{STRONG:#}"),
        None => format!("{MUTED}{:<5}{MUTED:#}", "-"),
    }
}

fn fmt_avg_compact(o: Option<f32>) -> String {
    match o {
        Some(v) => format!("{STRONG}{v:.2}{STRONG:#}"),
        None => format!("{MUTED}-{MUTED:#}"),
    }
}

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
