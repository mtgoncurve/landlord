//! # Simulation engine and card observations
use crate::card::{Card, Collection};
use crate::hand::{AutoTapResult, Hand, PlayOrder, SimCard};
use crate::mulligan::Mulligan;
use rand::prelude::*;
use rand::rngs::SmallRng;

pub struct SimulationConfig<'a, 'b, M: Mulligan> {
  pub run_count: usize,
  pub draw_count: usize,
  pub deck: &'a Collection,
  pub mulligan: &'b M,
  pub on_the_play: bool,
}

#[derive(Debug, Default)]
pub struct Simulation {
  pub hands: Vec<Hand>,
  pub accumulated_opening_hand_size: usize,
  pub accumulated_opening_hand_land_count: usize,
  pub on_the_play: bool,
}

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
pub struct Observations {
  pub mana: usize,
  pub cmc: usize,
  pub play: usize,
  pub tapped: usize,
  pub in_opening_hand: usize,
  pub total_runs: usize,
}

impl Observations {
  pub fn new() -> Self {
    Self::default()
  }
  pub fn p_mana(&self) -> f64 {
    self.mana as f64 / self.total_runs as f64
  }

  pub fn p_mana_given_cmc(&self) -> f64 {
    self.mana as f64 / self.cmc as f64
  }

  pub fn p_play(&self) -> f64 {
    self.play as f64 / self.total_runs as f64
  }

  pub fn p_tapped(&self) -> f64 {
    self.tapped as f64 / self.total_runs as f64
  }

  pub fn p_tapped_given_cmc(&self) -> f64 {
    self.tapped as f64 / self.cmc as f64
  }
}

impl Simulation {
  pub fn from_config<M: Mulligan>(config: &SimulationConfig<M>) -> Self {
    assert!(config.run_count > 0);
    let mut rng = SmallRng::from_entropy();
    let hands: Vec<_> = (0..config.run_count)
      .map(|_| Hand::from_mulligan(config.mulligan, &mut rng, config.deck, config.draw_count))
      .collect();
    let accumulated_opening_hand_size =
      hands.iter().map(|hand| hand.opening().len()).sum::<usize>();
    let accumulated_opening_hand_land_count = hands
      .iter()
      .map(|hand| hand.count_in_opening_with_draws(0, |c| c.kind.is_land()))
      .sum::<usize>();
    Simulation {
      hands,
      accumulated_opening_hand_size,
      accumulated_opening_hand_land_count,
      on_the_play: config.on_the_play,
    }
  }

  pub fn observations_for_card(&self, card: &Card) -> Observations {
    self.observations_for_card_by_turn(card, card.turn as usize)
  }

  pub fn observations_for_card_by_turn(&self, card: &Card, turn: usize) -> Observations {
    let mut observations = Observations::new();
    observations.total_runs = self.hands.len();
    let mut scratch = Vec::with_capacity(self.hands[0].len());
    let play_order = if self.on_the_play {
      PlayOrder::First
    } else {
      PlayOrder::Second
    };
    'next_hand: for hand in &self.hands {
      // Check all potential mana costs of a card
      let mut result = AutoTapResult::new();
      for mana_cost in &card.all_mana_costs {
        // NOTE Do not mutate observations in this loop
        let goal = SimCard {
          hash: card.hash,
          mana_cost: *mana_cost,
          kind: card.kind,
        };
        result = hand.auto_tap_with_scratch(&goal, turn, play_order, &mut scratch);
        if result.paid {
          break;
        }
      }
      if result.in_opening_hand {
        observations.in_opening_hand += 1;
      }
      if !result.cmc {
        continue 'next_hand;
      }
      // Did we make it this far? Count a CMC lands on curve event
      observations.cmc += 1;
      // Can we pay? Count a mana on curve event
      if result.paid {
        observations.mana += 1;
        // Was the card in question in our initial hand? Did we draw it on curve?
        if result.in_opening_hand || result.in_draw_hand {
          observations.play += 1;
        }
      } else if result.tapped {
        // We can't pay AND we saw a maybe_tapped event? Count a tapped on curve event
        observations.tapped += 1;
      }
    }
    assert!(observations.tapped + observations.mana <= observations.cmc);
    observations
  }
}

