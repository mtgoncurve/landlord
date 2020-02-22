//! # Simulation hands and auto tap algorithm
use crate::card::{Card, CardKind, Collection, ManaColor, ManaCost};
use crate::mulligan::Mulligan;
use rand::prelude::*;

/// Hand represents the opening hand after the mulligan process, along with any cards drawn
/// Note that the card draw is in order and represents the cards drawn during the draw step
#[derive(Debug)]
pub struct Hand {
  cards: Vec<SimCard>,
  pub starting_hand_size: usize,
  pub opening_hand_size: usize,
  pub mulligan_count: usize,
}

/// SimCard is an internal compact card representation
/// and consists of a subset of the attributes defined on `Card`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimCard {
  pub hash: u64,
  pub kind: CardKind,
  pub mana_cost: ManaCost,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum PlayOrder {
  First,
  Second,
}

/// AutoTapResult represents the result of the process that attempts
/// to tap land cards for mana in order to pay some mana cost
#[derive(Debug, Copy, Clone, Default)]
pub struct AutoTapResult {
  /// True if the lands in the opening hand and draws can tap for the mana cost of the goal
  pub paid: bool,
  /// True if CMC lands are in the opening hand and draws
  pub cmc: bool,
  /// True if the goal card is in the opening hand
  pub in_opening_hand: bool,
  /// True if the goal card is in the opening hand
  pub in_draw_hand: bool,
}

impl AutoTapResult {
  pub fn new() -> Self {
    Self::default()
  }
}

impl SimCard {
  pub fn new() -> Self {
    Self {
      kind: CardKind::Unknown,
      hash: 0,
      mana_cost: ManaCost::new(),
    }
  }
}

impl Hand {
  /// Returns a new hand with opening hand from `opening`, and card draw from `draws`
  pub fn from_opening_and_draws(opening: &[&Card], draws: &[&Card]) -> Self {
    let mut cards: Vec<SimCard> = Vec::with_capacity(opening.len() + draws.len());
    for card in opening {
      cards.push(SimCard {
        hash: card.hash,
        kind: card.kind,
        mana_cost: card.mana_cost,
      });
    }
    for card in draws {
      cards.push(SimCard {
        hash: card.hash,
        kind: card.kind,
        mana_cost: card.mana_cost,
      });
    }
    // TODO: hard coded starting hand size is bad and potentially incorrect
    // since the mulligan process defines the starting hand size
    let starting_hand_size = 7;
    let opening_hand_size = opening.len();
    Self {
      cards,
      starting_hand_size,
      opening_hand_size,
      mulligan_count: starting_hand_size - opening_hand_size,
    }
  }
  /// Returns a new random hand from `deck` using a mulligan strategy
  pub fn from_mulligan<T: Mulligan>(
    mulligan: &T,
    rng: &mut impl Rng,
    deck: &Collection,
    draws: usize,
  ) -> Self {
    mulligan.simulate_hand(rng, &deck.cards, draws)
  }

  /// Attempts to tap mana in the opening hand and `draws` to pay the mana cost of the `goal` card
  /// and returns the resulting information about this attempt
  ///
  ///# Details
  ///
  /// For a given goal card with CMC mana cost, one potential solution is to consider the set of all combinations
  /// of CMC land cards, and find at least one set that can pay the cost. The downside of this solution is the
  /// potential number of allocations required to examine the set of all combinations. Additionally, for large
  /// combination sets, we must exhaust the entire space in order to determine a false result. Instead, we choose
  /// to implement a greedy algorithm that has fairly bounded runtime and allocation costs. Keep in mind that much
  /// of the complexity of the following solution is in determining which mana color a dual land should tap for,
  /// and how to handle conditional lands, such as a tap land.
  ///
  /// An outline of the greedy algorithm follows:
  /// 1. Sort all the land cards in the opening hand and draws by color contribution,
  ///    with the goal of spending sources that only contribute a single mana color first.
  ///   - What do we mean by color contribution? Suppose the goal card costs `{1}{R}{R}{W}`, then:
  ///     * A basic Island {U} has a color contribution of 0
  ///     * A basic Mountain {R} has a color contribution of 1
  ///     * A dual mana card that can tap for {R} or {W} has a color contribution of 2
  ///   - Conceptually, this sorting represents both the play order and the tap order. It assumes a player with perfect foresight
  ///     that plays all the correct lands on previous turns in order to pay for the goal card in question.
  /// 2. For each sorted land, tap for the color that we have the least available of followed by the color that has the most remaining to pay.
  ///   - This last clause helps break ties. For instance, if we have a total of 2 green and 2 blue mana available, and '{G}{G}{U}'
  ///     remaining to pay, and we come across a land that can tap for green or blue, we would tap it for green.
  ///     If we only had 1 blue mana available, we would instead tap for blue due to the first clause taking precedence.
  ///
  /// I suspect it is possible to prove the correctness of the above algorithm via induction, but I have yet to do so.
  /// In lieu of proofs, we have a test cases!
  pub fn auto_tap_by_turn(
    &self,
    goal: &Card,
    turn: usize,
    player_order: PlayOrder,
  ) -> AutoTapResult {
    let mut scratch = Vec::with_capacity(60);
    let goal = SimCard {
      kind: goal.kind,
      hash: goal.hash,
      mana_cost: goal.mana_cost,
    };
    self.auto_tap_with_scratch(&goal, turn, player_order, &mut scratch)
  }

