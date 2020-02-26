use crate::card::*;
use crate::data::*;
use crate::deck::*;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::BufRead;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerCardsV3Payload {
  id: u64,
  payload: HashMap<String, usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Log {
  player_cards_payload: Option<GetPlayerCardsV3Payload>,
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

impl Log {
  pub fn from_str(log: &str) -> Result<Self, LogError> {
    lazy_static! {
        //https://regex101.com/r/OluNfe/3
        static ref GET_PLAYER_CARDS_V3_REGEX : Regex =
            Regex::new(r"^.*<== PlayerInventory.GetPlayerCardsV3\s?(?P<payload>.*)")
                .expect("Failed to compile GET_PLAYER_CARDS_V3_REGEX");
    }
    let cursor = std::io::Cursor::new(log);
    let lines_iter = cursor.lines().map(|l| l.unwrap());
    let mut collections: Vec<GetPlayerCardsV3Payload> = Vec::new();
    for line in lines_iter {
      if let Some(caps) = GET_PLAYER_CARDS_V3_REGEX.captures(&line) {
        let payload = &caps["payload"];
        if let Ok(payload) = serde_json::from_str(payload) {
          collections.push(payload);
        }
      }
    }
    Ok(Self {
      player_cards_payload: collections.last().map(|c| c.clone()),
    })
  }

  pub fn collection(&self) -> Result<Deck, LogError> {
    lazy_static! {
      static ref ID_LOOKUP: HashMap<&'static String, &'static Card> = ALL_CARDS.group_by_id();
      static ref NAME_LOOKUP: HashMap<&'static String, Vec<&'static Card>> =
        ALL_CARDS.group_by_name();
    }

    let mut builder = DeckBuilder::new();
    if let Some(player_cards_payload) = &self.player_cards_payload {
      for (arena_id_str, count) in &player_cards_payload.payload {
        let arena_id = arena_id_str.parse::<u64>().expect("parse to u64 works");
        if let Some(id_name) = ARENA_2_SCRYFALL.get(&arena_id) {
          let id = &id_name.0;
          let name = &id_name.1;
          if !id.is_empty() {
            let mut card = Card::clone(ID_LOOKUP.get(id).expect("id lookup must work"));
            // Ugh. We found the card but it might have a weird name (like the adventure cards)
            // whatever. search again via a name_lookup and just take the first entry...
            if &card.name != name {
              debug!(
                "Found card by id w/ name \"{}\", but expected \"{}\"",
                card.name, name
              );
              card = Card::clone(
                NAME_LOOKUP
                  .get(name)
                  .expect("name lookup must work")
                  .first()
                  .expect("nothing"),
              );
            }
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
        } else {
          warn!(
            "Cannot find https://api.scryfall.com/cards/arena/{}",
            arena_id
          );
        }
      }
    }
    Ok(builder.build())
  }
}
