use crate::card::Collection;
use crate::deck::Deck;
use flate2::read::GzDecoder;
use std::io::prelude::*;

/// Returns a new collection of unique oracle cards from data/oracle_cards.landlord
pub fn oracle_cards() -> Result<Collection, bincode::Error> {
    // NOTE(jshrake): This file is generated!
    // Run scryfall2landlord to generate this file
    // See the `make card-update` task in the top-level Makefile
    let b = include_bytes!("../../data/oracle_cards.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}

/// Returns a new collection of all cards from data/all_cards.landlord
pub fn all_cards() -> Result<Collection, bincode::Error> {
    let b = include_bytes!("../../data/all_cards.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}

/// Returns a list of standard format net decks from mtg goldfish
pub fn net_decks() -> Result<Vec<Deck>, bincode::Error> {
    let b = include_bytes!("../../data/net_decks.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}

lazy_static! {
    pub static ref ORACLE_CARDS: Collection = oracle_cards().expect("oracle_cards() failed");
    pub static ref ALL_CARDS: Collection = all_cards().expect("all_cards() failed");
    pub static ref NET_DECKS: Vec<Deck> = net_decks().expect("net_decks() failed");
}

#[cfg(test)]
mod tests {
    use crate::data::*;

    #[test]
    fn oracle_cards_have_non_empty_image_uri() {
        let any_empty_image_uri = ORACLE_CARDS.cards.iter().any(|c| c.image_uri.is_empty());
        assert_eq!(any_empty_image_uri, false);
    }

    #[test]
    fn oracle_cards_have_unique_names() {
        let mut deduped = ORACLE_CARDS.clone();
        deduped.cards.dedup();
        assert_eq!(deduped.cards.len(), ORACLE_CARDS.cards.len());
    }
}
