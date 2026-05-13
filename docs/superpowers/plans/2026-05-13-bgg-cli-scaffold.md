# bgg-cli Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a working `bgg` CLI that authenticates with BGG, syncs a user's collection to a local JSON cache, and exposes it via `list`/`show`/`status`. Implements the scaffold design at `docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md`.

**Architecture:** Single Rust binary. `bgg/` module wraps all network I/O (cookie-authed `reqwest::blocking`, 202-retry loop, 5s rate gate). `cache.rs` owns the on-disk JSON cache. `secrets.rs` wraps the OS keyring. `cmd/*` files orchestrate; each subcommand is one file. `main.rs` dispatches via clap and maps typed errors to exit codes.

**Tech Stack:** Rust 2021. `clap` (derive), `reqwest` (blocking + cookies + rustls), `quick-xml` (serde), `serde`/`serde_json`, `keyring`, `directories`, `chrono`, `rpassword`, `thiserror`, `anyhow`, `tracing`. Dev: `wiremock`, `assert_cmd`, `insta`, `tempfile`.

**Out of scope for this plan:** Encrypted-file cookie fallback (Argon2 + AES-GCM passphrase flow). Tracked separately so this plan stays focused. Until that ships, `bgg login` errors out cleanly when the keyring is unavailable.

---

## File map

```
Cargo.toml
src/
  main.rs            // entry: parse cli, dispatch, map errors to exit codes
  cli.rs             // clap derive structs
  error.rs           // thiserror Error enum
  paths.rs           // XDG paths via `directories`
  model.rs           // CollectionItem, Status, Stats, CacheFile, Cookies
  cache.rs           // load / save / merge
  secrets.rs         // keyring wrapper (store/load/delete cookies)
  auth.rs            // POST /login/api/v1, cookie extraction
  bgg/
    mod.rs           // re-exports
    client.rs        // HttpClient: cookie header, 202 retry, rate gate
    collection.rs    // fetch(username, modifiedsince) -> raw XML
    parse.rs         // XML -> Vec<CollectionItem>
  cmd/
    mod.rs
    auth.rs
    sync.rs
    list.rs
    show.rs
    status.rs
tests/
  fixtures/
    collection_empty.xml
    collection_owned.xml
    collection_wishlist.xml
    collection_expansion.xml
  cli_smoke.rs
```

---

### Task 1: Cargo init and base dependencies

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Initialize cargo package**

Run: `cargo init --name bgg-cli --bin`
Expected: creates `Cargo.toml`, `src/main.rs`, `.gitignore`.

- [ ] **Step 2: Replace Cargo.toml with project deps**

```toml
[package]
name = "bgg-cli"
version = "0.1.0"
edition = "2021"
authors = ["Chris Beaven"]
description = "Sync a BoardGameGeek user's collection to a local cache."
license = "MIT"

[[bin]]
name = "bgg"
path = "src/main.rs"

[dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive", "cargo"] }
directories = "5"
keyring = "3"
quick-xml = { version = "0.36", features = ["serialize"] }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "cookies", "json", "rustls-tls"] }
rpassword = "7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
assert_cmd = "2"
insta = { version = "1", features = ["yaml"] }
tempfile = "3"
wiremock = "0.6"
tokio = { version = "1", features = ["rt", "macros"] } # wiremock requires an async runtime
```

- [ ] **Step 3: Stub main.rs**

```rust
fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: compiles, downloads deps. No warnings about unused crates yet (they'll all get used).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore src/main.rs
git commit -m "Initialize cargo package with base dependencies"
```

---

### Task 2: Error type

**Files:**
- Create: `src/error.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the error module**

`src/error.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("authentication required: run `bgg auth`")]
    AuthRequired,

    #[error("BGG queued the request and did not return data after {attempts} retries")]
    QueueTimeout { attempts: u32 },

    #[error("BGG rate limit hit")]
    RateLimited,

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("failed to parse BGG XML: {0}")]
    Parse(String),

    #[error("cache error at {path}: {source}")]
    Cache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("secrets error: {0}")]
    Secrets(String),

    #[error("no cached collection for user {0}; run `bgg sync`")]
    NoCache(String),

    #[error("no logged-in user; run `bgg auth`")]
    NoUser,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Process exit code for this error class.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::AuthRequired | Error::NoUser => 2,
            _ => 1,
        }
    }
}
```

- [ ] **Step 2: Wire error module into main**

Replace `src/main.rs`:

```rust
mod error;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/error.rs src/main.rs
git commit -m "Add typed Error enum with exit-code mapping"
```

---

### Task 3: Paths module

**Files:**
- Create: `src/paths.rs`
- Modify: `src/main.rs`
- Test: inline `#[cfg(test)]` in `src/paths.rs`

- [ ] **Step 1: Write the failing test**

`src/paths.rs`:

```rust
use directories::ProjectDirs;
use std::path::PathBuf;

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("", "", "bgg-cli")
        .expect("could not determine project directories for current OS")
}

pub fn state_dir() -> PathBuf {
    project_dirs().state_dir().unwrap_or_else(|| project_dirs().data_dir()).to_path_buf()
}

pub fn config_dir() -> PathBuf {
    project_dirs().config_dir().to_path_buf()
}

pub fn data_dir() -> PathBuf {
    project_dirs().data_dir().to_path_buf()
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn cache_file(username: &str) -> PathBuf {
    state_dir().join(format!("collection-{username}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_uses_username_in_name() {
        let p = cache_file("alice");
        assert!(p.to_string_lossy().ends_with("collection-alice.json"));
    }

    #[test]
    fn config_file_is_under_config_dir() {
        assert!(config_file().starts_with(config_dir()));
    }
}
```

