use crate::auth as bgg_auth;
use crate::bgg::client::{HttpClient, ProgressFn};
use crate::bgg::collection;
use crate::cache;
use crate::cmd::auth::BGG_BASE;
use crate::config;
use crate::error::{Error, Result};
use crate::model::{CacheFile, CollectionItem, StoredCreds};
use crate::paths;
use crate::secrets;
use anstream::println;
use anstyle::{Effects, Style};
use chrono::{DateTime, Utc};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

const DIM: Style = Style::new().effects(Effects::DIMMED);

/// Subtypes we sync. BGG's collection endpoint only returns one subtype per
/// request, so a full picture needs one call per entry here.
const SUBTYPES: &[(&str, &str)] = &[
    ("boardgame", "base games"),
    ("boardgameexpansion", "expansions"),
];

pub fn run(full: bool) -> Result<()> {
    let username = config::require_username()?;
    let mut creds = secrets::load(&username)?;

    let cache_path = paths::cache_file(&username);
    let mut cache = match cache::load(&cache_path, &username) {
        Ok(c) => c,
        Err(Error::NoCache(_)) => CacheFile::empty(&username),
        Err(e) => return Err(e),
    };

    let modified_since = if full { None } else { cache.last_sync };
    let m = MultiProgress::new();

    let mut all_items: Vec<CollectionItem> = Vec::new();
    for (subtype, label) in SUBTYPES {
        let active = format!("Fetching {label}");
        let pb = add_step(&m, &format!("{active}…"));
        let items =
            fetch_with_refresh(&username, &mut creds, modified_since, subtype, &active, &pb)?;
        all_items.extend(items);
        pb.finish_with_message(format!("Fetched {label}"));
    }

    let pb = add_step(&m, "Merging into local cache…");
    let report = cache::merge(&mut cache, all_items, full);
    cache::save(&cache_path, &cache)?;
    pb.finish_with_message("Updated local cache");

    let total = cache.items.len();
    let processed = report.new + report.updated + report.unchanged;
    if processed == 0 && report.removed == 0 {
        println!("No changes since last sync. Total: {total} items.");
    } else if full {
        println!(
            "Full sync: {} new, {} updated, {} unchanged, {} removed. Total: {total}.",
            report.new, report.updated, report.unchanged, report.removed,
        );
    } else {
        println!(
            "{processed} items processed ({} new, {} updated, {} unchanged). Total: {total}.",
            report.new, report.updated, report.unchanged,
        );
    }
    if !full {
        println!(
            "{DIM}Tip: incremental sync cannot detect deletions. Run `bgg sync --full` periodically.{DIM:#}"
        );
    }
    Ok(())
}

fn step_style() -> ProgressStyle {
    // Last tick char is what indicatif renders after `finish_with_message`,
    // so we end with a check mark to mark completed steps.
    ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏✓")
        .template("{spinner:.green} {msg}")
        .unwrap()
}

fn add_step(m: &MultiProgress, message: &str) -> ProgressBar {
    let pb = m.add(ProgressBar::new_spinner());
    pb.set_style(step_style());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_message(message.to_string());
    pb
}

fn build_client(creds: &StoredCreds, pb: &ProgressBar, prefix: &str) -> Result<HttpClient> {
    let pb_for_cb = pb.clone();
    let prefix = prefix.to_string();
    let cb: Box<ProgressFn> = Box::new(move |msg| {
        pb_for_cb.set_message(format!("{prefix} — {msg}"));
    });
    HttpClient::new(Some(creds.cookies.clone())).map(|c| c.with_progress(cb))
}

/// Fetch one subtype, refreshing the cookie via stored password when we know
/// the SessionID is stale (proactive) or when the server tells us so via 401
/// (reactive safety net). Updated cookies are persisted back to the keyring.
fn fetch_with_refresh(
    username: &str,
    creds: &mut StoredCreds,
    modified_since: Option<DateTime<Utc>>,
    subtype: &str,
    prefix: &str,
    pb: &ProgressBar,
) -> Result<Vec<CollectionItem>> {
    let stale = creds
        .session_fresh_until
        .map(|t| Utc::now() >= t)
        .unwrap_or(false);
    if stale {
        pb.set_message(format!("{prefix} — refreshing BGG session…"));
        refresh(username, creds)?;
        pb.set_message(format!("{prefix}…"));
    }
    let client = build_client(creds, pb, prefix)?;
    match collection::fetch(&client, username, modified_since, Some(subtype)) {
        Ok(items) => Ok(items),
        Err(Error::AuthRequired) => {
            pb.set_message(format!("{prefix} — session expired, refreshing…"));
            refresh(username, creds)?;
            pb.set_message(format!("{prefix}…"));
            let client = build_client(creds, pb, prefix)?;
            collection::fetch(&client, username, modified_since, Some(subtype))
        }
        Err(e) => Err(e),
    }
}

fn refresh(username: &str, creds: &mut StoredCreds) -> Result<()> {
    let r = match bgg_auth::login(BGG_BASE, username, &creds.password) {
        Ok(r) => r,
        Err(Error::BadCredentials) => {
            return Err(Error::Secrets(
                "stored password no longer works (changed on BGG?). Run `bgg auth` to re-enter."
                    .into(),
            ));
        }
        Err(e) => return Err(e),
    };
    creds.cookies = r.cookies;
    creds.session_fresh_until = r.session_fresh_until;
    secrets::store(username, creds)
}
