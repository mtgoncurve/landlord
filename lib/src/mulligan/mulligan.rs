use crate::card::Card;
use crate::hand::Hand;
use rand::prelude::*;

/// The base trait for any mulligan type
pub trait Mulligan {
  /// Returns a randomly shuffled `Hand`
  ///
  /// # Arguments
  ///
  /// * `rng` - A random number generator used to shuffle the deck
  /// * `deck` - A collection of cards that a player starts a game with. See [Deck](https://mtg.gamepedia.com/Deck)
  /// * `draws` - The number of cards to draw after the mulligan process
  fn simulate_hand(&self, rng: &mut impl Rng, deck: &[Card], draws: usize) -> Hand;
}
