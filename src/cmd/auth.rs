use crate::auth as bgg_auth;
use crate::config::{self, Config};
use crate::error::{Error, Result};
use crate::model::StoredCreds;
use crate::secrets;

pub const BGG_BASE: &str = "https://boardgamegeek.com";

pub fn run(username_arg: Option<String>, clear: bool) -> Result<()> {
    if clear {
        return clear_cookies();
    }
    login(username_arg)
}

fn login(username_arg: Option<String>) -> Result<()> {
    let cfg = config::load()?;
    let username = username_arg
        .or(cfg.username.clone())
        .unwrap_or_else(prompt_username);
    let password = rpassword::prompt_password(format!("Password for BGG user {username}: "))
        .map_err(|e| Error::Secrets(format!("password prompt: {e}")))?;
    let r = bgg_auth::login(BGG_BASE, &username, &password)?;
    secrets::store(
        &username,
        &StoredCreds {
            password,
            cookies: r.cookies,
            session_fresh_until: r.session_fresh_until,
        },
    )?;
    config::save(&Config {
        username: Some(username.clone()),
    })?;
    println!("Authenticated as {username}. Credentials stored in OS keyring.");
    Ok(())
}

fn clear_cookies() -> Result<()> {
    let cfg = config::load()?;
    let Some(username) = cfg.username else {
        println!("No stored auth.");
        return Ok(());
    };
    secrets::delete(&username)?;
    println!("Cleared stored cookies for {username}. (Config and cache retained.)");
    Ok(())
}

fn prompt_username() -> String {
    use std::io::{self, BufRead, Write};
    print!("BGG username: ");
    let _ = io::stdout().flush();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).expect("stdin");
    s.trim().to_string()
}
