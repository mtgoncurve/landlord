//! # Simulation hands and auto tap algorithm
use crate::bipartite::maximum_bipartite_matching;
use crate::card::{Card, CardKind, ManaCost};
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

// Scratch space for the bipartite matching algorithm
// Used to reduce allocations at runtime
pub struct Scratch<'a> {
  lands: Vec<&'a SimCard>,
  edges: Vec<u8>,
  seen: Vec<bool>,
  matches: Vec<i32>,
}

impl<'a> Scratch<'a> {
  /// Returns a new Scratch object based on the number of land cards in a deck
  /// and the maximum pip count of any one card. It's OK if you guess wrong for
  /// these numbers, there will simply be one additional allocation to make up
  /// the difference.
  pub fn new(max_land_count: usize, max_pip_count: usize) -> Self {
    Self {
      lands: Vec::with_capacity(max_land_count),
      edges: vec![0; max_land_count * max_pip_count],
      seen: vec![false; max_land_count],
      matches: vec![-1; max_land_count],
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
    deck: &Vec<&Card>,
    draws: usize,
  ) -> Self {
    mulligan.simulate_hand(rng, deck, draws)
  }

  /// Returns the result of attempting to tap the `goal` card
  /// with the land cards in hand (`self`) by the `turn` given the `player_order`.
  ///
  /// Allocates a fresh `Scratch` object every call, which is useful
  /// for test cases. Call `auto_tap_with_scratch` directly to reuse
  /// a single `Scratch` object for performance.
  /// See `auto_tap_with_scratch` for the actual implementation details
  pub fn auto_tap_by_turn(
    &self,
    goal: &Card,
    turn: usize,
    player_order: PlayOrder,
  ) -> AutoTapResult {
    let mut scratch = Scratch::new(30, 8);
    let goal = SimCard {
      kind: goal.kind,
      hash: goal.hash,
      mana_cost: goal.mana_cost,
    };
    self.auto_tap_with_scratch(&goal, turn, player_order, &mut scratch)
  }

  /// Returns the result of attempting to tap the `goal` card
  /// with the land cards in hand (`self`) by the turn equal to the CMC of the goal card
  /// when playing first
  pub fn play_cmc_auto_tap(&self, goal: &Card) -> AutoTapResult {
    let turn = std::cmp::max(1, goal.turn) as usize;
    self.auto_tap_by_turn(goal, turn, PlayOrder::First)
  }

  /// Returns the result of attempting to tap the `goal` card
  /// with the land cards in hand (`self`) by the turn equal to the CMC of the goal card
  /// when playing second
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

  /// The actual auto_tap implementation that exposes
  /// the scratch space data structure for performance purposes.
  /// The implementation constructs a bipartite graph between the
  /// land cards in hand, and the mana pips of the goal card mana cost,
  /// and then attempts to find the size of the maximum matching set,
  /// see http://discrete.openmathbooks.org/dmoi2/sec_matchings.html.
  /// If the size of the maximum matching set is equal to the number
  /// of mana pips of the goal card mana cost, then the land cards in hand
  /// can successfully tap for the goal card.
  /// Kudos to user https://github.com/msg555 for the suggestion to model the
  /// problem as a bipartite matching problem (https://github.com/mtgoncurve/landlord/issues/16)
  pub fn auto_tap_with_scratch<'a>(
    &'a self,
    goal: &SimCard,
    turland_count: usize,
    play_order: PlayOrder,
    scratch: &mut Scratch<'a>,
  ) -> AutoTapResult {
    let draw_count = match play_order {
      PlayOrder::First => turland_count - 1,
      PlayOrder::Second => turland_count,
    };
    let opening_hand = self.opening();
    let draws = self.draws(draw_count);

    // Populate scratch lands
    scratch.lands.clear();

    // Iterate through opening_hand, add lands to scratch,
    // and return if the goal is found in the opening hand
    let in_opening_hand = {
      let mut found = false;
      for card in opening_hand {
        if card.kind.is_land() {
          scratch.lands.push(card);
        }
        if card.hash == goal.hash {
          found = true;
        }
      }
      found
    };

    // Iterate through draws, add lands to scratch,
    // and return if the goal is found in the drawn cards
    let in_draw_hand = {
      let mut found = false;
      for card in draws {
        if card.kind.is_land() {
          scratch.lands.push(card);
        }
        if card.hash == goal.hash {
          found = true;
        }
      }
      found
    };

    let pip_count = goal.mana_cost.cmc() as usize; // rows (height)
    let land_count = scratch.lands.len(); // columns (width)

    // Exit early if there aren't enough lands
    if land_count < pip_count {
      return AutoTapResult {
        paid: false,
        cmc: false,
        in_opening_hand,
        in_draw_hand,
      };
    }

    // Resize the scratch space data structures required
    // for the maximum bipartite matching algorithm
    scratch.edges.resize(pip_count * land_count, 0);
    scratch.seen.resize(land_count, false);
    scratch.matches.resize(land_count, -1);
    // Build the adjaceny matrix representing the bipartite
    // graph between land cards and the goal card mana cost pips
    let r_pips = goal.mana_cost.r as usize;
    let g_pips = goal.mana_cost.g as usize;
    let b_pips = goal.mana_cost.b as usize;
    let u_pips = goal.mana_cost.u as usize;
    let w_pips = goal.mana_cost.w as usize;
    let c_pips = goal.mana_cost.c as usize;
    let r_range = 0..r_pips;
    let g_range = r_range.end..(r_range.end + g_pips);
    let b_range = g_range.end..(g_range.end + b_pips);
    let u_range = b_range.end..(b_range.end + u_pips);
    let w_range = u_range.end..(u_range.end + w_pips);
    let c_range = w_range.end..(w_range.end + c_pips);
    for m in r_range {
      for (n, land) in scratch.lands.iter().enumerate() {
        scratch.edges[land_count * m + n] = land.mana_cost.r;
      }
    }
    for m in g_range {
      for (n, land) in scratch.lands.iter().enumerate() {
        scratch.edges[land_count * m + n] = land.mana_cost.g;
      }
    }
    for m in b_range {
      for (n, land) in scratch.lands.iter().enumerate() {
        scratch.edges[land_count * m + n] = land.mana_cost.b;
      }
    }
    for m in u_range {
      for (n, land) in scratch.lands.iter().enumerate() {
        scratch.edges[land_count * m + n] = land.mana_cost.u;
      }
    }
    for m in w_range {
      for (n, land) in scratch.lands.iter().enumerate() {
        scratch.edges[land_count * m + n] = land.mana_cost.w;
      }
    }
    for m in c_range {
      for (n, _) in scratch.lands.iter().enumerate() {
        scratch.edges[land_count * m + n] = 1;
      }
    }
    // Find the size of the maximum bipartite matching for
    // the graph. This corresponds to the number
    // of pips we can sucessfully pay with lands in hand
    let pips_paid = maximum_bipartite_matching(
      &scratch.edges,
      pip_count,
      land_count,
      &mut scratch.seen,
      &mut scratch.matches,
    );
    assert!(pips_paid <= pip_count);
    AutoTapResult {
      paid: pips_paid == pip_count,
      cmc: true,
      in_opening_hand,
      in_draw_hand,
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

  #[test]
  fn test_issue_16() {
    let mana_cost = ManaCost::from_rgbuwc(1, 1, 1, 2, 1, 0);
    let card = Card {
      mana_cost,
      all_mana_costs: vec![mana_cost],
      kind: CardKind::Creature,
      turn: mana_cost.cmc(),
      ..Default::default()
    };
    let h = vec![
      card!("Temple of Enlightenment"), // {W}{U}
      card!("Temple of Deceit"),        // {U}{B}
      card!("Temple of Deceit"),        // {U}{B}
      card!("Temple of Plenty"),        // {W}{G}
      card!("Temple of Abandon"),       // {R}{G}
      card!("Temple of Abandon"),       // {R}{G}
    ];
    let hand = Hand::from_opening_and_draws(&h, &[]);
    let result = hand.play_cmc_auto_tap(&card);
    assert_eq!(result.paid, true);
    assert_eq!(result.cmc, true);
  }
}
