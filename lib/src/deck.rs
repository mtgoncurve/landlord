use crate::card::*;
use crate::mana_cost::parse_mana_costs;
use crate::scryfall::GameFormat;

use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
  pub cards: std::collections::BTreeMap<Card, usize>,
  pub format: GameFormat,
}

#[derive(Debug)]
pub struct DeckcodeError(pub String);

impl Deck {
  pub fn new() -> Self {
    Self {
      cards: std::collections::BTreeMap::new(),
      format: GameFormat::Standard,
    }
  }

  pub fn from_cards(cards: Vec<Card>) -> Self {
    let mut deck = Deck::new();
    for card in cards {
      deck.insert(card);
    }
    deck
  }

  pub fn insert(&mut self, card: Card) {
    let total_count = self.cards.entry(card).or_insert(0);
    *total_count += 1;
  }

  pub fn insert_count(&mut self, card: Card, count: usize) {
    let total_count = self.cards.entry(card).or_insert(0);
    *total_count += count;
  }

  pub fn flatten(&self) -> Vec<&Card> {
    let mut result = Vec::with_capacity(60);
    for (card, count) in &self.cards {
      for _ in 0..*count {
        result.push(card);
      }
    }
    result
  }

  pub fn card_from_name(&self, name: &str) -> Option<&Card> {
    let lower = name.to_lowercase();
    for (k, _) in &self.cards {
      if k.name.to_lowercase() == lower {
        return Some(k);
      }
    }
    None
  }

