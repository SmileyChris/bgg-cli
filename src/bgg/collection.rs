use crate::bgg::client::HttpClient;
use crate::bgg::parse;
use crate::error::Result;
use crate::model::CollectionItem;
use chrono::{DateTime, Duration, Utc};

pub fn fetch(
    client: &HttpClient,
    username: &str,
    modified_since: Option<DateTime<Utc>>,
    subtype: Option<&str>,
) -> Result<Vec<CollectionItem>> {
    let mut query: Vec<(&str, String)> = vec![
        ("username", username.to_string()),
        ("stats", "1".to_string()),
    ];
    if let Some(s) = subtype {
        query.push(("subtype", s.to_string()));
    }
    if let Some(ts) = modified_since {
        let safe = ts - Duration::minutes(1);
        query.push((
            "modifiedsince",
            safe.format("%y-%m-%d %H:%M:%S").to_string(),
        ));
    }
    let xml = client.get("/xmlapi2/collection", &query)?;
    parse::parse_collection(&xml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn includes_modifiedsince_when_provided() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .and(query_param("username", "alice"))
            .and(query_param("stats", "1"))
            .and(query_param("modifiedsince", "26-05-13 11:59:00"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                std::fs::read_to_string("tests/fixtures/collection_empty.xml").unwrap(),
            ))
            .mount(&server)
            .await;

        let since = DateTime::parse_from_rfc3339("2026-05-13T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let url = server.uri();
        let items = tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(None, url).unwrap().with_fast_timing();
            fetch(&c, "alice", Some(since), None)
        })
        .await
        .unwrap()
        .unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn includes_subtype_when_provided() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .and(query_param("subtype", "boardgameexpansion"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                std::fs::read_to_string("tests/fixtures/collection_empty.xml").unwrap(),
            ))
            .mount(&server)
            .await;

        let url = server.uri();
        tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(None, url).unwrap().with_fast_timing();
            fetch(&c, "alice", None, Some("boardgameexpansion"))
        })
        .await
        .unwrap()
        .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn omits_modifiedsince_when_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xmlapi2/collection"))
            .and(query_param("username", "alice"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                std::fs::read_to_string("tests/fixtures/collection_owned.xml").unwrap(),
            ))
            .mount(&server)
            .await;

        let url = server.uri();
        let items = tokio::task::spawn_blocking(move || {
            let c = HttpClient::with_base(None, url).unwrap().with_fast_timing();
            fetch(&c, "alice", None, None)
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(items.len(), 1);
    }
}
