use crate::cache;
use crate::config;
use crate::error::Result;
use crate::paths;

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

    println!("{:>7}  {:<6}  {:<4}  {}", "BGG ID", "OWN", "YEAR", "NAME");
    for item in items {
        let year = item.year_published.map(|y| y.to_string()).unwrap_or_else(|| "-".into());
        let own = if item.status.own { "yes" } else { "no" };
        println!("{:>7}  {:<6}  {:<4}  {}", item.id, own, year, item.name);
    }
    Ok(())
}
