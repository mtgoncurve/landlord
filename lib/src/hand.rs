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
    let mut goal_mana_cost = goal.mana_cost;
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
    if land_count < goal_mana_cost.cmc() as usize {
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
    scratch.sort_unstable_by_key(|land| land.mana_cost.color_contribution(&goal_mana_cost));

    // Now, iterate through the lands in our scratch space
    for (i, land) in scratch.iter().enumerate() {
      let remaining = goal_mana_cost.cmc();
      // The cost is paid -- break!
      if remaining == 0 {
        break;
      }
      let land_mana = &land.mana_cost;
      // GREEDY ALGORITHM: STEP 2
      // Sort by the least mana color available followed by the largest mana color cost remaining to pay
      let mut color_order = [
        (ManaColor::Red, 0, -(goal_mana_cost.r as i16)),
        (ManaColor::Green, 0, -(goal_mana_cost.g as i16)),
        (ManaColor::Black, 0, -(goal_mana_cost.b as i16)),
        (ManaColor::Blue, 0, -(goal_mana_cost.u as i16)),
        (ManaColor::White, 0, -(goal_mana_cost.w as i16)),
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

      for (color, _, _) in &color_order {
        match color {
          ManaColor::Red => {
            let tap_for_r = land_mana.r != 0 && goal_mana_cost.r != 0;
            if tap_for_r {
              goal_mana_cost.r -= 1;
              break;
            }
          }
          ManaColor::Green => {
            let tap_for_g = land_mana.g != 0 && goal_mana_cost.g != 0;
            if tap_for_g {
              goal_mana_cost.g -= 1;
              break;
            }
          }
          ManaColor::Black => {
            let tap_for_b = land_mana.b != 0 && goal_mana_cost.b != 0;
            if tap_for_b {
              goal_mana_cost.b -= 1;
              break;
            }
          }
          ManaColor::Blue => {
            let tap_for_u = land_mana.u != 0 && goal_mana_cost.u != 0;
            if tap_for_u {
              goal_mana_cost.u -= 1;
              break;
            }
          }
          ManaColor::White => {
            let tap_for_w = land_mana.w != 0 && goal_mana_cost.w != 0;
            if tap_for_w {
              goal_mana_cost.w -= 1;
              break;
            }
          }
          ManaColor::Colorless => {
            let tap_for_c = goal_mana_cost.c != 0;
            if tap_for_c {
              goal_mana_cost.c -= 1;
              break;
            }
          }
        }
      }
    }

    // OK! We've considered all of our lands -- did we pay the cost?
    let paid = goal_mana_cost.cmc() == 0;
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
  use crate::card::Collection;
  use crate::hand::*;

  lazy_static! {
    static ref ALL_CARDS: Collection = Collection::all().expect("Collection::all failed");
  }

  #[test]
  fn cards_can_pay_0() {
    let card = ALL_CARDS
      .card_from_name("Adeliz, the Cinder Wind")
      .expect("Card named \"Adeliz, the Cinder Wind\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Sulfur Falls\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_1() {
    let card = ALL_CARDS
      .card_from_name("Adeliz, the Cinder Wind")
      .expect("Card named \"Adeliz, the Cinder Wind\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Forest")
        .expect("Card named \"Forest\""),
      ALL_CARDS
        .card_from_name("Forest")
        .expect("Card named \"Forest\""),
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Sulfur Falls\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_2() {
    let card = ALL_CARDS
      .card_from_name("Adeliz, the Cinder Wind")
      .expect("Card named \"Adeliz, the Cinder Wind\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Woodland Cemetery")
        .expect("Card named \"Woodland Cemetery\""),
      ALL_CARDS
        .card_from_name("Woodland Cemetery")
        .expect("Card named \"Woodland Cemetery\""),
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Sulfur Falls\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_3() {
    let card = ALL_CARDS
      .card_from_name("Adeliz, the Cinder Wind")
      .expect("Card named \"Adeliz, the Cinder Wind\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_4() {
    let card = ALL_CARDS
      .card_from_name("Adeliz, the Cinder Wind")
      .expect("Card named \"Adeliz, the Cinder Wind\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Clifftop Retreat")
        .expect("Card named \"Clifftop Retreat\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_5() {
    let card = ALL_CARDS
      .card_from_name("Adeliz, the Cinder Wind")
      .expect("Card named \"Adeliz, the Cinder Wind\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Clifftop Retreat")
        .expect("Card named \"Clifftop Retreat\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_6() {
    let card = ALL_CARDS
      .card_from_name("Nicol Bolas, the Ravager")
      .expect("Card named \"Nicol Bolas, the Ravager\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Island")
        .expect("Card named \"Island\""),
      ALL_CARDS
        .card_from_name("Dragonskull Summit")
        .expect("Card named \"Dragonskull Summit\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_7() {
    let card = ALL_CARDS
      .card_from_name("Nicol Bolas, the Ravager")
      .expect("Card named \"Nicol Bolas, the Ravager\"");
    let opening = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
      ALL_CARDS.card_from_name("Blood Crypt").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_8() {
    let card = ALL_CARDS
      .card_from_name("History of Benalia")
      .expect("Card named \"History of Benalia\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Plains")
        .expect("Card named \"Plains\""),
      ALL_CARDS
        .card_from_name("Sacred Foundry")
        .expect("Card named \"Sacred Foundry\""),
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Dragonskull Summit")
        .expect("Card named \"Dragonskull Summit\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_9() {
    let card = ALL_CARDS
      .card_from_name("Niv-Mizzet, Parun")
      .expect("Card named \"Niv-Mizzet, Parun\"");
    let opening = vec![
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Sulfur Falls\""),
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Sulfur Falls\""),
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Sulfur Falls\""),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_10() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
      ALL_CARDS.card_from_name("Blood Crypt").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_10_1() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_2() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
      ALL_CARDS.card_from_name("Blood Crypt").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_3() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
  }

  #[test]
  fn cards_can_pay_10_4() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Dragonskull Summit").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_10_5() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Memorial to Folly").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_10_6() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Highland Lake").unwrap(),
      ALL_CARDS.card_from_name("Memorial to Folly").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_7() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dimir Guildgate").unwrap(),
      ALL_CARDS.card_from_name("Memorial to Folly").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_8() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Island").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_9() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_10() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_11() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dimir Guildgate").unwrap(),
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
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dimir Guildgate").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn cards_can_pay_10_12() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_13() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_14() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dimir Guildgate").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_15() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Dimir Guildgate").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_16() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_17() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_18() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
      ALL_CARDS.card_from_name("Mountain").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn cards_can_pay_10_19() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_10_21() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Drowned Catacomb").unwrap(),
      ALL_CARDS.card_from_name("Island").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
  }

  #[test]
  fn cards_can_pay_11() {
    let card = ALL_CARDS
      .card_from_name("Niv-Mizzet, Parun")
      .expect("Card named \"Niv-Mizzet, Parun\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
      ALL_CARDS
        .card_from_name("Dragonskull Summit")
        .expect("Card named \"Dragonskull Summit\""),
      ALL_CARDS
        .card_from_name("Dragonskull Summit")
        .expect("Card named \"Dragonskull Summit\""),
      ALL_CARDS
        .card_from_name("Dragonskull Summit")
        .expect("Card named \"Dragonskull Summit\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_12() {
    let card = ALL_CARDS
      .card_from_name("Niv-Mizzet, Parun")
      .expect("Card named \"Niv-Mizzet, Parun\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Mountain")
        .expect("Card named \"Mountain \""),
      ALL_CARDS
        .card_from_name("Drowned Catacomb")
        .expect("Card named \"Drowned Catacomb\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Blood Crypt")
        .expect("Card named \"Blood Crypt\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_13() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Detection Tower\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_14() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Plains\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_15() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![ALL_CARDS
      .card_from_name("Watery Grave")
      .expect("Card named \"Watery Grave\"")];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_16() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Sulfur Falls").unwrap(),
    ];
    // Always play Sulfur Falls t1, Swamp t2
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_17() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Swamp")
        .expect("Card named \"Swamp\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_18() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Sulfur Falls\""),
      ALL_CARDS
        .card_from_name("Watery Grave")
        .expect("Card named \"Watery Grave\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_19() {
    let card = ALL_CARDS
      .card_from_name("Cast Down")
      .expect("Card named \"Cast Down\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Detection Tower")
        .expect("Card named \"Sulfur Falls\""),
      ALL_CARDS
        .card_from_name("Sulfur Falls")
        .expect("Card named \"Watery Grave\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_20() {
    let card = ALL_CARDS
      .card_from_name("Niv-Mizzet, Parun")
      .expect("Card named \"Niv-Mizzet, Parun\"");
    let lands = vec![
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Steam Vents")
        .expect("Card named \"Steam Vents\""),
      ALL_CARDS
        .card_from_name("Blood Crypt")
        .expect("Card named \"Blood Crypt\""),
      ALL_CARDS
        .card_from_name("Blood Crypt")
        .expect("Card named \"Blood Crypt\""),
      ALL_CARDS
        .card_from_name("Blood Crypt")
        .expect("Card named \"Blood Crypt\""),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_21() {
    let card = ALL_CARDS.card_from_name("Niv-Mizzet, Parun").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Plains").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Steam Vents").unwrap(),
      ALL_CARDS.card_from_name("Blood Crypt").unwrap(),
      ALL_CARDS.card_from_name("Blood Crypt").unwrap(),
      ALL_CARDS.card_from_name("Blood Crypt").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, false);
  }

  #[test]
  fn cards_can_pay_22() {
    let card = ALL_CARDS.card_from_name("Appetite For Brains").unwrap();
    let lands = vec![ALL_CARDS.card_from_name("Memorial to Folly").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
  }

  #[test]
  fn cards_can_pay_23() {
    let card = ALL_CARDS.card_from_name("Darksteel Colossus").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn cards_can_pay_24() {
    let card = ALL_CARDS.card_from_name("Darksteel Colossus").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
    ];
    let draws = vec![
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Detection Tower").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    assert_eq!(hand.play_cmc_auto_tap(&card).paid, true);
  }

  #[test]
  fn colorless_0() {
    let card = ALL_CARDS.card_from_name("The Immortal Sun").unwrap();
    let land = ALL_CARDS.card_from_name("Boros Guildgate").unwrap();
    let draws = vec![land, land, land, land, land, land];
    let hand = Hand::from_opening_and_draws(&[], &draws);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn colorless_1() {
    let card = ALL_CARDS.card_from_name("The Immortal Sun").unwrap();
    let land = ALL_CARDS.card_from_name("Steam Vents").unwrap();
    let lands = vec![land, land, land, land, land, land];
    let hand = Hand::from_opening_and_draws(&[], &lands);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  #[test]
  fn colorless_2() {
    // checklands are ignored
    let card = ALL_CARDS.card_from_name("The Immortal Sun").unwrap();
    let land = ALL_CARDS.card_from_name("Sulfur Falls").unwrap();
    let lands = vec![land, land, land, land, land, land];
    let hand = Hand::from_opening_and_draws(&[], &lands);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }

  //
  #[test]
  fn on_the_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let lands = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Island").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, false);
  }

  #[test]
  fn on_the_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let lands = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Island").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.draw_cmc_auto_tap(card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
  }

  // shockland
  #[test]
  fn shock_land_0() {
    let card = ALL_CARDS.card_from_name("Appetite For Brains").unwrap();
    let lands = vec![ALL_CARDS.card_from_name("Overgrown Tomb").unwrap()];
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
    let card = ALL_CARDS.card_from_name("Ancestral Vision").unwrap();
    let opening = &[card];
    let hand = Hand::from_opening_and_draws(opening, &[]);
    let obs = hand.play_cmc_auto_tap(&card);
    assert_eq!(obs.paid, true);
    assert_eq!(obs.cmc, true);
  }

  #[test]
  fn yarok_test_0() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let h = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Waterlogged Grove").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&h, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }
}
