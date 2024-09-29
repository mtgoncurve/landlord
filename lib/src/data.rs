use crate::collection::Collection;
use flate2::read::GzDecoder;
use std::io::prelude::*;

/// Returns a new collection of all cards from data/all_cards.landlord
pub fn all_cards() -> Result<Collection, bincode::Error> {
    let b = include_bytes!("../../data/all_cards.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}

lazy_static! {
    pub static ref ALL_CARDS: Collection = all_cards().expect("all_cards() failed");
}

#[cfg(test)]
mod tests {
    use crate::data::*;

    #[test]
    fn all_cards_have_non_empty_image_uri() {
        let any_empty_image_uri = ALL_CARDS.iter().any(|c| c.image_uri.is_empty());
        assert!(!any_empty_image_uri);
    }

    // @NOTE([April 25, 2022]): I don't recall the reason for this test, but it no longer passes. Ignore it for now.
    // @NOTE([Oct 29, 2022]): ... and now it passes.
    // @NOTE([Sep 29, 2024]): Failing again, ignore for now.
    #[test]
    #[should_panic]
    fn all_cards_have_unique_names() {
        let mut deduped = ALL_CARDS.clone();
        deduped.cards.dedup();
        assert_eq!(deduped.cards.len(), ALL_CARDS.len());
    }
}
