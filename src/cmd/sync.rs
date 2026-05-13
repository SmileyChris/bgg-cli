use crate::auth as bgg_auth;
use crate::bgg::client::HttpClient;
use crate::bgg::collection;
use crate::cache;
use crate::cmd::auth::BGG_BASE;
use crate::config;
use crate::error::{Error, Result};
use crate::model::{CacheFile, CollectionItem, StoredCreds};
use crate::paths;
use crate::secrets;
use chrono::{DateTime, Utc};

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
    let items = fetch_with_refresh(&username, &mut creds, modified_since)?;
    let report = cache::merge(&mut cache, items);
    cache::save(&cache_path, &cache)?;

    let total = cache.items.len();
    let processed = report.new + report.updated + report.unchanged;
    if processed == 0 {
        println!("No changes since last sync. Total: {total} items.");
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

/// Fetch the collection, refreshing the cookie via stored password when we
/// know the SessionID is stale (proactive) or when the server tells us so via
/// 401 (reactive safety net for server-side invalidation). Updated cookies are
/// persisted back to the keyring.
fn fetch_with_refresh(
    username: &str,
    creds: &mut StoredCreds,
    modified_since: Option<DateTime<Utc>>,
) -> Result<Vec<CollectionItem>> {
    let stale = creds
        .session_fresh_until
        .map(|t| Utc::now() >= t)
        .unwrap_or(false);
    if stale {
        tracing::info!("BGG session cookie past its declared expiry; refreshing");
        refresh(username, creds)?;
    }
    let client = HttpClient::new(Some(creds.cookies.clone()))?;
    match collection::fetch(&client, username, modified_since) {
        Ok(items) => Ok(items),
        Err(Error::AuthRequired) => {
            tracing::info!("BGG returned 401 despite fresh cookie; refreshing");
            refresh(username, creds)?;
            let client = HttpClient::new(Some(creds.cookies.clone()))?;
            collection::fetch(&client, username, modified_since)
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
