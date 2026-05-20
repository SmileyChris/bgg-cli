use crate::error::Result;
use crate::model::CacheFile;
use crate::stats::common;
use crate::stats::owned::{trim_outlier_years, TopEntry};
use serde::Serialize;
use std::collections::BTreeMap;

const DECADE_BUCKETS: &[(i32, i32, &str)] = &[
    (0, 1969, "< 1970"),
    (1970, 1979, "1970s"),
    (1980, 1989, "1980s"),
    (1990, 1999, "1990s"),
    (2000, 2009, "2000s"),
    (2010, 2019, "2010s"),
    (2020, i32::MAX, "2020s"),
];

#[derive(Debug, Serialize)]
pub(crate) struct YearReport {
    pub(crate) oldest: Option<TopEntry>,
    pub(crate) newest: Option<TopEntry>,
    pub(crate) by_year: BTreeMap<i32, usize>,
    pub(crate) by_decade: Vec<DecadeBucket>,
    pub(crate) sorted: Vec<TopEntry>,
    /// Trimmed range for the bar chart.
    pub(crate) chart_range: Option<(i32, i32)>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DecadeBucket {
    pub(crate) label: String,
    pub(crate) count: usize,
}

pub(crate) fn build(cache: &CacheFile) -> YearReport {
    let owned = common::owned_boardgames_from_cache(cache);
    let with_year = common::published_years(owned.iter().copied());

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

    let by_decade: Vec<DecadeBucket> = DECADE_BUCKETS
        .iter()
        .map(|&(lo, hi, label)| {
            let count = with_year
                .iter()
                .filter(|(_, y)| *y >= lo && *y <= hi)
                .count();
            DecadeBucket {
                label: label.to_string(),
                count,
            }
        })
        .filter(|b| b.count > 0)
        .collect();

    let chart_range = trim_outlier_years(&by_year, 0.02);

    // Sorted by year (oldest first for the list)
    let mut sorted = with_year.clone();
    sorted.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.name.cmp(&b.0.name)));
    let sorted: Vec<TopEntry> = sorted
        .iter()
        .map(|(i, y)| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: *y as f64,
        })
        .collect();

    YearReport {
        oldest,
        newest,
        by_year,
        by_decade,
        sorted,
        chart_range,
    }
}

pub(crate) fn print_text(r: &YearReport) {
    use crate::stats::render::{
        bar_chart, print_line, print_section, ACCENT, LABEL, MUTED, STRONG,
    };
    use anstream::println;

    println!("{ACCENT}━━━ Year published deep dive ━━━{ACCENT:#}");
    println!();

    if let Some(o) = &r.oldest {
        print_line(
            "Oldest",
            format_args!("{ACCENT}{}{ACCENT:#}  {}", o.value as i32, o.name),
        );
    }
    if let Some(n) = &r.newest {
        print_line(
            "Newest",
            format_args!("{ACCENT}{}{ACCENT:#}  {}", n.value as i32, n.name),
        );
    }
    println!();

    // Decade summary
    if !r.by_decade.is_empty() {
        print_section("By decade");
        let max = r
            .by_decade
            .iter()
            .map(|b| b.count)
            .max()
            .unwrap_or(1)
            .max(1);
        for b in &r.by_decade {
            let bar = bar_chart(b.count, max, 20);
            println!(
                "  {LABEL}{:>8}{LABEL:#} {ACCENT}{}{ACCENT:#} {}{}{}",
                b.label,
                bar,
                STRONG.render(),
                b.count,
                STRONG.render_reset()
            );
        }
        println!();
    }

    let picks = random_decade_picks(r);
    if !picks.is_empty() {
        print_section("Random picks by decade");
        println!();
        for (label, game) in picks {
            println!(
                "  {LABEL}{:>8}{LABEL:#}  {ACCENT}{}{ACCENT:#}  {}",
                label, game.value as i32, game.name
            );
        }
        println!();
    }

    // Year bar chart
    if let Some((min_y, max_y)) = r.chart_range {
        print_section("By year");
        let dense: Vec<usize> = (min_y..=max_y)
            .map(|y| r.by_year.get(&y).copied().unwrap_or(0))
            .collect();
        let max_dense = dense.iter().max().copied().unwrap_or(1).max(1);
        // Show every 5th year label to avoid clutter
        for (i, &count) in dense.iter().enumerate() {
            let year = min_y + i as i32;
            let bar = bar_chart(count, max_dense, 30);
            if i % 5 == 0 {
                println!(
                    "  {LABEL}{:>4}{LABEL:#} {ACCENT}{}{ACCENT:#} {}",
                    year, bar, count,
                );
            } else {
                println!(
                    "  {MUTED}{:>4}{MUTED:#} {ACCENT}{}{ACCENT:#} {}",
                    "", bar, count,
                );
            }
        }
        println!();
    }

    if !r.sorted.is_empty() {
        println!("{MUTED}List games in this order: `bgg list --sort year`{MUTED:#}");
    }
}

fn random_decade_picks(r: &YearReport) -> Vec<(&'static str, &TopEntry)> {
    let seed = common::random_seed();
    DECADE_BUCKETS
        .iter()
        .filter_map(|&(lo, hi, label)| {
            let games: Vec<&TopEntry> = r
                .sorted
                .iter()
                .filter(|g| {
                    let year = g.value as i32;
                    year >= lo && year <= hi
                })
                .collect();
            if games.len() <= 1 {
                return None;
            }
            let idx = common::random_index(seed, label, games.len())?;
            Some((label, games[idx]))
        })
        .collect()
}

pub(crate) fn run(cache: &CacheFile, json: bool) -> Result<()> {
    let report = build(cache);
    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| crate::error::Error::Parse(format!("json: {e}")))?;
        std::println!("{out}");
    } else {
        print_text(&report);
    }
    Ok(())
}