#[cfg(test)]
mod tests {
  use crate::mulligan::Never;
  use crate::simulation::*;

  lazy_static! {
    static ref ALL_CARDS: Collection = Collection::all().expect("Collection::all failed");
  }

  #[test]
  fn deck_with_not_enough_cards_should_not_panic() {
    let code = include_str!("decks/not_enough_cards");
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    Simulation::from_config(&SimulationConfig {
      run_count: 10,
      draw_count: 10,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
  }

  #[test]
  fn deck_with_single_zero_mana_card() {
    let card = ALL_CARDS
      .card_from_name("Ornithopter")
      .expect("Card named \"Ornithopter\"");
    let deck = Collection::from_cards(vec![card.clone()]);
    let runs = 10;
    let draws = 0;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&card);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn small_deck_1() {
    let deck_list = "
    1 Llanowar Elves
    6 Forest
    ";
    let deck = ALL_CARDS
      .from_deck_list(deck_list)
      .expect("good deck list")
      .0;
    let runs = 10;
    let draws = 0;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&deck.cards[0]);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
    assert_eq!(obs.tapped, 0);
  }

  // on the draw, always draw all 8 cards
  #[test]
  fn small_deck_2() {
    let deck_list = "
    1 Llanowar Elves
    7 Forest
    ";
    let deck = ALL_CARDS
      .from_deck_list(deck_list)
      .expect("good deck list")
      .0;
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: false,
    });
    let obs = sim.observations_for_card(&deck.cards[0]);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
    assert_eq!(obs.tapped, 0);
  }

  #[test]
  fn small_deck_3() {
    let deck_list = "
    6 Llanowar Elves
    1 Forest
    ";
    let deck = ALL_CARDS
      .from_deck_list(deck_list)
      .expect("good deck list")
      .0;
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&deck.cards[0]);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
    assert_eq!(obs.tapped, 0);
    assert_eq!(obs.in_opening_hand, runs);
  }

  // on the draw
  #[test]
  fn small_deck_4() {
    let deck_list = "
    7 Llanowar Elves
    1 Forest
    ";
    let deck = ALL_CARDS
      .from_deck_list(deck_list)
      .expect("good deck list")
      .0;
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: false,
    });
    let obs = sim.observations_for_card(&deck.cards[0]);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
    assert_eq!(obs.tapped, 0);
    assert_eq!(obs.in_opening_hand, runs);
  }

  #[test]
  fn small_deck_5() {
    let card = ALL_CARDS.card_from_name("Aura of Dominion").unwrap();
    let land0 = ALL_CARDS.card_from_name("Island").unwrap();
    let land1 = ALL_CARDS.card_from_name("Sulfur Falls").unwrap();
    let deck = Collection::from_cards(vec![card.clone(), land0.clone(), land1.clone()]);
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&card);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn small_deck_6() {
    let card = ALL_CARDS.card_from_name("Aura of Dominion").unwrap();
    let land0 = ALL_CARDS.card_from_name("Island").unwrap();
    let land1 = ALL_CARDS.card_from_name("Sulfur Falls").unwrap();
    let deck = Collection::from_cards(vec![card.clone(), land0.clone(), land1.clone()]);
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&card);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn tap_test_with_hybrid_mana_1() {
    let code = "
            38 Integrity
            22 Wind-Scarred Crag
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 0;
    let runs = 1000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let o = sim.observations_for_card(ALL_CARDS.card_from_name("Integrity").unwrap());
    assert!(o.mana + o.tapped == o.cmc);
  }

  // http://mtgoncurve.com/?v0=eyJjb2RlIjoiNCBUZW1wbGUgb2YgTXlzdGVyeVxuMyBHaWxkZWQgR29vc2UgKEVMRCkgMTYwXG4xIEZvcmVzdCIsIm9uX3RoZV9wbGF5Ijp0cnVlLCJpbml0aWFsX2hhbmRfc2l6ZSI6NywibXVsbGlnYW5fZG93bl90byI6NywibXVsbGlnYW5fb25fbGFuZHMiOlswLDEsNiw3XSwiY2FyZHNfdG9fa2VlcCI6IiJ9
  #[test]
  // The idea, there are 8 cards and we pick 7 at random (8 choose 7 = 8)
  // One of these selections does not contain the forest, which would cause
  // us to not be able to tap because our hand is full of tap lands
  fn contrived_tap_test_0() {
    let code = "
        3 Gilded Goose
        4 Temple of Mystery
        1 Forest
    ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 1;
    let runs = 10000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    // Go first -- no draws so need to play out of hand
    let obs = sim.observations_for_card(&deck.cards[0]);
    // denominator is (8 choose 7 = 8)
    assert!(f64::abs(obs.p_tapped() - 1.0 / 8.0) < 0.01);
    assert!(f64::abs(obs.p_mana() - 7.0 / 8.0) < 0.01);
    assert!(f64::abs(obs.p_mana_given_cmc() - 7.0 / 8.0) < 0.01);
    // Go second -- if you dont have the forest, then you get to draw
    // and play the forest, never tapped
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: false,
    });
    let obs = sim.observations_for_card(&deck.cards[0]);
    assert_eq!(obs.tapped, 0);
  }

  #[test]
  fn contrived_tap_test_1() {
    let code = "
            1 Yarok, the Desecrated
            1 Temple of Deceit
            1 Temple of Mystery
            1 Watery Grave
            1 Overgrown Tomb
            3 Fabled Passage
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 4;
    let runs = 1000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: false,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Yarok, the Desecrated").unwrap());
    // Never tapped because by turn 5 (Yarok CMC = 5), we have drawn all of our deck (on turn 1)
    // and should have been able to play our lands in an appropriate way to avoid tapping on turn 5
    assert_eq!(obs.tapped, 0);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
  }

  #[test]
  fn contrived_tap_test_2() {
    let code = "
            3 Wind-Scarred Crag
            13 Plains
            6 Mountain
            38 Venerable Knight
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 0;
    let runs = 10000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Venerable Knight").unwrap());
    // ((3 choose 1)*(13 choose 0)*(44 choose 6) + (3 choose 2) * (13 choose 0) * (44 choose 5) + (3 choose 3) * (13 choose 0) * (44 choose 4)) / (60 choose 7) = 0.06362
    assert!(f64::abs(obs.p_tapped() - 0.06362) < 0.01);
  }

  #[test]
  fn contrived_tap_test_3() {
    let code = "
            45 Hydroblast
            10 Island
            5 Temple of Deceit
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 0;
    let runs = 10000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Hydroblast").unwrap());
    // ((5 choose 1)*(10 choose 0)*(45 choose 6) + (5 choose 2) * (10 choose 0) * (45 choose 5) + (5 choose 3) * (10 choose 0) * (45 choose 4) + (5 choose 4)*(10 choose 0)*(45 choose 3) + (5 choose 5)*(10 choose 0)*(45 choose 2)) / (60 choose 7) = 0.14112
    assert!(f64::abs(obs.p_tapped() - 0.14112) < 0.01);
  }

  #[test]
  fn tap_test_with_hybrid_mana_0() {
    let code = "
            38 Integrity
            6 Mountain
            8 Plains
            3 Wind-Scarred Crag
            1 Sacred Foundry
            4 Tournament Grounds
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 0;
    let runs = 10000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let o = sim.observations_for_card(ALL_CARDS.card_from_name("Integrity").unwrap());
    assert!(o.mana + o.tapped == o.cmc);
  }

  #[test]
  fn hypergeometric_0() {
    let code = "
            2 Cleansing Nova (M19) 9
            1 Vraska, Relic Seeker (XLN) 232
            4 Sinister Sabotage (GRN) 54
            4 Opt (XLN) 65
            2 Vraska's Contempt (XLN) 129
            2 Isolated Chapel (DAR) 241
            3 Cry of the Carnarium (RNA) 70
            1 Devious Cover-Up (GRN) 35
            3 Teferi, Hero of Dominaria (DAR) 207
            3 Hydroid Krasis (RNA) 183
            2 Assassin's Trophy (GRN) 152
            2 Overgrown Tomb (GRN) 253
            3 Breeding Pool (RNA) 246
            3 Glacial Fortress (XLN) 255
            2 Moment of Craving (RIX) 79
            3 Hallowed Fountain (RNA) 251
            4 Drowned Catacomb (XLN) 253
            4 Godless Shrine (RNA) 248
            2 Search for Azcanta (XLN) 74
            3 Chemister's Insight (GRN) 32
            2 Wilderness Reclamation (RNA) 149
            1 Mastermind's Acquisition (RIX) 77
            4 Watery Grave (GRN) 259
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 8;
    let runs = 20000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Opt").unwrap());
    let actual = obs.p_mana();
    // All 17 of the 17 blue land sources can enter turn 1 untapped.
    let expected = 0.917; // Hypergeometric, 60, 17, 7, 1
    let difference = f64::abs(expected - actual);
    assert!(difference < 0.01); // To within 1%
  }

  #[test]
  fn hypergeometric_1() {
    let code = "
            2 Chemister's Insight
            3 Crackling Drake
            2 Discovery // Dispersal
            2 Disdainful Stroke
            2 Dive Down
            1 Dragonskull Summit
            1 Drowned Catacomb
            3 Fiery Cannonade
            8 Island
            3 Lava Coil
            3 Lightning Strike
            8 Mountain
            3 Niv-Mizzet, Parun
            2 Opt
            2 Ral, Izzet Viceroy
            2 Search for Azcanta
            3 Sinister Sabotage
            4 Steam Vents
            4 Sulfur Falls
            2 Syncopate
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 8;
    let runs = 20000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Opt").unwrap());
    let actual = obs.p_mana();
    // All 17 blue lands are valid
    let expected = 0.917; // Hypergeometric, 60, 17, 7, 1
    let difference = f64::abs(expected - actual);
    assert!(difference < 0.01); // To within 1%
  }

  #[test]
  fn hypergeometric_2() {
    let code = "
        2 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        4 Sinister Sabotage (GRN) 54
        4 Opt (XLN) 65
        2 Vraska's Contempt (XLN) 129
        2 Isolated Chapel (DAR) 241
        3 Cry of the Carnarium (RNA) 70
        1 Devious Cover-Up (GRN) 35
        3 Teferi, Hero of Dominaria (DAR) 207
        3 Hydroid Krasis (RNA) 183
        2 Assassin's Trophy (GRN) 152
        2 Overgrown Tomb (GRN) 253
        3 Breeding Pool (RNA) 246
        3 Meandering River
        2 Moment of Craving (RIX) 79
        3 Hallowed Fountain (RNA) 251
        4 Dimir Guildgate
        4 Godless Shrine (RNA) 248
        2 Search for Azcanta (XLN) 74
        3 Chemister's Insight (GRN) 32
        2 Wilderness Reclamation (RNA) 149
        1 Mastermind's Acquisition (RIX) 77
        4 Watery Grave (GRN) 259
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let draws = 8;
    let runs = 20000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Opt").unwrap());
    let actual = obs.p_mana();
    // Only 10 of the 17 blue land sources can enter turn 1 untapped. The other 7 are taplands
    let expected = 0.741370766; // Hypergeometric, 60, 10, 7, 1
    let difference = f64::abs(expected - actual);
    assert!(difference < 0.01); // To within 1%
  }

  #[test]
  fn hypergeometric_3() {
    let code = "
        2 Cleansing Nova (M19) 9
        1 Vraska, Relic Seeker (XLN) 232
        4 Sinister Sabotage (GRN) 54
        4 Opt (XLN) 65
        2 Vraska's Contempt (XLN) 129
        2 Isolated Chapel (DAR) 241
        3 Cry of the Carnarium (RNA) 70
        1 Devious Cover-Up (GRN) 35
        3 Teferi, Hero of Dominaria (DAR) 207
        3 Hydroid Krasis (RNA) 183
        2 Assassin's Trophy (GRN) 152
        2 Overgrown Tomb (GRN) 253
        3 Breeding Pool (RNA) 246
        3 Glacial Fortress (XLN) 255
        2 Moment of Craving (RIX) 79
        3 Hallowed Fountain (RNA) 251
        4 Dimir Guildgate
        4 Godless Shrine (RNA) 248
        2 Search for Azcanta (XLN) 74
        3 Chemister's Insight (GRN) 32
        2 Wilderness Reclamation (RNA) 149
        1 Mastermind's Acquisition (RIX) 77
        4 Watery Grave (GRN) 259
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 20000;
    let draws = 8;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Opt").unwrap());
    let actual = obs.p_mana();
    // Only 13 of the 17 blue land sources can enter turn 1 untapped. The other 4 are tap ands
    let expected = 0.837; // Hypergeometric, 60, 13, 7, 1
    let difference = f64::abs(expected - actual);
    assert!(difference < 0.01); // To within 1%
  }

  #[test]
  fn multi_hypergeometric_0() {
    let code = "
        17 Plains
        9 Swamp
        34 History of Benalia
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 20000;
    let draws = 8;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("History of Benalia").unwrap());
    let actual = obs.p_mana();
    // Use https://deckulator.appspot.com/ to calculate this number. Need to perform 3 calculations (Draw 2 plains + 1 Swamp) + (Draw 3 plains + 0 Swamp) - (Draw 3 plains + 1 swamp)
    let expected = 0.746;
    let difference = f64::abs(expected - actual);
    assert!(difference < 0.01); // To within 1%
  }

  #[test]
  fn multi_hypergeometric_1() {
    let code = "
        16 Forest
        8 Swamp
        36 Jadelight Ranger
        ";
    let deck = ALL_CARDS.from_deck_list(code).expect("Bad deckcode").0;
    let runs = 20000;
    let draws = 8;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(ALL_CARDS.card_from_name("Jadelight Ranger").unwrap());
    let actual = obs.p_mana();
    // Multivariate hypergeom, see example 7 from https://www.channelfireball.com/articles/an-introduction-to-the-multivariate-hypergeometric-distribution-for-magic-players/
    let expected = 0.692;
    let difference = f64::abs(expected - actual);
    assert!(difference < 0.01); // To within 1%
  }

  #[test]
  fn yarok_test() {
    let code = "
    1 yarok, the desecrated
    1 overgrown tomb
    1 watery grave
    1 waterlogged grove
      2 mountain
      ";
    let deck = ALL_CARDS.from_deck_list(code).unwrap().0;
    let card = ALL_CARDS.card_from_name("yarok, the desecrated").unwrap();
    let runs = 100;
    let draws = 0;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn clarion_ultimatum_test_0() {
    let code = "
      1 Clarion Ultimatum
      2 Temple Garden
      1 Hallowed Fountain
      1 Breeding Pool
      1 Forest
      1 Plains
      1 Island
      ";
    let deck = ALL_CARDS.from_deck_list(code).unwrap().0;
    let card = ALL_CARDS.card_from_name("Clarion Ultimatum").unwrap();
    let runs = 1000;
    let draws = 6;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn clarion_ultimatum_test_1() {
    let code = "
      1 Clarion Ultimatum
      1 Temple Garden
      2 Hallowed Fountain
      1 Breeding Pool
      1 Forest
      1 Plains
      1 Island
      ";
    let deck = ALL_CARDS.from_deck_list(code).unwrap().0;
    let card = ALL_CARDS.card_from_name("Clarion Ultimatum").unwrap();
    let runs = 1000;
    let draws = 6;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn clarion_ultimatum_test_2() {
    let code = "
      1 Clarion Ultimatum
      1 Temple Garden
      1 Hallowed Fountain
      2 Breeding Pool
      1 Forest
      1 Plains
      1 Island
      ";
    let deck = ALL_CARDS.from_deck_list(code).unwrap().0;
    let card = ALL_CARDS.card_from_name("Clarion Ultimatum").unwrap();
    let runs = 1000;
    let draws = 6;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn clarion_ultimatum_test_3() {
    let code = "
      1 Clarion Ultimatum
      1 Temple Garden
      1 Hallowed Fountain
      1 Breeding Pool
      2 Forest
      1 Plains
      1 Island
      ";
    let deck = ALL_CARDS.from_deck_list(code).unwrap().0;
    let card = ALL_CARDS.card_from_name("Clarion Ultimatum").unwrap();
    let runs = 1000;
    let draws = 6;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn contrived_tap_test_4() {
    let code = "
    1 Agonizing Remorse # T = 6
    59 Cinder Barrens
      ";
    let deck = ALL_CARDS.from_deck_list(code).unwrap().0;
    let card = &deck.cards[0];
    assert_eq!(card.kind.is_land(), false);
    let runs = 100;
    let draws = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card);
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.tapped, 0);
  }
}
