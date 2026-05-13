use crate::error::{Error, Result};
use crate::model::StoredCreds;

const SERVICE: &str = "bgg-cli";

fn entry(username: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, username).map_err(|e| Error::Secrets(e.to_string()))
}

pub fn store(username: &str, creds: &StoredCreds) -> Result<()> {
    let blob =
        serde_json::to_string(creds).map_err(|e| Error::Secrets(format!("serialize: {e}")))?;
    entry(username)?
        .set_password(&blob)
        .map_err(|e| Error::Secrets(e.to_string()))
}

pub fn load(username: &str) -> Result<StoredCreds> {
    let blob = match entry(username)?.get_password() {
        Ok(s) => s,
        Err(keyring::Error::NoEntry) => return Err(Error::AuthRequired),
        Err(e) => return Err(Error::Secrets(e.to_string())),
    };
    serde_json::from_str(&blob).map_err(|_| {
        Error::Secrets("stored credentials are in an old format; run `bgg auth` to re-login".into())
    })
}

pub fn delete(username: &str) -> Result<()> {
    match entry(username)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Secrets(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_for_unknown_user_returns_auth_required_or_secrets_error() {
        let u = format!(
            "bgg-cli-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        match load(&u) {
            Err(Error::AuthRequired) => {}
            Err(Error::Secrets(_)) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }
}
