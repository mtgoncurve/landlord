extern crate bincode;
extern crate flate2;
extern crate serde;
#[macro_use]
extern crate log;
#[macro_use]
extern crate landlord;

extern crate reqwest;
extern crate select;

use flate2::write::GzEncoder;
use flate2::Compression;
use landlord::card::*;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;

macro_rules! fetch {
  ($url:expr) => {{
    std::thread::sleep(std::time::Duration::from_secs(1));
    info!("Fetching {}", $url);
    reqwest::blocking::get($url)?.text()?
  }};
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  env_logger::init();
  let args: Vec<String> = env::args().collect();
  let default_out = String::from("data/net_decks.landlord");
  let out_path_string = args.get(1).unwrap_or(&default_out);

  let formats: Vec<&'static str> = vec![
    "standard",
    /*
      "modern",
      "pioneer",
      "pauper",
      "legacy",
      "vintage",
      "penny_dreaful",
      "commander_1v1",
      "commander",
      "brawl",
      "arena_standard",
      "historic",
    */
  ];

  let mut results = Vec::new();
  for format in &formats {
    let mut format_results = Vec::with_capacity(20);
    let entry_url = "https://www.mtggoldfish.com/metagame/standard/full#paper";
    let entry_html_text = fetch!(entry_url);
    let entry_doc = Document::from(entry_html_text.as_str());

    let deck_url_nodes: Vec<_> = entry_doc
      .find(Class("deck-price-paper").descendant(Name("a")))
      .collect();
    let deck_data: Vec<_> = deck_url_nodes
      .iter()
      .map(|node| (node.text(), node.attr("href").expect("href attribute")))
      .collect();
    let deck_data: Vec<_> = deck_data
      .into_iter()
      .filter(|(_, url)| url.starts_with("/archetype"))
      .collect();

    for (title, deck_url_path) in &deck_data {
      let deck_url = format!("https://www.mtggoldfish.com{}", deck_url_path);
      std::thread::sleep(std::time::Duration::from_secs(3));
      let deck_text = fetch!(&deck_url);
      let deck_doc = Document::from(deck_text.as_str());
      let down_url_paths: Vec<_> = deck_doc
        .find(Class("deck-view-tool-btn"))
        .map(|node| node.attr("href"))
        .filter_map(|node| node)
        .map(|href| href)
        .collect();
      let down_url_path = down_url_paths
        .into_iter()
        .filter(|url| url.starts_with("/deck/arena_download"))
        .next()
        .expect("/deck/arena_download");
      let down_url = format!("https://www.mtggoldfish.com{}", down_url_path);
      std::thread::sleep(std::time::Duration::from_secs(3));
      let down_text = fetch!(&down_url);
      let down_doc = Document::from(down_text.as_str());
      let deck_text = down_doc
        .find(Class("copy-paste-box"))
        .next()
        .expect("copy-paste-box to exist")
        .text();
      let deck = decklist!(&deck_text);
      assert!(!deck.cards.is_empty());
      info!(
        "Recording deck {} with card length {}",
        title,
        deck.cards.len()
      );
      format_results.push((title.clone(), deck));
    }
    results.push((format.clone(), format_results));
  }
  info!("Writing compressing bincode to {}", out_path_string);
  let encoded_collection = bincode::serialize(&results)?;
  let file: File = OpenOptions::new()
    .write(true)
    .create(true)
    .open(out_path_string)
    .unwrap();
  let mut e = GzEncoder::new(file, Compression::default());
  e.write_all(&encoded_collection[..])?;
  e.finish()?;
  Ok(())
}