- [ ] **Step 2: Wire into main**

`src/main.rs`:

```rust
mod error;
mod paths;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test paths`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add src/paths.rs src/main.rs
git commit -m "Add XDG-aware paths module"
```

---

### Task 4: Domain model types

**Files:**
- Create: `src/model.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write model types**

`src/model.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single entry in the cached collection. Mirrors what we keep from a
/// BGG `<item>` element under /xmlapi2/collection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionItem {
    pub id: u32,
    pub collid: Option<u64>,
    pub subtype: String,
    pub name: String,
    pub year_published: Option<i32>,
    pub image: Option<String>,
    pub thumbnail: Option<String>,
    pub status: Status,
    pub num_plays: u32,
    pub stats: Option<Stats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Status {
    pub own: bool,
    pub prev_owned: bool,
    pub for_trade: bool,
    pub want: bool,
    pub want_to_play: bool,
    pub want_to_buy: bool,
    pub wishlist: bool,
    pub wishlist_priority: Option<u8>,
    pub preordered: bool,
    pub last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stats {
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
    pub playing_time: Option<u32>,
    pub user_rating: Option<f32>,
    pub average: Option<f32>,
    pub bayes_average: Option<f32>,
    pub users_rated: Option<u32>,
}

/// On-disk cache file: header plus a map keyed by BGG id (string-keyed for
/// JSON friendliness).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    pub username: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub items: BTreeMap<String, CollectionItem>,
}

impl CacheFile {
    pub fn empty(username: impl Into<String>) -> Self {
        Self { username: username.into(), last_sync: None, items: BTreeMap::new() }
    }
}

/// The three cookies BGG hands out at login. Serialized as a JSON blob into
/// the keyring under entry name = username.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookies {
    pub bggusername: String,
    pub bggpassword: String,
    pub session_id: String,
}

impl Cookies {
    /// Render as a `Cookie:` header value.
    pub fn header(&self) -> String {
        format!(
            "bggusername={}; bggpassword={}; SessionID={}",
            self.bggusername, self.bggpassword, self.session_id
        )
    }
}
```

- [ ] **Step 2: Wire into main**

`src/main.rs`:

```rust
mod error;
mod model;
mod paths;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/model.rs src/main.rs
git commit -m "Add CollectionItem, CacheFile, Cookies model types"
```

---

### Task 5: XML parsing — fixtures and failing test

**Files:**
- Create: `tests/fixtures/collection_empty.xml`
- Create: `tests/fixtures/collection_owned.xml`
- Create: `tests/fixtures/collection_wishlist.xml`
- Create: `tests/fixtures/collection_expansion.xml`
- Create: `src/bgg/mod.rs`
- Create: `src/bgg/parse.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create XML fixtures**

`tests/fixtures/collection_empty.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<items totalitems="0" termsofuse="https://boardgamegeek.com/xmlapi/termsofuse" pubdate="Sun, 11 May 2026 19:00:00 +0000">
</items>
```

`tests/fixtures/collection_owned.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<items totalitems="1" termsofuse="https://boardgamegeek.com/xmlapi/termsofuse" pubdate="Sun, 11 May 2026 19:00:00 +0000">
  <item objecttype="thing" objectid="174430" subtype="boardgame" collid="987654321">
    <name sortindex="1">Gloomhaven</name>
    <yearpublished>2017</yearpublished>
    <image>https://cf.geekdo-images.com/full/gloomhaven.jpg</image>
    <thumbnail>https://cf.geekdo-images.com/thumb/gloomhaven.jpg</thumbnail>
    <stats minplayers="1" maxplayers="4" playingtime="120">
      <rating value="9">
        <usersrated value="50000"/>
        <average value="8.6"/>
        <bayesaverage value="8.4"/>
      </rating>
    </stats>
    <status own="1" prevowned="0" fortrade="0" want="0" wanttoplay="0" wanttobuy="0"
            wishlist="0" preordered="0" lastmodified="2026-04-01 12:34:56"/>
    <numplays>17</numplays>
  </item>
</items>
```

`tests/fixtures/collection_wishlist.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<items totalitems="1" termsofuse="https://boardgamegeek.com/xmlapi/termsofuse" pubdate="Sun, 11 May 2026 19:00:00 +0000">
  <item objecttype="thing" objectid="332686" subtype="boardgame" collid="111111111">
    <name sortindex="1">John Company: Second Edition</name>
    <yearpublished>2022</yearpublished>
    <status own="0" prevowned="0" fortrade="0" want="0" wanttoplay="1" wanttobuy="0"
            wishlist="1" wishlistpriority="2" preordered="0" lastmodified="2026-03-15 09:00:00"/>
    <numplays>0</numplays>
  </item>