  /// On the play, auto tap on curve
  pub fn play_cmc_auto_tap(&self, goal: &Card) -> AutoTapResult {
    let turn = std::cmp::max(1, goal.turn) as usize;
    self.auto_tap_by_turn(goal, turn, PlayOrder::First)
  }

  /// On the draw, auto tap on curve
  pub fn draw_cmc_auto_tap(&self, goal: &Card) -> AutoTapResult {
    let turn = std::cmp::max(1, goal.turn) as usize;
    self.auto_tap_by_turn(goal, turn, PlayOrder::Second)
  }

  /// Returns a slice consisting of cards in the opening hand, after the mulligan process
  #[inline]
  pub fn opening(&self) -> &[SimCard] {
    self.slice(0, self.opening_hand_size)
  }

  /// Returns a slice consisting of cards drawn after the opening hand
  #[inline]
  pub fn draws(&self, draws: usize) -> &[SimCard] {
    self.slice(self.opening_hand_size, self.opening_hand_size + draws)
  }

  /// Returns a slice consisting of cards in the opening hand
  #[inline]
  pub fn opening_with_draws(&self, draws: usize) -> &[SimCard] {
    self.slice(0, self.opening_hand_size + draws)
  }

  /// Returns the total number of cards in hand
  pub fn len(&self) -> usize {
    self.cards.len()
  }

  /// Returns true if any card in the opening hand and draws satisfies the predicate
  pub fn any_in_opening_with_draws<P>(&self, draws: usize, p: P) -> bool
  where
    P: FnMut(&SimCard) -> bool,
  {
    self.opening_with_draws(draws).iter().any(p)
  }

  /// Returns the number of cards in the opening hand and draws that satisfies the predicate
  pub fn count_in_opening_with_draws<P>(&self, draws: usize, p: P) -> usize
  where
    P: Fn(&SimCard) -> bool,
  {
    self
      .opening_with_draws(draws)
      .iter()
      .fold(0, |count, card| if p(card) { count + 1 } else { count })
  }

  #[inline]
  fn slice(&self, from: usize, to: usize) -> &[SimCard] {
    let to = std::cmp::min(to, self.cards.len());
    unsafe { &self.cards.get_unchecked(from..to) }
  }

