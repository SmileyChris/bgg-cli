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
use chrono::{DateTime, Utc};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

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
    let pb = make_spinner();

    let mut all_items: Vec<CollectionItem> = Vec::new();
    for (subtype, label) in SUBTYPES {
        pb.set_message(format!("Fetching {label}…"));
        let items = fetch_with_refresh(&username, &mut creds, modified_since, subtype, &pb)?;
        all_items.extend(items);
    }

    pb.set_message("Merging into local cache…");
    let report = cache::merge(&mut cache, all_items, full);
    cache::save(&cache_path, &cache)?;
    pb.finish_and_clear();

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
            "Tip: incremental sync cannot detect deletions. Run `bgg sync --full` periodically."
        );
    }
    Ok(())
}

fn make_spinner() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

fn build_client(creds: &StoredCreds, pb: &ProgressBar) -> Result<HttpClient> {
    let pb_for_cb = pb.clone();
    let cb: Box<ProgressFn> = Box::new(move |msg| pb_for_cb.set_message(msg.to_string()));
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
    pb: &ProgressBar,
) -> Result<Vec<CollectionItem>> {
    let stale = creds
        .session_fresh_until
        .map(|t| Utc::now() >= t)
        .unwrap_or(false);
    if stale {
        pb.set_message("Refreshing BGG session…");
        refresh(username, creds)?;
    }
    let client = build_client(creds, pb)?;
    match collection::fetch(&client, username, modified_since, Some(subtype)) {
        Ok(items) => Ok(items),
        Err(Error::AuthRequired) => {
            pb.set_message("Session expired, refreshing…");
            refresh(username, creds)?;
            let client = build_client(creds, pb)?;
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