</items>
```

`tests/fixtures/collection_expansion.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<items totalitems="1" termsofuse="https://boardgamegeek.com/xmlapi/termsofuse" pubdate="Sun, 11 May 2026 19:00:00 +0000">
  <item objecttype="thing" objectid="291453" subtype="boardgameexpansion" collid="222222222">
    <name sortindex="5">The Crew: Mission Deep Sea</name>
    <yearpublished>2021</yearpublished>
    <status own="1" prevowned="0" fortrade="0" want="0" wanttoplay="0" wanttobuy="0"
            wishlist="0" preordered="0"/>
    <numplays>3</numplays>
  </item>
</items>
```

- [ ] **Step 2: Create bgg module skeleton**

`src/bgg/mod.rs`:

```rust
pub mod parse;
```

- [ ] **Step 3: Write failing parse test**

`src/bgg/parse.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::CollectionItem;

pub fn parse_collection(_xml: &str) -> Result<Vec<CollectionItem>> {
    Err(Error::Parse("not implemented".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()
    }

    #[test]
    fn empty_collection_parses_to_empty_vec() {
        let items = parse_collection(&fixture("collection_empty.xml")).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn owned_item_parses_core_fields() {
        let items = parse_collection(&fixture("collection_owned.xml")).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id, 174430);
        assert_eq!(it.name, "Gloomhaven");
        assert_eq!(it.subtype, "boardgame");
        assert_eq!(it.year_published, Some(2017));
        assert!(it.status.own);
        assert_eq!(it.num_plays, 17);
        let stats = it.stats.as_ref().expect("stats present");
        assert_eq!(stats.users_rated, Some(50000));
        assert_eq!(stats.user_rating, Some(9.0));
    }

    #[test]
    fn wishlist_item_captures_priority() {
        let items = parse_collection(&fixture("collection_wishlist.xml")).unwrap();
        let it = &items[0];
        assert!(it.status.wishlist);
        assert_eq!(it.status.wishlist_priority, Some(2));
        assert!(it.status.want_to_play);
    }

    #[test]
    fn expansion_subtype_preserved() {
        let items = parse_collection(&fixture("collection_expansion.xml")).unwrap();
        assert_eq!(items[0].subtype, "boardgameexpansion");
    }
}
```

`src/main.rs`:

```rust
mod bgg;
mod error;
mod model;
mod paths;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 4: Run tests, confirm they fail**

Run: `cargo test bgg::parse`
Expected: 4 tests fail with "not implemented" or assertion errors.

- [ ] **Step 5: Commit failing tests**

```bash
git add tests/fixtures src/bgg src/main.rs
git commit -m "Add collection XML fixtures and failing parse tests"
```

---

### Task 6: XML parsing — implementation

**Files:**
- Modify: `src/bgg/parse.rs`

- [ ] **Step 1: Implement parse_collection with quick-xml serde**

Replace contents of `src/bgg/parse.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::{CollectionItem, Stats, Status};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ItemsXml {
    #[serde(default, rename = "item")]
    items: Vec<ItemXml>,
}

#[derive(Debug, Deserialize)]
struct ItemXml {
    #[serde(rename = "@objectid")]
    objectid: u32,
    #[serde(rename = "@subtype")]
    subtype: String,
    #[serde(rename = "@collid")]
    collid: Option<u64>,
    name: NameXml,
    yearpublished: Option<i32>,
    image: Option<String>,
    thumbnail: Option<String>,
    stats: Option<StatsXml>,
    status: StatusXml,
    numplays: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct NameXml {
    #[serde(rename = "$text")]
    text: String,
}

#[derive(Debug, Deserialize)]
struct StatsXml {
    #[serde(rename = "@minplayers")]
    minplayers: Option<u32>,
    #[serde(rename = "@maxplayers")]
    maxplayers: Option<u32>,
    #[serde(rename = "@playingtime")]
    playingtime: Option<u32>,
    rating: Option<RatingXml>,
}

#[derive(Debug, Deserialize)]
struct RatingXml {
    #[serde(rename = "@value")]
    value: Option<String>,
    usersrated: Option<ValuedU32>,
    average: Option<ValuedF32>,
    bayesaverage: Option<ValuedF32>,
}

#[derive(Debug, Deserialize)]
struct ValuedU32 {
    #[serde(rename = "@value")]
    value: u32,
}

#[derive(Debug, Deserialize)]
struct ValuedF32 {
    #[serde(rename = "@value")]
    value: f32,
}

#[derive(Debug, Deserialize)]
struct StatusXml {
    #[serde(rename = "@own", default)]
    own: BoolFlag,
    #[serde(rename = "@prevowned", default)]
    prevowned: BoolFlag,
    #[serde(rename = "@fortrade", default)]
    fortrade: BoolFlag,
    #[serde(rename = "@want", default)]
    want: BoolFlag,
    #[serde(rename = "@wanttoplay", default)]
    wanttoplay: BoolFlag,
    #[serde(rename = "@wanttobuy", default)]
    wanttobuy: BoolFlag,
    #[serde(rename = "@wishlist", default)]
    wishlist: BoolFlag,
    #[serde(rename = "@wishlistpriority")]
    wishlistpriority: Option<u8>,
    #[serde(rename = "@preordered", default)]
    preordered: BoolFlag,
    #[serde(rename = "@lastmodified")]
    lastmodified: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
struct BoolFlag(String);

impl BoolFlag {
    fn truthy(&self) -> bool {
        matches!(self.0.as_str(), "1" | "true")
    }
}

pub fn parse_collection(xml: &str) -> Result<Vec<CollectionItem>> {
    let parsed: ItemsXml = quick_xml::de::from_str(xml)
        .map_err(|e| Error::Parse(format!("collection: {e}")))?;
    parsed.items.into_iter().map(item_from_xml).collect()
}

fn item_from_xml(x: ItemXml) -> Result<CollectionItem> {
    let status = Status {
        own: x.status.own.truthy(),
        prev_owned: x.status.prevowned.truthy(),
        for_trade: x.status.fortrade.truthy(),
        want: x.status.want.truthy(),
        want_to_play: x.status.wanttoplay.truthy(),
        want_to_buy: x.status.wanttobuy.truthy(),
        wishlist: x.status.wishlist.truthy(),
        wishlist_priority: x.status.wishlistpriority,
        preordered: x.status.preordered.truthy(),
        last_modified: x.status.lastmodified.as_deref().and_then(parse_bgg_datetime),
    };
    let stats = x.stats.map(|s| Stats {
        min_players: s.minplayers,
        max_players: s.maxplayers,
        playing_time: s.playingtime,
        user_rating: s.rating.as_ref().and_then(|r| r.value.as_deref()).and_then(parse_rating),
        average: s.rating.as_ref().and_then(|r| r.average.as_ref()).map(|v| v.value),
        bayes_average: s.rating.as_ref().and_then(|r| r.bayesaverage.as_ref()).map(|v| v.value),
        users_rated: s.rating.as_ref().and_then(|r| r.usersrated.as_ref()).map(|v| v.value),
    });
    Ok(CollectionItem {
        id: x.objectid,
        collid: x.collid,
        subtype: x.subtype,
        name: x.name.text,
        year_published: x.yearpublished,
        image: x.image,
        thumbnail: x.thumbnail,
        status,
        num_plays: x.numplays.unwrap_or(0),
        stats,
    })
}

fn parse_rating(s: &str) -> Option<f32> {
    if s.eq_ignore_ascii_case("n/a") { None } else { s.parse().ok() }
}

fn parse_bgg_datetime(s: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()
    }

    #[test]
    fn empty_collection_parses_to_empty_vec() {
        let items = parse_collection(&fixture("collection_empty.xml")).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn owned_item_parses_core_fields() {
        let items = parse_collection(&fixture("collection_owned.xml")).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id, 174430);
        assert_eq!(it.name, "Gloomhaven");
        assert_eq!(it.subtype, "boardgame");
        assert_eq!(it.year_published, Some(2017));
        assert!(it.status.own);
        assert_eq!(it.num_plays, 17);
        let stats = it.stats.as_ref().expect("stats present");
        assert_eq!(stats.users_rated, Some(50000));
        assert_eq!(stats.user_rating, Some(9.0));
    }

    #[test]
    fn wishlist_item_captures_priority() {
        let items = parse_collection(&fixture("collection_wishlist.xml")).unwrap();
        let it = &items[0];
        assert!(it.status.wishlist);
        assert_eq!(it.status.wishlist_priority, Some(2));
        assert!(it.status.want_to_play);
    }

    #[test]
    fn expansion_subtype_preserved() {
        let items = parse_collection(&fixture("collection_expansion.xml")).unwrap();
        assert_eq!(items[0].subtype, "boardgameexpansion");
    }
}
```

- [ ] **Step 2: Run tests, confirm pass**

Run: `cargo test bgg::parse`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/bgg/parse.rs
git commit -m "Implement collection XML parsing with quick-xml"
```

---

### Task 7: HTTP client — 202 retry and rate gate

**Files:**
- Create: `src/bgg/client.rs`
- Modify: `src/bgg/mod.rs`

- [ ] **Step 1: Write failing wiremock test**

`src/bgg/client.rs`:

```rust
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

    /// Test hook: shrink the rate floor and retry delay.
    #[cfg(test)]
    pub fn with_fast_timing(mut self) -> Self {
        self.rate_floor = Duration::from_millis(0);
        self.queue_retry_delay = Duration::from_millis(10);
        self
    }

    pub fn raw(&self) -> &Client {
        &self.inner
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn cookies(&self) -> Option<&Cookies> {
        self.cookies.as_ref()
    }

    /// GET with cookie header, 5s rate gate, and 202 retry.
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
```

- [ ] **Step 2: Update bgg/mod.rs**

`src/bgg/mod.rs`:

```rust
pub mod client;
pub mod parse;
```

- [ ] **Step 3: Run tests**

Run: `cargo test bgg::client`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/bgg/client.rs src/bgg/mod.rs
git commit -m "Add HttpClient with cookie auth, 202 retry, and rate gate"
```

---

### Task 8: Collection endpoint wrapper

**Files:**
- Create: `src/bgg/collection.rs`
- Modify: `src/bgg/mod.rs`

- [ ] **Step 1: Write the endpoint wrapper with test**

`src/bgg/collection.rs`:

```rust
use crate::bgg::client::HttpClient;
use crate::bgg::parse;
use crate::error::Result;
use crate::model::CollectionItem;
use chrono::{DateTime, Duration, Utc};

pub fn fetch(
    client: &HttpClient,
    username: &str,
    modified_since: Option<DateTime<Utc>>,
) -> Result<Vec<CollectionItem>> {
    let mut query: Vec<(&str, String)> = vec![
        ("username", username.to_string()),
        ("stats", "1".to_string()),
    ];
    if let Some(ts) = modified_since {
        // Pull the floor back 1 minute to avoid missing edits in the boundary second.
        let safe = ts - Duration::minutes(1);
        query.push(("modifiedsince", safe.format("%y-%m-%d %H:%M:%S").to_string()));
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
            fetch(&c, "alice", Some(since))
        })
        .await
        .unwrap()
        .unwrap();
        assert!(items.is_empty());
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
            fetch(&c, "alice", None)
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(items.len(), 1);
    }
}
```

- [ ] **Step 2: Update bgg/mod.rs**

`src/bgg/mod.rs`:

```rust
pub mod client;
pub mod collection;
pub mod parse;
```

- [ ] **Step 3: Run tests**

Run: `cargo test bgg::collection`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/bgg/collection.rs src/bgg/mod.rs
git commit -m "Add collection endpoint wrapper with modifiedsince support"
```

---

### Task 9: Cache load / save / merge

**Files:**
- Create: `src/cache.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write cache module with tests**

`src/cache.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::{CacheFile, CollectionItem};
use chrono::Utc;
use std::path::Path;

pub fn load(path: &Path, username: &str) -> Result<CacheFile> {
    if !path.exists() {
        return Err(Error::NoCache(username.to_string()));
    }
    let bytes = std::fs::read(path).map_err(|e| Error::Cache { path: path.to_path_buf(), source: e })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::Parse(format!("cache {}: {e}", path.display())))
}

pub fn save(path: &Path, cache: &CacheFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Cache { path: parent.to_path_buf(), source: e })?;
    }
    let bytes = serde_json::to_vec_pretty(cache)
        .map_err(|e| Error::Parse(format!("cache serialize: {e}")))?;
    std::fs::write(path, bytes)
        .map_err(|e| Error::Cache { path: path.to_path_buf(), source: e })
}

/// Merge result: counts for user-facing reporting.
#[derive(Debug, Default, PartialEq)]
pub struct MergeReport {
    pub new: u32,
    pub updated: u32,
    pub unchanged: u32,
}

/// Merge incoming items into the cache. Updates `last_sync` to now.
pub fn merge(cache: &mut CacheFile, incoming: Vec<CollectionItem>) -> MergeReport {
    let mut report = MergeReport::default();
    for item in incoming {
        let key = item.id.to_string();
        match cache.items.get(&key) {
            None => {
                report.new += 1;
                cache.items.insert(key, item);
            }
            Some(existing) if existing == &item => {
                report.unchanged += 1;
            }
            Some(_) => {
                report.updated += 1;
                cache.items.insert(key, item);
            }
        }
    }
    cache.last_sync = Some(Utc::now());
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use tempfile::tempdir;

    fn item(id: u32, name: &str) -> CollectionItem {
        CollectionItem {
            id,
            collid: None,
            subtype: "boardgame".into(),
            name: name.into(),
            year_published: None,
            image: None,
            thumbnail: None,
            status: Status::default(),
            num_plays: 0,
            stats: None,
        }
    }

    #[test]
    fn load_missing_returns_no_cache() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("missing.json");
        let err = load(&p, "alice").unwrap_err();
        assert!(matches!(err, Error::NoCache(u) if u == "alice"));
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.json");
        let mut cache = CacheFile::empty("alice");
        cache.items.insert("1".into(), item(1, "Azul"));
        save(&p, &cache).unwrap();
        let loaded = load(&p, "alice").unwrap();
        assert_eq!(loaded.username, "alice");
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items["1"].name, "Azul");
    }

    #[test]
    fn merge_classifies_new_updated_unchanged() {
        let mut cache = CacheFile::empty("alice");
        cache.items.insert("1".into(), item(1, "Azul"));
        cache.items.insert("2".into(), item(2, "Catan"));

        let incoming = vec![
            item(1, "Azul"),         // unchanged
            item(2, "Catan: Cities"), // updated (name differs)
            item(3, "Wingspan"),     // new
        ];
        let report = merge(&mut cache, incoming);
        assert_eq!(report, MergeReport { new: 1, updated: 1, unchanged: 1 });
        assert_eq!(cache.items["2"].name, "Catan: Cities");
        assert!(cache.last_sync.is_some());
    }
}
```

- [ ] **Step 2: Wire into main**

`src/main.rs`:

```rust
mod bgg;
mod cache;
mod error;
mod model;
mod paths;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test cache`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cache.rs src/main.rs
git commit -m "Add cache load/save/merge with MergeReport"
```

---

### Task 10: Secrets module (keyring-only)

**Files:**
- Create: `src/secrets.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the keyring wrapper**

`src/secrets.rs`:

```rust
use crate::error::{Error, Result};
use crate::model::Cookies;

const SERVICE: &str = "bgg-cli";

fn entry(username: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, username).map_err(|e| Error::Secrets(e.to_string()))
}

pub fn store(username: &str, cookies: &Cookies) -> Result<()> {
    let blob = serde_json::to_string(cookies)
        .map_err(|e| Error::Secrets(format!("serialize: {e}")))?;
    entry(username)?
        .set_password(&blob)
        .map_err(|e| Error::Secrets(e.to_string()))
}

pub fn load(username: &str) -> Result<Cookies> {
    let blob = match entry(username)?.get_password() {
        Ok(s) => s,
        Err(keyring::Error::NoEntry) => return Err(Error::AuthRequired),
        Err(e) => return Err(Error::Secrets(e.to_string())),
    };
    serde_json::from_str(&blob).map_err(|e| Error::Secrets(format!("deserialize: {e}")))
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

    // Keyring crate ships an in-memory mock backend when its `mock` feature
    // (or default features) is enabled; the v3 default backend on each OS
    // varies, so guard the round-trip behind a feature-gated mock setup if
    // CI lacks a Secret Service. For now we only assert the no-entry path
    // by using a high-entropy username unlikely to exist.

    #[test]
    fn load_for_unknown_user_returns_auth_required() {
        let u = format!("bgg-cli-test-{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        match load(&u) {
            Err(Error::AuthRequired) => {}
            // On headless boxes without a backend, surface as Secrets — also acceptable here.
            Err(Error::Secrets(_)) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Wire into main**

`src/main.rs`:

```rust
mod bgg;
mod cache;
mod error;
mod model;
mod paths;
mod secrets;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test secrets`
Expected: 1 test passes (tolerant of either error path).

- [ ] **Step 4: Commit**

```bash
git add src/secrets.rs src/main.rs
git commit -m "Add keyring-backed secrets module"
```

---

### Task 11: Auth — login POST and cookie extraction

**Files:**
- Create: `src/auth.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write auth module with wiremock tests**

`src/auth.rs`:

```rust
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
        .json(&LoginBody { credentials: Credentials { username, password } })
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
        // Each Set-Cookie is "name=value; attr=...; attr=..."
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
        (Some(u), Some(p), Some(s)) => Ok(Cookies { bggusername: u, bggpassword: p, session_id: s }),
        _ => Err(Error::Secrets("login response missing expected cookies".into())),
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
                    .append_header("set-cookie", "bggusername=alice; Path=/; Domain=.boardgamegeek.com")
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
```

- [ ] **Step 2: Wire into main**

`src/main.rs`:

```rust
mod auth;
mod bgg;
mod cache;
mod error;
mod model;
mod paths;
mod secrets;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test auth`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/auth.rs src/main.rs
git commit -m "Add BGG login endpoint client with cookie extraction"
```

---

### Task 12: Config file (username persistence)

**Files:**
- Modify: `src/paths.rs` (only if needed)
- Create: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the config module**

`src/config.rs`:

```rust
use crate::error::{Error, Result};
use crate::paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub username: Option<String>,
}

pub fn load() -> Result<Config> {
    let path = paths::config_file();
    if !path.exists() {
        return Ok(Config::default());
    }
    let s = std::fs::read_to_string(&path)
        .map_err(|e| Error::Cache { path: path.clone(), source: e })?;
    // Minimal hand-rolled parser: avoid pulling in a TOML crate for one field.
    // Lines look like `username = "alice"`.
    let mut cfg = Config::default();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(rest) = line.strip_prefix("username") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let val = rest.trim_matches('"').to_string();
            cfg.username = Some(val);
        }
    }
    Ok(cfg)
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = paths::config_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Cache { path: parent.to_path_buf(), source: e })?;
    }
    let mut out = String::new();
    if let Some(u) = &cfg.username {
        out.push_str(&format!("username = \"{u}\"\n"));
    }
    std::fs::write(&path, out).map_err(|e| Error::Cache { path, source: e })
}

