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
  /// True if paid is false, cmc is true, and paid is false due to a tap land condition
  pub tapped: bool,
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
  ///     that plays all the correct lands on previous turns in order to pay for the goal card in question. Care must be taken
  ///     when handling conditional land cards that come into the game conditionally untapped, such as tap lands (see below).
  /// 2. For each sorted land, tap for the color that we have the least available of followed by the color that has the most remaining to pay.
  ///   - This last clause helps break ties. For instance, if we have a total of 2 green and 2 blue mana available, and '{G}{G}{U}'
  ///     remaining to pay, and we come across a land that can tap for green or blue, we would tap it for green.
  ///     If we only had 1 blue mana available, we would instead tap for blue due to the first clause taking precedence.
  ///
  /// The algorithm requires a few modifications to handle tap lands properly:
  ///   - A tap land drawn on the last draw turn cannot be sorted in step 1 above.
  ///   - The step 1 sorting does not consider land types, so situations arise
  ///     where the last card played on the draw turn is a tap land, when it could have been possible
  ///     to swap the play order with a non-conditional land played during an earlier turn. The
  ///     algorithm implements special code to handle these situations.
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
    let goal_turn = turn_count;
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

    // demarcate the number of lands observed in the
    // opening hand, before continuing to push land
    // cards from draws into the scratch buffer
    let lands_in_opening_hand = scratch.len();

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
    let lands_in_draws = land_count - lands_in_opening_hand;

    // Early exit if we don't have CMC lands
    if land_count < goal_mana_cost.cmc() as usize {
      return AutoTapResult {
        paid: false,
        cmc: false,
        tapped: false,
        in_opening_hand: goal_in_opening_hand,
        in_draw_hand: goal_in_draws,
      };
    }

    // Determine if the last card drawn on turn CMC corresponds to a tap land
    // If draws < CMC, then technically we are in an invalid game state and decked ourselves
    // however, in order to get expected results from contrived decks we will handle it
    let last_card_drawn = draws.last();
    let last_card_drawn_is_tap_land =
      last_card_drawn.map_or(false, |card| card.kind == CardKind::TapLand);
    // Did we draw all of our cards after the cmc turn?
    let last_card_drawn_on_draw_count = draws.len() >= draw_count;
    // If the last card drawn was a tap land, we can not sort it
    // it must be considered last
    let do_not_sort_last_draw = last_card_drawn_is_tap_land && last_card_drawn_on_draw_count;
    let sort_to = if do_not_sort_last_draw {
      land_count - 1
    } else {
      land_count
    };

    // GREEDY ALGORITHM: STEP 1
    // We want to tap lands that contribute the least to the mana cost first,
    // i.e. basic lands and dual lands that only contribute one relevant mana
    (scratch[..sort_to])
      .sort_unstable_by_key(|land| land.mana_cost.color_contribution(&goal_mana_cost));

    // Now, iterate through the lands in our scratch space
    let mut maybe_tapped = false;
    let mut played_non_tap_land = false;
    let mut played_any_from_opening_hand = false;
    let mut skipped_turns = draws.len() - lands_in_draws;
    let mut turn = 1;
    for (i, land) in scratch.iter().enumerate() {
      let remaining = goal_mana_cost.cmc();
      // The cost is paid -- break!
      if remaining == 0 {
        break;
      }
      let land_mana = &land.mana_cost;
      let is_tap_land = land.kind == CardKind::TapLand;
      let is_from_opening_hand = i < lands_in_opening_hand;
      let is_from_draws = !is_from_opening_hand;
      // Tapland bookkeeping
      {
        let is_last_land = i == land_count - 1;
        let is_fixed_tap_land = is_last_land && do_not_sort_last_draw;
        let is_first_card_from_draws = i == lands_in_opening_hand;
        let no_t1_play_from_hand = is_first_card_from_draws && !played_any_from_opening_hand;

        // This one is subtle. See test tap_land_43_10
        if no_t1_play_from_hand && play_order == PlayOrder::First {
          turn += 1;
        }
        if is_fixed_tap_land {
          turn += skipped_turns;
        }

        // The actual tapland check
        let is_last_turn = turn >= goal_turn;
        // Stupid order can occur due to sorting scratch in step 1 above
        let stupid_order = !is_fixed_tap_land && played_non_tap_land;
        let tap_condition = is_tap_land & is_last_turn && !stupid_order;
        if tap_condition {
          // Note that this could be a tap land that doesn't contribute to the mana cost
          // In this case, we do not consider this a tapped true event, but rather
          // a cmc true, paid false event (i.e. wrong colors)
          maybe_tapped = land_mana.color_contribution(&goal_mana_cost) > 0 || goal_mana_cost.c > 0;
          continue;
        }
      }

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

      let mut tap_for_any = false;
      for (color, _, _) in &color_order {
        match color {
          ManaColor::Red => {
            let tap_for_r = land_mana.r != 0 && goal_mana_cost.r != 0;
            if tap_for_r {
              goal_mana_cost.r -= 1;
              tap_for_any = true;
              break;
            }
          }
          ManaColor::Green => {
            let tap_for_g = land_mana.g != 0 && goal_mana_cost.g != 0;
            if tap_for_g {
              goal_mana_cost.g -= 1;
              tap_for_any = true;
              break;
            }
          }
          ManaColor::Black => {
            let tap_for_b = land_mana.b != 0 && goal_mana_cost.b != 0;
            if tap_for_b {
              goal_mana_cost.b -= 1;
              tap_for_any = true;
              break;
            }
          }
          ManaColor::Blue => {
            let tap_for_u = land_mana.u != 0 && goal_mana_cost.u != 0;
            if tap_for_u {
              goal_mana_cost.u -= 1;
              tap_for_any = true;
              break;
            }
          }
          ManaColor::White => {
            let tap_for_w = land_mana.w != 0 && goal_mana_cost.w != 0;
            if tap_for_w {
              goal_mana_cost.w -= 1;
              tap_for_any = true;
              break;
            }
          }
          ManaColor::Colorless => {
            let tap_for_c = goal_mana_cost.c != 0;
            if tap_for_c {
              goal_mana_cost.c -= 1;
              tap_for_any = true;
              break;
            }
          }
        }
      }

      // Tapland bookkeeping
      {
        if tap_for_any {
          turn += 1;
        } else if is_from_draws {
          skipped_turns += 1;
        }
        played_any_from_opening_hand |= tap_for_any && is_from_opening_hand;
        played_non_tap_land |= tap_for_any && !is_tap_land;
      }
    }

    // OK! We've considered all of our lands -- did we pay the cost?
    let paid = goal_mana_cost.cmc() == 0;
    // We got tapped if we can't pay and we were maybe tapped above
    let tapped = !paid && maybe_tapped;
    AutoTapResult {
      paid,
      cmc: true,
      tapped,
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
    assert_eq!(result.tapped, false);
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
    assert_eq!(result.tapped, false);
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
    assert_eq!(result.tapped, false);
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
    assert_eq!(res.paid, false);
    assert_eq!(res.tapped, true);
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
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
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
    assert_eq!(result.tapped, false);
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
    assert_eq!(result.tapped, false);
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
    assert_eq!(res.tapped, false);
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
    assert_eq!(res.tapped, false);
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
    assert_eq!(result.tapped, false);
  }
  // tapland

  #[test]
  fn tap_land_0_play() {
    let card = ALL_CARDS.card_from_name("Appetite For Brains").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_0_draw() {
    let card = ALL_CARDS.card_from_name("Appetite For Brains").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_1() {
    let card = ALL_CARDS.card_from_name("Appetite For Brains").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let res = hand.play_cmc_auto_tap(card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  // order doesn't matter
  #[test]
  fn tap_land_2() {
    let card = ALL_CARDS.card_from_name("Appetite For Brains").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let res = hand.play_cmc_auto_tap(card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_39() {
    let card = ALL_CARDS.card_from_name("Bloodghast").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
      ALL_CARDS.card_from_name("Swamp").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let res = hand.play_cmc_auto_tap(card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_4() {
    let card = ALL_CARDS.card_from_name("Bloodghast").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_5() {
    let card = ALL_CARDS.card_from_name("Bloodghast").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Plains").unwrap(),
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_6() {
    let card = ALL_CARDS.card_from_name("Bloodghast").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Plains").unwrap(),
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
      ALL_CARDS.card_from_name("Plains").unwrap(),
    ];
    // Play Forsaken -> Swamp
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_7() {
    let card = ALL_CARDS.card_from_name("Bloodghast").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
      ALL_CARDS.card_from_name("Bloodghast").unwrap(),
    ];
    // Play Forsaken -> Swamp
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_7_sorted() {
    let card = ALL_CARDS.card_from_name("Bloodghast").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Swamp").unwrap(),
      ALL_CARDS.card_from_name("Bloodghast").unwrap(),
      ALL_CARDS.card_from_name("Forsaken Sanctuary").unwrap(),
    ];
    // Play Forsaken -> Swamp
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_8() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  #[test]
  fn tap_land_8_0() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let basic = ALL_CARDS.card_from_name("Island").unwrap();
    let land = ALL_CARDS.card_from_name("Swamp").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![card, basic, land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }
  #[test]
  fn tap_land_8_1() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let basic = ALL_CARDS.card_from_name("Island").unwrap();
    let land = ALL_CARDS.card_from_name("Swamp").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![basic, card, land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_8_2() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let basic = ALL_CARDS.card_from_name("Island").unwrap();
    let land = ALL_CARDS.card_from_name("Swamp").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![basic, tap, land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_8_3() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let _basic = ALL_CARDS.card_from_name("Island").unwrap();
    let land = ALL_CARDS.card_from_name("Swamp").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![tap, land, land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_8_4() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let _basic = ALL_CARDS.card_from_name("Island").unwrap();
    let land = ALL_CARDS.card_from_name("Swamp").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  #[test]
  fn tap_land_8_5() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let _basic = ALL_CARDS.card_from_name("Island").unwrap();
    let land = ALL_CARDS.card_from_name("Swamp").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![tap, land, land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  #[test]
  fn tap_land_9_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let _basic = ALL_CARDS.card_from_name("Island").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 1, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  #[test]
  fn tap_land_9_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let lands = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, false);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_0_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    // On the play, so draw one less
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, false);
    assert_eq!(res.tapped, false);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 2, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_0_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_1_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    // On the play, so draw one less
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_1_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_2_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let draws = vec![];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_2_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let draws = vec![];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_5_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Mountain").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    // On the play, so draw one less
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 2, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_5_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Mountain").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    // On the play, so draw one less
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_6_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Island").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
    let res = hand.auto_tap_by_turn(card, turn + 2, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn core_tap_test_6_draw() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Island").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    // On the play, so draw one less
    let turn = card.turn as usize;
    let res = hand.auto_tap_by_turn(card, turn, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
    let res = hand.auto_tap_by_turn(card, turn + 1, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  // We want to play opt on t1, and tap land cannot be tapped on the same turn
  #[test]
  fn tap_land_10() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  // We draw and play izzet guildgate on t1, and want to play opt on t2, so we have the mana
  #[test]
  fn tap_land_10_0_play() {
    let card: Card = ALL_CARDS.card_from_name("Opt").unwrap().clone();
    let opening = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, false);
    assert_eq!(res.tapped, false);
  }

  // We draw and play izzet guildgate on t1, and want to play opt on t2, so we have the mana
  #[test]
  fn tap_land_10_0_draw() {
    let card: Card = ALL_CARDS.card_from_name("Opt").unwrap().clone();
    let opening = vec![];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.draw_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  // We draw and play izzet guildgate on t1, and want to play opt on t2, so we have the mana
  #[test]
  fn tap_land_10_0() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_10_2_play() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Mountain").unwrap()];
    let draws = vec![ALL_CARDS.card_from_name("Izzet Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(&card, 2, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  // On the draw, draw 1 Mountain
  #[test]
  fn tap_land_10_3_0() {
    let card: Card = ALL_CARDS.card_from_name("Opt").unwrap().clone();
    let opening = vec![];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Izzet Guildgate").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.draw_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  // On the play, draw nothing
  #[test]
  fn tap_land_10_3_1() {
    let card: Card = ALL_CARDS.card_from_name("Opt").unwrap().clone();
    let opening = vec![];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Izzet Guildgate").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.play_cmc_auto_tap(&card);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, false);
    assert_eq!(res.tapped, false);
  }

  // Izzet guildgate drawn on t2, cannot be tapped for mana to play Opt on t2
  #[test]
  fn tap_land_10_4() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![];
    let draws = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Izzet Guildgate").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  #[test]
  fn tap_land_10_5() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Izzet Guildgate").unwrap(),
    ];
    let draws = vec![];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(res.paid, true);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }

  #[test]
  fn tap_land_10_6() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Izzet Guildgate").unwrap(),
    ];
    let draws = vec![];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 1, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }
  #[test]
  fn tap_land_10_7() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let mnt = ALL_CARDS.card_from_name("Mountain").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![mnt];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 1, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, false);
  }
  #[test]
  fn tap_land_10_8() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let mnt = ALL_CARDS.card_from_name("Mountain").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let opening = vec![mnt];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let res = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(res.paid, false);
    assert_eq!(res.cmc, true);
    assert_eq!(res.tapped, true);
  }

  #[test]
  fn tap_land_11() {
    let card = ALL_CARDS.card_from_name("Opt").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Boros Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  // https://mtgoncurve.com/?v0=eyJjb2RlIjoiMSBIZXJvIG9mIFByZWNpbmN0IE9uZVxuMiBCb3JvcyBHdWlsZGdhdGUiLCJvbl90aGVfcGxheSI6ZmFsc2UsImluaXRpYWxfaGFuZF9zaXplIjo3LCJtdWxsaWdhbl9kb3duX3RvIjowLCJtdWxsaWdhbl9vbl9sYW5kcyI6WzAsMSw2LDcsMiwzLDUsNF0sImNhcmRzX3RvX2tlZXAiOiIifQ%3D%3D#/
  #[test]
  fn tap_land_12() {
    let card = ALL_CARDS.card_from_name("Hero of Precinct One").unwrap();
    let opening = vec![
      ALL_CARDS.card_from_name("Boros Guildgate").unwrap(),
      ALL_CARDS.card_from_name("Boros Guildgate").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &[]);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  // no draws for t1
  #[test]
  fn tap_land_13_0() {
    let card = ALL_CARDS.card_from_name("Adorable Kitten").unwrap();
    let draws = vec![ALL_CARDS.card_from_name("Boros Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&[], &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  // 1 tap land draw for t1
  #[test]
  fn tap_land_13_1() {
    let card = ALL_CARDS.card_from_name("Adorable Kitten").unwrap();
    let draws = vec![ALL_CARDS.card_from_name("Boros Guildgate").unwrap()];
    let hand = Hand::from_opening_and_draws(&[], &draws);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_14() {
    let card = ALL_CARDS.card_from_name("Assassin's Trophy").unwrap(); // {B}{G}
    let lands = vec![
      ALL_CARDS.card_from_name("Forest").unwrap(), // G Basic
      ALL_CARDS.card_from_name("Isolated Chapel").unwrap(), // W/B Check
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_15() {
    let card = ALL_CARDS.card_from_name("Savage Knuckleblade").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Island").unwrap(),
      ALL_CARDS.card_from_name("Frontier Bivouac").unwrap(),
      ALL_CARDS.card_from_name("Frontier Bivouac").unwrap(),
      ALL_CARDS.card_from_name("Frontier Bivouac").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_16() {
    let card = ALL_CARDS.card_from_name("Boros Swiftblade").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Mountain").unwrap(),
      ALL_CARDS.card_from_name("Stone Quarry").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_17() {
    let card = ALL_CARDS.card_from_name("Boros Swiftblade").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Stone Quarry").unwrap(),
      ALL_CARDS.card_from_name("Plains").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_18() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![
      card,
      ALL_CARDS.card_from_name("Temple of Deceit").unwrap(),
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
    ];
    let draws = vec![ALL_CARDS.card_from_name("Fabled Passage").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_19() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![
      card,
      ALL_CARDS.card_from_name("Temple of Deceit").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
    ];
    let draws = vec![ALL_CARDS.card_from_name("Temple of Mystery").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_20() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![
      card,
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Temple of Deceit").unwrap(),
    ];
    let draws = vec![ALL_CARDS.card_from_name("Temple of Mystery").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_21() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![
      card,
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(),
    ];
    let draws = vec![ALL_CARDS.card_from_name("Temple of Deceit").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_22() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![
      card,
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(),
    ];
    let draws = vec![ALL_CARDS.card_from_name("Temple of Deceit").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_23() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      card,
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
    ];
    let draws = vec![ALL_CARDS.card_from_name("Temple of Deceit").unwrap()];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_24_0() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![ALL_CARDS.card_from_name("Fabled Passage").unwrap()];
    let draws = vec![
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(), // t2
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),   // t3
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(), // t4
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(), // t5, tapped
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_24_1() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let opening = vec![ALL_CARDS.card_from_name("Fabled Passage").unwrap()];
    let draws = vec![
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Thought Erasure").unwrap(),
      ALL_CARDS.card_from_name("Temple of Deceit").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  // All land cards drawn by t4, for t5 play -- not tapped
  #[test]
  fn tap_land_24_2() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![ALL_CARDS.card_from_name("Fabled Passage").unwrap()];
    let draws = vec![
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.draw_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_24_4() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![ALL_CARDS.card_from_name("Fabled Passage").unwrap()];
    let draws = vec![
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(), // t2
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),   // t3
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(), // t4
      ALL_CARDS.card_from_name("Temple of Mystery").unwrap(), // t5
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_24_3() {
    let card = ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap();
    let lands = vec![ALL_CARDS.card_from_name("Fabled Passage").unwrap()];
    let draws = vec![
      ALL_CARDS.card_from_name("Fabled Passage").unwrap(),
      ALL_CARDS.card_from_name("Watery Grave").unwrap(),
      ALL_CARDS.card_from_name("Overgrown Tomb").unwrap(),
    ];
    let hand = Hand::from_opening_and_draws(&lands, &draws);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_25_onthedraw() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_25_ontheplay() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_25_0_ontheplay() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, (card.turn + 1) as usize, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_25_0_onthedraw() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, (card.turn + 1) as usize, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_25_1_onthedraw() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card, land, land, land];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, card.turn as usize, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_25_1_ontheplay() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card, land, land, land];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, card.turn as usize, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_25_2_ontheplay() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, land, land, basic];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, card.turn as usize, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_25_3_ontheplay() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let land = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![land, land, land, basic];
    let mut draws = vec![];
    for _ in 0..59 {
      draws.push(land);
    }
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, card.turn as usize, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_26() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, basic];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_27() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_28() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card];
    let draws = vec![tap, basic];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(*&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }
  #[test]
  fn tap_land_29() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, basic, other, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_30() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, tap, basic, other];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_31() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, basic, basic, tap];
    //t1,   //t2,  // t3, //t4
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_32_0() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, basic, basic, other, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_32_1() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, basic, basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_32_2() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_32_3() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_32_4() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_32_5() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let opening = vec![card];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_33_0() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![other, basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_33_1() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_33_2() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_33_3() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_34_0() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  // never tapped, not enough lands
  #[test]
  fn tap_land_34_1() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  // never tapped, play t1 tap, t2 basic
  #[test]
  fn tap_land_34_2_0() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![tap, basic];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  // always tapped, draw tap on t2, the goal turn
  #[test]
  fn tap_land_34_2_1() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card];
    let draws = vec![basic, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }
  // never tapped, play t1 tap, t2 basic
  #[test]
  fn tap_land_34_2_2() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card, tap];
    let draws = vec![basic];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  // never tapped, play t1 tap, t2 basic
  #[test]
  fn tap_land_34_2_3() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let _other = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
    let opening = vec![card, basic, tap];
    let draws = vec![];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  // cannot swap, tapped (on the play)
  #[test]
  fn tap_land_34_2_4() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, basic];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(*&card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  // play t1 tap, t2 basic (on the draw)
  #[test]
  fn tap_land_34_2_5() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, basic];
    let draws = vec![tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.draw_cmc_auto_tap(card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_34_2_6() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, basic];
    let draws = vec![tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.play_cmc_auto_tap(card);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_35() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, basic];
    let draws = vec![card, card, card, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_36() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![card, basic];
    let draws = vec![card, card, card, tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_37() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![tap, basic];
    let draws = vec![card, card, card, card, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_38() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let _basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![tap, tap];
    let draws = vec![tap, card, card, card, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_40() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Cinder Barrens").unwrap();
    let _basic = ALL_CARDS.card_from_name("Mountain").unwrap();
    let opening = vec![tap];
    let draws = vec![tap, card, card, card, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_41_0() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![card, tap, swamp, plains];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, false);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_41_1() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![swamp, tap, card, plains];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_41_2() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![swamp, plains, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_41_3() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![tap, swamp, card, plains];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  // This is a color mismatch, rather than a tap event
  #[test]
  fn tap_land_41_4() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![plains, tap, card, swamp];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_41_5() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![plains, swamp, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_0() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let _plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![swamp];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_42_1() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let _plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![swamp];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 3, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_2() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let _plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![tap];
    let draws = vec![swamp];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_3() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let _plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![swamp, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 3, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_42_4() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let _plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![swamp, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_5() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Izzet Guildgate").unwrap();
    let swamp = ALL_CARDS.card_from_name("Swamp").unwrap();
    let _plains = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![swamp, tap];
    let draws = vec![];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_6() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Rakdos Guildgate").unwrap();
    let opening = vec![tap, tap];
    let draws = vec![tap, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_42_7() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Rakdos Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![];
    let draws = vec![tap, land, land, tap, land];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 5, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_8() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Rakdos Guildgate").unwrap();
    let opening = vec![];
    let draws = vec![tap, card, card, card, card, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 7, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_42_9() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Rakdos Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![tap, card, card, card, card, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 7, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_42_10() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Rakdos Guildgate").unwrap();
    let _land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![tap];
    let draws = vec![card, card, card, card, card, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 7, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_42_11() {
    let card = ALL_CARDS.card_from_name("Agonizing Remorse").unwrap();
    let tap = ALL_CARDS.card_from_name("Rakdos Guildgate").unwrap();
    let _land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![tap];
    let draws = vec![card, card, card, card, card, tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 7, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_0() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let _land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![tap];
    let draws = vec![tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_43_1() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let _land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![tap];
    let draws = vec![tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 3, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_2() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![land, tap];
    let draws = vec![card, card, land, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_43_3() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let _land = ALL_CARDS.card_from_name("Plains").unwrap();
    let opening = vec![tap];
    let draws = vec![tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_43_4() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Forest").unwrap();
    let opening = vec![];
    let draws = vec![card, land, tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_5() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Forest").unwrap();
    let opening = vec![];
    let draws = vec![land, tap, card, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_6() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Forest").unwrap();
    let opening = vec![];
    let draws = vec![card, land, tap, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_43_7() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Forest").unwrap();
    let opening = vec![land];
    let draws = vec![card, tap, land, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_8() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let land = ALL_CARDS.card_from_name("Forest").unwrap();
    let opening = vec![tap];
    let draws = vec![card, land, land, card];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_9() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 4, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_10() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 3, PlayOrder::First);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_11() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::First);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_43_12() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 3, PlayOrder::Second);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, false);
  }

  #[test]
  fn tap_land_43_13() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![card, card, tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 3, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
  }

  #[test]
  fn tap_land_43_14() {
    let card = ALL_CARDS.card_from_name("Growth Spiral").unwrap();
    let tap = ALL_CARDS.card_from_name("Simic Guildgate").unwrap();
    let opening = vec![tap];
    let draws = vec![tap];
    let hand = Hand::from_opening_and_draws(&opening, &draws);
    let result = hand.auto_tap_by_turn(card, 2, PlayOrder::Second);
    assert_eq!(result.paid, false);
    assert_eq!(result.cmc, true);
    assert_eq!(result.tapped, true);
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
    assert_eq!(obs.tapped, false);
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
    assert_eq!(result.tapped, false);
  }
}
