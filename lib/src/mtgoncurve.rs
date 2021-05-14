//! # https://mtgoncurve.com interface
//!
//! Defines the interface between landlord and [https://mtgoncurve.com](https://mtgoncurve.com)
use crate::card::{Card, CardKind, ManaColorCount, ManaCost};
use crate::data::ALL_CARDS;
use crate::deck::Deck;
use crate::mulligan::London;
use crate::simulation::{Observations, Simulation, SimulationConfig};

use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[derive(Debug, Serialize, Deserialize)]
enum Error {
    BadDeckcode(String),
    BadCardNameInRow(usize, String),
    EmptyDeckcode,
}

/// Input format expected from https://mtgoncurve.com
#[derive(Debug, Serialize, Deserialize)]
struct Input {
    /// The decklist code
    pub code: String,
    /// The number of runs to perform
    pub runs: usize,
    /// True if we play first, false if we play second
    pub on_the_play: bool,
    /// The maximum number of cards we are willing to mulligan down to
    pub mulligan_down_to: usize,
    /// We mulligan any hand that contains a land count found in mulligan_on_lands
    pub mulligan_on_lands: HashSet<usize>,
    #[doc(hidden)]
    pub acceptable_hand_list: Vec<Vec<String>>,
}

/// Output format expected by https://mtgoncurve.com
#[derive(Debug, Serialize, Deserialize)]
struct Output {
    pub card_observations: Vec<CardObservation>,
    pub land_counts: Vec<CardObservation>,
    pub deck_size: usize,
    pub accumulated_opening_hand_size: usize,
    pub accumulated_opening_hand_land_count: usize,
    pub deck_average_cmc: f64,

    pub total_land_counts: ManaColorCount,
    pub basic_land_counts: ManaColorCount,
    pub tap_land_counts: ManaColorCount,
    pub check_land_counts: ManaColorCount,
    pub shock_land_counts: ManaColorCount,
    pub other_land_counts: ManaColorCount,
    pub non_land_counts: ManaColorCount,
}

