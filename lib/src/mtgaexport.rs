use crate::arena::Log;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[derive(Debug, Serialize, Deserialize)]
enum Error {
  BadDate,
  BadArenaLog,
  BadCollection,
}

#[derive(Debug, Serialize, Deserialize)]
struct Output {
  pub cards_in_collection: usize,
  pub wc_mythic: usize,
  pub wc_rare: usize,
  pub wc_uncommon: usize,
  pub wc_common: usize,
  pub gems: usize,
  pub gold: usize,
  pub mtggoldfish_string: String,
}

#[wasm_bindgen]
pub fn mtgaexport_run(arena_log: &str) -> JsValue {
  let result = match mtgaexport_run_impl(arena_log) {
    Err(e) => {
      return JsValue::from_str(&format!("Error running simulation for input: {:#?}", e));
    }
    Ok(v) => v,
  };
  JsValue::from_serde(&result).expect("this can't fail")
}

fn mtgaexport_run_impl(arena_log: &str) -> Result<Output, Error> {
  let log = Log::from_str(arena_log).map_err(|_| Error::BadArenaLog)?;
  let collection = log.collection().map_err(|_| Error::BadCollection)?;
  let mtggoldfish_header = "Card,Set ID,Set Name,Quantity,Foil\n";
  let mut mtggoldfish_string =
    String::with_capacity(collection.len() * (3 + 3 + 1 + 1 + 20 + mtggoldfish_header.len())); // 3 comma, 3 letters for set code, 1 for the count, 1 for the newline, 20 for the name
  mtggoldfish_string.push_str(mtggoldfish_header);
  for cc in collection.iter() {
    mtggoldfish_string.push_str(&cc.card.name);
    mtggoldfish_string.push_str(",");
    mtggoldfish_string.push_str(&cc.card.set.to_string());
    mtggoldfish_string.push_str(",");
    mtggoldfish_string.push_str(",");
    mtggoldfish_string.push_str(&cc.count.to_string());
    mtggoldfish_string.push_str("\n");
  }

  Ok(Output {
    cards_in_collection: collection.len(),
    wc_mythic: log.wc_mythic_count(),
    wc_rare: log.wc_rare_count(),
    wc_uncommon: log.wc_uncommon_count(),
    wc_common: log.wc_common_count(),
    gems: log.gems(),
    gold: log.gold(),
    mtggoldfish_string,
  })
}
