//! # Magic: The Gathering Simulation Library
//!
//! landlord is a library that simulates the card draw and mulligan process in Magic: The Gathering
//! in order to determine the probability to play cards on curve. It can theoretically be used
//! be used for determining the probability of other events. It is currently used by [https://mtgoncurve.com](https://mtgoncurve.com).

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate flate2;
extern crate rand;
extern crate regex;
extern crate wasm_bindgen;
#[macro_use]
extern crate log;

pub mod arena;
#[macro_use]
pub mod card;
#[macro_use]
pub mod deck;
mod bipartite;
pub mod collection;
pub mod data;
pub mod hand;
pub mod mulligan;
pub mod prelude;
pub mod scryfall;
pub mod simulation;

// mtgoncurve.com
mod mtgoncurve;
pub use crate::mtgoncurve::mtgoncurve_run;
