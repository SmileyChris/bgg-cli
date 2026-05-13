use crate::error::{Error, Result};
use crate::model::Cookies;
use reqwest::blocking::Client;
use serde::Serialize;

const LOGIN_PATH: &str = "/login/api/v1";

#[derive(Serialize)]
struct LoginBody<'a> {
    credentials: Credentials<'a>,
}

#[derive(Serialize)]
struct Credentials<'a> {
    username: &'a str,
    password: &'a str,
}

pub fn login(base: &str, username: &str, password: &str) -> Result<Cookies> {
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
        return Err(Error::AuthRequired);
    }
    extract_cookies(resp.headers())
}

fn extract_cookies(headers: &reqwest::header::HeaderMap) -> Result<Cookies> {
    let mut bggusername = None;
    let mut bggpassword = None;
    let mut session_id = None;
    for v in headers.get_all(reqwest::header::SET_COOKIE) {
        let s = v.to_str().unwrap_or("");
        let (pair, _) = s.split_once(';').unwrap_or((s, ""));
        if let Some((name, value)) = pair.split_once('=') {
            match name.trim() {
                "bggusername" => bggusername = Some(value.to_string()),
                "bggpassword" => bggpassword = Some(value.to_string()),
                "SessionID" => session_id = Some(value.to_string()),
                _ => {}
            }
        }
    }
    match (bggusername, bggpassword, session_id) {
        (Some(u), Some(p), Some(s)) => Ok(Cookies {
            bggusername: u,
            bggpassword: p,
            session_id: s,
        }),
        _ => Err(Error::Secrets(
            "login response missing expected cookies".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn extracts_three_cookies_from_login_response() {
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
                    .append_header("set-cookie", "SessionID=abc123; Path=/")
                    .append_header("set-cookie", "other=ignored; Path=/"),
            )
            .mount(&server)
            .await;

        let url = server.uri();
        let cookies = tokio::task::spawn_blocking(move || login(&url, "alice", "pw"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cookies.bggusername, "alice");
        assert_eq!(cookies.bggpassword, "cookiepw");
        assert_eq!(cookies.session_id, "abc123");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bad_credentials_returns_auth_required() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/api/v1"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let url = server.uri();
        let err = tokio::task::spawn_blocking(move || login(&url, "alice", "wrong"))
            .await
            .unwrap()
            .unwrap_err();
        assert!(matches!(err, Error::AuthRequired));
    }
}
