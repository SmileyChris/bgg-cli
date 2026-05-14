use crate::cache;
use crate::config;
use crate::error::{Error, Result};
use crate::paths;
use crate::stats;
use anstream::println;

pub fn run(json: bool) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;
    let report = stats::build(&cache);
    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Parse(format!("json: {e}")))?;
        std::println!("{out}");
    } else {
        stats::print_text(&report);
    }
    Ok(())
}

/// One-screen summary printed when `bgg` is run with no subcommand.
pub fn run_summary() -> Result<()> {
    let Some(username) = config::load()?.username else {
        println!("No logged-in user. Run `bgg auth`.");
        return Ok(());
    };
    let cache = match cache::load(&paths::cache_file(&username), &username) {
        Ok(c) => c,
        Err(Error::NoCache(_)) => {
            println!("Logged in as {username}. Run `bgg sync` to fetch your collection.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    stats::print_summary(&stats::build(&cache));
    Ok(())
}
