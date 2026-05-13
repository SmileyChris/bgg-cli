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
    let parsed: ItemsXml =
        quick_xml::de::from_str(xml).map_err(|e| Error::Parse(format!("collection: {e}")))?;
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
        last_modified: x
            .status
            .lastmodified
            .as_deref()
            .and_then(parse_bgg_datetime),
    };
    let stats = x.stats.map(|s| Stats {
        min_players: s.minplayers,
        max_players: s.maxplayers,
        playing_time: s.playingtime,
        user_rating: s
            .rating
            .as_ref()
            .and_then(|r| r.value.as_deref())
            .and_then(parse_rating),
        average: s
            .rating
            .as_ref()
            .and_then(|r| r.average.as_ref())
            .map(|v| v.value),
        bayes_average: s
            .rating
            .as_ref()
            .and_then(|r| r.bayesaverage.as_ref())
            .map(|v| v.value),
        users_rated: s
            .rating
            .as_ref()
            .and_then(|r| r.usersrated.as_ref())
            .map(|v| v.value),
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
    if s.eq_ignore_ascii_case("n/a") {
        None
    } else {
        s.parse().ok()
    }
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
