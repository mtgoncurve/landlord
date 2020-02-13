//! # Card representation and deck list parsing
//!
use crate::parse_mana_costs::parse_mana_costs;
use flate2::read::GzDecoder;
use regex::Regex;
use std::hash::{Hash, Hasher};
use std::io::prelude::*;

#[derive(Debug)]
pub struct DeckcodeError(pub String);

/// A Collection represents a deck or a library of cards
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub cards: Vec<Card>,
}

/// Card represents a Magic: The Gathering card
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Card {
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
    /// All potential mana cost combinations, for cards with split mana costs like "{R/G}"
    pub all_mana_costs: Vec<ManaCost>,
}

impl Card {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cmc(&self) -> u8 {
        self.mana_cost.cmc()
    }
    /// Returns true if the card type is a land
    #[inline]
    pub fn is_land(&self) -> bool {
        self.kind.is_land()
    }
}

impl PartialEq for Card {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Card {}

impl Hash for Card {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

/// CardKind represents an internal card type representation.
/// It is a superset of the [official card types](https://mtg.gamepedia.com/Card_type)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CardKind {
    // Lands
    BasicLand = 0,
    TapLand = 1,
    CheckLand = 2,
    ShockLand = 3,
    OtherLand = 4,
    ForcedLand = 5,
    // Other
    Creature,
    Spell,
    Enchantment,
    Instant,
    Planeswalker,
    Sorcery,
    Artifact,
    Unknown,
}

/// ManaCost represents the card [mana cost](https://mtg.gamepedia.com/Mana_cost)
#[derive(
    Default, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct ManaCost {
    pub bits: u8,
    pub r: u8,
    pub w: u8,
    pub b: u8,
    pub u: u8,
    pub g: u8,
    pub c: u8,
}

/// ManaColor represents a [color](https://mtg.gamepedia.com/Color)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManaColor {
    #[serde(rename = "R")]
    Red = 0,
    #[serde(rename = "G")]
    Green = 1,
    #[serde(rename = "B")]
    Black = 2,
    #[serde(rename = "U")]
    Blue = 3,
    #[serde(rename = "W")]
    White = 4,
    #[serde(other)]
    Colorless = 5,
}

impl Default for CardKind {
    fn default() -> Self {
        Self::Unknown
    }
}

impl ManaCost {
    /// Returns a new ManaCost worth 0 CMC
    pub fn new() -> Self {
        Self {
            bits: 0,
            r: 0,
            w: 0,
            b: 0,
            u: 0,
            g: 0,
            c: 0,
        }
    }

    /// Returns a new ManaCost with the given color counts
    pub fn from_rgbuwc(r: u8, g: u8, b: u8, u: u8, w: u8, c: u8) -> Self {
        Self {
            bits: Self::calculate_signature_rgbuwc(r, g, b, u, w, c),
            r,
            w,
            b,
            u,
            g,
            c,
        }
    }

    /// Returns the amount of color overlap between self and other
    #[inline]
    pub fn color_contribution(&self, other: &ManaCost) -> u32 {
        (self.bits & other.bits).count_ones()
    }

    /// Returns the converted mana cost
    #[inline]
    pub fn cmc(self) -> u8 {
        self.r + self.w + self.b + self.u + self.g + self.c
    }

    #[inline]
    pub fn update_bits(mut self) -> Self {
        self.bits =
            Self::calculate_signature_rgbuwc(self.r, self.g, self.b, self.u, self.w, self.c);
        self
    }

    #[inline]
    fn calculate_signature_rgbuwc(r: u8, g: u8, b: u8, u: u8, w: u8, c: u8) -> u8 {
        use std::cmp::min;
        (min(1, r) << 0 & Self::R_BITS)
            | (min(1, g) << 1 & Self::G_BITS)
            | (min(1, b) << 2 & Self::B_BITS)
            | (min(1, u) << 3 & Self::U_BITS)
            | (min(1, w) << 4 & Self::W_BITS)
            | (min(1, c) << 5 & Self::C_BITS)
    }

    pub const R_BITS: u8 = 0b0000_0001;
    pub const G_BITS: u8 = 0b0000_0010;
    pub const B_BITS: u8 = 0b0000_0100;
    pub const U_BITS: u8 = 0b0000_1000;
    pub const W_BITS: u8 = 0b0001_0000;
    pub const C_BITS: u8 = 0b0010_0000;
}

