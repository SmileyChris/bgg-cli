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
