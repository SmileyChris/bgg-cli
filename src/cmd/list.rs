use crate::cache;
use crate::config;
use crate::error::Result;
use crate::model::CollectionItem;
use crate::paths;
use std::io::IsTerminal;

pub fn run(owned: bool, json: bool) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;
    let items: Vec<_> = cache
        .items
        .values()
        .filter(|i| !owned || i.status.own)
        .collect();

    if json {
        let out = serde_json::to_string_pretty(&items)
            .map_err(|e| crate::error::Error::Parse(format!("json: {e}")))?;
        println!("{out}");
        return Ok(());
    }

    let linkify = std::io::stdout().is_terminal();

    if owned {
        println!("{:<4}  {}", "YEAR", "NAME");
    } else {
        println!("{:<3}  {:<4}  {}", "OWN", "YEAR", "NAME");
    }
    for item in items {
        let year = item
            .year_published
            .map(|y| y.to_string())
            .unwrap_or_else(|| "-".into());
        let name = if linkify {
            hyperlink(&bgg_url(item), &item.name)
        } else {
            item.name.clone()
        };
        if owned {
            println!("{:<4}  {}", year, name);
        } else {
            let own = if item.status.own { "yes" } else { "no" };
            println!("{:<3}  {:<4}  {}", own, year, name);
        }
    }
    Ok(())
}

fn bgg_url(item: &CollectionItem) -> String {
    // `/boardgame/<id>` redirects to the canonical URL even for expansions.
    format!("https://boardgamegeek.com/boardgame/{}", item.id)
}

fn hyperlink(url: &str, text: &str) -> String {
    // OSC 8 hyperlink. Modern terminals render this; older ones show plain text.
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}