impl From<Card> for ManaCost {
    fn from(item: Card) -> Self {
        item.mana_cost
    }
}

impl ManaColor {
    pub fn from_str(color: &str) -> Self {
        match color.chars().next() {
            Some('B') => Self::Black,
            Some('U') => Self::Blue,
            Some('G') => Self::Green,
            Some('R') => Self::Red,
            Some('W') => Self::White,
            _ => Self::Colorless,
        }
    }
}

impl CardKind {
    /// Returns true if self is any of the land types
    #[inline]
    pub fn is_land(self) -> bool {
        self == Self::BasicLand
            || self == Self::ShockLand
            || self == Self::CheckLand
            || self == Self::BasicLand
            || self == Self::TapLand
            || self == Self::OtherLand
            || self == Self::ForcedLand
    }
}

impl Collection {
    /// Returns a new collection of all cards defined in AllCards.landlord
    pub fn all() -> Result<Self, bincode::Error> {
        // NOTE(jshrake): This file is generated!
        // Run scryfall2landlord to generate this file
        // See the `make card-update` task in the top-level Makefile
        let b = include_bytes!("./AllCards.landlord");
        let mut gz = GzDecoder::new(&b[..]);
        let mut s: Vec<u8> = Vec::new();
        gz.read_to_end(&mut s).expect("gz decode failed");
        bincode::deserialize(&s)
    }

    /// Returns a new collection of cards
    pub fn from_cards(cards: Vec<Card>) -> Self {
        Self { cards }
    }

    /// Returns a card from the card name
    #[inline]
    pub fn card_from_name(&self, name: &str) -> Option<&Card> {
        let name_lowercase = name.to_lowercase();
        self.cards
            .iter()
            .find(|card| card.name.to_lowercase() == name_lowercase)
    }

    /// Returns (Mainboard, Sideboard) collections from a deck list string exported by the Magic: The Gathering Arena game client
    pub fn from_deck_list(&self, code: &str) -> Result<(Self, Self), DeckcodeError> {
        lazy_static! {
            //https://regex101.com/r/OluNfe/3
            static ref ARENA_LINE_REGEX: Regex =
                Regex::new(r"^\s*(?P<amount>\d+)\s+(?P<name>[^\(#\n\r]+)(?:\s*\((?P<set>\w+)\)\s+(?P<setnum>\d+))?\s*#?(?:\s*[Xx]\s*=\s*(?P<X>\d+))?(?:\s*[Tt]\s*=\s*(?P<T>\d+))?(?:\s*[Mm]\s*=\s*(?P<M>[RGWUB\d{}]+))?")
                    .expect("Failed to compile ARENA_LINE_REGEX regex");
        }
        let mut main = Vec::with_capacity(100);
        let mut side = Vec::with_capacity(100);
        let mut cards = &mut main;
        for line in code.trim().lines() {
            let trimmed = line.trim();
            // An empty line divides the main board cards from the side board cards
            if trimmed.is_empty() {
                cards = &mut side;
                continue;
            }
            // Ignore reserved words
            if trimmed == "Deck" {
                continue;
            }
            if trimmed == "Sideboard" {
                cards = &mut side;
                continue;
            }
            if trimmed == "Maybeboard" {
                continue;
            }
            if trimmed == "Commander" {
                continue;
            }
            // Ignore line comments
            if trimmed.starts_with('#') {
                continue;
            }
            let caps = ARENA_LINE_REGEX.captures(trimmed).ok_or_else(|| {
                DeckcodeError(format!("Cannot regex capture deck list line: {}", line))
            })?;
            let amount = caps["amount"].parse::<usize>().or_else(|_| {
                Err(DeckcodeError(format!(
                    "Cannot parse usize card amount from deck list line: {}",
                    line
                )))
            })?;
            let name = caps["name"].trim().to_string();
            // By default, we represent split cards with the left face
            let left_card_name = name
                .split("//")
                .next()
                .ok_or_else(|| {
                    DeckcodeError(format!(
                        "Cannot parse card name from deck list line: {}",
                        line
                    ))
                })?
                .trim()
                .to_string();
            let card = self.card_from_name(&left_card_name).ok_or_else(|| {
                DeckcodeError(format!("Cannot find card named \"{}\" in collection", name))
            })?;
            // Clone the card as mutable so we can apply modifiers
            let mut card = card.clone();
            // Handle the X = modifier
            if let Some(x_val) = caps.name("X") {
                // Only modify the colorless mana cost if the mana cost string contains an X value
                // otherwise ignore the attribute
                if card.mana_cost_string.contains('X') {
                    let x_val = x_val.as_str().parse::<u8>().or_else(|_| {
                        Err(DeckcodeError(format!(
                            "Cannot parse u8 X= value from deck list line: {}",
                            line
                        )))
                    })?;
                    card.mana_cost.c = x_val;
                    card.all_mana_costs
                        .iter_mut()
                        .for_each(|cost| cost.c = x_val);
                    card.mana_cost_string = card.mana_cost_string.replace('X', &x_val.to_string());
                    card.turn = card.mana_cost.cmc();
                }
            }
            // Handle the M = modifier
            if let Some(m_val) = caps.name("M") {
                let mana_cost_str = m_val.as_str();
                let all_mana_costs = parse_mana_costs(mana_cost_str);
                if all_mana_costs.is_empty() {
                    return Err(DeckcodeError(format!(
                        "Problematic mana cost ('M = ') specifed at line {}",
                        line
                    )));
                }
                card.mana_cost = all_mana_costs[0];
                card.all_mana_costs = all_mana_costs;
                card.turn = card.mana_cost.cmc();
                card.kind = CardKind::ForcedLand;
            }
            // Hanlde the T = modifier
            if let Some(turn_val) = caps.name("T") {
                // TODO(jshrake): Set the desired turn to play this card
                let turn_val = turn_val.as_str().parse::<u8>().or_else(|_| {
                    Err(DeckcodeError(format!(
                        "Cannot parse u8 T= value from deck list line: {}",
                        line
                    )))
                })?;
                card.turn += turn_val;
            }
            card.name = name;
            for _ in 0..amount {
                cards.push(card.clone());
            }
        }
        Ok((Self::from_cards(main), Self::from_cards(side)))
    }

