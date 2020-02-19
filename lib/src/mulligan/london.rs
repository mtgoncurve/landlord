use crate::card::Card;
use crate::hand::Hand;
use crate::mulligan::Mulligan;
use rand::prelude::*;
use std::collections::HashSet;

const STARTING_HAND_SIZE: usize = 7;

/// London represents a mulligan strategy that adheres to the
/// [London mulligan rule](https://mtg.gamepedia.com/Mulligan#London_mulligan)
#[derive(Debug, Serialize, Deserialize)]
pub struct London {
  pub starting_hand_size: usize,
  pub mulligan_down_to: usize,
  /// A set of land counts on which to always mulligan
  pub mulligan_on_lands: HashSet<usize>,
  /// A list of card sets that represent keepable hands
  /// The card is represented by it's `u64` hash value
  pub acceptable_hand_list: Vec<HashSet<u64>>,
}

impl London {
  /// Returns a London mulligan strategy that never mulligans
  pub fn never() -> Self {
    Self {
      starting_hand_size: STARTING_HAND_SIZE,
      mulligan_down_to: STARTING_HAND_SIZE,
      mulligan_on_lands: HashSet::new(),
      acceptable_hand_list: Default::default(),
    }
  }

  /// Returns a London mulligan strategy that always mulligans
  /// down to `down_to` card count
  pub fn always(down_to: usize) -> Self {
    let mut mulligan_on_lands = HashSet::new();
    for i in 0..=STARTING_HAND_SIZE {
      mulligan_on_lands.insert(i);
    }
    Self {
      starting_hand_size: STARTING_HAND_SIZE,
      mulligan_down_to: down_to,
      mulligan_on_lands,
      acceptable_hand_list: Default::default(),
    }
  }
}

