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
      }
    }
    assert!(observations.mana <= observations.cmc);
    observations
  }
}

#[cfg(test)]
mod tests {
  use crate::card::*;
  use crate::mulligan::Never;
  use crate::simulation::*;

  #[test]
  fn deck_with_not_enough_cards_should_not_panic() {
    let deck = decklist!(include_str!("decks/not_enough_cards"));
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
    let card = card!("Ornithopter");
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
    let deck = decklist!(
      "
    1 Llanowar Elves
    6 Forest
    "
    );
    let runs = 10;
    let draws = 0;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&card!("Llanowar Elves"));
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
  }

  // on the draw, always draw all 8 cards
  #[test]
  fn small_deck_2() {
    let deck = decklist!(
      "
    1 Llanowar Elves
    7 Forest
    "
    );
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: false,
    });
    let obs = sim.observations_for_card(&card!("Llanowar Elves"));
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
  }

  #[test]
  fn small_deck_3() {
    let deck = decklist!(
      "
    6 Llanowar Elves
    1 Forest
    "
    );
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(&card!("Llanowar Elves"));
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
    assert_eq!(obs.in_opening_hand, runs);
  }

  // on the draw
  #[test]
  fn small_deck_4() {
    let deck_list = "
    7 Llanowar Elves
    1 Forest
    ";
    let deck = decklist!(deck_list);
    let draws = 1;
    let runs = 10;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: false,
    });
    let obs = sim.observations_for_card(&card!("Llanowar Elves"));
    assert_eq!(obs.cmc, runs);
    assert_eq!(obs.mana, runs);
    assert_eq!(obs.play, runs);
    assert_eq!(obs.in_opening_hand, runs);
  }

  #[test]
  fn small_deck_5() {
    let card = card!("Aura of Dominion");
    let land0 = card!("Island");
    let land1 = card!("Sulfur Falls");
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
    let card = card!("Aura of Dominion");
    let land0 = card!("Island");
    let land1 = card!("Sulfur Falls");
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
    let deck = decklist!(code);
    let draws = 0;
    let runs = 1000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let o = sim.observations_for_card(card!("Integrity"));
    assert!(o.mana == o.cmc);
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
    let deck = decklist!(code);
    let draws = 8;
    let runs = 20000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card!("Opt"));
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
    let deck = decklist!(code);
    let draws = 8;
    let runs = 20000;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card!("Opt"));
    let actual = obs.p_mana();
    // All 17 blue lands are valid
    let expected = 0.917; // Hypergeometric, 60, 17, 7, 1
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
    let deck = decklist!(code);
    let runs = 20000;
    let draws = 8;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card!("History of Benalia"));
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
    let deck = decklist!(code);
    let runs = 20000;
    let draws = 8;
    let sim = Simulation::from_config(&SimulationConfig {
      run_count: runs,
      draw_count: draws,
      mulligan: &Never::never(),
      deck: &deck,
      on_the_play: true,
    });
    let obs = sim.observations_for_card(card!("Jadelight Ranger"));
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
    let deck = decklist!(code);
    let card = card!("yarok, the desecrated");
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
    let deck = decklist!(code);
    let card = card!("Clarion Ultimatum");
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
    let deck = decklist!(code);
    let card = card!("Clarion Ultimatum");
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
    let deck = decklist!(code);
    let card = card!("Clarion Ultimatum");
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
    let deck = decklist!(code);
    let card = card!("Clarion Ultimatum");
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
    let deck = decklist!(code);
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
  }

  #[test]
  fn syr_no_mana() {
    let code = "
      2 Syr Gwyn, Hero of Ashvale (ELD) 330
      4 Dalakos, Crafter of Wonders (THB) 212
      2 Omen of the Sea (THB) 58
      4 Drawn from Dreams (M20) 56
      4 Colossus Hammer (M20) 223
      4 Fires of Invention (ELD) 125
      4 Shatter the Sky (THB) 37
      4 Opt (ELD) 59
      4 Temple of Triumph (M20) 257
      3 Plains (XLN) 262
      4 Deafening Clarion (GRN) 165
      4 Steam Vents (GRN) 257
      4 Fabled Passage (ELD) 244
      1 Field of Ruin (THB) 242
      2 Castle Vantress (ELD) 242
      3 Island (THB) 251
      3 Mountain (THB) 285
      4 Teferi, Time Raveler (WAR) 221
      ";
    let deck = decklist!(code);
    let card = card!("Syr Gwyn, Hero of Ashvale");
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
    dbg!(obs);
    assert_eq!(obs.mana, 0);
  }
}
