use crate::error::{Error, Result};
use crate::model::Cookies;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const USER_AGENT: &str = concat!("bgg-cli/", env!("CARGO_PKG_VERSION"));

pub struct HttpClient {
    inner: Client,
    cookies: Option<Cookies>,
    base: String,
    rate_floor: Duration,
    queue_retry_delay: Duration,
    max_queue_retries: u32,
    last_call: Mutex<Option<Instant>>,
}

impl HttpClient {
    pub fn new(cookies: Option<Cookies>) -> Result<Self> {
        Self::with_base(cookies, "https://boardgamegeek.com".to_string())
    }

    pub fn with_base(cookies: Option<Cookies>, base: String) -> Result<Self> {
        let inner = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(Error::Network)?;
        Ok(Self {
            inner,
            cookies,
            base,
            rate_floor: Duration::from_secs(5),
            queue_retry_delay: Duration::from_secs(12),
            max_queue_retries: 25,
            last_call: Mutex::new(None),
        })
    }

    #[cfg(test)]
    pub fn with_fast_timing(mut self) -> Self {
        self.rate_floor = Duration::from_millis(0);
        self.queue_retry_delay = Duration::from_millis(10);
        self
    }

    pub fn get(&self, path: &str, query: &[(&str, String)]) -> Result<String> {
        let url = format!("{}{}", self.base, path);
        let mut attempts: u32 = 0;
        loop {
            self.enforce_rate_floor();
            let mut req = self.inner.get(&url).query(query);
            if let Some(cookies) = &self.cookies {
                req = req.header(reqwest::header::COOKIE, cookies.header());
            }
            let resp = req.send().map_err(Error::Network)?;
            match resp.status() {
                StatusCode::OK => return resp.text().map_err(Error::Network),
                StatusCode::ACCEPTED => {
                    attempts += 1;
                    if attempts >= self.max_queue_retries {
                        return Err(Error::QueueTimeout { attempts });
                    }
                    std::thread::sleep(self.queue_retry_delay);
                }
                StatusCode::UNAUTHORIZED => return Err(Error::AuthRequired),
                StatusCode::TOO_MANY_REQUESTS => return Err(Error::RateLimited),
                s => {
                    let body = resp.text().unwrap_or_default();
                    return Err(Error::Parse(format!("unexpected status {s}: {body}")));
                }
            }
        }
    }

    fn enforce_rate_floor(&self) {
        let mut last = self.last_call.lock().unwrap();
        if let Some(t) = *last {
            let elapsed = t.elapsed();
            if elapsed < self.rate_floor {
                std::thread::sleep(self.rate_floor - elapsed);
            }
        }
        *last = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn returns_body_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .and(query_param("username", "alice"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<items/>"))
            .mount(&server)
            .await;

        let url = server.uri();
        let body = tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(None, url).unwrap().with_fast_timing();
            c.get("/xmlapi2/collection", &[("username", "alice".into())])
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(body, "<items/>");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn retries_on_202_then_returns_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .respond_with(ResponseTemplate::new(202))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<items/>"))
            .mount(&server)
            .await;

        let url = server.uri();
        let body = tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(None, url).unwrap().with_fast_timing();
            c.get("/xmlapi2/collection", &[("username", "alice".into())])
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(body, "<items/>");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn maps_401_to_auth_required() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let url = server.uri();
        let err = tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(None, url).unwrap().with_fast_timing();
            c.get("/xmlapi2/collection", &[("username", "alice".into())])
        })
        .await
        .unwrap()
        .unwrap_err();
        assert!(matches!(err, Error::AuthRequired));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sends_cookie_header_when_present() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .and(header("cookie", "bggusername=alice; bggpassword=pw; SessionID=sid"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<items/>"))
            .mount(&server)
            .await;

        let cookies = Cookies {
            bggusername: "alice".into(),
            bggpassword: "pw".into(),
            session_id: "sid".into(),
        };
        let url = server.uri();
        let body = tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(Some(cookies), url).unwrap().with_fast_timing();
            c.get("/xmlapi2/collection", &[("username", "alice".into())])
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(body, "<items/>");
    }
}
