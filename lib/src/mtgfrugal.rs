use crate::arena::Log;
use crate::card::ManaColorCount;
use crate::data::*;
use crate::deck::Deck;
use time::Date;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[derive(Debug, Serialize, Deserialize)]
enum Error {
  BadDate,
  BadArenaLog,
  BadCollection,
}

/// Output format expected by https://mtgawildspend.com
#[derive(Debug, Serialize, Deserialize)]
struct Output {
  pub decks: Vec<DeckInfo>,
  pub cards_in_collection: usize,
  pub wc_mythic: usize,
  pub wc_rare: usize,
  pub wc_uncommon: usize,
  pub wc_common: usize,
  pub gems: usize,
  pub gold: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeckInfo {
  pub deck: DeckResult,
  pub have: Option<DeckResult>,
  pub need: Option<DeckResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeckResult {
  pub deck: Deck,
  pub days_remaining: f64,
  pub mythic_count: usize,
  pub rare_count: usize,
  pub uncommon_count: usize,
  pub common_count: usize,
  pub lands_mana_count: ManaColorCount,
  pub nonlands_mana_count: ManaColorCount,
  pub craftables_mana_count: ManaColorCount,
}

impl DeckResult {
  fn from_deck(deck: &Deck, today: Date) -> DeckResult {
    DeckResult {
      deck: deck.clone(),
      days_remaining: deck.average_time_remaining_in_standard(today),
      mythic_count: deck.mythic_count(),
      rare_count: deck.rare_count(),
      uncommon_count: deck.uncommon_count(),
      common_count: deck.common_count(),
      lands_mana_count: deck.mana_counts_for_lands(),
      nonlands_mana_count: deck.mana_counts_for_nonlands(),
      craftables_mana_count: deck.mana_counts_for_craftables(),
    }
  }
}

/*
#[wasm_bindgen]
pub fn arena_log_parse(arena_log: &str) -> JsValue {
  let log = Log::from_str(arena_log);
  match log {
    Err(_) => JsValue::from_str(&format!("Error parsing log file")),
    Ok(v) => JsValue::from_serde(&v).expect("ok"),
  }
}
*/

#[wasm_bindgen]
pub fn mtgfrugal_run(today_str: &str, arena_log: &str) -> JsValue {
  let result = match run_impl(today_str, arena_log) {
    Err(e) => {
      return JsValue::from_str(&format!("Error running simulation for input: {:#?}", e));
    }
    Ok(v) => v,
  };
  JsValue::from_serde(&result).expect("this can't fail")
}

fn run_impl(today_str: &str, arena_log: &str) -> Result<Output, Error> {
  let today = Date::parse(today_str, "%F").map_err(|_| Error::BadDate)?;
  let log = Log::from_str(arena_log).map_err(|_| Error::BadArenaLog)?;
  let collection = log.collection().map_err(|_| Error::BadCollection)?;
  let mut results = Vec::new();
  for deck in NET_DECKS.iter() {
    let d = DeckResult::from_deck(deck, today);
    let (have, need) = deck.have_need(&collection);
    let h = DeckResult::from_deck(&have, today);
    let n = DeckResult::from_deck(&need, today);
    results.push(DeckInfo {
      deck: d,
      have: Some(h),
      need: Some(n),
    })
  }
  results.sort_unstable_by(|a, b| {
    b.deck
      .days_remaining
      .partial_cmp(&a.deck.days_remaining)
      .unwrap()
  });
  Ok(Output {
    decks: results,
    cards_in_collection: collection.len(),
    wc_mythic: log.wc_mythic_count(),
    wc_rare: log.wc_rare_count(),
    wc_uncommon: log.wc_uncommon_count(),
    wc_common: log.wc_common_count(),
    gems: log.gems(),
    gold: log.gold(),
  })
}
