use crate::card::Card;
use crate::hand::Hand;
use crate::mulligan::Mulligan;
use rand::prelude::*;

const _STARTING_HAND_SIZE: usize = 7;

/// Vancouver represents a mulligan strategy that adheres to the
/// [Vancouver mulligan rule](https://mtg.gamepedia.com/Mulligan#Vancouver_mulligan)
///
/// TODO This strategy is currently unimplemented
#[derive(Debug, Serialize, Deserialize)]
pub struct Vancouver {
  pub starting_hand_size: usize,
}

impl Vancouver {
  pub fn never() -> Self {
    unimplemented!()
  }
  pub fn always(_down_to: usize) -> Self {
    unimplemented!()
  }
}

impl Mulligan for Vancouver {
  fn simulate_hand(&self, mut _rng: &mut impl Rng, _deck: &[&Card], _draws: usize) -> Hand {
    unimplemented!();
  }
}
