use crate::error::{Error, Result};
use crate::model::CollectionItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    Owned,
    PrevOwned,
    Wishlist,
    WantToPlay,
    WantToBuy,
    Preordered,
    ForTrade,
    Expansion,
    Rated,
    Played,
    Solo,
    All,
}

const FILTER_VALUES: &str = "owned, prev-owned, wishlist, want-to-play, want-to-buy, preordered, for-trade, expansion, rated, played, solo, all (prefix with `not:` to invert)";

impl FilterKind {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "owned" => FilterKind::Owned,
            "prev-owned" => FilterKind::PrevOwned,
            "wishlist" => FilterKind::Wishlist,
            "want-to-play" => FilterKind::WantToPlay,
            "want-to-buy" => FilterKind::WantToBuy,
            "preordered" => FilterKind::Preordered,
            "for-trade" => FilterKind::ForTrade,
            "expansion" => FilterKind::Expansion,
            "rated" => FilterKind::Rated,
            "played" => FilterKind::Played,
            "solo" => FilterKind::Solo,
            "all" => FilterKind::All,
            other => {
                return Err(Error::BadArg(format!(
                    "unknown --filter value `{other}`. Valid: {FILTER_VALUES}"
                )));
            }
        })
    }

    fn item_matches(self, i: &CollectionItem) -> bool {
        match self {
            FilterKind::Owned => i.status.own,
            FilterKind::PrevOwned => i.status.prev_owned,
            FilterKind::Wishlist => i.status.wishlist,
            FilterKind::WantToPlay => i.status.want_to_play,
            FilterKind::WantToBuy => i.status.want_to_buy,
            FilterKind::Preordered => i.status.preordered,
            FilterKind::ForTrade => i.status.for_trade,
            FilterKind::Expansion => i.subtype == "boardgameexpansion",
            FilterKind::Rated => i.stats.as_ref().and_then(|s| s.user_rating).is_some(),
            FilterKind::Played => i.num_plays > 0,
            FilterKind::Solo => i.stats.as_ref().is_some_and(|s| s.supports_player_count(1)),
            FilterKind::All => true,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FilterSpec {
    rules: Vec<(FilterKind, bool)>,
}

impl FilterSpec {
    pub(crate) fn parse(s: &str) -> Result<Self> {
        let mut rules = Vec::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (name, invert) = match part.strip_prefix("not:") {
                Some(rest) => (rest.trim(), true),
                None => (part, false),
            };
            rules.push((FilterKind::parse(name)?, invert));
        }
        if rules.is_empty() {
            return Err(Error::BadArg("--filter is empty".into()));
        }
        Ok(FilterSpec { rules })
    }

    pub(crate) fn matches(&self, i: &CollectionItem) -> bool {
        self.rules.iter().all(|(kind, invert)| {
            let m = kind.item_matches(i);
            if *invert {
                !m
            } else {
                m
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(owned: bool, subtype: &str, plays: u32) -> CollectionItem {
        use crate::model::Status;
        CollectionItem {
            id: 1,
            collid: Some(1),
            subtype: subtype.into(),
            name: "x".into(),
            year_published: None,
            image: None,
            thumbnail: None,
            status: Status {
                own: owned,
                ..Default::default()
            },
            num_plays: plays,
            stats: None,
        }
    }

    #[test]
    fn filter_default_keeps_owned_boardgames_and_drops_expansions() {
        let spec = FilterSpec::parse("owned,not:expansion").unwrap();
        let bg = item(true, "boardgame", 0);
        let exp = item(true, "boardgameexpansion", 0);
        let unowned = item(false, "boardgame", 0);
        assert!(spec.matches(&bg));
        assert!(!spec.matches(&exp));
        assert!(!spec.matches(&unowned));
    }

    #[test]
    fn filter_played_matches_when_num_plays_gt_zero() {
        let spec = FilterSpec::parse("played").unwrap();
        assert!(spec.matches(&item(false, "boardgame", 3)));
        assert!(!spec.matches(&item(false, "boardgame", 0)));
    }

    #[test]
    fn filter_solo_requires_valid_solo_range() {
        use crate::model::Stats;
        let mut i = item(true, "boardgame", 0);
        i.stats = Some(Stats {
            min_players: Some(1),
            max_players: Some(4),
            playing_time: None,
            user_rating: None,
            average: None,
            bayes_average: None,
            users_rated: None,
        });
        assert!(FilterSpec::parse("solo").unwrap().matches(&i));
        i.stats.as_mut().unwrap().min_players = Some(2);
        assert!(!FilterSpec::parse("solo").unwrap().matches(&i));
        i.stats.as_mut().unwrap().min_players = Some(0);
        assert!(!FilterSpec::parse("solo").unwrap().matches(&i));
    }

    #[test]
    fn filter_rejects_unknown_value() {
        assert!(FilterSpec::parse("owned,bogus").is_err());
    }

    #[test]
    fn filter_rejects_empty_value() {
        assert!(FilterSpec::parse("").is_err());
        assert!(FilterSpec::parse("  ,  ").is_err());
    }
}