#[derive(Debug, Serialize, Deserialize)]
struct CardObservation {
    card: MtgOnCurveCard,
    cmc: u8,
    card_count: usize,
    observations: Observations,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct MtgOnCurveCard {
    /// String representing the card name
    pub name: String,
    /// String representing the card mana cost, in "{X}{R}{R}" style format
    pub mana_cost_string: String,
    /// A URI to an image of the card
    pub image_uri: String,
    /// The card type
    pub kind: CardKind,
    /// A hash of the card name
    pub hash: u64,
    /// The turn to play the card, defaults to mana_cost.cmc()
    pub turn: u8,
    /// ManaCost representation of the card mana cost
    pub mana_cost: ManaCost,
}

impl From<&Card> for MtgOnCurveCard {
    fn from(card: &Card) -> Self {
        Self {
            name: card.name.clone(),
            mana_cost_string: card.mana_cost_string.clone(),
            image_uri: card.image_uri.clone(),
            kind: card.kind,
            hash: card.hash,
            turn: card.turn,
            mana_cost: card.mana_cost,
        }
    }
}

/// Runs a simulation given input
/// Assumes that input deserializes into a valid `Input`, and returns a serialized `Output`
/// # Example
///
///  ```js
///  const input = {...};
///  const output = require('@mtgoncurve/landlord').run(input);
///  console.log(output);
///  ```
#[wasm_bindgen]
pub fn mtgoncurve_run(input: &JsValue) -> JsValue {
    let input: Input = match input.into_serde() {
        Err(e) => {
            return JsValue::from_str(&format!("Error deserializing simulation inputs: {:#?}", e));
        }
        Ok(v) => v,
    };
    let result = match run_impl(&input) {
        Err(e) => {
            return JsValue::from_str(&format!("Error running simulation for input: {:#?}", e));
        }
        Ok(v) => v,
    };
    JsValue::from_serde(&result).expect("this can't fail")
}

fn run_impl(input: &Input) -> Result<Output, Error> {
    let deck = match Deck::from_list(&input.code) {
        Err(e) => return Err(Error::BadDeckcode(e.0)),
        Ok(deck) => deck,
    };
    if deck.is_empty() {
        return Err(Error::EmptyDeckcode);
    }
    let highest_turn = deck
        .iter()
        .fold(0, |max, c| std::cmp::max(max, c.card.turn as usize));
    let mut mulligan = London::never();
    mulligan.mulligan_down_to = input.mulligan_down_to;
    mulligan.mulligan_on_lands = input.mulligan_on_lands.clone();
    for (i, acceptable_hand) in input.acceptable_hand_list.iter().enumerate() {
        let mut keep_cards = HashSet::new();
        for card_name in acceptable_hand {
            if let Some(card) = ALL_CARDS.card_from_name(&card_name) {
                keep_cards.insert(card.hash);
            } else {
                return Err(Error::BadCardNameInRow(i, card_name.clone()));
            }
        }
        if !keep_cards.is_empty() {
            mulligan.acceptable_hand_list.push(keep_cards);
        }
    }
    let sim = Simulation::from_config(&SimulationConfig {
        run_count: input.runs,
        draw_count: highest_turn,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: input.on_the_play,
    });
    let mut outputs = Output::new();
    outputs.accumulated_opening_hand_size = sim.accumulated_opening_hand_size;
    outputs.accumulated_opening_hand_land_count = sim.accumulated_opening_hand_land_count;

    outputs.card_observations = deck
        .iter()
        .filter(|c| !c.card.is_land())
        .map(|c| {
            let card = &c.card;
            let count = c.count;
            let o = sim.observations_for_card_by_turn(&card, card.turn as usize);
            let cmc = card.mana_cost.cmc();
            CardObservation {
                card: card.into(),
                cmc,
                card_count: count,
                observations: o,
            }
        })
        .collect();
    // Return the collection sorted by CMC and then by Name
    outputs
        .card_observations
        .sort_by(|a, b| a.card.name.cmp(&b.card.name));
    outputs
        .card_observations
        .sort_by(|a, b| a.card.mana_cost.cmc().cmp(&b.card.mana_cost.cmc()));

    outputs.land_counts = deck
        .iter()
        .filter(|c| c.card.is_land())
        .map(|c| {
            let cmc = c.card.mana_cost.cmc();
            CardObservation {
                card: (&c.card).into(),
                cmc,
                card_count: c.count,
                observations: Observations::new(),
            }
        })
        .collect();
    // Return the collection sorted by kind and then by Name
    outputs
        .land_counts
        .sort_by(|a, b| a.card.name.cmp(&b.card.name));
    outputs
        .land_counts
        .sort_by(|a, b| a.card.kind.cmp(&b.card.kind));

    let deck_len = deck.len();
    // Calculate the other statistics
    outputs.deck_size = deck_len;
    outputs.deck_average_cmc = if deck_len == 0 {
        0.0
    } else {
        let n = deck
            .iter()
            .filter(|c| !c.card.is_land())
            .fold(0, |accum, c| accum + c.count);
        deck.iter()
            .filter(|c| !c.card.is_land())
            .map(|c| c.count * (c.card.mana_cost.cmc() as usize))
            .sum::<usize>() as f64
            / n as f64
    };

    for cc in deck.iter() {
        for _ in 0..cc.count {
            let card = &cc.card;
            if card.is_land() {
                outputs.total_land_counts.count(&card.mana_cost);
            }
            match card.kind {
                CardKind::BasicLand => outputs.basic_land_counts.count(&card.mana_cost),
                CardKind::CheckLand => outputs.check_land_counts.count(&card.mana_cost),
                CardKind::TapLand => outputs.tap_land_counts.count(&card.mana_cost),
                CardKind::ShockLand => outputs.shock_land_counts.count(&card.mana_cost),
                CardKind::OtherLand => outputs.other_land_counts.count(&card.mana_cost),
                _ => outputs.non_land_counts.count(&card.mana_cost),
            }
        }
    }
    Ok(outputs)
}

impl Default for ManaColorCount {
    fn default() -> Self {
        Self::new()
    }
}

impl Output {
    fn new() -> Self {
        Self {
            card_observations: Vec::new(),
            land_counts: Vec::new(),
            accumulated_opening_hand_size: 0,
            accumulated_opening_hand_land_count: 0,
            deck_average_cmc: 0.0,
            deck_size: 0,

            total_land_counts: ManaColorCount::new(),
            basic_land_counts: ManaColorCount::new(),
            check_land_counts: ManaColorCount::new(),
            tap_land_counts: ManaColorCount::new(),
            other_land_counts: ManaColorCount::new(),
            shock_land_counts: ManaColorCount::new(),
            non_land_counts: ManaColorCount::new(),
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::mtgoncurve::*;

    // The following tests confirm numbers from the tables in the article
    // https://www.channelfireball.com/articles/how-many-colored-mana-sources-do-you-need-to-consistently-cast-your-spells-a-guilds-of-ravnica-update/
    // While this article was based on the vancouver mulligan, it is still relevant today!
    macro_rules! karsten_check_raw {
        ($observations:expr, $name:expr, $expected:expr, $thresh:expr) => {{
            let o = $observations
                .iter()
                .find(|o| o.card.name.to_lowercase() == $name.to_lowercase())
                .unwrap_or_else(|| {
                    panic!("No card named: {}", $name);
                });
            let actual = o.observations.p_mana_given_cmc();
            let difference = f64::abs($expected - actual);
            assert!(difference < $thresh);
        }};
    }

    macro_rules! karsten_check {
        ($observations:expr, $name:expr, $expected:expr) => {{
            karsten_check_raw!($observations, $name, $expected, 0.015)
        }};
    }

    #[test]
    fn test_0() {
        let code = "
            1 Appetite for Brains
            1 Abnormal Endurance
            1 Bloodghast
            1 Ammit Eternal
            1 Blood Operative
            1 Doomsday
            1 Ancient Craving
            1 Akuta, Born of Ash
            1 Grave Pact
            1 Anointed Deacon
            1 Phyrexian Obliterator
            1 Aku Djinn
            1 Hellfire
            1 Bogstomper
            1 Acid-Spewer Dragon
            1 Cosmic Horror
            8 Swamp #(This corresponds to the row in the table)
            16 Detection Tower #(This needs to sum with Swamps to 24)
            20 Darksteel Colossus #(This needs to sum to 60 cards total)
        ";
        let mut mulligan_on_lands = HashSet::new();
        mulligan_on_lands.insert(0);
        mulligan_on_lands.insert(1);
        mulligan_on_lands.insert(6);
        mulligan_on_lands.insert(7);
        let total = 1;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands,
            acceptable_hand_list: Vec::new(),
        };
        run_impl(&input).expect("simulation ok");
    }

    #[test]
    fn test_1() {
        let code = "
            4 Doom Foretold (ELD) 187
            4 Omen of the Sea (THB) 58
            4 Thought Erasure (GRN) 206
            4 Teferi, Time Raveler (WAR) 221
            3 Treacherous Blessing (THB) 117
            4 Oath of Kaya (WAR) 209
            3 Kaya's Wrath (RNA) 187
            2 Archon of Sun's Grace (THB) 3
            2 Dream Trawler (THB) 214
            1 Time Wipe (WAR) 223
            3 Dance of the Manse (ELD) 186
            4 Hallowed Fountain (RNA) 251
            4 Watery Grave (GRN) 259
            4 Godless Shrine (RNA) 248
            2 Temple of Deceit (THB) 245
            2 Temple of Silence (M20) 256
            2 Temple of Enlightenment (THB) 246
            1 Castle Ardenvale (ELD) 238
            2 Castle Vantress (ELD) 242
            2 Plains (M20) 261
            2 Swamp (M20) 272
            1 Island (M20) 267

            3 Glass Casket (ELD) 15
            2 Dovin's Veto (WAR) 193
            2 Ashiok, Dream Render (WAR) 228
            2 Revoke Existence (THB) 34
            1 Soul-Guide Lantern (THB) 237
            2 Duress (XLN) 105
            1 Devout Decree (M20) 13
            2 Banishing Light (THB) 4
        ";
        let mut mulligan_on_lands = HashSet::new();
        mulligan_on_lands.insert(0);
        mulligan_on_lands.insert(1);
        mulligan_on_lands.insert(6);
        mulligan_on_lands.insert(7);
        let total = 1;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands,
            acceptable_hand_list: Vec::new(),
        };
        run_impl(&input).expect("simulation ok");
    }
    #[test]
    fn test_2() {
        let code = "
            4 Growth Spiral (RNA) 178
            2 Murderous Rider (ELD) 97
            2 Temple of Mystery (M20) 255
            2 Temple of Malady (M20) 254
            2 Fabled Passage (ELD) 244
            4 Breeding Pool (RNA) 246
            4 Overgrown Tomb (GRN) 253
            4 Watery Grave (GRN) 259
            3 Uro, Titan of Nature's Wrath (THB) 229
            4 Medomai's Prophecy (THB) 53
            3 Elspeth's Nightmare (THB) 91
            4 Dryad of the Ilysian Grove (THB) 169
            1 Field of Ruin (THB) 242
            4 Hydroid Krasis (RNA) 183
            2 Island (ELD) 254
            4 Nissa, Who Shakes the World (WAR) 169
            2 Swamp (M20) 269
            3 Forest (M19) 280
            4 Ashiok, Nightmare Muse (THB) 208
            2 Eat to Extinction (THB) 90

            3 Mystical Dispute (ELD) 58
            3 Destiny Spinner (THB) 168
            3 Agonizing Remorse (THB) 83
            1 Casualties of War (WAR) 187
            1 Massacre Girl (WAR) 99
            2 Lovestruck Beast (ELD) 165
            1 Ashiok, Dream Render (WAR) 228
            1 Shadowspear (THB) 236
        ";
        let mut mulligan_on_lands = HashSet::new();
        mulligan_on_lands.insert(0);
        mulligan_on_lands.insert(1);
        mulligan_on_lands.insert(6);
        mulligan_on_lands.insert(7);
        let mut acceptable_hand_list = Vec::new();
        acceptable_hand_list.push(vec!["Forest".to_string()].into_iter().collect());
        let total = 1000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands,
            acceptable_hand_list,
        };
        run_impl(&input).expect("simulation ok");
    }

