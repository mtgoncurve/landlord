use crate::card::*;
use crate::data::*;
use regex::Regex;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
  pub title: Option<String>,
  pub url: Option<String>,
  pub cards: Vec<DeckCard>,
  pub format: GameFormat,
  pub card_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckCard {
  pub card: Card,
  pub count: usize,
}

#[derive(Debug, Clone)]
pub struct DeckBuilder {
  pub cards: HashMap<Card, usize>,
}

impl DeckBuilder {
  pub fn new() -> Self {
    Self {
      cards: HashMap::new(),
    }
  }

  pub fn insert(mut self, mut card: Card) -> Self {
    card.name = card.name.clone();
    let total_count = self.cards.entry(card).or_insert(0);
    *total_count += 1;
    Self { cards: self.cards }
  }

  pub fn insert_count(mut self, mut card: Card, count: usize) -> Self {
    card.name = card.name.clone();
    let total_count = self.cards.entry(card).or_insert(0);
    *total_count += count;
    Self { cards: self.cards }
  }

  pub fn build(self) -> Deck {
    let mut deck = Deck::new();
    let mut count = 0;
    for (k, v) in self.cards {
      deck.cards.push(DeckCard { card: k, count: v });
      count += v;
    }
    deck.card_count = count;
    deck
      .cards
      .sort_unstable_by(|a, b| a.card.name.cmp(&b.card.name));
    deck
  }
}

#[derive(Debug)]
pub struct DeckcodeError(pub String);

impl Deck {
  pub fn new() -> Self {
    Self {
      title: None,
      url: None,
      cards: Vec::with_capacity(20),
      format: GameFormat::Standard,
      card_count: 0,
    }
  }

  pub fn common_count(&self) -> usize {
    self
      .cards
      .iter()
      .filter(|cc| cc.card.rarity == Rarity::Common && cc.card.kind != CardKind::BasicLand)
      .fold(0, |accum, cc| accum + cc.count)
  }

  pub fn uncommon_count(&self) -> usize {
    self
      .cards
      .iter()
      .filter(|cc| cc.card.rarity == Rarity::Uncommon && cc.card.kind != CardKind::BasicLand)
      .fold(0, |accum, cc| accum + cc.count)
  }

  pub fn rare_count(&self) -> usize {
    self
      .cards
      .iter()
      .filter(|cc| cc.card.rarity == Rarity::Rare && cc.card.kind != CardKind::BasicLand)
      .fold(0, |accum, cc| accum + cc.count)
  }

  pub fn mythic_count(&self) -> usize {
    self
      .cards
      .iter()
      .filter(|cc| cc.card.rarity == Rarity::Mythic && cc.card.kind != CardKind::BasicLand)
      .fold(0, |accum, cc| accum + cc.count)
  }

  pub fn mana_counts(&self) -> ManaColorCount {
    let mut mcc = ManaColorCount::new();
    for cc in &self.cards {
      for _ in 0..cc.count {
        mcc.count(&cc.card.mana_cost);
      }
    }
    mcc
  }

  pub fn mana_counts_for_lands(&self) -> ManaColorCount {
    let mut mcc = ManaColorCount::new();
    for cc in &self.cards {
      if !cc.card.is_land() {
        continue;
      }
      for _ in 0..cc.count {
        mcc.count(&cc.card.mana_cost);
      }
    }
    mcc
  }

  pub fn mana_counts_for_nonlands(&self) -> ManaColorCount {
    let mut mcc = ManaColorCount::new();
    for cc in &self.cards {
      if cc.card.is_land() {
        continue;
      }
      for _ in 0..cc.count {
        mcc.count(&cc.card.mana_cost);
      }
    }
    mcc
  }

  pub fn mana_counts_for_craftables(&self) -> ManaColorCount {
    let mut mcc = ManaColorCount::new();
    for cc in &self.cards {
      if cc.card.kind == CardKind::BasicLand {
        continue;
      }
      for _ in 0..cc.count {
        mcc.count(&cc.card.mana_cost);
      }
    }
    mcc
  }

  pub fn from_cards<I>(cards: I) -> Self
  where
    I: IntoIterator<Item = Card>,
  {
    let mut b = DeckBuilder::new();
    for card in cards {
      b = b.insert(card);
    }
    b.build()
  }

