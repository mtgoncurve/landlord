use crate::card::ManaColorCount;
use crate::data::*;
use crate::deck::Deck;
use time::Date;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

// Import the `window.alert` function from the Web.
#[wasm_bindgen]
extern "C" {
  fn alert(s: &str);
}

#[derive(Debug, Serialize, Deserialize)]
enum Error {
  BadDate,
  BadCollection,
}

/// Output format expected by https://mtgawildspend.com
#[derive(Debug, Serialize, Deserialize)]
struct Output {
  pub decks: Vec<DeckInfo>,
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

#[wasm_bindgen]
pub fn mtgawildspend_run(today_str: &str, collection: &str) -> JsValue {
  let result = match run_impl(today_str, collection) {
    Err(e) => {
      return JsValue::from_str(&format!("Error running simulation for input: {:#?}", e));
    }
    Ok(v) => v,
  };
  JsValue::from_serde(&result).expect("this can't fail")
}

fn run_impl(today_str: &str, _collection: &str) -> Result<Output, Error> {
  let today = Date::parse(today_str, "%F").map_err(|_| Error::BadDate)?;
  let mut results = Vec::new();
  for deck in NET_DECKS.iter() {
    let d = DeckResult::from_deck(deck, today);
    results.push(DeckInfo {
      deck: d,
      need: None,
      have: None,
    })
  }
  results.sort_unstable_by(|a, b| {
    b.deck
      .days_remaining
      .partial_cmp(&a.deck.days_remaining)
      .unwrap()
  });
  Ok(Output { decks: results })
}
