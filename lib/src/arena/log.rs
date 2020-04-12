use crate::card::*;
use crate::data::*;
use crate::deck::*;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::BufRead;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerCardsV3 {
  id: u64,
  payload: HashMap<String, usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetDeckListsV3 {
  id: u64,
  payload: Vec<GetDeckListsV3Payload>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerInventory {
  id: u64,
  payload: GetPlayerInventoryPayload,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetDeckListsV3Payload {
  #[serde(default)]
  name: String,
  #[serde(default)]
  format: String,
  #[serde(rename = "lastUpdated", default)]
  last_updated: String,
  #[serde(rename = "mainDeck", default)]
  main_deck: Vec<u64>,
  #[serde(default)]
  sideboard: Vec<u64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerInventoryPayload {
  #[serde(rename = "wcCommon", default)]
  wc_common_count: usize,
  #[serde(rename = "wcUncommon", default)]
  wc_uncommon_count: usize,
  #[serde(rename = "wcRare", default)]
  wc_rare_count: usize,
  #[serde(rename = "wcMythic", default)]
  wc_mythic_count: usize,
  #[serde(default)]
  gems: usize,
  #[serde(default)]
  gold: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Log {
  player_cards: Option<GetPlayerCardsV3>,
  player_inventory: Option<GetPlayerInventory>,
  deck_lists: Option<GetDeckListsV3>,
}

#[derive(Debug)]
pub enum LogError {
  BadPayload,
}

impl fmt::Display for LogError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "log error")
  }
}

impl Error for LogError {
  fn description(&self) -> &str {
    match self {
      &Self::BadPayload => "bad payload",
    }
  }

  fn source(&self) -> Option<&(dyn Error + 'static)> {
    None
  }
}

lazy_static! {
  static ref NAME_LOOKUP: HashMap<&'static String, Vec<&'static Card>> = ALL_CARDS.group_by_name();
}

impl Log {
  pub fn from_str(log: &str) -> Result<Self, LogError> {
    lazy_static! {
        //https://regex101.com/r/OluNfe/3
        static ref GET_PLAYER_CARDS_V3_REGEX : Regex =
            Regex::new(r"<== PlayerInventory.GetPlayerCardsV3 (?P<data>.*)")
                .expect("Failed to compile GET_PLAYER_CARDS_V3_REGEX");
          static ref GET_PLAYER_INVENTORY_REGEX : Regex =
            Regex::new(r"<== PlayerInventory.GetPlayerInventory (?P<data>.*)")
            .expect("Failed to compile GET_PLAYER_INVENTORY_REGEX");
          static ref GET_DECK_LISTS_V3_REGEX: Regex =
            Regex::new(r"<== Deck.GetDeckListsV3 (?P<data>.*)")
            .expect("Failed to compile GET_DECK_LISTS_V3_REGEX");
    }
    let cursor = std::io::Cursor::new(log);
    let lines_iter = cursor.lines().map(|l| l.unwrap());
    let mut player_cards: Vec<GetPlayerCardsV3> = Vec::new();
    let mut player_inventory: Vec<GetPlayerInventory> = Vec::new();
    let mut deck_lists: Vec<GetDeckListsV3> = Vec::new();
    for line in lines_iter {
      if let Some(caps) = GET_PLAYER_CARDS_V3_REGEX.captures(&line) {
        let data = &caps["data"];
        if let Ok(data) = serde_json::from_str(data) {
          player_cards.push(data);
        } else {
          warn!("bad player cards");
        }
      } else if let Some(caps) = GET_PLAYER_INVENTORY_REGEX.captures(&line) {
        let data = &caps["data"];
        if let Ok(data) = serde_json::from_str(data) {
          player_inventory.push(data);
        } else {
          warn!("bad player inventory");
        }
      } else if let Some(caps) = GET_DECK_LISTS_V3_REGEX.captures(&line) {
        let data = &caps["data"];
        if let Ok(data) = serde_json::from_str(data) {
          deck_lists.push(data);
        } else {
          warn!("bad deck lists");
        }
      }
    }
    Ok(Self {
      player_cards: player_cards.last().map(|c| c.clone()),
      player_inventory: player_inventory.last().map(|c| c.clone()),
      deck_lists: deck_lists.last().map(|c| c.clone()),
    })
  }

  pub fn wc_common_count(&self) -> usize {
    self
      .player_inventory
      .as_ref()
      .map(|o| o.payload.wc_common_count)
      .unwrap_or(0)
  }

  pub fn wc_uncommon_count(&self) -> usize {
    self
      .player_inventory
      .as_ref()
      .map(|o| o.payload.wc_uncommon_count)
      .unwrap_or(0)
  }

  pub fn wc_rare_count(&self) -> usize {
    self
      .player_inventory
      .as_ref()
      .map(|o| o.payload.wc_rare_count)
      .unwrap_or(0)
  }

  pub fn wc_mythic_count(&self) -> usize {
    self
      .player_inventory
      .as_ref()
      .map(|o| o.payload.wc_mythic_count)
      .unwrap_or(0)
  }

  pub fn gems(&self) -> usize {
    self
      .player_inventory
      .as_ref()
      .map(|o| o.payload.gems)
      .unwrap_or(0)
  }

  pub fn gold(&self) -> usize {
    self
      .player_inventory
      .as_ref()
      .map(|o| o.payload.gold)
      .unwrap_or(0)
  }

  pub fn collection(&self) -> Result<Deck, LogError> {
    let mut builder = DeckBuilder::new();
    if let Some(player_cards) = &self.player_cards {
      for (arena_id_str, count) in &player_cards.payload {
        let arena_id = arena_id_str.parse::<u64>().expect("parse to u64 works");
        if let Some(id_name) = ARENA_2_SCRYFALL.get(&arena_id) {
          let name = &id_name.1;
          let card = Card::clone(
            NAME_LOOKUP
              .get(name)
              .expect("name lookup must work")
              .first()
              .expect("nothing"),
          );
          // This should never happen
          if card.arena_id != 0 && card.arena_id != arena_id {
            warn!("{:?} but got {}", card, arena_id);
            unreachable!();
          }
          //let split: Vec<_> = card.name.split("//").collect();
          //card.name = split.first().expect("ok").trim().to_string();
          builder = builder.insert_count(card, *count);
        } else {
          warn!("No scryfall id for arena id {}", arena_id);
        }
      }
    }
    Ok(builder.build())
  }

  pub fn player_decks(&self) -> Result<Vec<Deck>, LogError> {
    let mut results = Vec::new();
    if let Some(player_decks) = &self.deck_lists {
      for player_deck in &player_decks.payload {
        let mut builder = DeckBuilder::new();
        // Every 2 items in player_deck.main_deck correspond to an arena id + card count
        for id_count in player_deck.main_deck.chunks(2) {
          let arena_id = id_count[0];
          let count = id_count[1] as usize;
          if let Some(id_name) = ARENA_2_SCRYFALL.get(&arena_id) {
            let id = &id_name.0;
            let name = &id_name.1;
            if !id.is_empty() {
              let card = Card::clone(
                NAME_LOOKUP
                  .get(name)
                  .expect("name lookup must work")
                  .first()
                  .expect("nothing"),
              );
              // This should never happen
              if card.arena_id != 0 && card.arena_id != arena_id {
                warn!("{:?} but got {}", card, arena_id);
                unreachable!();
              }
              //let split: Vec<_> = card.name.split("//").collect();
              //card.name = split.first().expect("ok").trim().to_string();
              builder = builder.insert_count(card, count);
            } else {
              warn!("No scryfall id for arena id {}", arena_id);
            }
          } else {
            warn!(
              "Cannot find https://api.scryfall.com/cards/arena/{}",
              arena_id
            );
          }
        }
        let mut deck = builder.build();
        deck.title = Some(player_deck.name.clone());
        results.push(deck);
      }
    }
    Ok(results)
  }
}