pub fn require_username() -> Result<String> {
    load()?.username.ok_or(Error::NoUser)
}
```

Rationale for hand-rolled parse: the config has one field today. Pulling in `toml` is dead weight; if config grows, swap in `toml` then.

- [ ] **Step 2: Wire into main**

`src/main.rs`:

```rust
mod auth;
mod bgg;
mod cache;
mod config;
mod error;
mod model;
mod paths;
mod secrets;

fn main() {
    println!("bgg-cli stub");
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "Add minimal config file for persisted username"
```

---

### Task 13: CLI surface (clap structs)

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Define clap structures**

`src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "Sync a BoardGameGeek user's collection to a local cache.")]
pub struct Cli {
    /// Verbose logging (-v info, -vv debug, -vvv trace).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Subcommand to run. If omitted, runs `status`.
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Authenticate with BGG and store cookies in the OS keyring.
    /// Use `--clear` to remove stored cookies instead.
    Auth {
        /// Username (defaults to the value in config.toml if set).
        username: Option<String>,
        /// Clear stored cookies for the current user instead of logging in.
        #[arg(long)]
        clear: bool,
    },
    /// Sync the collection. By default, incremental via `modifiedsince`.
    Sync {
        /// Ignore modifiedsince and pull the whole collection. Required to detect deletions.
        #[arg(long)]
        full: bool,
    },
    /// List cached collection items.
    List {
        /// Only owned items.
        #[arg(long)]
        owned: bool,
        /// JSON output instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Show one cached item by BGG id.
    Show { id: u32 },
    /// Show auth state, cached username, item count, and last sync time.
    Status,
}
```

- [ ] **Step 2: Wire into main with placeholder dispatch**

`src/main.rs`:

```rust
mod auth;
mod bgg;
mod cache;
mod cli;
mod config;
mod error;
mod model;
mod paths;
mod secrets;

use clap::Parser;

fn main() {
    let _cli = cli::Cli::parse();
    eprintln!("dispatch not yet wired");
    std::process::exit(1);
}
```

- [ ] **Step 3: Verify help renders**

Run: `cargo run -- --help`
Expected: clap help text listing all six subcommands.

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "Add clap CLI surface with all subcommands"
```

---

### Task 14: cmd::auth

**Files:**
- Create: `src/cmd/mod.rs`
- Create: `src/cmd/auth.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write cmd::auth (login + clear in one)**

`src/cmd/mod.rs`:

```rust
pub mod auth;
```

`src/cmd/auth.rs`:

```rust
use crate::auth as bgg_auth;
use crate::config::{self, Config};
use crate::error::{Error, Result};
use crate::secrets;

const BGG_BASE: &str = "https://boardgamegeek.com";

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
    let cookies = bgg_auth::login(BGG_BASE, &username, &password)?;
    secrets::store(&username, &cookies)?;
    config::save(&Config { username: Some(username.clone()) })?;
    println!("Authenticated as {username}. Cookies stored in OS keyring.");
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
```

- [ ] **Step 2: Wire dispatch in main, default to status**

`src/main.rs`:

```rust
mod auth;
mod bgg;
mod cache;
mod cli;
mod cmd;
mod config;
mod error;
mod model;
mod paths;
mod secrets;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let parsed = Cli::parse();
    init_logging(parsed.verbose);
    let result = match parsed.command {
        Some(Command::Auth { username, clear }) => cmd::auth::run(username, clear),
        // status is the default when no subcommand is given
        None | Some(Command::Status) => {
            eprintln!("status not yet implemented");
            std::process::exit(1);
        }
        _ => {
            eprintln!("not yet implemented");
            std::process::exit(1);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code());
    }
}

