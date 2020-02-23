extern crate bincode;
extern crate flate2;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate serde_derive;
extern crate landlord;
#[macro_use]
extern crate lazy_static;
extern crate regex;

//use landlord::card::Card;
//use std::fs::File;
//use std::fs::OpenOptions;
//use std::path::Path;
use flate2::read::GzDecoder;
use landlord::card::Collection;
use regex::Regex;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::io::prelude::*;
use std::io::BufRead;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerCardsV3Payload {
    id: u64,
    payload: BTreeMap<String, u8>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Log {
    collection: Option<GetPlayerCardsV3Payload>,
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
            collection: collections.last().map(|c| c.clone()),
        })
    }
}

/// Returns a new collection of all cards from data/all_cards.landlord
fn all_cards() -> Result<Collection, bincode::Error> {
    let b = include_bytes!("../../../data/all_cards.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}

/*
fn net_decks() -> Result<Vec<(String, String, Vec<(String, String, Collection)>)>, bincode::Error> {
    let b = include_bytes!("../../../data/net_decks.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}
*/

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let app_data = env::var("APP_DATA").expect("$APP_DATA should be set");
    let log_path: std::path::PathBuf = [
        &app_data,
        "LocalLow",
        "Wizards of The Coast",
        "MTGA",
        "output_log.txt",
    ]
    .iter()
    .collect();
    info!("Opening log file @ {:?}", log_path);
    let log_string = std::fs::read_to_string(log_path.as_path())?;
    let log = Log::from_str(&log_string)?;
    let all_cards = all_cards()?.sort_by_arena_id();
    let mut result = BTreeMap::new();
    if let Some(collection) = log.collection {
        for (arena_id_str, count) in collection.payload {
            let arena_id = arena_id_str.parse::<u64>().expect("parse to u64 works");
            if let Some(card) = all_cards.card_from_arena_id(arena_id) {
                result.insert(card.hash, count);
            } else {
                warn!(
                    "Cannot find https://api.scryfall.com/cards/arena/{}",
                    arena_id
                );
            }
        }
    }
    info!("Collection: {:?}", result);
    Ok(())
}
