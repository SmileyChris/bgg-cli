use crate::cache;
use crate::config;
use crate::error::{Error, Result};
use crate::list::{print_footer, render_table, resolve_columns, FilterSpec, SortSpec};
use crate::model::CollectionItem;
use crate::paths;
use std::io::IsTerminal;

pub fn run(
    filter_arg: String,
    sort_arg: String,
    cols_arg: Option<String>,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;

    if json {
        let items: Vec<&CollectionItem> = cache.items.values().collect();
        let out =
            serde_json::to_string_pretty(&items).map_err(|e| Error::Parse(format!("json: {e}")))?;
        std::println!("{out}");
        return Ok(());
    }

    let filter = FilterSpec::parse(&filter_arg)?;
    let sort = SortSpec::parse(&sort_arg)?;
    let cols = resolve_columns(cols_arg.as_deref(), sort.field)?;

    let mut items: Vec<&CollectionItem> =
        cache.items.values().filter(|i| filter.matches(i)).collect();
    items.sort_by(|a, b| sort.compare(a, b));

    let total = items.len();
    if let Some(n) = limit {
        items.truncate(n);
    }
    render_table(&items, &cols);
    if std::io::stdout().is_terminal() {
        print_footer(items.len(), total);
    }
    Ok(())
}
