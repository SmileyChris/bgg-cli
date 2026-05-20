use crate::error::Result;
use crate::model::{CacheFile, CollectionItem};
use crate::stats::common;
use crate::stats::owned::TopEntry;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PlaysReport {
    pub(crate) total: u64,
    pub(crate) played_count: usize,
    pub(crate) unplayed_count: usize,
    pub(crate) avg_per_owned: f32,
    pub(crate) h_index: usize,
    pub(crate) dimes: usize,
    pub(crate) nickels: usize,
    pub(crate) quarters: usize,
    pub(crate) histogram: Vec<HistogramBucket>,
    pub(crate) top_plays: Vec<TopEntry>,
    pub(crate) recent_plays: Vec<RecentEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecentEntry {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) plays: u32,
    pub(crate) last_modified: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HistogramBucket {
    pub(crate) label: String,
    pub(crate) count: usize,
}

pub(crate) fn build(cache: &CacheFile) -> PlaysReport {
    let owned = common::owned_boardgames_from_cache(cache);

    let total: u64 = owned.iter().map(|i| i.num_plays as u64).sum();
    let played: Vec<&CollectionItem> = owned.iter().copied().filter(|i| i.num_plays > 0).collect();
    let played_count = played.len();
    let unplayed_count = owned.len() - played_count;
    let avg = if owned.is_empty() {
        0.0
    } else {
        total as f32 / owned.len() as f32
    };

    // H-index: h games played at least h times
    let mut play_counts: Vec<u32> = played.iter().map(|i| i.num_plays).collect();
    play_counts.sort_unstable_by(|a, b| b.cmp(a));
    let h_index = play_counts
        .iter()
        .enumerate()
        .take_while(|(i, &c)| c as usize > *i)
        .count();

    let dimes = played.iter().filter(|i| i.num_plays >= 10).count();
    let nickels = played.iter().filter(|i| i.num_plays >= 5).count();
    let quarters = played.iter().filter(|i| i.num_plays >= 25).count();

    // Histogram buckets
    let buckets: &[(u32, u32, &str)] = &[
        (1, 1, "1"),
        (2, 5, "2-5"),
        (6, 10, "6-10"),
        (11, 20, "11-20"),
        (21, 50, "21-50"),
        (51, 100, "51-100"),
        (101, u32::MAX, "100+"),
    ];
    let histogram: Vec<HistogramBucket> = buckets
        .iter()
        .map(|&(lo, hi, label)| {
            let count = played
                .iter()
                .filter(|i| i.num_plays >= lo && i.num_plays <= hi)
                .count();
            HistogramBucket {
                label: label.to_string(),
                count,
            }
        })
        .collect();

    // Top 20 by play count
    let mut top = played.clone();
    top.sort_by(|a, b| b.num_plays.cmp(&a.num_plays).then(a.name.cmp(&b.name)));
    let top_plays: Vec<TopEntry> = top
        .iter()
        .take(20)
        .map(|i| TopEntry {
            id: i.id,
            name: i.name.clone(),
            value: i.num_plays as f64,
        })
        .collect();

    // Recent 20 — played games with a last_modified date, sorted newest first
    let mut recent: Vec<&&CollectionItem> = owned
        .iter()
        .filter(|i| i.num_plays > 0 && i.status.last_modified.is_some())
        .collect();
    recent.sort_by(|a, b| {
        b.status
            .last_modified
            .cmp(&a.status.last_modified)
            .then(a.name.cmp(&b.name))
    });
    let recent_plays: Vec<RecentEntry> = recent
        .iter()
        .take(20)
        .map(|i| RecentEntry {
            id: i.id,
            name: i.name.clone(),
            plays: i.num_plays,
            last_modified: i.status.last_modified.unwrap(),
        })
        .collect();

    PlaysReport {
        total,
        played_count,
        unplayed_count,
        avg_per_owned: avg,
        h_index,
        dimes,
        nickels,
        quarters,
        histogram,
        top_plays,
        recent_plays,
    }
}

pub(crate) fn print_text(r: &PlaysReport) {
    use crate::stats::render::{
        bar_chart, print_count, print_line, print_section, ACCENT, LABEL, MUTED, STRONG,
    };
    use anstream::println;

    println!("{ACCENT}━━━ Plays deep dive ━━━{ACCENT:#}");
    println!();

    print_count("Total plays", r.total);
    print_line(
        "Played / unplayed",
        format_args!(
            "{STRONG}{}{STRONG:#}  /  {STRONG}{}{STRONG:#}",
            r.played_count, r.unplayed_count
        ),
    );
    print_line(
        "Average plays/owned",
        format_args!("{STRONG}{:.1}{STRONG:#}", r.avg_per_owned),
    );
    println!();

    // Highlights
    print_section("Highlights");
    print_line(
        "H-index",
        format_args!(
            "{ACCENT}{}{ACCENT:#}  {MUTED}({} played at least {} times){MUTED:#}",
            r.h_index, r.h_index, r.h_index
        ),
    );
    print_count("Nickels (5+ plays)", r.nickels);
    print_count("Dimes (10+ plays)", r.dimes);
    if r.quarters > 0 {
        print_count("Quarters (25+ plays)", r.quarters);
    }
    println!();

    // Histogram
    print_section("Play-count distribution");
    let max = r
        .histogram
        .iter()
        .map(|b| b.count)
        .max()
        .unwrap_or(1)
        .max(1);
    for b in &r.histogram {
        let bar = bar_chart(b.count, max, 24);
        println!(
            "  {LABEL}{:>6}{LABEL:#} {ACCENT}{}{ACCENT:#} {}{}{}",
            b.label,
            bar,
            STRONG.render(),
            b.count,
            STRONG.render_reset()
        );
    }
    println!();

    // Top 20 plays
    if !r.top_plays.is_empty() {
        print_section("Top 20 by plays");
        println!();
        for (idx, t) in r.top_plays.iter().enumerate() {
            let name = truncate(&t.name, 48);
            println!(
                "  {MUTED}{:>3}.{MUTED:#} {:<50} {ACCENT}{}{ACCENT:#}",
                idx + 1,
                name,
                t.value as u64
            );
        }
    }

    // Recent 20 plays
    if !r.recent_plays.is_empty() {
        println!();
        print_section("Recent 20 (by last modified)");
        println!();
        for (idx, e) in r.recent_plays.iter().enumerate() {
            let name = truncate(&e.name, 42);
            let date = e.last_modified.format("%Y-%m-%d");
            println!(
                "  {MUTED}{:>3}.{MUTED:#} {:<44} {ACCENT}{:>3}{ACCENT:#}  {MUTED}{}{MUTED:#}",
                idx + 1,
                name,
                e.plays,
                date
            );
        }
    }
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