fn init_logging(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level))
        .with_target(false)
        .try_init();
}
```

- [ ] **Step 3: Smoke-build**

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/cmd src/main.rs
git commit -m "Wire bgg auth: prompt+store login; --clear removes cookies"
```

---

### Task 15: cmd::sync

**Files:**
- Create: `src/cmd/sync.rs`
- Modify: `src/cmd/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write cmd::sync**

`src/cmd/sync.rs`:

```rust
use crate::bgg::client::HttpClient;
use crate::bgg::collection;
use crate::cache;
use crate::config;
use crate::error::Result;
use crate::model::CacheFile;
use crate::paths;
use crate::secrets;

pub fn run(full: bool) -> Result<()> {
    let username = config::require_username()?;
    let cookies = secrets::load(&username)?;
    let client = HttpClient::new(Some(cookies))?;

    let cache_path = paths::cache_file(&username);
    let mut cache = match cache::load(&cache_path, &username) {
        Ok(c) => c,
        Err(crate::error::Error::NoCache(_)) => CacheFile::empty(&username),
        Err(e) => return Err(e),
    };

    let modified_since = if full { None } else { cache.last_sync };
    let items = collection::fetch(&client, &username, modified_since)?;
    let report = cache::merge(&mut cache, items);
    cache::save(&cache_path, &cache)?;

    let total = cache.items.len();
    println!(
        "Synced {} items into cache ({} new, {} updated, {} unchanged). Total: {total}.",
        report.new + report.updated + report.unchanged,
        report.new,
        report.updated,
        report.unchanged,
    );
    if !full {
        println!("Tip: incremental sync cannot detect deletions. Run `bgg sync --full` periodically.");
    }
    Ok(())
}
```

- [ ] **Step 2: Update cmd/mod.rs and main dispatch**

`src/cmd/mod.rs`:

```rust
pub mod auth;
pub mod sync;
```

In `src/main.rs`, extend the match to include sync (full replacement of the match block):

```rust
    let result = match parsed.command {
        Some(Command::Auth { username, clear }) => cmd::auth::run(username, clear),
        Some(Command::Sync { full }) => cmd::sync::run(full),
        None | Some(Command::Status) => {
            eprintln!("status not yet implemented");
            std::process::exit(1);
        }
        _ => {
            eprintln!("not yet implemented");
            std::process::exit(1);
        }
    };
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/cmd src/main.rs
git commit -m "Wire bgg sync: fetch, merge, save cache"
```

---

### Task 16: cmd::list, cmd::show, cmd::status

**Files:**
- Create: `src/cmd/list.rs`
- Create: `src/cmd/show.rs`
- Create: `src/cmd/status.rs`
- Modify: `src/cmd/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write cmd::list**

