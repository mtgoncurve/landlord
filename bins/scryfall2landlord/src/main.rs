extern crate bincode;
extern crate flate2;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate landlord;

use flate2::write::GzEncoder;
use flate2::Compression;
use landlord::card::{Card, Legality};
use landlord::collection::Collection;
use landlord::scryfall::ScryfallCard;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::Path;

#[derive(Debug)]
enum Error {
    Json(serde_json::Error),
    Bincode(bincode::Error),
    Io(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<bincode::Error> for Error {
    fn from(error: bincode::Error) -> Self {
        Self::Bincode(error)
    }
}

fn main() -> Result<(), Error> {
    let _ = env_logger::try_init();
    let args: Vec<String> = env::args().collect();
    assert!(args.len() > 2, "Expected 2 arguments, URI and output path");
    let uri_string = &args[1];
    let out_path_string = &args[2];

    let uri_path = Path::new(uri_string);
    info!("Loading JSON file @ {}", uri_string);
    let mut json_file_contents = String::new();
    File::open(uri_path)?.read_to_string(&mut json_file_contents)?;
    let json_val = serde_json::from_str(&json_file_contents)?;
    info!("Deserializing Scryfall JSON");
    let mut scryfall_cards: Vec<ScryfallCard> = serde_json::from_value(json_val)?;
    // Filter out any cards that are not legal in all formats
    // This should filter out any tokens
    // See https://github.com/mtgoncurve/landlord/issues/4
    scryfall_cards = scryfall_cards
        .into_iter()
        .filter(|c| c.legalities.values().any(|l| l != &Legality::NotLegal))
        .collect();
    // Flatten the card_faces out into scryfall_cards
    // To do that, we clone and update the image_uris to that of the parent card
    let mut card_faces = Vec::with_capacity(500);
    for card in &scryfall_cards {
        for face in &card.card_faces {
            let mut face = face.clone();
            // Copy various attributes from the parent card to the face
            if face.image_uris.is_empty() {
                face.image_uris = card.image_uris.clone();
            }
            face.set = card.set;
            face.oracle_id = card.oracle_id.clone();
            face.id = card.id.clone();
            face.rarity = card.rarity;
            face.collector_number = card.collector_number.clone();
            card_faces.push(face);
        }
    }
    scryfall_cards.extend(card_faces);
    info!("Generating landlord output");
    let landlord_cards: Vec<Card> = scryfall_cards.into_iter().map(|c| c.into()).collect();
    let collection = Collection::from_cards(landlord_cards);
    info!("Running bincode::serialize on output");
    let encoded_collection = bincode::serialize(&collection)?;
    info!("Writing AllSets.landlord");
    let file: File = OpenOptions::new()
        .write(true)
        .create(true)
        .open(out_path_string)
        .unwrap();
    let mut e = GzEncoder::new(file, Compression::default());
    info!("Compressing");
    e.write_all(&encoded_collection[..])?;
    e.finish()?;
    Ok(())
}