  pub fn flatten(&self) -> Vec<&Card> {
    let mut result = Vec::with_capacity(self.card_count);
    for card_count in &self.cards {
      for _ in 0..card_count.count {
        result.push(&card_count.card);
      }
    }
    result
  }

  pub fn card_from_name(&self, name: &str) -> Option<&Card> {
    self.card_count_from_name(name).map(|o| &o.card)
  }

  pub fn card_count_from_name(&self, name: &str) -> Option<&DeckCard> {
    let name_lowercase = name.to_lowercase();
    let res = self
      .cards
      .binary_search_by(|probe| probe.card.name.to_lowercase().cmp(&name_lowercase));
    res.map(|idx| &self.cards[idx]).ok()
  }

  pub fn len(&self) -> usize {
    self.card_count
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
    let mut builder = DeckBuilder::new();
    let mut looking_for_deck_line = false;
    for line in list.trim().lines() {
      let trimmed = line.trim();
      let trimmed_lower = trimmed.to_lowercase();
      // Ignore reserved words
      if trimmed_lower == "deck" {
        looking_for_deck_line = false;
        continue;
      }
      if trimmed_lower == "commander" {
        looking_for_deck_line = true;
        continue;
      }
      if trimmed_lower == "companion" {
        looking_for_deck_line = true;
        continue;
      }
      if trimmed_lower == "sideboard" {
        // Assumes sideboard comes after deck
        break;
      }
      if trimmed_lower == "maybeboard" {
        // Assumes maybeboard comes after deck
        break;
      }
      // Ignore line comments
      if trimmed.starts_with('#') {
        continue;
      }
      if looking_for_deck_line {
        continue;
      }
      // An empty line divides the main board cards from the side board cards
      if trimmed.is_empty() {
        break;
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
      let set = if let Some(set) = caps.name("set") {
        set
          .as_str()
          .parse::<SetCode>()
          .expect("parse::<SetCode>() cannot fail")
      } else {
        SetCode::Unknown
      };
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
      // Find the card from the name, and clone it so we can apply card modifiers
      let mut card = ALL_CARDS
        .card_from_name(&left_card_name)
        .ok_or_else(|| DeckcodeError(format!("Cannot find card named \"{}\" in collection", name)))?
        .clone();
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
        let all_mana_costs = mana_costs_from_str(mana_cost_str);
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
      card.set = set;
      builder = builder.insert_count(card, amount);
    }
    Ok(builder.build())
  }

  pub fn to_string(&self) -> String {
    let mut res = Vec::with_capacity(self.cards.len());
    for cc in &self.cards {
      res.push(format!(
        "{} {} ({:?})\n",
        cc.count, cc.card.name, cc.card.set
      ));
    }
    res.concat()
  }

  pub fn have_need(&self, collection: &Deck) -> (Deck, Deck) {
    let mut have = DeckBuilder::new();
    let mut need = DeckBuilder::new();
    for need_cc in &self.cards {
      let need_card = &need_cc.card;
      let need_count = need_cc.count;
      let need_name = &need_card.name;
      let have_cc = collection.card_count_from_name(need_name);
      let have_count = std::cmp::min(have_cc.map(|o| o.count).unwrap_or(0), need_count);
      let diff_count = need_count - have_count;
      if diff_count == 0 {
        have = have.insert_count(need_card.clone(), need_count);
      } else {
        have = have.insert_count(need_card.clone(), have_count);
        need = need.insert_count(need_card.clone(), diff_count);
      }
    }
    (have.build(), need.build())
  }
}

impl Deref for Deck {
  type Target = [DeckCard];

