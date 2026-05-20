use crate::error::Result;
use crate::model::CacheFile;
use crate::stats::common;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct RatingsReport {
    pub(crate) rated_count: usize,
    pub(crate) total_owned: usize,
    pub(crate) your_average: Option<f32>,
    pub(crate) bgg_average: Option<f32>,
    pub(crate) your_distribution: [usize; 10],
    pub(crate) bgg_distribution: [usize; 10],
    pub(crate) ranked: Vec<RatedEntry>,
    pub(crate) biggest_deltas: Vec<DeltaEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RatedEntry {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) your_rating: f32,
    pub(crate) bgg_average: Option<f32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeltaEntry {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) your_rating: f32,
    pub(crate) bgg_average: f32,
    pub(crate) delta: f32,
    /// Positive = you rated higher than BGG.
    pub(crate) delta_sign: String,
}

pub(crate) fn build(cache: &CacheFile) -> RatingsReport {
    let owned = common::owned_boardgames_from_cache(cache);

    let rated = common::user_ratings(owned.iter().copied());

    let your_average = if rated.is_empty() {
        None
    } else {
        Some(rated.iter().map(|(_, r)| *r).sum::<f32>() / rated.len() as f32)
    };

    let bgg_vals = common::bgg_averages(owned.iter().copied());
    let bgg_average = if bgg_vals.is_empty() {
        None
    } else {
        Some(bgg_vals.iter().map(|(_, r)| *r).sum::<f32>() / bgg_vals.len() as f32)
    };

    let mut your_distribution = [0usize; 10];
    for (_, r) in &rated {
        if let Some(b) = common::bucket_1_to_10(*r) {
            your_distribution[b] += 1;
        }
    }
    let mut bgg_distribution = [0usize; 10];
    for (_, v) in &bgg_vals {
        if let Some(b) = common::bucket_1_to_10(*v) {
            bgg_distribution[b] += 1;
        }
    }

    // Full ranked list by your rating
    let mut ranked = rated.clone();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.name.cmp(&b.0.name))
    });
    let ranked: Vec<RatedEntry> = ranked
        .iter()
        .map(|(i, ur)| RatedEntry {
            id: i.id,
            name: i.name.clone(),
            your_rating: *ur,
            bgg_average: i.stats.as_ref().and_then(|s| s.average),
        })
        .collect();

    // Biggest deltas (absolute difference between your rating and BGG)
    let mut deltas: Vec<_> = common::user_and_bgg_ratings(owned.iter().copied())
        .into_iter()
        .map(|(i, ur, ba)| (i, ur, ba, (ur - ba).abs()))
        .collect();
    deltas.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    let biggest_deltas: Vec<DeltaEntry> = deltas
        .iter()
        .take(10)
        .map(|(i, ur, ba, _)| {
            let d = ur - ba;
            DeltaEntry {
                id: i.id,
                name: i.name.clone(),
                your_rating: *ur,
                bgg_average: *ba,
                delta: d.abs(),
                delta_sign: if d > 0.0 {
                    "▲".into()
                } else if d < 0.0 {
                    "▼".into()
                } else {
                    "─".into()
                },
            }
        })
        .collect();

    RatingsReport {
        rated_count: rated.len(),
        total_owned: owned.len(),
        your_average,
        bgg_average,
        your_distribution,
        bgg_distribution,
        ranked,
        biggest_deltas,
    }
}

pub(crate) fn print_text(r: &RatingsReport) {
    use crate::stats::render::{
        bar_chart, fmt_avg, inline_spark, print_line, print_section, ACCENT, LABEL, MUTED, STRONG,
    };
    use anstream::println;

    println!("{ACCENT}━━━ Ratings deep dive ━━━{ACCENT:#}");
    println!();

    print_line(
        "Rated",
        format_args!(
            "{STRONG}{}{STRONG:#} {MUTED}/ {}{MUTED:#}",
            r.rated_count, r.total_owned
        ),
    );
    print_line(
        "Your average",
        format_args!(
            "{}{}",
            fmt_avg(r.your_average),
            inline_spark(&r.your_distribution),
        ),
    );
    print_line(
        "BGG average",
        format_args!(
            "{}{}",
            fmt_avg(r.bgg_average),
            inline_spark(&r.bgg_distribution),
        ),
    );
    println!();

    // Side-by-side distribution bar chart
    print_section("Distribution");
    let max = r
        .your_distribution
        .iter()
        .chain(r.bgg_distribution.iter())
        .max()
        .copied()
        .unwrap_or(1)
        .max(1);
    println!("  {MUTED}{:>11} {:>24} {:>24}{MUTED:#}", "", "You", "BGG");
    for i in 0..10 {
        let label = i + 1;
        let your = r.your_distribution[i];
        let bgg = r.bgg_distribution[i];
        let ybar = bar_chart(your, max, 12);
        let bbar = bar_chart(bgg, max, 12);
        println!(
            "  {LABEL}{:>8}{LABEL:#}  {ACCENT}{}{ACCENT:#} {:>3}  {LABEL}{}{LABEL:#} {:>3}",
            label, ybar, your, bbar, bgg,
        );
    }
    println!();

    // Biggest deltas
    if !r.biggest_deltas.is_empty() {
        print_section("Biggest rating deltas (you vs BGG)");
        println!();
        for d in &r.biggest_deltas {
            let sign = d.delta_sign.as_str();
            let name = crate::stats::render::truncate(&d.name, 42);
            println!(
                "  {:<44} {ACCENT}{}{ACCENT:#} {MUTED}{:.1}{MUTED:#}  {sign} {:.1}",
                name, d.your_rating, d.bgg_average, d.delta,
            );
        }
        println!();
    }

    // Full ranked list
    if !r.ranked.is_empty() {
        print_section("Ranked by your rating");
        println!();
        for (idx, g) in r.ranked.iter().enumerate() {
            let name = crate::stats::render::truncate(&g.name, 44);
            let bgg = match g.bgg_average {
                Some(v) => format!("{MUTED}BGG {v:.1}{MUTED:#}"),
                None => String::new(),
            };
            println!(
                "  {MUTED}{:>3}.{MUTED:#} {:<46} {ACCENT}{:.1}{ACCENT:#}  {}",
                idx + 1,
                name,
                g.your_rating,
                bgg,
            );
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
