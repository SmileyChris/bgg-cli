use crate::model::{CacheFile, CollectionItem};
use crate::stats::owned::{self, OwnedStats};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub(crate) username: String,
    pub(crate) last_sync: Option<DateTime<Utc>>,
    pub(crate) items: ItemsBlock,
    pub(crate) statuses: Statuses,
    pub(crate) owned: OwnedStats,
}

#[derive(Debug, Serialize)]
pub(crate) struct ItemsBlock {
    pub(crate) total: usize,
    pub(crate) boardgames: usize,
    pub(crate) expansions: usize,
    pub(crate) other: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Statuses {
    pub(crate) own: usize,
    pub(crate) own_boardgames: usize,
    pub(crate) own_expansions: usize,
    pub(crate) prev_owned: usize,
    pub(crate) for_trade: usize,
    pub(crate) want: usize,
    pub(crate) want_to_play: usize,
    pub(crate) want_to_buy: usize,
    pub(crate) wishlist: usize,
    pub(crate) preordered: usize,
    pub(crate) wishlist_by_priority: BTreeMap<u8, usize>,
}

pub(crate) fn build(cache: &CacheFile) -> Report {
    let all: Vec<&CollectionItem> = cache.items.values().collect();
    let items = items_block(&all);
    let statuses = statuses(&all);
    let owned = owned::build(&all);
    Report {
        username: cache.username.clone(),
        last_sync: cache.last_sync,
        items,
        statuses,
        owned,
    }
}

fn items_block(all: &[&CollectionItem]) -> ItemsBlock {
    let mut boardgames = 0;
    let mut expansions = 0;
    let mut other = 0;
    for i in all {
        match i.subtype.as_str() {
            "boardgame" => boardgames += 1,
            "boardgameexpansion" => expansions += 1,
            _ => other += 1,
        }
    }
    ItemsBlock {
        total: all.len(),
        boardgames,
        expansions,
        other,
    }
}

fn statuses(all: &[&CollectionItem]) -> Statuses {
    let mut s = Statuses {
        own: 0,
        own_boardgames: 0,
        own_expansions: 0,
        prev_owned: 0,
        for_trade: 0,
        want: 0,
        want_to_play: 0,
        want_to_buy: 0,
        wishlist: 0,
        preordered: 0,
        wishlist_by_priority: BTreeMap::new(),
    };
    for i in all {
        let st = &i.status;
        if st.own {
            s.own += 1;
            match i.subtype.as_str() {
                "boardgame" => s.own_boardgames += 1,
                "boardgameexpansion" => s.own_expansions += 1,
                _ => {}
            }
        }
        if st.prev_owned {
            s.prev_owned += 1;
        }
        if st.for_trade {
            s.for_trade += 1;
        }
        if st.want {
            s.want += 1;
        }
        if st.want_to_play {
            s.want_to_play += 1;
        }
        if st.want_to_buy {
            s.want_to_buy += 1;
        }
        if st.wishlist {
            s.wishlist += 1;
            if let Some(p) = st.wishlist_priority {
                *s.wishlist_by_priority.entry(p).or_insert(0) += 1;
            }
        }
        if st.preordered {
            s.preordered += 1;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CollectionItem, Status};

    fn item(id: u32, name: &str, owned: bool) -> CollectionItem {
        CollectionItem {
            id,
            collid: Some(id as u64),
            subtype: "boardgame".into(),
            name: name.into(),
            year_published: Some(2020),
            image: None,
            thumbnail: None,
            status: Status {
                own: owned,
                ..Default::default()
            },
            num_plays: 0,
            stats: None,
        }
    }

    fn cache_with(items: Vec<CollectionItem>) -> CacheFile {
        let mut c = CacheFile::empty("tester");
        for i in items {
            c.items.insert(i.collid.unwrap().to_string(), i);
        }
        c
    }

    #[test]
    fn counts_status_flags_across_all_subtypes() {
        let mut a = item(1, "A", true);
        let mut b = item(2, "B", false);
        b.status.wishlist = true;
        b.status.wishlist_priority = Some(2);
        a.subtype = "boardgameexpansion".into();
        let r = build(&cache_with(vec![a, b]));
        assert_eq!(r.items.boardgames, 1);
        assert_eq!(r.items.expansions, 1);
        assert_eq!(r.statuses.own, 1);
        assert_eq!(r.statuses.wishlist, 1);
        assert_eq!(r.statuses.wishlist_by_priority.get(&2), Some(&1));
    }
}