`src/cmd/list.rs`:

```rust
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
```

- [ ] **Step 2: Write cmd::show**

`src/cmd/show.rs`:

```rust
use crate::cache;
use crate::config;
use crate::error::{Error, Result};
use crate::paths;

pub fn run(id: u32) -> Result<()> {
    let username = config::require_username()?;
    let cache = cache::load(&paths::cache_file(&username), &username)?;
    let item = cache
        .items
        .get(&id.to_string())
        .ok_or_else(|| Error::Parse(format!("no cached item with id {id}")))?;
    let out = serde_json::to_string_pretty(item)
        .map_err(|e| Error::Parse(format!("json: {e}")))?;
    println!("{out}");
    Ok(())
}
```

- [ ] **Step 3: Write cmd::status**

`src/cmd/status.rs`:

```rust
use crate::cache;
use crate::config;
use crate::error::Result;
use crate::paths;
use crate::secrets;

pub fn run() -> Result<()> {
    let cfg = config::load()?;
    let Some(username) = cfg.username else {
        println!("No logged-in user. Run `bgg login`.");
        return Ok(());
    };
    let auth_state = match secrets::load(&username) {
        Ok(_) => "cookies present",
        Err(crate::error::Error::AuthRequired) => "no cookies stored — run `bgg auth`",
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
            let last = c.last_sync.map(|t| t.to_rfc3339()).unwrap_or_else(|| "never".into());
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
```

