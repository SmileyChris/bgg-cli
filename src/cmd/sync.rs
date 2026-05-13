use crate::bgg::client::HttpClient;
use crate::bgg::collection;
use crate::cache;
use crate::config;
use crate::error::Result;
use crate::model::CacheFile;
use crate::paths;
use crate::secrets;

pub fn run(full: bool) -> Result<()> {
    let username = config::require_username()?;
    let cookies = secrets::load(&username)?;
    let client = HttpClient::new(Some(cookies))?;

    let cache_path = paths::cache_file(&username);
    let mut cache = match cache::load(&cache_path, &username) {
        Ok(c) => c,
        Err(crate::error::Error::NoCache(_)) => CacheFile::empty(&username),
        Err(e) => return Err(e),
    };

    let modified_since = if full { None } else { cache.last_sync };
    let items = collection::fetch(&client, &username, modified_since)?;
    let report = cache::merge(&mut cache, items);
    cache::save(&cache_path, &cache)?;

    let total = cache.items.len();
    println!(
        "Synced {} items into cache ({} new, {} updated, {} unchanged). Total: {total}.",
        report.new + report.updated + report.unchanged,
        report.new,
        report.updated,
        report.unchanged,
    );
    if !full {
        println!("Tip: incremental sync cannot detect deletions. Run `bgg sync --full` periodically.");
    }
    Ok(())
}
