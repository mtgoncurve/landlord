extern crate bincode;
extern crate flate2;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate landlord;

use landlord::arena::{DataCard, DataLoc, IsoCode};
use landlord::card::SetCode;
use landlord::data::*;
use std::collections::HashMap;

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
struct CardSetKey {
  pub name: String,
  pub set: SetCode,
}

// Keep data files here on macos
#[cfg(target_os = "macos")]
fn data_directory() -> std::path::PathBuf {
  ["arena-data"].iter().collect()
}

// Same as Windows, but for running under WSL
#[cfg(target_os = "linux")]
fn data_directory() -> std::path::PathBuf {
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

// Default install location on windows 10
#[cfg(target_os = "windows")]
fn data_directory() -> std::path::PathBuf {
  [
    "C:",
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
  // !!!!!!!!!!!!!!!!!
  // data_loc_path and data_card_path are downloaded by the game
  // client.
  // TODO: Search for these files by iterating through each file in data_dir
  let data_dir = data_directory();
  let data_loc_path: std::path::PathBuf =
    data_dir.join("data_loc_3bd5b82dadbd15fd73622330b3396c64.mtga");
  let data_card_path: std::path::PathBuf =
    data_dir.join("data_cards_7c6e2fd8116d32ea30df234867f770c8.mtga");
  // !!!!!!!!!!!!!!!!!
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
  let scryfall_names = ALL_CARDS.group_by_name();
  let mut results = HashMap::new();
  for data_card in &data_cards {
    let arena_id = data_card.grpid;
    let titleid = data_card.titleid;
    let title = string_lookup.get(&titleid).expect("ok");
    let title_lower = title.to_lowercase();
    let arena_set_string = data_card.set.to_uppercase();
    let arena_set = arena_set_string.parse::<SetCode>().unwrap();
    // Ignore uncraftable cards
    if !data_card.is_craftable {
      debug!(
        "Skipping uncraftable {} {} ({})",
        title, arena_set_string, arena_id
      );
      continue;
    }
    let cards = scryfall_names.get(&title_lower);
    // Does the title lookup fail?
    if cards.is_none() {
      warn!("Could not find card in scryfall data by name \"{}\"", title);
      continue;
    }
    let cards = cards.unwrap();
    let find_card_idx = || {
      // Check if one of the cards has a matching arena id
      // if so, that's our card
      for (i, card) in cards.iter().enumerate() {
        if card.arena_id == arena_id {
          return Some(i);
        }
      }
      for (i, card) in cards.iter().enumerate() {
        if card.set == arena_set {
          return Some(i);
        }
      }
      return None;
    };
    let card_idx = find_card_idx();
    if card_idx.is_none() {
      warn!(
        "Could not resolve scryfall id for card/set/arena id: {} {:?} {}",
        title, arena_set, arena_id
      );
      continue;
    }
    let card_idx = card_idx.unwrap();
    let found_card = cards[card_idx];
    results.insert(arena_id, (found_card.id.clone(), title_lower));
  }
  let results_rev: HashMap<String, u64> = results.iter().map(|(k, v)| (v.0.clone(), *k)).collect();
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