- [ ] **Step 4: Wire dispatch**

`src/cmd/mod.rs`:

```rust
pub mod auth;
pub mod list;
pub mod show;
pub mod status;
pub mod sync;
```

In `src/main.rs`, replace the match body with the full set (status is the default when no subcommand is given):

```rust
    let result = match parsed.command {
        Some(Command::Auth { username, clear }) => cmd::auth::run(username, clear),
        Some(Command::Sync { full }) => cmd::sync::run(full),
        Some(Command::List { owned, json }) => cmd::list::run(owned, json),
        Some(Command::Show { id }) => cmd::show::run(id),
        None | Some(Command::Status) => cmd::status::run(),
    };
```

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: compiles cleanly. No warnings.

- [ ] **Step 6: Commit**

```bash
git add src/cmd src/main.rs
git commit -m "Wire list, show, status subcommands; status is default"
```

---

### Task 17: CLI smoke tests

**Files:**
- Create: `tests/cli_smoke.rs`

- [ ] **Step 1: Write integration tests**

`tests/cli_smoke.rs`:

```rust
use assert_cmd::Command;

#[test]
fn help_lists_all_subcommands() {
    let out = Command::cargo_bin("bgg")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    for sub in ["auth", "sync", "list", "show", "status"] {
        assert!(text.contains(sub), "help missing subcommand: {sub}\n---\n{text}");
    }
}

#[test]
fn status_with_no_config_says_no_user() {
    // Redirect XDG to a fresh tempdir so this test sees no config.
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("bgg")
        .unwrap()
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("XDG_STATE_HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .arg("status")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("No logged-in user"), "got:\n{text}");
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: all unit + integration tests pass.

- [ ] **Step 3: Smoke-run binary**

Run: `cargo run -- --help`
Expected: clap help text.

Run: `cargo run` (no args, no creds configured)
Expected: status output saying "No logged-in user. Run `bgg auth`."

- [ ] **Step 4: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "Add CLI smoke tests for help and status"
```

