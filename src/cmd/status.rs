use crate::cache;
use crate::config;
use crate::error::Result;
use crate::paths;
use crate::secrets;

pub fn run() -> Result<()> {
    let cfg = config::load()?;
    let Some(username) = cfg.username else {
        println!("No logged-in user. Run `bgg auth`.");
        return Ok(());
    };
    let auth_state = match secrets::load(&username) {
        Ok(creds) => match creds.session_fresh_until {
            Some(until) if until > chrono::Utc::now() => {
                format!(
                    "credentials stored; session fresh until {}",
                    until.to_rfc3339()
                )
            }
            Some(_) => "credentials stored; session expired, will refresh on next sync".to_string(),
            None => "credentials stored; expiry unknown, refresh on demand".to_string(),
        },
        Err(crate::error::Error::AuthRequired) => {
            "no credentials stored — run `bgg auth`".to_string()
        }
        Err(e) => {
            println!("User: {username}");
            println!("Auth: error ({e})");
            return Ok(());
        }
    };
    println!("User:  {username}");
    println!("Auth:  {auth_state}");

    let path = paths::cache_file(&username);
    match cache::load(&path, &username) {
        Ok(c) => {
            let last = c
                .last_sync
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "never".into());
            println!("Cache: {} items at {}", c.items.len(), path.display());
            println!("Last sync: {last}");
            println!("Note: incremental sync does not detect deletions. Use `bgg sync --full`.");
        }
        Err(crate::error::Error::NoCache(_)) => {
            println!("Cache: none yet. Run `bgg sync`.");
        }
        Err(e) => println!("Cache: error ({e})"),
    }
    Ok(())
}
