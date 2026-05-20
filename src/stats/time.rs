use crate::error::Result;
use crate::model::{CacheFile, CollectionItem};
use crate::stats::common;
use crate::stats::owned::TopEntry;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct TimeReport {
    pub(crate) avg_minutes: Option<u32>,
    pub(crate) quick_under_30: Vec<TopEntry>,
    pub(crate) medium_30_to_89: Vec<TopEntry>,
    pub(crate) long_90_to_180: Vec<TopEntry>,
    pub(crate) epic_over_180: Vec<TopEntry>,
    pub(crate) sorted: Vec<TopEntry>,
    pub(crate) histogram: Vec<HistogramBucket>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HistogramBucket {
    pub(crate) label: String,
    pub(crate) count: usize,
}

pub(crate) fn build(cache: &CacheFile) -> TimeReport {
    let owned = common::owned_boardgames_from_cache(cache);
    let with_time = common::playing_times(owned.iter().copied());

    let avg_minutes = if with_time.is_empty() {
        None
    } else {
        Some(
            (with_time.iter().map(|(_, t)| *t as f64).sum::<f64>() / with_time.len() as f64).round()
                as u32,
        )
    };

    let mut quick: Vec<&CollectionItem> = Vec::new();
    let mut medium: Vec<&CollectionItem> = Vec::new();
    let mut long: Vec<&CollectionItem> = Vec::new();
    let mut epic: Vec<&CollectionItem> = Vec::new();

    for &(i, t) in &with_time {
        match t {
            0..=29 => quick.push(i),
            30..=89 => medium.push(i),
            90..=180 => long.push(i),
            _ => epic.push(i),
        }
    }

    fn sort_by_time<'a>(items: &mut [&'a CollectionItem], with_time: &[(&CollectionItem, u32)]) {
        items.sort_by(|a, b| {
            let ta = with_time.iter().find(|(i, _)| i.id == a.id).map(|(_, t)| t);
            let tb = with_time.iter().find(|(i, _)| i.id == b.id).map(|(_, t)| t);
            ta.cmp(&tb).then(a.name.cmp(&b.name))
        });
    }

    sort_by_time(&mut quick, &with_time);
    sort_by_time(&mut medium, &with_time);
    sort_by_time(&mut long, &with_time);
    sort_by_time(&mut epic, &with_time);

    let to_entries =
        |items: &[&CollectionItem], time_map: &[(&CollectionItem, u32)]| -> Vec<TopEntry> {
            items
                .iter()
                .map(|i| {
                    let t = time_map
                        .iter()
                        .find(|(ti, _)| ti.id == i.id)
                        .map(|(_, t)| *t);
                    TopEntry {
                        id: i.id,
                        name: i.name.clone(),
                        value: t.unwrap_or(0) as f64,
                    }
                })
                .collect()
        };

    // Full sorted list (by time ascending)
    let mut all_sorted = with_time.clone();
    all_sorted.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.name.cmp(&b.0.name)));
    let sorted: Vec<TopEntry> = all_sorted
        .iter()
        .map(|(i, t)| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: *t as f64,
        })
        .collect();

    // Histogram: 15-min buckets
    let buckets: &[(u32, u32, &str)] = &[
        (1, 15, "1-15m"),
        (16, 30, "16-30m"),
        (31, 45, "31-45m"),
        (46, 60, "46-60m"),
        (61, 90, "61-90m"),
        (91, 120, "91-120m"),
        (121, 180, "121-180m"),
        (181, 240, "181-240m"),
        (241, u32::MAX, "240m+"),
    ];
    let histogram: Vec<HistogramBucket> = buckets
        .iter()
        .map(|&(lo, hi, label)| {
            let count = with_time
                .iter()
                .filter(|(_, t)| *t >= lo && *t <= hi)
                .count();
            HistogramBucket {
                label: label.to_string(),
                count,
            }
        })
        .collect();

    TimeReport {
        avg_minutes,
        quick_under_30: to_entries(&quick, &with_time),
        medium_30_to_89: to_entries(&medium, &with_time),
        long_90_to_180: to_entries(&long, &with_time),
        epic_over_180: to_entries(&epic, &with_time),
        sorted,
        histogram,
    }
}

pub(crate) fn print_text(r: &TimeReport) {
    use crate::stats::render::{
        bar_chart, print_count, print_line, print_section, truncate, ACCENT, LABEL, MUTED, STRONG,
    };
    use anstream::println;

    println!("{ACCENT}━━━ Playing time deep dive ━━━{ACCENT:#}");
    println!();

    let avg = r
        .avg_minutes
        .map(|m| format!("{STRONG}{m} min{STRONG:#}"))
        .unwrap_or_else(|| format!("{MUTED}-{MUTED:#}"));
    print_line("Average", format_args!("{avg}"));
    println!();

    // Summaries
    print_section("By bucket");
    print_count("Quick   (<30m)", r.quick_under_30.len());
    print_count("Medium  (30-89m)", r.medium_30_to_89.len());
    print_count("Long    (90-180m)", r.long_90_to_180.len());
    print_count("Epic    (>180m)", r.epic_over_180.len());
    println!();

    // Histogram
    print_section("Duration distribution");
    let max_h = r
        .histogram
        .iter()
        .map(|b| b.count)
        .max()
        .unwrap_or(1)
        .max(1);
    for b in &r.histogram {
        let bar = bar_chart(b.count, max_h, 20);
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

    let picks = random_bucket_picks(r);
    if !picks.is_empty() {
        print_section("Random picks by bucket");
        println!();
        for (label, game) in picks {
            let name = truncate(&game.name, 48);
            println!(
                "  {LABEL}{:<18}{LABEL:#} {ACCENT}{:>4}m{ACCENT:#}  {}",
                label, game.value as u32, name
            );
        }
        println!();
    }

    if !r.sorted.is_empty() {
        println!("{MUTED}List games in this order: `bgg list --sort time`{MUTED:#}");
    }
}

fn random_bucket_picks(r: &TimeReport) -> Vec<(&'static str, &TopEntry)> {
    let seed = common::random_seed();
    [
        ("Quick (<30m)", r.quick_under_30.as_slice()),
        ("Medium (30-89m)", r.medium_30_to_89.as_slice()),
        ("Long (90-180m)", r.long_90_to_180.as_slice()),
        ("Epic (>180m)", r.epic_over_180.as_slice()),
    ]
    .into_iter()
    .filter_map(|(label, games)| {
        if games.len() <= 1 {
            return None;
        }
        let idx = common::random_index(seed, label, games.len())?;
        Some((label, &games[idx]))
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