    /// Returns the number of cards in the collection
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Returns true if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use crate::card::*;

    lazy_static! {
        static ref ALL_CARDS: Collection = Collection::all().expect("Collection::all failed");
    }

    #[test]
    fn card_field_of_ruin() {
        let card = ALL_CARDS
            .card_from_name("Field of Ruin")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.turn, 1);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_carnival_carnage() {
        let card = ALL_CARDS.card_from_name("Carnival").unwrap();
        assert_eq!(card.turn, 1);
        assert_eq!(card.all_mana_costs[0].b, 0);
        assert_eq!(card.all_mana_costs[0].u, 0);
        assert_eq!(card.all_mana_costs[0].g, 0);
        assert_eq!(card.all_mana_costs[0].r, 1);
        assert_eq!(card.all_mana_costs[0].w, 0);
        assert_eq!(card.all_mana_costs[0].c, 0);

        assert_eq!(card.all_mana_costs[1].b, 1);
        assert_eq!(card.all_mana_costs[1].u, 0);
        assert_eq!(card.all_mana_costs[1].g, 0);
        assert_eq!(card.all_mana_costs[1].r, 0);
        assert_eq!(card.all_mana_costs[1].w, 0);
        assert_eq!(card.all_mana_costs[1].c, 0);
    }

    #[test]
    fn card_steam_vents() {
        let card = ALL_CARDS
            .card_from_name("Steam Vents")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 0);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_sulfur_falls() {
        let card = ALL_CARDS
            .card_from_name("Sulfur Falls")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::CheckLand);
        assert_eq!(card.turn, 2);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 0);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_jungle_shrine() {
        let card = ALL_CARDS
            .card_from_name("Jungle Shrine")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::TapLand);
        assert_eq!(card.turn, 3);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 0);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
    }

    #[test]
    fn card_arcades_the_strategist() {
        let card = ALL_CARDS
            .card_from_name("Arcades, the Strategist")
            .expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 4);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 1);
    }

    #[test]
    fn card_gateway_plaza() {
        let card = ALL_CARDS
            .card_from_name("Gateway Plaza")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::TapLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_guildmage_forum() {
        let card = ALL_CARDS
            .card_from_name("Guildmages' Forum")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_unclaimed_territory() {
        let card = ALL_CARDS
            .card_from_name("Unclaimed Territory")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
        assert_eq!(card.turn, 1);
    }

    #[test]
    fn card_vivid_crag() {
        let card = ALL_CARDS
            .card_from_name("Vivid Crag")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_city_of_brass() {
        let card = ALL_CARDS
            .card_from_name("City of Brass")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_ancient_ziggurat() {
        let card = ALL_CARDS
            .card_from_name("Ancient Ziggurat")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
        assert_eq!(card.turn, 1);
    }

    #[test]
    fn card_mana_confluence() {
        let card = ALL_CARDS
            .card_from_name("Mana Confluence")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_unkown_shores() {
        let card = ALL_CARDS
            .card_from_name("Unknown Shores")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_rupture_spire() {
        let card = ALL_CARDS
            .card_from_name("Rupture Spire")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::TapLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.turn, 6);
    }

    #[test]
    fn card_ghalta_primal_hunger() {
        let card = ALL_CARDS
            .card_from_name("Ghalta, Primal Hunger")
            .expect("can't find card");
        assert_eq!(card.turn, 12);
        assert_eq!(card.is_land(), false);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 10);
        assert_eq!(card.mana_cost.g, 2);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_nicol_bolas_the_ravager() {
        let card = ALL_CARDS
            .card_from_name("Nicol Bolas, the Ravager")
            .expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 4);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_swamp() {
        let card = ALL_CARDS.card_from_name("Swamp").expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 0);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_treasure_map() {
        let card = ALL_CARDS
            .card_from_name("Treasure Map")
            .expect("can't find card");
        assert_eq!(card.turn, 2);
        assert_eq!(card.is_land(), false);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 2);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_teferi_hero_of_dominaria() {
        let card = ALL_CARDS
            .card_from_name("Teferi, Hero of Dominaria")
            .expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 5);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 3);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 1);
    }

    #[test]
    fn card_syncopate() {
        let card = ALL_CARDS
            .card_from_name("Syncopate")
            .expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 2);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
    }

    #[test]
    fn card_cinder_glade() {
        let card = ALL_CARDS
            .card_from_name("Cinder Glade")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.turn, 2);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 0);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 0);
        assert_eq!(card.kind, CardKind::OtherLand);
    }

    #[test]
    fn card_discovery() {
        // NOTE(jshrake): This card has mana cost {1}{U/B}
        // Our code does not properly handle mana costs specified
        // in this fashion and treats the {U/B} as {U}
        let card = ALL_CARDS
            .card_from_name("Discovery")
            .expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 2);
        assert_eq!(card.all_mana_costs[0].b, 1);
        assert_eq!(card.all_mana_costs[0].u, 0);
        assert_eq!(card.all_mana_costs[0].c, 1);
        assert_eq!(card.all_mana_costs[0].g, 0);
        assert_eq!(card.all_mana_costs[0].r, 0);
        assert_eq!(card.all_mana_costs[0].w, 0);

        assert_eq!(card.all_mana_costs[1].b, 0);
        assert_eq!(card.all_mana_costs[1].u, 1);
        assert_eq!(card.all_mana_costs[1].c, 1);
        assert_eq!(card.all_mana_costs[1].g, 0);
        assert_eq!(card.all_mana_costs[1].r, 0);
        assert_eq!(card.all_mana_costs[1].w, 0);
    }

    #[test]
    fn card_find() {
        // NOTE(jshrake): This card has mana cost {B/G}{B/G}
        // Our code does not properly handle mana costs specified
        // in this fashion and treats the {B/G} as {B}
        let card = ALL_CARDS.card_from_name("Find").expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 2);

        assert_eq!(card.all_mana_costs[0].b, 0);
        assert_eq!(card.all_mana_costs[0].u, 0);
        assert_eq!(card.all_mana_costs[0].c, 0);
        assert_eq!(card.all_mana_costs[0].g, 2);
        assert_eq!(card.all_mana_costs[0].r, 0);
        assert_eq!(card.all_mana_costs[0].w, 0);

        assert_eq!(card.all_mana_costs[1].u, 0);
        assert_eq!(card.all_mana_costs[1].c, 0);
        assert_eq!(card.all_mana_costs[1].g, 0);
        assert_eq!(card.all_mana_costs[1].r, 0);
        assert_eq!(card.all_mana_costs[1].w, 0);
        assert_eq!(card.all_mana_costs[1].b, 2);

        assert_eq!(card.all_mana_costs[2].u, 0);
        assert_eq!(card.all_mana_costs[2].c, 0);
        assert_eq!(card.all_mana_costs[2].g, 1);
        assert_eq!(card.all_mana_costs[2].r, 0);
        assert_eq!(card.all_mana_costs[2].w, 0);
        assert_eq!(card.all_mana_costs[2].b, 1);
    }

    #[test]
    fn card_dispersal() {
        let card = ALL_CARDS
            .card_from_name("Dispersal")
            .expect("can't find card");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 5);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 3);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
    }

    // Cards that we special case
    #[test]
    fn card_slayers_stronghold() {
        let card = ALL_CARDS
            .card_from_name("Slayers' Stronghold")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.w, 0);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.turn, 1);
    }

    #[test]
    fn card_alchemists_refuge() {
        let card = ALL_CARDS
            .card_from_name("Alchemist's Refuge")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.w, 0);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.turn, 1);
    }

    #[test]
    fn card_desolate_lighthouse() {
        let card = ALL_CARDS
            .card_from_name("Desolate Lighthouse")
            .expect("can't find card");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.w, 0);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.turn, 1);
    }

    #[test]
    fn good_deckcode_0() {
        let code = "
        4 Legion's Landing (XLN) 22
        4 Adanto Vanguard (XLN) 1
        4 Skymarcher Aspirant (RIX) 21
        4 Snubhorn Sentry (RIX) 23
        4 Clifftop Retreat (DAR) 239
        4 History of Benalia (DAR) 21
        2 Ajani, Adversary of Tyrants (M19) 3
        4 Heroic Reinforcements (M19) 217
        4 Sacred Foundry (GRN) 254
        2 Mountain (XLN) 272
        12 Plains (XLN) 263
        3 Hunted Witness (GRN) 15
        4 Conclave Tribunal (GRN) 6
        4 Venerated Loxodon (GRN) 30
        1 Doom Whisperer (GRN) 30
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        //println!("{:#?}", deck.unwrap().0.cards);
        assert_eq!(deck.unwrap().0.cards.len(), 60);
    }

    #[test]
    fn good_deckcode_1() {
        let code = "
        4 Legion's Landing
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        assert_eq!(deck.unwrap().0.cards.len(), 60);
    }

    #[test]
    fn good_deckcode_2() {
        let code = "
        # This is a comment
        4 Legion's Landing
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        # This is another comment
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        # This is the last comment
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        assert_eq!(deck.unwrap().0.cards.len(), 60);
    }

    #[test]
    fn good_deckcode_3() {
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        assert_eq!(deck.unwrap().0.cards.len(), 60);
    }

    #[test]
    fn good_deckcode_4() {
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        assert_eq!(deck.unwrap().0.cards.len(), 60);
    }

    #[test]
    fn good_deckcode_5_set_x() {
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183 #X=4
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        let deck = deck.unwrap().0;
        assert_eq!(deck.cards.len(), 60);
        let card = deck.card_from_name("Hydroid Krasis").unwrap();
        assert_eq!(card.mana_cost.c, 4);
    }

    #[test]
    fn good_deckcode_6_set_x() {
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183#x = 6
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        let deck = deck.unwrap().0;
        assert_eq!(deck.cards.len(), 60);
        let card = deck.card_from_name("Hydroid Krasis").unwrap();
        assert_eq!(card.mana_cost.c, 6);
    }

    #[test]
    fn good_deckcode_7_set_x() {
        let code = "
        2 Carnage Tyrant (XLN) 179
        3 Hydroid Krasis (RNA) 183#x = 6
        4 Jadelight Ranger (RIX) 136
        4 Llanowar Elves (DAR) 168
        4 Merfolk Branchwalker (XLN) 197
        2 Midnight Reaper (GRN) 77#x=3
        2 Ravenous Chupacabra (RIX) 82
        1 Seekers' Squire (XLN) 121
        4 Wildgrowth Walker (XLN) 216
        3 Vivien Reid (M19) 208
        4 Forest (XLN) 277
        1 Island (XLN) 265
        2 Swamp (XLN) 269
        4 Breeding Pool (RNA) 246
        1 Drowned Catacomb (XLN) 253
        2 Memorial to Folly (DAR) 242
        4 Overgrown Tomb (GRN) 253
        2 Watery Grave (GRN) 259
        4 Woodland Cemetery (DAR) 248
        2 Cast Down (DAR) 81
        2 Vraska's Contempt (XLN) 129
        3 Find // Finality (GRN) 225

        1 Tendershoot Dryad (RIX) 147
        2 The Eldest Reborn (DAR) 90
        1 Crushing Canopy (XLN) 183
        1 Disdainful Stroke (GRN) 37
        2 Negate (RIX) 44
        1 Vraska's Contempt (XLN) 129
        3 Cry of the Carnarium (RNA) 70
        4 Duress (XLN) 105
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        let deck = deck.unwrap().0;
        assert_eq!(deck.cards.len(), 60);
        let card = deck.card_from_name("Hydroid Krasis").unwrap();
        assert_eq!(card.mana_cost.c, 6);
        // can't set x value of midnight reaper since it doesn't have {X} mana cost
        let card = deck.card_from_name("Midnight Reaper").unwrap();
        assert_eq!(card.mana_cost.c, 2);
    }

    #[test]
    fn good_deckcode_8_set_x() {
        let code = "
        # This is a comment
        4 Legion's Landing
        3 Hydroid Krasis#x=5
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        # This is another comment
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        # This is the last comment
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        let deck = deck.unwrap().0;
        assert_eq!(deck.cards.len(), 63);
        let card = deck.card_from_name("Hydroid Krasis").unwrap();
        assert_eq!(card.mana_cost.c, 5);
    }

    #[test]
    fn good_deckcode_9_set_x() {
        let code = "
        # This is a comment
        4 Legion's Landing
        3 Hydroid Krasis # X=-5
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        # This is another comment
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        # This is the last comment
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        let deck = deck.unwrap().0;
        assert_eq!(deck.cards.len(), 63);
        // Ignore negatives
        let card = deck.card_from_name("Hydroid Krasis").unwrap();
        assert_eq!(card.mana_cost.c, 1);
    }

    #[test]
    fn bad_deckcode_0() {
        // The last card, Doom Whisperer, is misspelled
        let code = "
        4 Legion's Landing (XLN) 22
        4 Adanto Vanguard (XLN) 1
        4 Skymarcher Aspirant (RIX) 21
        4 Snubhorn Sentry (RIX) 23
        4 Clifftop Retreat (DAR) 239
        4 History of Benalia (DAR) 21
        2 Ajani, Adversary of Tyrants (M19) 3
        4 Heroic Reinforcements (M19) 217
        4 Sacred Foundry (GRN) 254
        2 Mountain (XLN) 272
        13 Plains (XLN) 263
        3 Hunted Witness (GRN) 15
        4 Conclave Tribunal (GRN) 6
        4 Venerated Loxodon (GRN) 30
        1 Doo Whisperer (GRN) 30
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), false);
    }

    #[test]
    fn good_deckcode_with_0_0() {
        let code = "
        0 Island
        4 Legion's Landing
        4 Adanto Vanguard
        4 Skymarcher Aspirant
        4 Snubhorn Sentry
        4 Clifftop Retreat
        4 History of Benalia
        2 Ajani, Adversary of Tyrants
        4 Heroic Reinforcements
        4 Sacred Foundry
        2 Mountain
        12 Plains
        3 Hunted Witness
        4 Conclave Tribunal
        4 Venerated Loxodon
        1 Doom Whisperer
        ";
        let deck = ALL_CARDS.from_deck_list(&code);
        assert_eq!(deck.is_ok(), true);
        assert_eq!(deck.unwrap().0.cards.len(), 60);
    }

    #[test]
    fn deck_aetherhub_45936() {
        let code = include_str!("decks/45936");
        let deck = ALL_CARDS.from_deck_list(code).expect("good deckcode");
        assert_eq!(deck.0.len(), 60);
    }

    #[test]
    fn deck_aetherhub_50520() {
        let code = include_str!("decks/50520");
        let deck = ALL_CARDS.from_deck_list(code).expect("good deckcode");
        assert_eq!(deck.0.len(), 60);
    }

    #[test]
    fn deck_aetherhub_50817() {
        let code = include_str!("decks/50817");
        let deck = ALL_CARDS.from_deck_list(code).expect("good deckcode");
        assert_eq!(deck.0.len(), 60);
    }
}
