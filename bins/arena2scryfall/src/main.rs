extern crate bincode;
extern crate flate2;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate landlord;
#[macro_use]
extern crate lazy_static;

use flate2::read::GzDecoder;
use landlord::arena::{DataCard, DataLoc, IsoCode};
use landlord::card::{Collection, SetCode};
use std::collections::HashMap;
use std::env;
use std::io::prelude::*;

/// Returns a new collection of all cards from data/all_cards.landlord
fn all_cards() -> Result<Collection, bincode::Error> {
  let b = include_bytes!("../../../data/all_cards.landlord");
  let mut gz = GzDecoder::new(&b[..]);
  let mut s: Vec<u8> = Vec::new();
  gz.read_to_end(&mut s).expect("gz decode failed");
  bincode::deserialize(&s)
}

lazy_static! {
  pub static ref ALL_CARDS: Collection = all_cards().expect("all_cards() failed");
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
struct CardSetKey {
  pub name: String,
  pub set: SetCode,
}

#[cfg(target_os = "macos")]
fn data_dir() -> std::path::PathBuf {
  ["arena-data"].iter().collect()
}

#[cfg(target_os = "linux")]
fn data_dir() -> std::path::PathBuf {
  let app_data = env::var("APP_DATA").expect("$APP_DATA should be set");
  [
    "/mnt",
    "c",
    "Program Files (x86)",
    "Wizards of The Coast",
    "MTGA",
    "MTGA_Data",
    "Downloads",
    "Data",
  ]
  .iter()
  .collect()
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
  env_logger::init();
  let _args: Vec<String> = env::args().collect();
  let data_loc_path: std::path::PathBuf =
    data_dir().join("data_loc_3bd5b82dadbd15fd73622330b3396c64.mtga");
  let data_card_path: std::path::PathBuf =
    data_dir().join("data_cards_7c6e2fd8116d32ea30df234867f770c8.mtga");
  let data_loc_string = std::fs::read_to_string(data_loc_path.as_path())?;
  let data_locs: Vec<DataLoc> = serde_json::from_str(&data_loc_string)?;
  let data_card_string = std::fs::read_to_string(data_card_path.as_path())?;
  let data_cards: Vec<DataCard> = serde_json::from_str(&data_card_string)?;
  let string_lookup = {
    let data_loc = data_locs
      .iter()
      .find(|&loc| loc.iso_code == IsoCode::EnUS)
      .expect("ok");
    let mut m = HashMap::new();
    for k in &data_loc.keys {
      m.insert(k.id, k.text.clone());
    }
    m
  };
  let all_cards = all_cards()?;
  let card_lookup = all_cards.group_by_name();
  let mut results = HashMap::new();
  for data_card in &data_cards {
    let titleid = data_card.titleid;
    let title = string_lookup.get(&titleid).expect("ok");
    let title_lower = title.to_lowercase();
    let arena_id = data_card.grpid;
    let arena_set_string = data_card.set.to_uppercase();
    let arena_set = arena_set_string.parse::<SetCode>().unwrap();
    let scryfall_id = {
      if let Some(cards) = card_lookup.get(&title_lower) {
        let mut id = None;
        let mut check_by_set = true;
        for card in cards {
          if card.arena_id == arena_id {
            id = Some(card.id.clone());
            check_by_set = false;
            break;
          }
        }
        if check_by_set {
          for card in cards {
            if card.set == arena_set {
              id = Some(card.id.clone());
              break;
            }
          }
        }
        id
      } else {
        None
      }
    };
    if scryfall_id.is_none() {
      warn!(
        "Could not resolve scryfall oracle id for card/set/arena id: {} {:?} {}",
        title, arena_set, arena_id
      );
    }
    results.insert(arena_id, (scryfall_id, title_lower));
  }
  let nullkey = String::from("null");
  let results_rev: HashMap<String, u64> = results
    .iter()
    .map(|(k, v)| (v.0.as_ref().unwrap_or(&nullkey).clone(), *k))
    .collect();
  serde_json::to_writer(
    &std::fs::File::create("data/arena2scryfall.json")?,
    &results,
  )?;
  serde_json::to_writer(
    &std::fs::File::create("data/scryfall2arena.json")?,
    &results_rev,
  )?;
  info!("Resolved {}/{} cards", results_rev.len(), results.len());
  Ok(())
}
