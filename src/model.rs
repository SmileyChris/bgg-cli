use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

impl Stats {
    pub fn supports_player_count(&self, players: u32) -> bool {
        let (Some(min), Some(max)) = (self.min_players, self.max_players) else {
            return false;
        };
        players > 0 && min > 0 && max > 0 && min <= players && max >= players
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    pub username: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub items: BTreeMap<String, CollectionItem>,
}

impl CacheFile {
    pub fn empty(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            last_sync: None,
            items: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookies {
    pub bggusername: String,
    pub bggpassword: String,
    pub session_id: String,
}

impl Cookies {
    pub fn header(&self) -> String {
        format!(
            "bggusername={}; bggpassword={}; SessionID={}",
            self.bggusername, self.bggpassword, self.session_id
        )
    }
}

/// The full blob persisted to the OS keyring per user. We keep the password
/// alongside the cookies so we can silently re-login when the SessionID cookie
/// expires (~1 hour) without re-prompting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCreds {
    pub password: String,
    pub cookies: Cookies,
    /// When we expect the SessionID cookie to stop being accepted (best
    /// effort; the server may invalidate earlier). None means we don't
    /// know — fall back to lazy refresh on 401.
    #[serde(default)]
    pub session_fresh_until: Option<DateTime<Utc>>,
}