---

### Task 18: README and final polish

**Files:**
- Create: `README.md`
- Modify: `Cargo.toml` (add `readme = "README.md"`)

- [ ] **Step 1: Write a minimal README**

`README.md`:

````markdown
# bgg-cli

A single-purpose CLI that fetches a BoardGameGeek user's collection, caches it
locally, and keeps the cache in sync. One-way (BGG → local). Single user.

## Install

```
cargo install --path .
```

## Use

```
bgg                        # default: status (auth state, item count, last sync)
bgg auth                   # prompt for username + password, store cookies in keyring
bgg auth --clear           # remove stored cookies
bgg sync                   # incremental sync
bgg sync --full            # full sync (required to detect deletions)
bgg list                   # table view of cached collection
bgg list --owned --json    # JSON, owned only
bgg show 174430            # one item by BGG id
bgg status                 # explicit status (same as `bgg`)
```

Cookies live in the OS keyring (Secret Service / macOS Keychain / Windows
Credential Manager). Headless Linux boxes without a running Secret Service
are not supported in v1.

The cache lives at `$XDG_STATE_HOME/bgg-cli/collection-<username>.json`.

## Status

Alpha. Read the spec at
[`docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md`](docs/superpowers/specs/2026-05-13-bgg-cli-scaffold-design.md).
````

- [ ] **Step 2: Reference README in Cargo.toml**

In `Cargo.toml`, add inside `[package]`:

```toml
readme = "README.md"
```

- [ ] **Step 3: Final full test run**

Run: `cargo test`
Expected: green.

Run: `cargo clippy -- -D warnings`
Expected: no warnings.

Run: `cargo fmt --check`
Expected: no diff.

- [ ] **Step 4: Commit**

```bash
git add README.md Cargo.toml
git commit -m "Add README and final lint pass"
```

---

## Out of scope (follow-up plans)

- **Encrypted-file cookie fallback** for headless Linux without Secret Service.
  Argon2 + AES-GCM, passphrase prompt on store/load. Should be a small
  self-contained plan touching only `src/secrets.rs` (split into a trait with
  two backends) and `cmd::login` (consent prompt).
- **Shell completion** generation (`bgg completion <shell>`).
- **Release automation** and `cargo install` packaging.
- **Schema migrations** for the JSON cache once fields evolve.