  pub fn len(&self) -> usize {
    self.cards.iter().fold(0, |accum, (_, count)| accum + count)
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn from_list(list: &str) -> Result<Self, DeckcodeError> {
    lazy_static! {
        //https://regex101.com/r/OluNfe/3
        static ref ARENA_LINE_REGEX: Regex =
            Regex::new(r"^\s*(?P<amount>\d+)\s+(?P<name>[^\(#\n\r]+)(?:\s*\((?P<set>\w+)\)\s+(?P<setnum>\d+))?\s*#?(?:\s*[Xx]\s*=\s*(?P<X>\d+))?(?:\s*[Tt]\s*=\s*(?P<T>\d+))?(?:\s*[Mm]\s*=\s*(?P<M>[RGWUB\d{}]+))?")
                .expect("Failed to compile ARENA_LINE_REGEX regex");
    }
    let mut deck = Deck::new();
    for line in list.trim().lines() {
      let trimmed = line.trim();
      let trimmed_lower = trimmed.to_lowercase();
      // An empty line divides the main board cards from the side board cards
      if trimmed.is_empty() {
        break;
      }
      // Ignore reserved words
      if trimmed_lower == "deck" {
        continue;
      }
      if trimmed_lower == "commander" {
        continue;
      }
      if trimmed_lower == "sideboard" {
        break;
      }
      if trimmed_lower == "maybeboard" {
        break;
      }
      // Ignore line comments
      if trimmed.starts_with('#') {
        continue;
      }
      let caps = ARENA_LINE_REGEX
        .captures(trimmed)
        .ok_or_else(|| DeckcodeError(format!("Cannot regex capture deck list line: {}", line)))?;
      let amount = caps["amount"].parse::<usize>().or_else(|_| {
        Err(DeckcodeError(format!(
          "Cannot parse usize card amount from deck list line: {}",
          line
        )))
      })?;
      let name = caps["name"].trim().to_string();
      // By default, we represent split cards with the left face
      let left_card_name = name
        .split("//")
        .next()
        .ok_or_else(|| {
          DeckcodeError(format!(
            "Cannot parse card name from deck list line: {}",
            line
          ))
        })?
        .trim()
        .to_string();
      let card = ALL_CARDS.card_from_name(&left_card_name).ok_or_else(|| {
        DeckcodeError(format!("Cannot find card named \"{}\" in collection", name))
      })?;
      // Clone the card as mutable so we can apply modifiers
      let mut card = card.clone();
      // Handle the X = modifier
      if let Some(x_val) = caps.name("X") {
        // Only modify the colorless mana cost if the mana cost string contains an X value
        // otherwise ignore the attribute
        if card.mana_cost_string.contains('X') {
          let x_val = x_val.as_str().parse::<u8>().or_else(|_| {
            Err(DeckcodeError(format!(
              "Cannot parse u8 X= value from deck list line: {}",
              line
            )))
          })?;
          card.mana_cost.c = x_val;
          card
            .all_mana_costs
            .iter_mut()
            .for_each(|cost| cost.c = x_val);
          card.mana_cost_string = card.mana_cost_string.replace('X', &x_val.to_string());
          card.turn = card.mana_cost.cmc();
        }
      }
      // Handle the M = modifier
      if let Some(m_val) = caps.name("M") {
        let mana_cost_str = m_val.as_str();
        let all_mana_costs = parse_mana_costs(mana_cost_str);
        if all_mana_costs.is_empty() {
          return Err(DeckcodeError(format!(
            "Problematic mana cost ('M = ') specifed at line {}",
            line
          )));
        }
        card.mana_cost = all_mana_costs[0];
        card.all_mana_costs = all_mana_costs;
        card.turn = card.mana_cost.cmc();
        card.kind = CardKind::ForcedLand;
      }
      // Hanlde the T = modifier
      if let Some(turn_val) = caps.name("T") {
        // TODO(jshrake): Set the desired turn to play this card
        let turn_val = turn_val.as_str().parse::<u8>().or_else(|_| {
          Err(DeckcodeError(format!(
            "Cannot parse u8 T= value from deck list line: {}",
            line
          )))
        })?;
        card.turn += turn_val;
      }
      card.name = name;
      deck.cards.insert(card, amount);
    }
    Ok(deck)
  }
}

#[macro_export]
macro_rules! decklist {
  ($list:expr) => {
    $crate::deck::Deck::from_list($list).unwrap_or_else(|_| panic!("Bad deck list"))
  };
}

#[cfg(test)]
mod tests {
  #[test]
  fn good_deckcode_0() {
    let code = "
        4 Legion's Landing (XLN) 22
        4 Adanto Vanguard (XLN) 1
        4 Skymarcher Aspirant (RIX) 21
        4 Snubhorn Sentry (RIX) 23
        4 Clifftop Retreat (DAR) 239
        4 History of Benalia (DAR) 21
        2 Ajani, Adversary of Tyrants (M19) 3
        4 Heroic Reinforcements (M19) 217
        4 Sacred Foundry (GRN) 254
        2 Mountain (XLN) 272
        12 Plains (XLN) 263
        3 Hunted Witness (GRN) 15
        4 Conclave Tribunal (GRN) 6
        4 Venerated Loxodon (GRN) 30
        1 Doom Whisperer (GRN) 30
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn good_deckcode_1() {
    let code = "
        4 Legion's Landing
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn good_deckcode_2() {
    let code = "
        # This is a comment
        4 Legion's Landing
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        # This is another comment
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        # This is the last comment
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn good_deckcode_3() {
    let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn good_deckcode_4() {
    let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn good_deckcode_5_set_x() {
    let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183 #X=4
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
    let deck = decklist!(&code);
    assert_eq!(deck.len(), 60);
    let card = deck.card_from_name("Hydroid Krasis").unwrap();
    assert_eq!(card.mana_cost.c, 4);
  }

  #[test]
  fn good_deckcode_6_set_x() {
    let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183#x = 6
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
    let card = deck.card_from_name("Hydroid Krasis").unwrap();
    assert_eq!(card.mana_cost.c, 6);
  }

  #[test]
  fn good_deckcode_7_set_x() {
    let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183#x = 6
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77#x=3
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
    let deck = decklist!(&code);
    assert_eq!(deck.len(), 60);
    let card = deck.card_from_name("Hydroid Krasis").unwrap();
    assert_eq!(card.mana_cost.c, 6);
    // can't set x value of midnight reaper since it doesn't have {X} mana cost
    let card = deck.card_from_name("Midnight Reaper").unwrap();
    assert_eq!(card.mana_cost.c, 2);
  }

  #[test]
  fn good_deckcode_8_set_x() {
    let code = "
        # This is a comment
        4 Legion's Landing
        3 Hydroid Krasis#x=5
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        # This is another comment
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        # This is the last comment
        ";
    let deck = decklist!(&code);
    assert_eq!(deck.len(), 63);
    let card = deck.card_from_name("Hydroid Krasis").unwrap();
    assert_eq!(card.mana_cost.c, 5);
  }

  #[test]
  fn good_deckcode_9_set_x() {
    let code = "
        # This is a comment
        4 Legion's Landing
        3 Hydroid Krasis # X=-5
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        # This is another comment
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        # This is the last comment
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 63);
    // Ignore negatives
    let card = deck.card_from_name("Hydroid Krasis").unwrap();
    assert_eq!(card.mana_cost.c, 1);
  }

  #[test]
  #[should_panic]
  fn bad_deckcode_0() {
    // The last card, Doom Whisperer, is misspelled
    let code = "
        4 Legion's Landing (XLN) 22
        4 Adanto Vanguard (XLN) 1
        4 Skymarcher Aspirant (RIX) 21
        4 Snubhorn Sentry (RIX) 23
        4 Clifftop Retreat (DAR) 239
        4 History of Benalia (DAR) 21
        2 Ajani, Adversary of Tyrants (M19) 3
        4 Heroic Reinforcements (M19) 217
        4 Sacred Foundry (GRN) 254
        2 Mountain (XLN) 272
        13 Plains (XLN) 263
        3 Hunted Witness (GRN) 15
        4 Conclave Tribunal (GRN) 6
        4 Venerated Loxodon (GRN) 30
        1 Doo Whisperer (GRN) 30
        ";
    let _deck = decklist!(code);
  }

  #[test]
  fn good_deckcode_with_0_0() {
    let code = "
        0 Island
        4 Legion's Landing
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn deck_aetherhub_45936() {
    let code = include_str!("decks/45936");
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn deck_aetherhub_50520() {
    let code = include_str!("decks/50520");
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn deck_aetherhub_50817() {
    let code = include_str!("decks/50817");
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }
}