    #[test]
    fn krasis_for_23_bug() {
        let code = "
        1 Hydroid Krasis#X=23
        12 Island
        12 Forest
        1 Memorial to Folly
        ";
        let n = 1000;
        let input = Input {
            code: code.to_string(),
            runs: n,
            on_the_play: false,
            mulligan_down_to: 7,
            mulligan_on_lands: Default::default(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations[0];
        dbg!(&obs);
        assert_eq!(obs.observations.mana, n);
        assert_eq!(obs.observations.cmc, n);
        assert_eq!(obs.observations.play, n);
    }

    // 60 card deck, 24 lands, Sources 8
    // table: https://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-60-cards-768x209.png
    #[test]
    fn karsten_test_24_60_8() {
        let code = "
            1 Appetite for Brains
            1 Abnormal Endurance
            1 Bloodghast
            1 Ammit Eternal
            1 Blood Operative
            1 Doomsday
            1 Ancient Craving
            1 Akuta, Born of Ash
            1 Grave Pact
            1 Anointed Deacon
            1 Phyrexian Obliterator
            1 Aku Djinn
            1 Hellfire
            1 Bogstomper
            1 Acid-Spewer Dragon
            1 Cosmic Horror
            8 Swamp # (This corresponds to the row in the table)
            16 Detection Tower # (This needs to sum with Swamps to 24)
            20 Darksteel Colossus # (This needs to sum to 60 cards total)
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        // validate against sources: 8  in table
        // http://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-60-cards.png
        karsten_check!(obs, "Appetite for Brains", 0.702);
        karsten_check!(obs, "Abnormal Endurance", 0.756);
        karsten_check!(obs, "Bloodghast", 0.322);

        karsten_check!(obs, "Ammit Eternal", 0.822);
        karsten_check!(obs, "Blood Operative", 0.415);
        karsten_check!(obs, "Doomsday", 0.111);

        karsten_check!(obs, "Ancient Craving", 0.881);
        karsten_check!(obs, "Akuta, Born of Ash", 0.525);
        karsten_check!(obs, "Grave Pact", 0.177);

        karsten_check!(obs, "Anointed Deacon", 0.924);
        karsten_check!(obs, "Aku Djinn", 0.635);
        karsten_check!(obs, "Hellfire", 0.265);

        karsten_check!(obs, "Acid-Spewer Dragon", 0.953);
        karsten_check!(obs, "Bogstomper", 0.733);
        karsten_check!(obs, "Cosmic Horror", 0.368);
    }

    // 60 card deck, 24 lands, Sources 14
    // table: https://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-60-cards-768x209.png
    #[test]
    fn karsten_test_24_60_14() {
        let code = "
            1 Appetite for Brains
            1 Abnormal Endurance
            1 Bloodghast
            1 Ammit Eternal
            1 Blood Operative
            1 Doomsday
            1 Ancient Craving
            1 Akuta, Born of Ash
            1 Grave Pact
            1 Anointed Deacon
            1 Phyrexian Obliterator
            1 Aku Djinn
            1 Hellfire
            1 Bogstomper
            1 Acid-Spewer Dragon
            1 Cosmic Horror
            14 Swamp # (This corresponds to the row in the table)
            10 Detection Tower # (This needs to sum with Swamps to 24)
            20 Darksteel Colossus # (This needs to sum to 60 cards total)
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        // validate against sources 14 of table
        // http://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-60-cards.png
        karsten_check!(obs, "Appetite for Brains", 0.914);
        karsten_check!(obs, "Abnormal Endurance", 0.942);
        karsten_check!(obs, "Bloodghast", 0.680);

        karsten_check!(obs, "Ammit Eternal", 0.973);
        karsten_check!(obs, "Blood Operative", 0.798);
        karsten_check!(obs, "Doomsday", 0.440);

        karsten_check!(obs, "Ancient Craving", 0.989);
        karsten_check!(obs, "Akuta, Born of Ash", 0.892);
        karsten_check!(obs, "Grave Pact", 0.609);

        karsten_check!(obs, "Anointed Deacon", 0.996);
        karsten_check!(obs, "Aku Djinn", 0.951);
        karsten_check!(obs, "Hellfire", 0.761);

        karsten_check!(obs, "Acid-Spewer Dragon", 0.999);
        karsten_check!(obs, "Bogstomper", 0.981);
        karsten_check!(obs, "Cosmic Horror", 0.875);
    }

    // 40 card deck, 17 lands, Sources 5
    // table: https://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-40-cards-768x167.png
    #[test]
    fn karsten_test_17_40_5() {
        let code = "
            1 Appetite for Brains
            1 Abnormal Endurance
            1 Bloodghast
            1 Ammit Eternal
            1 Blood Operative
            1 Doomsday
            1 Ancient Craving
            1 Akuta, Born of Ash
            1 Grave Pact
            1 Anointed Deacon
            1 Phyrexian Obliterator
            1 Aku Djinn
            1 Hellfire
            1 Bogstomper
            1 Acid-Spewer Dragon
            1 Cosmic Horror
            5 Swamp # (This corresponds to the row in the table)
            12 Detection Tower # (This needs to sum with Swamps to 17)
            7 Darksteel Colossus # (This needs to sum to 60 cards total)
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        // validate against sources  5 of table
        // https://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-40-cards-768x167.png
        karsten_check!(obs, "Appetite for Brains", 0.671);
        karsten_check!(obs, "Abnormal Endurance", 0.728);
        karsten_check!(obs, "Bloodghast", 0.275);

        karsten_check!(obs, "Ammit Eternal", 0.794);
        karsten_check!(obs, "Blood Operative", 0.355);
        karsten_check!(obs, "Doomsday", 0.074);

        karsten_check!(obs, "Ancient Craving", 0.855);
        karsten_check!(obs, "Akuta, Born of Ash", 0.455);
        karsten_check!(obs, "Grave Pact", 0.118);

        karsten_check!(obs, "Anointed Deacon", 0.906);
        karsten_check!(obs, "Aku Djinn", 0.562);
        karsten_check!(obs, "Hellfire", 0.182);

        karsten_check!(obs, "Acid-Spewer Dragon", 0.942);
        karsten_check!(obs, "Bogstomper", 0.664);
        karsten_check!(obs, "Cosmic Horror", 0.266);
    }

    // 99 card deck, 40 lands, Sources 15
    // table: http://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-99-cards.png
    #[test]
    fn karsten_test_40_99_15() {
        let code = "
            1 Appetite for Brains
            1 Abnormal Endurance
            1 Bloodghast
            1 Ammit Eternal
            1 Blood Operative
            1 Doomsday
            1 Ancient Craving
            1 Akuta, Born of Ash
            1 Grave Pact
            1 Anointed Deacon
            1 Phyrexian Obliterator
            1 Aku Djinn
            1 Hellfire
            1 Bogstomper
            1 Acid-Spewer Dragon
            1 Cosmic Horror
            15 Swamp # (This corresponds to the row in the table)
            25 Detection Tower # (This needs to sum with Swamps to 17)
            43 Darksteel Colossus # (This needs to sum to 60 cards total)
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        // validate against sources 15 of table
        // http://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-99-cards.png
        karsten_check!(obs, "Appetite for Brains", 0.745);
        karsten_check!(obs, "Abnormal Endurance", 0.796);
        karsten_check!(obs, "Bloodghast", 0.389);

        karsten_check!(obs, "Ammit Eternal", 0.855);
        karsten_check!(obs, "Blood Operative", 0.491);
        karsten_check!(obs, "Doomsday", 0.167);

        karsten_check!(obs, "Ancient Craving", 0.905);
        karsten_check!(obs, "Akuta, Born of Ash", 0.602);
        karsten_check!(obs, "Grave Pact", 0.253);

        karsten_check!(obs, "Anointed Deacon", 0.941);
        karsten_check!(obs, "Aku Djinn", 0.706);
        karsten_check!(obs, "Hellfire", 0.361);

        karsten_check!(obs, "Acid-Spewer Dragon", 0.964);
        karsten_check!(obs, "Bogstomper", 0.793);
        karsten_check!(obs, "Cosmic Horror", 0.476);
    }

    #[test]
    fn karsten_new_mana_is_great_devarti_verify_2x_hydroid() {
        /*
        Article: https://www.channelfireball.com/articles/the-mana-in-new-standard-is-great-but-dont-stretch-it-too-far/
        Tests values in the Anothony Devarti decklist @ https://www.mtggoldfish.com/deck/1613350#arena mentioned in Fank's article
        NOTE:
        +2 Island
        -2 Memorial to Folly
        to hit 10 blue sources as mentioned in the article
        */
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183 #x=2
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        3 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        0 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        karsten_check!(obs, "Hydroid Krasis", 0.94);
    }

    #[test]
    fn karsten_new_mana_is_great_devarti_verify_4x_hydroid() {
        /*
        Article: https://www.channelfireball.com/articles/the-mana-in-new-standard-is-great-but-dont-stretch-it-too-far/
        Tests values in the Anothony Devarti decklist @ https://www.mtggoldfish.com/deck/1613350#arena mentioned in Fank's article
        NOTE:
        +2 Island
        -2 Memorial to Folly
        to hit 10 blue sources as mentioned in the article
        */
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183#X = 4
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        3 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        0 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        karsten_check!(obs, "Hydroid Krasis", 0.98);
    }

    #[test]
    fn karsten_new_mana_is_great_hobbs() {
        /*
        Article: https://www.channelfireball.com/articles/the-mana-in-new-standard-is-great-but-dont-stretch-it-too-far/
        Tests values in the Johnathan Hobbs decklist
        */
        let code = "
        4 Angel of Grace (RNA) 1
        4 Frilled Mystic (RNA) 174
        4 Growth-Chamber Guardian (RNA) 128
        1 Shalai, Voice of Plenty (DAR) 35
        2 Teferi, Hero of Dominaria (DAR) 207
        1 Forest (XLN) 277
        1 Plains (XLN) 261
        4 Breeding Pool (RNA) 246
        4 Glacial Fortress (XLN) 255
        4 Hallowed Fountain (RNA) 251
        3 Hinterland Harbor (DAR) 240
        1 Memorial to Genius (DAR) 243
        4 Sunpetal Grove (XLN) 257
        4 Temple Garden (GRN) 258
        4 History of Benalia (DAR) 21
        3 Seal Away (DAR) 31
        1 Blink of an Eye (DAR) 46
        1 Chemister's Insight (GRN) 32
        2 Depose // Deploy (RNA) 225
        1 March of the Multitudes (GRN) 188
        2 Settle the Wreckage (XLN) 34
        2 Spell Pierce (XLN) 81
        2 Syncopate (DAR) 67
        1 Warrant // Warden (RNA) 230
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        dbg!(obs);
        karsten_check!(obs, "Frilled Mystic", 0.819);
        karsten_check!(obs, "History of Benalia", 0.882);
    }

    #[test]
    fn karsten_new_mana_is_great_magnuson() {
        /*
        Article: https://www.channelfireball.com/articles/the-mana-in-new-standard-is-great-but-dont-stretch-it-too-far/
        Tests values in the Max Magnuson decklist
        */
        let code = "
       4 Benalish Marshal (DAR) 6
        4 Dauntless Bodyguard (DAR) 14
        4 Deputy of Detention (RNA) 165
        1 Healer's Hawk (GRN) 14
        4 Hunted Witness (GRN) 15
        4 Snubhorn Sentry (RIX) 23
        4 Tithe Taker (RNA) 27
        4 Venerated Loxodon (GRN) 30
        13 Plains (XLN) 261
        4 Glacial Fortress (XLN) 255
        4 Hallowed Fountain (RNA) 251
        1 Conclave Tribunal (GRN) 6
        4 History of Benalia (DAR) 21
        1 Unbreakable Formation (RNA) 29
        4 Legion's Landing (XLN) 22
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        karsten_check!(obs, "Deputy of Detention", 0.858);
        karsten_check!(obs, "Benalish Marshal", 1.0);
    }

    #[test]
    fn karsten_new_mana_is_great_magnuson_replacement() {
        /*
        Article: https://www.channelfireball.com/articles/the-mana-in-new-standard-is-great-but-dont-stretch-it-too-far/
        Tests values in the Max Magnuson decklist
        -1 Plains
        +1 Island
        */
        let code = "
        4 Benalish Marshal (DAR) 6
        4 Dauntless Bodyguard (DAR) 14
        4 Deputy of Detention (RNA) 165
        1 Healer's Hawk (GRN) 14
        4 Hunted Witness (GRN) 15
        4 Snubhorn Sentry (RIX) 23
        4 Tithe Taker (RNA) 27
        4 Venerated Loxodon (GRN) 30
        12 Plains (XLN) 261
        4 Glacial Fortress (XLN) 255
        4 Hallowed Fountain (RNA) 251
        1 Conclave Tribunal (GRN) 6
        4 History of Benalia (DAR) 21
        1 Unbreakable Formation (RNA) 29
        4 Legion's Landing (XLN) 22
        1 Island
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        karsten_check!(obs, "Deputy of Detention", 0.896);
        karsten_check!(obs, "Benalish Marshal", 0.941);
    }

    #[test]
    fn karsten_new_mana_is_great_nick_cowden() {
        /*
        Article: https://www.channelfireball.com/articles/the-mana-in-new-standard-is-great-but-dont-stretch-it-too-far/
        Tests values in the nick cowden deck
        */
        let code = "
        1 Chromium, the Mutable (M19) 214
        4 Teferi, Hero of Dominaria (DAR) 207
        1 Island (XLN) 265
        1 Plains (XLN) 261
        2 Swamp (XLN) 269
        4 Drowned Catacomb (XLN) 253
        4 Glacial Fortress (XLN) 255
        3 Godless Shrine (RNA) 248
        4 Hallowed Fountain (RNA) 251
        4 Isolated Chapel (DAR) 241
        4 Watery Grave (GRN) 259
        4 Absorb (RNA) 151
        3 Cast Down (DAR) 81
        1 Chemister's Insight (GRN) 32
        1 Moment of Craving (RIX) 79
        2 Mortify (RNA) 192
        1 Negate (RIX) 44
        3 Precognitive Perception (RNA) 45
        4 Syncopate (DAR) 67
        4 Vraska's Contempt (XLN) 129
        2 Search for Azcanta (XLN) 74
        3 Kaya's Wrath (RNA) 187
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        karsten_check!(obs, "Absorb", 0.826);
        karsten_check!(obs, "Kaya's Wrath", 0.822);
    }

    #[test]
    fn karsten_misc_check_0() {
        /*
        60 card, 24 land deck, 4 tap lands
        Use http://227rsi2stdr53e3wto2skssd7xe-wpengine.netdna-ssl.com/wp-content/uploads/2018/10/How-many-sources-60-cards.png
        */
        let code = "
Deck
4 Burglar Rat (GRN) 64
4 Yarok's Fenlurker (M20) 123
4 Kroxa, Titan of Death's Hunger (THB) 221
4 Plaguecrafter (GRN) 82
4 Woe Strider (THB) 123
1 Davriel, Rogue Shadowmage (WAR) 83
4 Nightmare Shepherd (THB) 108
4 Gray Merchant of Asphodel (THB) 99
4 Temple of Malice (THB) 247
4 Forest
1 Mountain (ELD) 262
11 Swamp (ELD) 258
4 Castle Locthwain (ELD) 241
2 Drag to the Underworld (THB) 89
4 Tymaret Calls the Dead (THB) 118
1 Bolas's Citadel (WAR) 79
        ";
        let total = 100000;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        let results = run_impl(&input).expect("simulation ok");
        let obs = &results.card_observations;
        karsten_check!(obs, "Burglar Rat", 0.991); // 1C
        karsten_check!(obs, "Yarok's Fenlurker", 0.888); // CC
        karsten_check!(obs, "Plaguecrafter", 0.998); // 2C
        karsten_check!(obs, "Drag to the Underworld", 0.991); // 2CC
        karsten_check!(obs, "Gray Merchant of Asphodel", 0.999); // 3CC
        karsten_check!(obs, "Bolas's Citadel", 0.996); // 3CCC
    }

    #[test]
    fn single_zero_mana_card() {
        let code = "
        1 Ancestral Vision
        ";
        let total = 10;
        let input = Input {
            code: code.to_string(),
            runs: total,
            on_the_play: true,
            mulligan_down_to: 5,
            mulligan_on_lands: vec![0, 1, 6, 7].into_iter().collect(),
            acceptable_hand_list: Default::default(),
        };
        run_impl(&input).expect("simulation ok");
    }
}
