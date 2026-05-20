use crate::error::Result;
use crate::model::CacheFile;
use crate::stats::common;
use crate::stats::owned::TopEntry;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PlayersReport {
    pub(crate) solo_capable: usize,
    pub(crate) two_capable: usize,
    pub(crate) common_range: Option<(u32, u32)>,
    pub(crate) by_count: Vec<PlayerCountRow>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PlayerCountRow {
    pub(crate) players: u32,
    pub(crate) count: usize,
    pub(crate) best: Option<TopEntry>,
    pub(crate) only_at_count: Vec<String>,
}

pub(crate) fn build(cache: &CacheFile) -> PlayersReport {
    let owned = common::owned_boardgames_from_cache(cache);

    let mut solo = 0;
    let mut two = 0;
    let mut ranges: std::collections::BTreeMap<(u32, u32), usize> =
        std::collections::BTreeMap::new();

    // Build per-count data for 1..=12
    let by_count: Vec<PlayerCountRow> = (1..=12)
        .map(|players| {
            let mut supporting = Vec::new();
            let mut only = Vec::new();

            for i in owned.iter().copied() {
                let Some(stats) = i.stats.as_ref() else {
                    continue;
                };
                let Some((min, max)) = common::player_range(stats) else {
                    continue;
                };
                if stats.supports_player_count(players) {
                    supporting.push(i);
                    if min == players && max == players {
                        only.push(i);
                    }
                }
            }

            // Count solo/two capable as we go
            // (deduplicated: we count games that support 1 or 2 at least once)

            // Best game at this count: highest BGG average (or your rating if no BGG avg)
            let best = supporting
                .iter()
                .filter_map(|i| {
                    let s = i.stats.as_ref()?;
                    let score = s.bayes_average.or(s.average).or(s.user_rating)?;
                    Some((i, score))
                })
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, score)| TopEntry {
                    id: i.id,
                    name: i.name.clone(),
                    value: score as f64,
                });

            let only_names: Vec<String> = only.iter().map(|i| i.name.clone()).collect();

            PlayerCountRow {
                players,
                count: supporting.len(),
                best,
                only_at_count: only_names,
            }
        })
        .collect();

    // Solo/two capable
    for (i, (min, max)) in common::player_ranges(owned.iter().copied()) {
        if i.stats
            .as_ref()
            .is_some_and(|stats| stats.supports_player_count(1))
        {
            solo += 1;
        }
        if i.stats
            .as_ref()
            .is_some_and(|stats| stats.supports_player_count(2))
        {
            two += 1;
        }
        *ranges.entry((min, max)).or_insert(0) += 1;
    }

    let common_range = ranges
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(range, _)| range);

    PlayersReport {
        solo_capable: solo,
        two_capable: two,
        common_range,
        by_count,
    }
}

pub(crate) fn print_text(r: &PlayersReport) {
    use crate::stats::render::{
        bar_chart, print_count, print_line, print_section, ACCENT, LABEL, MUTED,
    };
    use anstream::println;

    println!("{ACCENT}━━━ Player counts deep dive ━━━{ACCENT:#}");
    println!();

    print_count("Solo capable", r.solo_capable);
    print_count("Plays at 2", r.two_capable);
    let range = r
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
    println!();

    // Per-count matrix
    print_section("Games supporting each player count");
    println!(
        "  {LABEL}{:>8} {:>5} {:<20} {}{LABEL:#}",
        "Players", "Count", "Best game", "Rating"
    );
    let max_count = r
        .by_count
        .iter()
        .map(|row| row.count)
        .max()
        .unwrap_or(1)
        .max(1);
    for row in &r.by_count {
        let bar = bar_chart(row.count, max_count, 16);
        let best_name = row
            .best
            .as_ref()
            .map(|b| crate::stats::render::truncate(&b.name, 20))
            .unwrap_or_else(|| "-".into());
        let best_rating = row
            .best
            .as_ref()
            .map(|b| format!("{ACCENT}{:.1}{ACCENT:#}", b.value))
            .unwrap_or_else(|| format!("{MUTED}-{MUTED:#}"));
        println!(
            "  {:>4}   {:>4}  {ACCENT}{}{ACCENT:#}  {:<22} {}",
            row.players, row.count, bar, best_name, best_rating,
        );
    }
    println!();

    // Games only-at-N
    let exclusives: Vec<&PlayerCountRow> = r
        .by_count
        .iter()
        .filter(|row| !row.only_at_count.is_empty())
        .collect();
    if !exclusives.is_empty() {
        print_section("Games that only support a single player count");
        println!();
        for row in &exclusives {
            for name in &row.only_at_count {
                println!("  {LABEL}{}p{LABEL:#}  {name}", row.players);
            }
        }
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
