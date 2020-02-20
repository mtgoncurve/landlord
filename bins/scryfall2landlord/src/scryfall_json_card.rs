use landlord::card::{Card, CardKind, ManaColor, ManaCost};
use landlord::parse_mana_costs;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScryfallJsonCard {
    pub name: String,
    #[serde(default)]
    pub mana_cost: String,
    #[serde(default)]
    pub oracle_text: String,
    #[serde(default)]
    pub type_line: String,
    #[serde(default)]
    pub color_identity: HashSet<ManaColor>,
    #[serde(default)]
    pub legalities: HashMap<String, Legality>,
    #[serde(default)]
    pub image_uris: HashMap<String, String>,
    #[serde(default)]
    pub cmc: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_faces: Vec<ScryfallJsonCard>,
    // NOTE(jshrake): SCRYFALL_JSON_URL only contains cards with a unique
    // oracle_id, else we would use this value to ensure unique cards
    //pub oracle_id: String,
    // NOTE(jshrake): SCRYFALL_JSON_URL only contains english cards
    // else we would use this value to select only english cards for now
    //pub lang: String,
    // NOTE(jshrake): SCRYFALL_JSON_URL contains the latest print of a card
    // else we would use this value to select the latest released card
    //pub released_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialOrd, PartialEq)]
pub enum Legality {
    #[serde(rename = "legal")]
    Legal,
    #[serde(rename = "not_legal")]
    NotLegal,
    #[serde(rename = "banned")]
    Banned,
    #[serde(rename = "restricted")]
    Restricted,
    #[serde(other)]
    Other,
}

// We use Scryfall's color_identity attribute to determine the color sources
// of a land card. In some cases, this is incorrect. Rather than parse the
// the oracle text, we simply keep a map of land cards and the mana cost
// we wish them to represent
lazy_static! {
    static ref SPECIAL_LANDS: HashMap<&'static str, ManaCost> = [
        (
            "Slayers' Stronghold",
            ManaCost::from_rgbuwc(0, 0, 0, 0, 0, 1)
        ),
        (
            "Alchemist's Refuge",
            ManaCost::from_rgbuwc(0, 0, 0, 0, 0, 1)
        ),
        (
            "Desolate Lighthouse",
            ManaCost::from_rgbuwc(0, 0, 0, 0, 0, 1)
        ),
        // fetch lands
        (
            "Arid Mesa",
            ManaCost::from_rgbuwc(1, 0, 0, 0, 1, 0)
        ),
        (
            "Bloodstained Mire",
            ManaCost::from_rgbuwc(1, 0, 1, 0, 0, 0)
        ),
        (
            "Flooded Strand",
            ManaCost::from_rgbuwc(0, 0, 0, 1, 1, 0)
        ),
        (
            "Marsh Flats",
            ManaCost::from_rgbuwc(0, 0, 1, 0, 1, 0)
        ),
        (
            "Misty Rainforest",
            ManaCost::from_rgbuwc(0, 1, 0, 1, 0, 0)
        ),
        (
            "Polluted Delta",
            ManaCost::from_rgbuwc(0, 0, 1, 1, 0, 0)
        ),
        (
            "Scalding Tarn",
            ManaCost::from_rgbuwc(1, 0, 0, 1, 0, 0)
        ),
        (
            "Verdant Catacombs",
            ManaCost::from_rgbuwc(0, 1, 1, 0, 0, 0)
        ),
        (
            "Windswept Heath",
            ManaCost::from_rgbuwc(0, 1, 0, 0, 1, 0)
        ),
        (
            "Wooded Foothills",
            ManaCost::from_rgbuwc(1, 1, 0, 0, 0, 0)
        ),
    ]
    .iter()
    .copied()
    .collect();
}

impl Into<Card> for ScryfallJsonCard {
    fn into(self) -> Card {
        let kind;
        let mana_cost;
        let all_mana_costs;
        let is_land = self.type_line.contains("Land");
        if is_land {
            fn is_color_01(card: &ScryfallJsonCard, color: ManaColor) -> u8 {
                if card.color_identity.contains(&color)
                    || (color == ManaColor::Colorless && card.color_identity.is_empty())
                    || (card.oracle_text.contains("Add one mana of any color.")
                        && !card
                            .oracle_text
                            .contains("Add one mana of any color. Spend this mana only"))
                {
                    1
                } else {
                    0
                }
            }
            mana_cost = if let Some(cost) = SPECIAL_LANDS.get::<str>(&self.name) {
                info!("Special case: {} using mana cost {:?}", self.name, cost);
                *cost
            } else {
                ManaCost::from_rgbuwc(
                    is_color_01(&self, ManaColor::Red),
                    is_color_01(&self, ManaColor::Green),
                    is_color_01(&self, ManaColor::Black),
                    is_color_01(&self, ManaColor::Blue),
                    is_color_01(&self, ManaColor::White),
                    is_color_01(&self, ManaColor::Colorless),
                )
            };
            let is_check = self
                .oracle_text
                .contains("enters the battlefield tapped unless you control a");
            let is_shock = self
                .oracle_text
                .contains("enters the battlefield, you may pay 2 life.");
            let is_tap = self.oracle_text.contains("enters the battlefield tapped.");
            let is_basic = self.type_line.contains("Basic Land");
            if is_shock {
                kind = CardKind::ShockLand;
            } else if is_check {
                kind = CardKind::CheckLand;
            } else if is_tap {
                kind = CardKind::TapLand;
            } else if is_basic {
                kind = CardKind::BasicLand;
            } else {
                kind = CardKind::OtherLand;
            }
            all_mana_costs = vec![mana_cost];
        } else {
            kind = CardKind::Unknown;
            all_mana_costs = parse_mana_costs(&self.mana_cost).into_iter().collect();
            mana_cost = ManaCost::from_rgbuwc(
                all_mana_costs[0].r,
                all_mana_costs[0].g,
                all_mana_costs[0].b,
                all_mana_costs[0].u,
                all_mana_costs[0].w,
                all_mana_costs[0].c,
            );
        }
        let name = self.name.trim().to_lowercase();
        let image_uri = match self.image_uris.get("normal") {
            None => "",
            Some(uri) => uri,
        }
        .to_string();
        // Calculate the earliest turn to play the card. By default, turn corresponds
        // to the CMC of the card (0 cost cards are played on t1)
        let turn =
            mana_cost.r + mana_cost.g + mana_cost.b + mana_cost.u + mana_cost.w + mana_cost.c;
        let turn = std::cmp::max(1, turn);
        let mut s = DefaultHasher::new();
        name.hash(&mut s);
        let hash = s.finish();

        Card {
            name,
            hash,
            mana_cost_string: self.mana_cost,
            image_uri,
            kind,
            turn,
            mana_cost,
            all_mana_costs,
        }
    }
}