impl Mulligan for London {
  fn simulate_hand(&self, mut rng: &mut impl Rng, deck: &[Card], draws: usize) -> Hand {
    let deck_size = deck.len();

    // The number of cards to draw for the starting hand, capped by deck_size
    let starting_hand_size = std::cmp::min(self.starting_hand_size, deck_size);
    // The number of cards to mulligan down to, capped by starting_hand_size
    let mulligan_down_to = std::cmp::min(self.mulligan_down_to, starting_hand_size);
    assert!(starting_hand_size >= mulligan_down_to);
    // The maximum number of mulligan rounds to consider
    let max_mulligan_rounds = starting_hand_size - mulligan_down_to + 1;
    assert!(max_mulligan_rounds > 0);

    // Need to draw starting_hand_size cards plus the number of draws specified, capped by deck_size
    // We add max_mulligan_rounds more cards to act as the back of the deck for the london mulligan discard process
    let cards_to_draw = std::cmp::min(starting_hand_size + draws + max_mulligan_rounds, deck_size);

    // Create an index range to shuffle on rather than shuffling the immutable `deck` slice
    let mut index_range: Vec<_> = (0..deck_size).collect();

    // Data structures used across multiple mulligan rounds to reduce the number of allocations
    let mut must_keep_card_indices = Vec::with_capacity(starting_hand_size);
    let mut seen_card_hashes = HashSet::with_capacity(starting_hand_size);

    // Iterate through the mulligan rounds. Note that round == 0 is considered the first starting hand draw
    for round in 0..max_mulligan_rounds {
      // Rather than shuffle the entire deck, only consider cards_to_draw
      let mut shuffled_deck: Vec<_> = index_range
        .partial_shuffle(&mut rng, cards_to_draw)
        .0
        .iter()
        .map(|i| &deck[*i])
        .collect();
      // Starting hand consists of the first starting_hand_size cards
      let starting_hand = &mut shuffled_deck[..starting_hand_size];

      // Have to keep the hand if this is the last round
      let is_last_round = round == max_mulligan_rounds - 1;

      // Do we have a sufficient number of lands in our opening hand according to
      // the mulligan strategy?
      let land_count = starting_hand
        .iter()
        .fold(0, |accum, c| if c.is_land() { accum + 1 } else { accum });
      let sufficient_land_count = !self.mulligan_on_lands.contains(&land_count);
      // Is this not the last round? Not enough lands? Great -- onto the next round
      if !is_last_round && !sufficient_land_count {
        continue;
      }

      // Check if our opening hand contains a subset of cards matching
      // one of the sets specified in the mulligan mulligan
      // NOTE: It is OK to insert the same index multiple times into
      // must_keep_card_indices since we call dedup before using it
      let mut found_acceptable_hand = false;
      for acceptable_hand in &self.acceptable_hand_list {
        must_keep_card_indices.clear();
        seen_card_hashes.clear();
        for (i, card) in starting_hand.iter().enumerate() {
          if seen_card_hashes.contains(&card.hash) {
            continue;
          }

          if acceptable_hand.contains(&card.hash) {
            must_keep_card_indices.push(i);
          }
          seen_card_hashes.insert(card.hash);
        }
        found_acceptable_hand = must_keep_card_indices.len() == acceptable_hand.len();
        if found_acceptable_hand {
          break;
        }
      }

      // Can we keep the hand?
      let disregard_found_acceptable_hand = self.acceptable_hand_list.is_empty();
      let keep = is_last_round
        || (sufficient_land_count && (disregard_found_acceptable_hand || found_acceptable_hand));
      if keep {
        let opening_hand_size = starting_hand_size - round;
        // We can keep the hand! Let's update the must_keep_card_indices list
        // with some land cards to keep as well. Try to keep enough lands to
        // satisfy the mulligan strategy
        // NOTE This process does not attempt to keep any specific sort of land or color
        // NOTE Removing this land saving process causes test cases karsten_check_{1,2} to fail
        let mut lands_saved = 0;
        for (i, card) in starting_hand.iter().enumerate() {
          if !card.kind.is_land() {
            continue;
          }
          let need_more_lands =
            self.mulligan_on_lands.contains(&lands_saved) && lands_saved < opening_hand_size;
          if need_more_lands {
            must_keep_card_indices.push(i);
            lands_saved += 1;
          } else {
            break;
          }
        }

        // Now, we are going to sort shuffled_deck in such a way
        // that the cards we intend to keep occupy the first [0..starting_hand_size-round]
        // indices and the cards to discard to the back of the deck occupy the last
        // [starting_hand_size - round, starting_hand_size] indices

        // CARDS TO KEEP
        // Put the must keep cards at the front of shuffled_deck
        // NOTE the following code assumes i <= must_keep_i
        // therefore we need to sort must_keep_card_indices
        must_keep_card_indices.sort();
        must_keep_card_indices.dedup();
        for (i, must_keep_i) in must_keep_card_indices.iter().enumerate() {
          assert!(i <= *must_keep_i);
          shuffled_deck.swap(i, *must_keep_i);
        }

        // CARDS TO DISCARD
        // rather than discard to the back of the deck, we swap cards to discard
        // with cards at the end of our drawn cards (drawn_deck_size).
        // This is why we added max_mulligan_rounds in the cards_to_draw calculation above.
        for (discard_count, i) in (opening_hand_size..starting_hand_size).enumerate() {
          shuffled_deck.swap(i, cards_to_draw - 1 - discard_count);
        }
        return Hand::from_opening_and_draws(
          &shuffled_deck[..opening_hand_size],
          &shuffled_deck[opening_hand_size..],
        );
      }
    }
    unreachable!();
  }
}

#[cfg(test)]
mod tests {
  use crate::card::Collection;
  use crate::hand::*;
  use crate::mulligan::london::*;
  use crate::simulation::*;
  use std::collections::HashSet;

