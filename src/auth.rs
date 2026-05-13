use crate::error::{Error, Result};
use crate::model::Cookies;
use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::Client;
use serde::Serialize;

const LOGIN_PATH: &str = "/login/api/v1";

/// Refresh slightly before BGG's stated expiry to avoid clock-skew false-OKs.
const EXPIRY_SAFETY_MARGIN: Duration = Duration::seconds(30);

#[derive(Serialize)]
struct LoginBody<'a> {
    credentials: Credentials<'a>,
}

#[derive(Serialize)]
struct Credentials<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Debug)]
pub struct LoginResult {
    pub cookies: Cookies,
    /// Best-effort upper bound on when the SessionID is still good. Derived
    /// from `Set-Cookie: SessionID=...; Max-Age=N` minus a small safety margin.
    pub session_fresh_until: Option<DateTime<Utc>>,
}

pub fn login(base: &str, username: &str, password: &str) -> Result<LoginResult> {
    let client = Client::builder()
        .user_agent(concat!("bgg-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(Error::Network)?;
    let resp = client
        .post(format!("{base}{LOGIN_PATH}"))
        .json(&LoginBody {
            credentials: Credentials { username, password },
        })
        .send()
        .map_err(Error::Network)?;

    if !resp.status().is_success() {
        return Err(Error::BadCredentials);
    }
    extract(resp.headers())
}

fn extract(headers: &reqwest::header::HeaderMap) -> Result<LoginResult> {
    let mut bggusername = None;
    let mut bggpassword = None;
    let mut session_id = None;
    let mut session_max_age_secs: Option<i64> = None;

    for v in headers.get_all(reqwest::header::SET_COOKIE) {
        let s = v.to_str().unwrap_or("");
        let mut parts = s.split(';');
        let Some(pair) = parts.next() else { continue };
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = value.to_string();
        match name {
            "bggusername" => bggusername = Some(value),
            "bggpassword" => bggpassword = Some(value),
            "SessionID" => {
                session_id = Some(value);
                for attr in parts {
                    let attr = attr.trim();
                    if let Some(rest) = attr
                        .strip_prefix("Max-Age=")
                        .or_else(|| attr.strip_prefix("max-age="))
                    {
                        session_max_age_secs = rest.parse().ok();
                    }
                }
            }
            _ => {}
        }
    }

    let cookies = match (bggusername, bggpassword, session_id) {
        (Some(u), Some(p), Some(s)) => Cookies {
            bggusername: u,
            bggpassword: p,
            session_id: s,
        },
        _ => {
            return Err(Error::Secrets(
                "login response missing expected cookies".into(),
            ));
        }
    };
    let session_fresh_until = session_max_age_secs
        .map(|secs| Utc::now() + Duration::seconds(secs) - EXPIRY_SAFETY_MARGIN);
    Ok(LoginResult {
        cookies,
        session_fresh_until,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn extracts_three_cookies_and_session_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/api/v1"))
            .and(body_json(serde_json::json!({
                "credentials": {"username": "alice", "password": "pw"}
            })))
            .respond_with(
                ResponseTemplate::new(204)
                    .append_header(
                        "set-cookie",
                        "bggusername=alice; Path=/; Domain=.boardgamegeek.com",
                    )
                    .append_header("set-cookie", "bggpassword=cookiepw; Path=/")
                    .append_header(
                        "set-cookie",
                        "SessionID=abc123; Max-Age=3600; Path=/; secure; HttpOnly",
                    )
                    .append_header("set-cookie", "other=ignored; Path=/"),
            )
            .mount(&server)
            .await;

        let url = server.uri();
        let before = Utc::now();
        let r = tokio::task::spawn_blocking(move || login(&url, "alice", "pw"))
            .await
            .unwrap()
            .unwrap();
        let after = Utc::now();
        assert_eq!(r.cookies.bggusername, "alice");
        assert_eq!(r.cookies.bggpassword, "cookiepw");
        assert_eq!(r.cookies.session_id, "abc123");
        let exp = r.session_fresh_until.expect("expiry parsed");
        // Should be ~1h from now, minus safety margin.
        assert!(exp >= before + Duration::seconds(3600 - 60));
        assert!(exp <= after + Duration::seconds(3600));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_max_age_yields_no_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/api/v1"))
            .respond_with(
                ResponseTemplate::new(204)
                    .append_header("set-cookie", "bggusername=alice; Path=/")
                    .append_header("set-cookie", "bggpassword=pw; Path=/")
                    .append_header("set-cookie", "SessionID=sid; Path=/"),
            )
            .mount(&server)
            .await;

        let url = server.uri();
        let r = tokio::task::spawn_blocking(move || login(&url, "alice", "pw"))
            .await
            .unwrap()
            .unwrap();
        assert!(r.session_fresh_until.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bad_credentials_returns_bad_credentials_for_400() {
        // BGG returns 400 for invalid username/password.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/api/v1"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;

        let url = server.uri();
        let err = tokio::task::spawn_blocking(move || login(&url, "alice", "wrong"))
            .await
            .unwrap()
            .unwrap_err();
        assert!(matches!(err, Error::BadCredentials));
    }
}
