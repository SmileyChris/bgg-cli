use crate::cache;
use crate::cli::ListSort;
use crate::config;
use crate::error::Result;
use crate::model::CollectionItem;
use crate::paths;
use std::cmp::Ordering;
use std::io::IsTerminal;

pub fn run(owned: bool, json: bool, sort: ListSort) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;
    let mut items: Vec<&CollectionItem> = cache
        .items
        .values()
        .filter(|i| !owned || i.status.own)
        .collect();
    items.sort_by(|a, b| compare(a, b, sort));

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

fn compare(a: &CollectionItem, b: &CollectionItem, sort: ListSort) -> Ordering {
    match sort {
        ListSort::Name => name_cmp(a, b),
        ListSort::Year => year_cmp(a, b).then_with(|| name_cmp(a, b)),
        ListSort::Bggid => a.id.cmp(&b.id).then_with(|| name_cmp(a, b)),
    }
}

fn name_cmp(a: &CollectionItem, b: &CollectionItem) -> Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

fn year_cmp(a: &CollectionItem, b: &CollectionItem) -> Ordering {
    match (a.year_published, b.year_published) {
        (Some(ay), Some(by)) => ay.cmp(&by),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn bgg_url(item: &CollectionItem) -> String {
    format!("https://boardgamegeek.com/boardgame/{}", item.id)
}

fn hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}