  fn deref(&self) -> &Self::Target {
    &self.cards
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
    let code = "
      4 Drowned Catacomb (XLN) 253
      4 Glacial Fortress (XLN) 255
      2 Search for Azcanta (XLN) 74
      2 Vraska's Contempt (XLN) 129
      1 Golden Demise (RIX) 73
      3 Moment of Craving (RIX) 79
      4 Isolated Chapel (DAR) 241
      3 Teferi, Hero of Dominaria (DAR) 207
      2 Cast Down (DAR) 81
      1 Lyra Dawnbringer (DAR) 26
      2 The Eldest Reborn (DAR) 90
      1 Cleansing Nova (M19) 9
      1 Chromium, the Mutable (M19) 214
      3 Evolving Wilds (RIX) 186
      3 Thought Erasure (GRN) 206
      2 Disinformation Campaign (GRN) 167
      4 Watery Grave (GRN) 259
      4 Sinister Sabotage (GRN) 54
      1 Chemister's Insight (GRN) 32
      3 Ritual of Soot (GRN) 84
      1 Price of Fame (GRN) 83
      2 Syncopate (DAR) 67
      2 Swamp (XLN) 268
      2 Island (XLN) 264
      3 Plains (XLN) 263

      1 Ixalan's Binding (XLN) 17
      1 Golden Demise (RIX) 73
      1 The Eldest Reborn (DAR) 90
      2 Fungal Infection (DAR) 94
      2 Duress (XLN) 105
      3 Thief of Sanity (GRN) 205
      2 Unmoored Ego (GRN) 212
      3 Blood Operative (GRN) 63
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn deck_aetherhub_50520() {
    let code = "
      4 Chart a Course (XLN) 48
      3 Dive Down (XLN) 53
      1 Drowned Catacomb (XLN) 253
      2 Search for Azcanta (XLN) 74
      3 Spell Pierce (XLN) 81
      4 Sulfur Falls (DAR) 247
      4 Opt (XLN) 65
      3 Enigma Drake (M19) 217
      3 Niv-Mizzet, Parun (GRN) 192
      1 Beacon Bolt (GRN) 154
      4 Crackling Drake (GRN) 163
      4 Steam Vents (GRN) 257
      5 Mountain (XLN) 272
      4 Discovery // Dispersal (GRN) 223
      4 Lava Coil (GRN) 108
      7 Island (XLN) 264
      4 Tormenting Voice (M19) 164
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn deck_aetherhub_50817() {
    let code = "
      4 Dragonskull Summit (XLN) 252
      4 Drowned Catacomb (XLN) 253
      2 Search for Azcanta (XLN) 74
      3 Vraska's Contempt (XLN) 129
      3 Angrath, the Flame-Chained (RIX) 152
      5 Swamp (XLN) 268
      1 Island (XLN) 264
      3 Moment of Craving (RIX) 79
      4 Sulfur Falls (DAR) 247
      3 Cast Down (DAR) 81
      2 The Eldest Reborn (DAR) 90
      4 Thought Erasure (GRN) 206
      4 Steam Vents (GRN) 257
      4 Watery Grave (GRN) 259
      2 Discovery // Dispersal (GRN) 223
      4 Sinister Sabotage (GRN) 54
      2 Chemister's Insight (GRN) 32
      4 Ritual of Soot (GRN) 84
      2 Ral, Izzet Viceroy (GRN) 195
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 60);
  }

  #[test]
  fn empty_code() {
    let code = "";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 0);
  }

  #[test]
  fn code_contains_companion() {
    let code = "
      Companion
      1 Lurrus of the Dream Den (IKO) 226

      Deck
      1 Island
      1 Plains
      1 Mountain
      1 Forest
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 4);
  }

  #[test]
  fn code_contains_commander() {
    let code = "
      Commander
      1 Lurrus of the Dream Den (IKO) 226

      Deck
      1 Island
      1 Plains
      1 Mountain
      1 Forest
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 4);
  }

  #[test]
  fn code_contains_deck() {
    let code = "
      Deck
      1 Island
      1 Plains
      1 Mountain
      1 Forest
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 4);
  }

  #[test]
  fn code_contains_sideboard() {
    let code = "
      Deck
      1 Island
      1 Plains
      1 Mountain
      1 Forest

      Sideboard
      1 Forest
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 4);
  }

  #[test]
  fn code_contains_maybeboard() {
    let code = "
      Deck
      1 Island
      1 Plains
      1 Mountain
      1 Forest

      Maybeboard
      1 Forest
    ";
    let deck = decklist!(code);
    assert_eq!(deck.len(), 4);
  }
}
