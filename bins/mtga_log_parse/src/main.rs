#[macro_use]
extern crate log;
extern crate env_logger;
extern crate landlord;
extern crate time;

use landlord::arena::Log;
use landlord::card::CardKind;
use landlord::data::*;
use std::collections::HashMap;
#[cfg(not(target_os = "macos"))]
use std::env;
use time::OffsetDateTime;

#[cfg(target_os = "macos")]
fn data_dir() -> std::path::PathBuf {
    ["arena-data", "output_log.txt"].iter().collect()
}

#[cfg(not(target_os = "macos"))]
fn data_dir() -> std::path::PathBuf {
    let app_data = env::var("APP_DATA").expect("$APP_DATA should be set");
    [
        &app_data,
        "LocalLow",
        "Wizards of The Coast",
        "MTGA",
        "output_log.txt",
    ]
    .iter()
    .collect()
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let log_path = data_dir();
    info!("Opening log file @ {:?}", log_path);
    let log_string = std::fs::read_to_string(log_path.as_path())?;
    let log = Log::from_str(&log_string)?;
    let collection = log.collection().expect("error parsing collection from log");
    let today = OffsetDateTime::now_local().date();
    info!("Mythic wild cards {}", log.wc_mythic_count());
    info!("Rare wild cards {}", log.wc_rare_count());
    info!("Uncommon wild cards {}", log.wc_uncommon_count());
    info!("Common wild cards {}", log.wc_common_count());
    info!("Gems {}", log.gems());
    info!("Gold {}", log.gold());
    let mut decks = net_decks()?;
    decks.extend(log.player_decks().unwrap());
    {
        let mut ranked = Vec::new();
        for (i, deck) in decks.iter_mut().enumerate() {
            let time_left = deck.average_time_remaining_in_standard(today);
            ranked.push((i, time_left));
        }
        ranked.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (i, time_left) in ranked {
            let deck = &decks[i];
            let title = deck.title.as_ref().unwrap();
            let url = &deck.url;
            let (_have, need) = deck.have_need(&collection);
            info!(
                "-[{}]({:?}): {:.0}",
                title,
                url.as_ref().unwrap_or(&"".to_string()),
                time_left
            );
            info!(
                "Price: mythic {:02} // rare {:02} // uncommon {:02} // common {:02}",
                deck.mythic_count(),
                deck.rare_count(),
                deck.uncommon_count(),
                deck.common_count()
            );
            info!(
                "Craft: mythic {:02} // rare {:02} // uncommon {:02} // common {:02}",
                need.mythic_count(),
                need.rare_count(),
                need.uncommon_count(),
                need.common_count()
            );
            for cc in &need.cards {
                info!(
                    "\t{} {} # {:?} days remaining",
                    cc.count,
                    cc.card.name,
                    cc.card.set.time_remaining_in_standard(today).whole_days()
                );
            }
            info!(
                "\tAverage days remaining: {}",
                need.average_time_remaining_in_standard(today)
            );
        }
    }

    // find the most popular card missing from my collection
    let mut card_count = HashMap::new();
    for deck in &decks {
        for cc in &deck.cards {
            if cc.card.kind == CardKind::BasicLand {
                continue;
            }
            let count = card_count.entry(&cc.card).or_insert(0);
            *count += 1; //cc.count;
        }
    }
    Ok(())
}