  lazy_static! {
    static ref ALL_CARDS: Collection = Collection::all().expect("Collection::all failed");
  }
  #[test]
  fn mulligan_discard_test_never() {
    let code = "
        1 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        1 Sinister Sabotage (GRN) 54
        1 Opt (XLN) 65
        1 Vraska's Contempt (XLN) 129
        1 Thought Erasure
        1 Cry of the Carnarium (RNA) 70
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 100;
    let draws = 0;
    // Ignore land requirements
    let mulligan = London::never();
    {
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      for hand in sim.hands {
        assert_eq!(hand.opening_hand_size, 7);
        assert_eq!(hand.mulligan_count, 0);
      }
    }
  }
  #[test]
  fn mulligan_discard_test_to_6() {
    let code = "
        1 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        1 Sinister Sabotage (GRN) 54
        1 Opt (XLN) 65
        1 Vraska's Contempt (XLN) 129
        1 Thought Erasure
        1 Cry of the Carnarium (RNA) 70
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(ALL_CARDS.card_from_name("Cleansing Nova").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Vraska, Relic Seeker")
          .unwrap()
          .hash,
      );
      hs.insert(ALL_CARDS.card_from_name("Sinister Sabotage").unwrap().hash);
      hs.insert(ALL_CARDS.card_from_name("Opt").unwrap().hash);
      hs.insert(ALL_CARDS.card_from_name("Vraska's Contempt").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Cry of the Carnarium")
          .unwrap()
          .hash,
      );
      vec![hs]
    };
    let runs = 100;
    let draws = 0;
    // Ignore land requirements
    let mut mulligan = London::always(6);
    mulligan.acceptable_hand_list = look_for;
    {
      let should_not_be_in_hand = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      for hand in sim.hands {
        let hand_contains_card = hand
          .opening()
          .iter()
          .any(|c| c.hash == should_not_be_in_hand.hash);
        assert!(!hand_contains_card);
        assert_eq!(hand.opening_hand_size, 6);
        assert_eq!(hand.mulligan_count, 1);
      }
    }
  }

  #[test]
  fn mulligan_discard_test_to_5() {
    let code = "
        1 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        1 Sinister Sabotage (GRN) 54
        1 Opt (XLN) 65
        1 Vraska's Contempt (XLN) 129
        1 Thought Erasure
        1 Cry of the Carnarium (RNA) 70
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(ALL_CARDS.card_from_name("Cleansing Nova").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Vraska, Relic Seeker")
          .unwrap()
          .hash,
      );
      hs.insert(ALL_CARDS.card_from_name("Sinister Sabotage").unwrap().hash);
      hs.insert(ALL_CARDS.card_from_name("Vraska's Contempt").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Cry of the Carnarium")
          .unwrap()
          .hash,
      );
      vec![hs]
    };
    let runs = 100;
    let draws = 0;
    // Ignore land requirements
    let mut mulligan = London::always(5);
    mulligan.acceptable_hand_list = look_for;
    {
      let bada = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
      let badb = ALL_CARDS.card_from_name("Opt").unwrap();
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      for hand in sim.hands {
        let hand_contains_cards = hand
          .opening()
          .iter()
          .any(|c| c.hash == bada.hash || c.hash == badb.hash);
        assert!(!hand_contains_cards);
        assert_eq!(hand.opening_hand_size, 5);
        assert_eq!(hand.mulligan_count, 2);
      }
    }
  }

  #[test]
  fn mulligan_discard_test_to_1() {
    let code = "
        1 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        1 Sinister Sabotage (GRN) 54
        1 Opt (XLN) 65
        1 Vraska's Contempt (XLN) 129
        1 Thought Erasure
        1 Cry of the Carnarium (RNA) 70
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(ALL_CARDS.card_from_name("Cleansing Nova").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Vraska, Relic Seeker")
          .unwrap()
          .hash,
      );
      hs.insert(ALL_CARDS.card_from_name("Sinister Sabotage").unwrap().hash);
      hs.insert(ALL_CARDS.card_from_name("Vraska's Contempt").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Cry of the Carnarium")
          .unwrap()
          .hash,
      );
      vec![hs]
    };
    let runs = 100;
    let draws = 0;
    // Ignore land requirements
    let mut mulligan = London::always(1);
    mulligan.acceptable_hand_list = look_for;
    {
      let bada = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
      let badb = ALL_CARDS.card_from_name("Opt").unwrap();
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      for hand in sim.hands {
        let hand_contains_cards = hand
          .opening()
          .iter()
          .any(|c| c.hash == bada.hash || c.hash == badb.hash);
        assert!(!hand_contains_cards);
        assert_eq!(hand.opening_hand_size, 1);
        assert_eq!(hand.mulligan_count, 6);
      }
    }
  }

  #[test]
  fn mulligan_discard_test_1() {
    let code = "
        1 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        1 Sinister Sabotage (GRN) 54
        1 Opt (XLN) 65
        1 Vraska's Contempt (XLN) 129
        1 Thought Erasure
        1 Cry of the Carnarium (RNA) 70
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(ALL_CARDS.card_from_name("Cleansing Nova").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Vraska, Relic Seeker")
          .unwrap()
          .hash,
      );
      hs.insert(ALL_CARDS.card_from_name("Sinister Sabotage").unwrap().hash);
      hs.insert(ALL_CARDS.card_from_name("Vraska's Contempt").unwrap().hash);
      hs.insert(
        ALL_CARDS
          .card_from_name("Cry of the Carnarium")
          .unwrap()
          .hash,
      );
      vec![hs]
    };
    let runs = 100;
    let draws = 0;
    // Ignore land requirements
    let mut mulligan = London::always(1);
    mulligan.acceptable_hand_list = look_for;
    {
      let bada = ALL_CARDS.card_from_name("Thought Erasure").unwrap();
      let badb = ALL_CARDS.card_from_name("Opt").unwrap();
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      for hand in sim.hands {
        let hand_contains_cards = hand
          .opening()
          .iter()
          .any(|c| c.hash == bada.hash || c.hash == badb.hash);
        assert!(!hand_contains_cards);
        assert_eq!(hand.opening_hand_size, 1);
        assert_eq!(hand.mulligan_count, 6);
      }
    }
  }

  #[test]
  fn mulligan_discard_test_to_0() {
    let code = "
        1 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        1 Sinister Sabotage (GRN) 54
        1 Opt (XLN) 65
        1 Vraska's Contempt (XLN) 129
        1 Thought Erasure
        1 Cry of the Carnarium (RNA) 70
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 100;
    let draws = 0;
    // Ignore land requirements
    let mulligan = London::always(0);
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &mulligan,
      deck: &deck,
      on_the_play: true,
    });
    for hand in sim.hands {
      assert_eq!(hand.opening_hand_size, 0);
      assert_eq!(hand.mulligan_count, 7);
    }
  }

  // Verify that our simulation can return results matched by various articles found by Frank
  // Karsten

  // Check the table under the section "How Often Will You Begin the Game with a Leyline on the
  // Battlefield?" in the article https://www.channelfireball.com/all-mulligan/articles/the-london-mulligan-rule-mathematically-benefits-strategies-that-rely-on-specific-cards/
  #[test]
  fn karsten_check_0() {
    let card = ALL_CARDS
      .card_from_name("Ornithopter")
      .expect("Card named \"Ornithopter\"");
    let other = ALL_CARDS
      .card_from_name("Mountain")
      .expect("Card named \"Ornithopter\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..4 {
      cards.push(card.clone());
    }
    for _ in 0..56 {
      cards.push(other.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(card.hash);
      vec![hs]
    };
    let runs = 30000;
    let draws = 0;
    // Ignore land requirements
    let mut mulligan = London::never();
    mulligan.acceptable_hand_list = look_for;
    // No mulligan
    {
      mulligan.mulligan_down_to = 7;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.399) < 0.01);
    }
    // Down to 6
    {
      mulligan.mulligan_down_to = 6;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.639) < 0.01);
    }
    // Down to 5
    {
      mulligan.mulligan_down_to = 5;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.783) < 0.01);
    }
    // Down to 4
    {
      mulligan.mulligan_down_to = 4;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.87) < 0.01);
    }
    // Down to 3
    {
      mulligan.mulligan_down_to = 3;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.922) < 0.01);
    }
    // Down to 2
    {
      mulligan.mulligan_down_to = 2;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.953) < 0.01);
    }
    // Down to 1
    {
      mulligan.mulligan_down_to = 1;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let obs = sim.observations_for_card(card);
      let p = obs.in_opening_hand as f64 / runs as f64;
      assert!(f64::abs(p - 0.972) < 0.01);
    }
  }

  // Verify table under section "How Often Will You Guarantee a Two Card Combo?"
  // https://www.channelfireball.com/all-mulligan/articles/the-london-mulligan-rule-mathematically-benefits-strategies-that-rely-on-specific-cards/
  #[test]
  fn karsten_check_1() {
    let combo_a = ALL_CARDS
      .card_from_name("Fountain of Youth")
      .expect("Card named \"Fountain of Youth\"");
    let combo_b = ALL_CARDS
      .card_from_name("Bone Saw")
      .expect("Card named \"Bone Saw\"");
    let land = ALL_CARDS
      .card_from_name("Island")
      .expect("Card named \"Island\"");
    let other = ALL_CARDS
      .card_from_name("Thought Erasure")
      .expect("Card named \"Thought Erasure\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..8 {
      cards.push(combo_a.clone());
      cards.push(combo_b.clone());
    }
    for _ in 0..20 {
      cards.push(land.clone());
    }
    for _ in 0..24 {
      cards.push(other.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(combo_a.hash);
      hs.insert(combo_b.hash);
      vec![hs]
    };
    let good_hand_count = |hands: &Vec<Hand>, draws| {
      let mut count = 0;
      for hand in hands.iter() {
        let cards = hand.opening_with_draws(draws);
        let has_combo_a = cards.iter().any(|c| c.hash == combo_a.hash);
        let has_combo_b = cards.iter().any(|c| c.hash == combo_b.hash);
        let has_2lands = cards.iter().fold(
          0,
          |accum, c| if c.kind.is_land() { accum + 1 } else { accum },
        ) >= 2;
        if has_combo_a && has_combo_b && has_2lands {
          count += 1;
        }
      }
      count
    };

    // Ignore land requirements
    let mut mulligan = London::never();
    mulligan.acceptable_hand_list = look_for;
    mulligan.mulligan_on_lands = vec![0, 1].into_iter().collect();
    let runs = 30000;
    let draws = 0;
    // No mulligan
    {
      mulligan.mulligan_down_to = 7;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 0);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.2628) < 0.01);
    }
    // Down to 6
    {
      mulligan.mulligan_down_to = 6;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 0);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.4565) < 0.01);
    }
    // Down to 5
    {
      mulligan.mulligan_down_to = 5;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 0);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.5993) < 0.01);
    }
    // Down to 4
    {
      mulligan.mulligan_down_to = 4;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 0);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.7046) < 0.01);
    }
  }

  // Verify table under section "Can You Cut a Land From Your Deck?"
  // https://www.channelfireball.com/all-mulligan/articles/the-london-mulligan-rule-mathematically-benefits-strategies-that-rely-on-specific-cards/
  // NOTE: this is not a perfect test because we can't have multiple copies of a single
  // card in our mulligan for list. We try to emulate it as close as possible and come up fairly
  // close... to within a few %
  #[test]
  fn karsten_check_2() {
    let land = ALL_CARDS
      .card_from_name("Island")
      .expect("Card named \"Island\"");
    let spell1 = ALL_CARDS
      .card_from_name("Thought Erasure")
      .expect("Card named \"Thought Erasure\"");
    let spell2 = ALL_CARDS
      .card_from_name("Dark Sphere")
      .expect("Card named \"Dark Sphere\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..21 {
      cards.push(land.clone());
    }
    for _ in 0..19 {
      cards.push(spell1.clone());
    }
    for _ in 0..20 {
      cards.push(spell2.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs1 = HashSet::new();
      let mut hs2 = HashSet::new();
      let mut hs3 = HashSet::new();
      hs1.insert(spell1.hash);
      hs2.insert(spell2.hash);
      hs3.insert(spell1.hash);
      hs3.insert(spell2.hash);
      vec![hs3, hs1, hs2]
    };

    let good_hand_count = |hands: &Vec<Hand>| {
      let mut count = 0;
      for hand in hands.iter() {
        let cards = hand.opening();
        let spell1_c = cards.iter().filter(|c| c.hash == spell1.hash).count();
        let spell2_c = cards.iter().filter(|c| c.hash == spell2.hash).count();
        let land_count = cards.iter().filter(|c| c.kind.is_land()).count();
        let has_good_lands = land_count == 2 || land_count == 3;
        let spell_good = (spell1_c >= 1 && spell2_c >= 1) || spell1_c >= 2 || spell2_c >= 2;
        if spell_good && has_good_lands {
          count += 1;
        }
      }
      count
    };
    let runs = 30000;
    let draws = 0;
    let mut mulligan = London::never();
    mulligan.mulligan_on_lands = vec![0, 1, 4, 5, 6, 7].into_iter().collect();
    mulligan.acceptable_hand_list = look_for;
    // No mulligan
    {
      mulligan.mulligan_down_to = 7;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.596) < 0.01);
    }
    // Down to 6
    {
      mulligan.mulligan_down_to = 6;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.8994) < 0.03);
    }
    // Down to 5
    {
      mulligan.mulligan_down_to = 5;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.976) < 0.02);
    }
    // Down to 4
    {
      mulligan.mulligan_down_to = 4;
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: draws,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.995) < 0.02);
    }
  }

  // Verify rows tables from
  // https://tmikonen.github.io/quantitatively/2019-03-01-london-mulligan/
  #[test]
  fn tmikonen_check_row_0() {
    let combo_a = ALL_CARDS
      .card_from_name("Fountain of Youth")
      .expect("Card named \"Fountain of Youth\"");
    let combo_b = ALL_CARDS
      .card_from_name("Bone Saw")
      .expect("Card named \"Bone Saw\"");
    let other = ALL_CARDS
      .card_from_name("Island")
      .expect("Card named \"Island\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..4 {
      cards.push(combo_a.clone());
    }
    for _ in 0..10 {
      cards.push(combo_b.clone());
    }
    for _ in 0..46 {
      cards.push(other.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(combo_a.hash);
      hs.insert(combo_b.hash);
      vec![hs]
    };
    let runs = 10000;
    let mut mulligan = London::never();
    mulligan.mulligan_down_to = 5;
    mulligan.acceptable_hand_list = look_for;
    let good_hand_count = |hands: &Vec<Hand>, draws| {
      let mut count = 0;
      for hand in hands.iter() {
        let cards = hand.opening_with_draws(draws);
        let has_combo_a = cards.iter().any(|c| c.hash == combo_a.hash);
        let has_combo_b = cards.iter().any(|c| c.hash == combo_b.hash);
        if has_combo_a && has_combo_b {
          count += 1;
        }
      }
      count
    };
    // Table 1: on the play
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: 0,
      mulligan: &mulligan,
      deck: &deck,
      on_the_play: true,
    });
    let good_hands = good_hand_count(&sim.hands, 0);
    let p = good_hands as f64 / runs as f64;
    assert!(f64::abs(p - 0.63) < 0.02);

    // Table 2: on the draw
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: 1,
      mulligan: &mulligan,
      deck: &deck,
      on_the_play: true,
    });
    let good_hands = good_hand_count(&sim.hands, 1);
    let p = good_hands as f64 / runs as f64;
    assert!(f64::abs(p - 0.66) < 0.02);
  }

  // Verify rows in tables from
  // https://tmikonen.github.io/quantitatively/2019-03-01-london-mulligan/
  #[test]
  fn tmikonen_check_row_1() {
    let combo_a = ALL_CARDS
      .card_from_name("Fountain of Youth")
      .expect("Card named \"Fountain of Youth\"");
    let combo_b = ALL_CARDS
      .card_from_name("Bone Saw")
      .expect("Card named \"Bone Saw\"");
    let other = ALL_CARDS
      .card_from_name("Island")
      .expect("Card named \"Island\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..8 {
      cards.push(combo_a.clone());
    }
    for _ in 0..8 {
      cards.push(combo_b.clone());
    }
    for _ in 0..44 {
      cards.push(other.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(combo_a.hash);
      hs.insert(combo_b.hash);
      vec![hs]
    };
    let runs = 10000;
    let mut mulligan = London::never();
    mulligan.mulligan_down_to = 5;
    mulligan.acceptable_hand_list = look_for;
    let good_hand_count = |hands: &Vec<Hand>, draws| {
      let mut count = 0;
      for hand in hands.iter() {
        let cards = hand.opening_with_draws(draws);
        let has_combo_a = cards.iter().any(|c| c.hash == combo_a.hash);
        let has_combo_b = cards.iter().any(|c| c.hash == combo_b.hash);
        if has_combo_a && has_combo_b {
          count += 1;
        }
      }
      count
    };

    {
      // Table 1: on the play
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: 0,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 0);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.79) < 0.02);
    }

    {
      // Table 2: on the draw
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: 1,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 1);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.82) < 0.02);
    }
  }

  // Verify rows in tables from
  // https://tmikonen.github.io/quantitatively/2019-03-01-london-mulligan/
  #[test]
  fn tmikonen_check_row_4() {
    let combo_a = ALL_CARDS
      .card_from_name("Fountain of Youth")
      .expect("Card named \"Fountain of Youth\"");
    let combo_b = ALL_CARDS
      .card_from_name("Bone Saw")
      .expect("Card named \"Bone Saw\"");
    let other = ALL_CARDS
      .card_from_name("Island")
      .expect("Card named \"Island\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..3 {
      cards.push(combo_a.clone());
    }
    for _ in 0..7 {
      cards.push(combo_b.clone());
    }
    for _ in 0..50 {
      cards.push(other.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(combo_a.hash);
      hs.insert(combo_b.hash);
      vec![hs]
    };
    let good_hand_count = |hands: &Vec<Hand>, draws| {
      let mut count = 0;
      for hand in hands.iter() {
        let cards = hand.opening_with_draws(draws);
        let has_combo_a = cards.iter().any(|c| c.hash == combo_a.hash);
        let has_combo_b = cards.iter().any(|c| c.hash == combo_b.hash);
        if has_combo_a && has_combo_b {
          count += 1;
        }
      }
      count
    };
    let runs = 10000;
    let mut mulligan = London::never();
    mulligan.mulligan_down_to = 5;
    mulligan.acceptable_hand_list = look_for;
    {
      // Table 1: on the play
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: 0,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 0);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.44) < 0.02);
    }

    // Table 2: on the draw
    {
      let sim = Simulation::from_config(&SimulationConfig {
        run_count: runs,
        draw_count: 1,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: true,
      });
      let good_hands = good_hand_count(&sim.hands, 1);
      let p = good_hands as f64 / runs as f64;
      assert!(f64::abs(p - 0.47) < 0.02);
    }
  }

  // Verify numbers from
  // https://tmikonen.github.io/quantitatively/2019-03-01-london-mulligan/
  // First probability result under "Probability of drawing a certain combination"
  #[test]
  fn tmikonen_misc_check_0() {
    let combo_a = ALL_CARDS
      .card_from_name("Ancestral Recall")
      .expect("Card named \"Fountain of Youth\"");
    let combo_b = ALL_CARDS
      .card_from_name("Island")
      .expect("Card named \"Island\"");
    let other = ALL_CARDS
      .card_from_name("Bone Saw")
      .expect("Card named \"Bone Saw\"");
    let mut cards = Vec::with_capacity(60);
    for _ in 0..1 {
      cards.push(combo_a.clone());
    }
    for _ in 0..21 {
      cards.push(combo_b.clone());
    }
    for _ in 0..38 {
      cards.push(other.clone());
    }
    let deck = Collection::from_cards(cards);
    let look_for = {
      let mut hs = HashSet::new();
      hs.insert(combo_a.hash);
      hs.insert(combo_b.hash);
      vec![hs]
    };
    let good_hand_count = |hands: &Vec<Hand>, draws| {
      let mut count = 0;
      for hand in hands.iter() {
        let cards = hand.opening_with_draws(draws);
        let has_combo_a = cards.iter().any(|c| c.hash == combo_a.hash);
        let has_combo_b = cards.iter().any(|c| c.hash == combo_b.hash);
        if has_combo_a && has_combo_b {
          count += 1;
        }
      }
      count
    };
    let runs = 30000;
    let mut mulligan = London::never();
    mulligan.acceptable_hand_list = look_for;
    mulligan.mulligan_down_to = 7;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: 0,
      mulligan: &mulligan,
      deck: &deck,
      on_the_play: true,
    });
    let good_hands = good_hand_count(&sim.hands, 0);
    let p = good_hands as f64 / runs as f64;
    assert!(f64::abs(p - 0.109) < 0.01);
  }

  #[test]
  fn no_land_small_hand_test_0() {
    let code = "
    1 Ancestral Vision
    1 Crimson Kobolds
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 10;
    let draws = 0;
    // Ignore land requirements
    let mut mulligan = London::never();
    mulligan.mulligan_on_lands = vec![0, 1, 6, 7].into_iter().collect();
    mulligan.mulligan_down_to = 6;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &mulligan,
      deck: &deck,
      on_the_play: true,
    });
    for hand in sim.hands {
      assert_eq!(hand.opening_hand_size, 2);
      assert_eq!(hand.mulligan_count, 5);
    }
  }

  #[test]
  fn no_land_small_hand_test_1() {
    let code = "
    1 Ancestral Vision
    1 Crimson Kobolds
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 10;
    let draws = 0;
    // Ignore land requirements
    let mulligan = London::always(0);
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &mulligan,
      deck: &deck,
      on_the_play: true,
    });
    for hand in sim.hands {
      assert_eq!(hand.opening_hand_size, 0);
    }
  }
}