  /// The actual `auto_tap` implementation that exposes
  /// the scratch space data structure for performance purposes
  pub fn auto_tap_with_scratch<'a>(
    &'a self,
    goal: &SimCard,
    turn_count: usize,
    play_order: PlayOrder,
    scratch: &mut Vec<&'a SimCard>,
  ) -> AutoTapResult {
    // The implementation attempts to get goal_mana_cost.cmc() == 0
    // by tapping land cards for the appropriate colors
    let mut r = goal.mana_cost.r;
    let mut g = goal.mana_cost.g;
    let mut b = goal.mana_cost.b;
    let mut u = goal.mana_cost.u;
    let mut w = goal.mana_cost.w;
    let mut c = goal.mana_cost.c;
    let draw_count = match play_order {
      PlayOrder::First => turn_count - 1,
      PlayOrder::Second => turn_count,
    };

    let opening_hand = self.opening();
    let draws = self.draws(draw_count);

    // Populate scratch
    scratch.clear();

    // Iterate through opening_hand, add lands to scratch,
    // and return if the goal is found in the opening hand
    let goal_in_opening_hand = {
      let mut found = false;
      for card in opening_hand {
        if card.kind.is_land() {
          scratch.push(card);
        }
        if card.hash == goal.hash {
          found = true;
        }
      }
      found
    };

    // Iterate through draws, add lands to scratch,
    // and return if the goal is found in the drawn cards
    let goal_in_draws = {
      let mut found = false;
      for card in draws {
        if card.kind.is_land() {
          scratch.push(card);
        }
        if card.hash == goal.hash {
          found = true;
        }
      }
      found
    };

    let land_count = scratch.len();

    // Early exit if we don't have CMC lands
    if land_count < (r + g + b + u + w + c) as usize {
      return AutoTapResult {
        paid: false,
        cmc: false,
        in_opening_hand: goal_in_opening_hand,
        in_draw_hand: goal_in_draws,
      };
    }

    // GREEDY ALGORITHM: STEP 1
    // We want to tap lands that contribute the least to the mana cost first,
    // i.e. basic lands and dual lands that only contribute one relevant mana
    scratch.sort_unstable_by_key(|land| land.mana_cost.color_contribution(&goal.mana_cost));

    // Now, iterate through the lands in our scratch space
    for (i, land) in scratch.iter().enumerate() {
      let remaining = r + g + b + u + w + c;
      // The cost is paid -- break!
      if remaining == 0 {
        break;
      }
      let land_mana = &land.mana_cost;
      // GREEDY ALGORITHM: STEP 2
      // Sort by the least mana color available followed by the largest mana color cost remaining to pay
      let mut color_order = [
        (ManaColor::Red, 0, -(r as i16)),
        (ManaColor::Green, 0, -(g as i16)),
        (ManaColor::Black, 0, -(b as i16)),
        (ManaColor::Blue, 0, -(u as i16)),
        (ManaColor::White, 0, -(w as i16)),
        (ManaColor::Colorless, 0, 0),
      ];
      for remaining_land in &scratch[i..] {
        let land_mana = &remaining_land.mana_cost;
        color_order[0].1 += land_mana.r;
        color_order[1].1 += land_mana.g;
        color_order[2].1 += land_mana.b;
        color_order[3].1 += land_mana.u;
        color_order[4].1 += land_mana.w;
      }
      color_order[..=4].sort_unstable_by_key(|c| (c.1, c.2));

      let tap_for_r = land_mana.r != 0 && r != 0;
      let tap_for_g = land_mana.g != 0 && g != 0;
      let tap_for_b = land_mana.b != 0 && b != 0;
      let tap_for_u = land_mana.u != 0 && u != 0;
      let tap_for_w = land_mana.w != 0 && w != 0;
      let tap_for_c = c != 0;
      for (color, _, _) in &color_order {
        match color {
          ManaColor::Red => {
            if tap_for_r {
              r -= 1;
              break;
            }
          }
          ManaColor::Green => {
            if tap_for_g {
              g -= 1;
              break;
            }
          }
          ManaColor::Black => {
            if tap_for_b {
              b -= 1;
              break;
            }
          }
          ManaColor::Blue => {
            if tap_for_u {
              u -= 1;
              break;
            }
          }
          ManaColor::White => {
            if tap_for_w {
              w -= 1;
              break;
            }
          }
          ManaColor::Colorless => {
            if tap_for_c {
              c -= 1;
              break;
            }
          }
        }
      }
    }

    // OK! We've considered all of our lands -- did we pay the cost?
    let paid = r + g + b + u + w + c == 0;
    AutoTapResult {
      paid,
      cmc: true,
      in_opening_hand: goal_in_opening_hand,
      in_draw_hand: goal_in_draws,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::card::*;
  use crate::hand::*;

  #[test]
  fn cards_can_pay_0() {
    let card = card!("Adeliz, the Cinder Wind");
    let opening = vec![
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_1() {
    let card = card!("Adeliz, the Cinder Wind");
    let opening = vec![card!("Forest"), card!("Forest"), card!("Sulfur Falls")];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_2() {
    let card = card!("Adeliz, the Cinder Wind");
    let opening = vec![
      card!("Woodland Cemetery"),
      card!("Woodland Cemetery"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_3() {
    let card = card!("Adeliz, the Cinder Wind");
    let opening = vec![
      card!("Detection Tower"),
      card!("Steam Vents"),
      card!("Steam Vents"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_4() {
    let card = card!("Adeliz, the Cinder Wind");
    let opening = vec![
      card!("Detection Tower"),
      card!("Clifftop Retreat"),
      card!("Steam Vents"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_5() {
    let card = card!("Adeliz, the Cinder Wind");
    let opening = vec![
      card!("Detection Tower"),
      card!("Steam Vents"),
      card!("Clifftop Retreat"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_6() {
    let card = card!("Nicol Bolas, the Ravager");
    let opening = vec![
      card!("Detection Tower"),
      card!("Island"),
      card!("Dragonskull Summit"),
      card!("Steam Vents"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_7() {
    let card = card!("Nicol Bolas, the Ravager");
    let opening = vec![
      card!("Sulfur Falls"),
      card!("Dragonskull Summit"),
      card!("Dragonskull Summit"),
      card!("Blood Crypt"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_8() {
    let card = card!("History of Benalia");
    let opening = vec![
      card!("Plains"),
      card!("Sacred Foundry"),
      card!("Detection Tower"),
      card!("Dragonskull Summit"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_9() {
    let card = card!("Niv-Mizzet, Parun");
    let opening = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_10() {
    let card = card!("Niv-Mizzet, Parun");
    let opening = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Dragonskull Summit"),
      card!("Dragonskull Summit"),
      card!("Blood Crypt"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_10_1() {
    let card = card!("Niv-Mizzet, Parun");
    let opening = vec![
      card!("Drowned Catacomb"),
      card!("Drowned Catacomb"),
      card!("Steam Vents"),
      card!("Sulfur Falls"),
      card!("Steam Vents"),
      card!("Dragonskull Summit"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_2() {
    let card = card!("Niv-Mizzet, Parun");
    let opening = vec![
      card!("Steam Vents"),
      card!("Steam Vents"),
      card!("Dragonskull Summit"),
      card!("Drowned Catacomb"),
      card!("Blood Crypt"),
      card!("Watery Grave"),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_3() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Island"), card!("Watery Grave"), card!("Steam Vents")];
    let draws = vec![
      card!("Mountain"),
      card!("Sulfur Falls"),
      card!("Dragonskull Summit"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
  }

  #[test]
  fn cards_can_pay_10_4() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Island"), card!("Watery Grave"), card!("Steam Vents")];
    let draws = vec![
      card!("Mountain"),
      card!("Detection Tower"),
      card!("Dragonskull Summit"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_10_5() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Island"), card!("Watery Grave"), card!("Steam Vents")];
    let draws = vec![
      card!("Mountain"),
      card!("Sulfur Falls"),
      card!("Memorial to Folly"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_10_6() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Island"), card!("Watery Grave"), card!("Steam Vents")];
    let draws = vec![
      card!("Mountain"),
      card!("Sulfur Falls"),
      card!("Highland Lake"),
      card!("Memorial to Folly"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_7() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Island"), card!("Watery Grave"), card!("Steam Vents")];
    let draws = vec![
      card!("Mountain"),
      card!("Sulfur Falls"),
      card!("Dimir Guildgate"),
      card!("Memorial to Folly"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_8() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Island"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_9() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Island"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_10() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_11() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Dimir Guildgate"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    // We start w/ 3 cards in our hand, draw 3 cards by turn 3, and by turn CMC 6
    // should have played all this mana so we can
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn cards_can_pay_10_11_0() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Dimir Guildgate"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn cards_can_pay_10_12() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Steam Vents"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_13() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Steam Vents"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_14() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Dimir Guildgate"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_15() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Dimir Guildgate"),
      card!("Steam Vents"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_16() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Steam Vents"),
      card!("Steam Vents"),
      card!("Steam Vents"),
    ];
    let draws = vec![
      card!("Steam Vents"),
      card!("Steam Vents"),
      card!("Sulfur Falls"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_17() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Mountain"), card!("Mountain"), card!("Sulfur Falls")];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Mountain"),
      card!("Drowned Catacomb"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_18() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![card!("Mountain"), card!("Mountain"), card!("Sulfur Falls")];
    let draws = vec![
      card!("Sulfur Falls"),
      card!("Drowned Catacomb"),
      card!("Mountain"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn cards_can_pay_10_19() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Mountain"),
      card!("Island"),
      card!("Drowned Catacomb"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_21() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
      card!("Sulfur Falls"),
    ];
    let draws = vec![
      card!("Mountain"),
      card!("Mountain"),
      card!("Drowned Catacomb"),
      card!("Island"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_11() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Watery Grave"),
      card!("Watery Grave"),
      card!("Watery Grave"),
      card!("Dragonskull Summit"),
      card!("Dragonskull Summit"),
      card!("Dragonskull Summit"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_12() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Steam Vents"),
      card!("Mountain"),
      card!("Drowned Catacomb"),
      card!("Watery Grave"),
      card!("Steam Vents"),
      card!("Blood Crypt"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_13() {
    let card = card!("Cast Down");
    let lands = vec![card!("Detection Tower"), card!("Watery Grave")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_14() {
    let card = card!("Cast Down");
    let lands = vec![card!("Watery Grave"), card!("Watery Grave")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_15() {
    let card = card!("Cast Down");
    let lands = vec![card!("Watery Grave")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_16() {
    let card = card!("Cast Down");
    let lands = vec![card!("Swamp"), card!("Sulfur Falls")];
    // Always play Sulfur Falls t1, Swamp t2
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_17() {
    let card = card!("Cast Down");
    let lands = vec![card!("Swamp"), card!("Watery Grave")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_18() {
    let card = card!("Cast Down");
    let lands = vec![card!("Detection Tower"), card!("Watery Grave")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_19() {
    let card = card!("Cast Down");
    let lands = vec![card!("Detection Tower"), card!("Sulfur Falls")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_20() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Steam Vents"),
      card!("Steam Vents"),
      card!("Steam Vents"),
      card!("Blood Crypt"),
      card!("Blood Crypt"),
      card!("Blood Crypt"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_21() {
    let card = card!("Niv-Mizzet, Parun");
    let lands = vec![
      card!("Plains"),
      card!("Steam Vents"),
      card!("Steam Vents"),
      card!("Blood Crypt"),
      card!("Blood Crypt"),
      card!("Blood Crypt"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_22() {
    let card = card!("Appetite For Brains");
    let lands = vec![card!("Memorial to Folly")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
  }

  #[test]
  fn cards_can_pay_23() {
    let card = card!("Darksteel Colossus");
    let lands = vec![
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
    ];
    let draws = vec![
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Detection Tower"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_24() {
    let card = card!("Darksteel Colossus");
    let lands = vec![
      card!("Detection Tower"),
      card!("Swamp"),
      card!("Detection Tower"),
      card!("Detection Tower"),
      card!("Swamp"),
      card!("Swamp"),
      card!("Detection Tower"),
    ];
    let draws = vec![
      card!("Detection Tower"),
      card!("Swamp"),
      card!("Detection Tower"),
      card!("Swamp"),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn colorless_0() {
    let card = card!("The Immortal Sun");
    let land = card!("Boros Guildgate");
    let draws = vec![land, land, land, land, land, land];
    let hand = Hand::from_opening_and_draws(&[], &draws);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn colorless_1() {
    let card = card!("The Immortal Sun");
    let land = card!("Steam Vents");
    let lands = vec![land, land, land, land, land, land];
    let hand = Hand::from_opening_and_draws(&[], &lands);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn colorless_2() {
    // checklands are ignored
    let card = card!("The Immortal Sun");
    let land = card!("Sulfur Falls");
    let lands = vec![land, land, land, land, land, land];
    let hand = Hand::from_opening_and_draws(&[], &lands);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  //
  #[test]
  fn on_the_play() {
    let card = card!("Opt");
    let lands = vec![];
    let draws = vec![card!("Island")];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, false);
  }

  #[test]
  fn on_the_draw() {
    let card = card!("Opt");
    let lands = vec![];
    let draws = vec![card!("Island")];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.draw_cmc_auto_tap(card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
  }

  // shockland
  #[test]
  fn shock_land_0() {
    let card = card!("Appetite For Brains");
    let lands = vec![card!("Overgrown Tomb")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn empty_0() {
    let hand = Hand::from_opening_and_draws(&[], &[]);
    assert_eq!(hand.play_cmc_auto_tap(&Card::new()).paid, true);
  }

  #[test]
  fn empty_1() {
    let hand = Hand::from_opening_and_draws(&[], &[]);
    let mut card = Card::new();
    card.mana_cost.r = 1;
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn zero_mana_card_0() {
    let card = card!("Ancestral Vision");
    let opening = &[card];
    let hand = Hand::from_opening_and_draws(opening, &[]);
    let obs = hand.play_cmc_auto_tap(&card);
    assert_eq!(obs.paid, true);
    assert_eq!(obs.cmc, true);
  }

  #[test]
  fn yarok_test_0() {
    let card = card!("Yarok, the Desecrated");
    let h = vec![
      card!("Mountain"),
      card!("Mountain"),
      card!("Waterlogged Grove"),
      card!("Watery Grave"),
      card!("Overgrown Tomb"),
    ];
    let hand = Hand::from_opening_and_draws(&h, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }
}
