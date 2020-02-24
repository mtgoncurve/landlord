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
extern crate time;

//use landlord::card::Card;
//use std::fs::File;
//use std::fs::OpenOptions;
//use std::path::Path;
use flate2::read::GzDecoder;
use landlord::card::{Card, Collection};
use landlord::deck::{Deck, DeckBuilder};
use regex::Regex;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::io::prelude::*;
use std::io::BufRead;
use time::Date;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerCardsV3Payload {
    id: u64,
    payload: BTreeMap<String, usize>,
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

fn net_decks() -> Result<Vec<Deck>, bincode::Error> {
    let b = include_bytes!("../../../data/net_decks.landlord");
    let mut gz = GzDecoder::new(&b[..]);
    let mut s: Vec<u8> = Vec::new();
    gz.read_to_end(&mut s).expect("gz decode failed");
    bincode::deserialize(&s)
}

fn arena2scryfall() -> HashMap<u64, (Option<String>, String)> {
    let s = include_str!("../../../data/arena2scryfall.json");
    serde_json::from_str(s).expect("arena2scryfall.json deserialize always works")
}

fn correct_wrong_mtggoldfish_set_codes(deck: &mut Deck, date: Date) {
    for cc in &mut deck.cards {
        let mut card = &mut cc.card;
        if card.set.in_standard() {
            continue;
        }
        let current = card.set;
        for other in &ALL_CARDS.cards {
            if other.hash == card.hash
                && other.set.in_standard()
                && other.set.time_remaining_in_standard(date)
                    > card.set.time_remaining_in_standard(date)
            {
                card.set = other.set;
            }
        }
        debug!(
            "Fix card \"{}\" set code from {:?} to {:?}",
            card.name, current, card.set
        );
    }
}

pub fn build_deck(collection: &Deck, deck: &Deck) -> (Deck, Deck) {
    let mut have = DeckBuilder::new();
    let mut need = DeckBuilder::new();
    for need_cc in &deck.cards {
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

lazy_static! {
    pub static ref ALL_CARDS: Collection = all_cards().expect("all_cards() failed");
    pub static ref NET_DECKS: Vec<Deck> = net_decks().expect("net_decks() failed");
}

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
    let oracle_id_lookup = all_cards.group_by_oracle_id();
    let name_lookup = all_cards.group_by_name();
    let arena_2_oracle_id = arena2scryfall();

    let mut builder = DeckBuilder::new();

    if let Some(collection) = log.collection {
        for (arena_id_str, count) in collection.payload {
            let arena_id = arena_id_str.parse::<u64>().expect("parse to u64 works");

            if let Some(p) = arena_2_oracle_id.get(&arena_id) {
                let name = &p.1;
                if let Some(oracle_id) = &p.0 {
                    let mut card = Card::clone(oracle_id_lookup.get(oracle_id).expect("ok"));
                    if &card.name != name {
                        card = Card::clone(name_lookup.get(name).expect("ok").first().expect("ok"));
                    }
                    //let split: Vec<_> = card.name.split("//").collect();
                    //card.name = split.first().expect("ok").trim().to_string();
                    builder = builder.insert_count(card, count);
                } else {
                    warn!("No oracle id for {}", arena_id);
                }
            } else {
                warn!(
                    "Cannot find https://api.scryfall.com/cards/arena/{}",
                    arena_id
                );
            }
        }
    }
    let collection = builder.build();
    println!("{:?}", collection);
    let today = Date::today();
    let mut ranked = Vec::new();
    let mut decks = net_decks()?;
    for (i, mut deck) in decks.iter_mut().enumerate() {
        correct_wrong_mtggoldfish_set_codes(&mut deck, today);
        let time_left = deck.average_time_remaining_in_standard(today);
        ranked.push((i, time_left));
    }
    ranked.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    for (i, time_left) in ranked {
        let deck = &decks[i];
        let title = deck.title.as_ref().unwrap();
        let url = deck.url.as_ref().unwrap();
        let (_have, need) = build_deck(&collection, &deck);
        info!(
            "{} average number of days remaining in standard: {}",
            title, time_left,
        );
        info!("\t{}", url);
        info!(
            "\tPrice: mythic {} // rare {} // uncommon {} // common {}",
            deck.mythic_count(),
            deck.rare_count(),
            deck.uncommon_count(),
            deck.common_count()
        );
        info!(
            "\tCost:  mythic {} // rare {} // uncommon {} // common {}",
            need.mythic_count(),
            need.rare_count(),
            need.uncommon_count(),
            need.common_count()
        );
        info!(
            "\tNeeded: average number of days remaining in standard: {}",
            need.average_time_remaining_in_standard(today)
        );
        for cc in &need.cards {
            info!(
                "\tNeed {}x {} ({:?} days left)",
                cc.count,
                cc.card.name,
                cc.card.set.time_remaining_in_standard(today).whole_days()
            );
        }
    }
    Ok(())
}
