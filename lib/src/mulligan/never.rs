use crate::card::Card;
use crate::hand::Hand;
use crate::mulligan::Mulligan;
use rand::prelude::*;

// Hardcoded starting handsize, consider allowing users to specify
const STARTING_HAND_SIZE: usize = 7;

/// Never represents the mulligan strategy wherein the
/// player always keeps their initially drawn starting hand
#[derive(Debug, Serialize, Deserialize)]
pub struct Never {
  pub starting_hand_size: usize,
}

impl Never {
  pub fn new() -> Self {
    Self {
      starting_hand_size: STARTING_HAND_SIZE,
    }
  }

  pub fn never() -> Self {
    Self::new()
  }
}

impl Default for Never {
  fn default() -> Self {
    Self::new()
  }
}

impl Mulligan for Never {
  fn simulate_hand(&self, mut rng: &mut impl Rng, deck: &[Card], draws: usize) -> Hand {
    // We need to draw our starting hand size +  the number of draws specified, capped by the deck_len
    let deck_len = deck.len();
    let cards_to_draw = std::cmp::min(deck_len, self.starting_hand_size + draws);
    let starting_hand_size = std::cmp::min(deck_len, self.starting_hand_size);
    let mut index_range: Vec<_> = (0..deck_len).collect();
    let shuffled_deck: Vec<_> = index_range
      .partial_shuffle(&mut rng, cards_to_draw)
      .0
      .iter()
      .map(|i| &deck[*i])
      .collect();
    return Hand::from_opening_and_draws(
      &shuffled_deck[..starting_hand_size],
      &shuffled_deck[starting_hand_size..],
    );
  }
}
