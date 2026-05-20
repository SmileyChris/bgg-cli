use crate::cache;
use crate::cli::StatsCommand;
use crate::config;
use crate::error::{Error, Result};
use crate::paths;
use crate::stats;
use anstream::println;

pub fn run(json: bool, section: Option<StatsCommand>) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;
    match section {
        None => {
            let report = stats::build(&cache);
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| Error::Parse(format!("json: {e}")))?;
                std::println!("{out}");
            } else {
                stats::print_text(&report);
            }
        }
        Some(StatsCommand::Plays { json: section_json }) => {
            stats::plays::run(&cache, json || section_json)?
        }
        Some(StatsCommand::Ratings { json: section_json }) => {
            stats::ratings::run(&cache, json || section_json)?
        }
        Some(StatsCommand::Year { json: section_json }) => {
            stats::year::run(&cache, json || section_json)?
        }
        Some(StatsCommand::Time { json: section_json }) => {
            stats::time::run(&cache, json || section_json)?
        }
        Some(StatsCommand::Players { json: section_json }) => {
            stats::players::run(&cache, json || section_json)?
        }
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
