//! # Card representation and deck list parsing
//!
use crate::mana_cost::*;
use flate2::read::GzDecoder;
use std::hash::{Hash, Hasher};
use std::io::prelude::*;

// TODO Rethink including these in the Card definition
pub use crate::scryfall::*;

/// A Collection represents a deck or a library of cards
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub cards: Vec<Card>,
    sort: CollectionSort,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CollectionSort {
    Name,
    ArenaId,
}

impl Default for CollectionSort {
    fn default() -> Self {
        Self::Name
    }
}

// TODO: [image_uri] Consider storing only the suffix and concatenate with the hostname on the UI side
// TODO: [mana_cost_string] Remove mana_cost_string and generate the string from a ManaCost
// TODO: [mana_cost] Remove mana_cost and use all_mana_costs[0]
// NOTE: PartialEq and Eq are implemented below
/// Card represents a Magic: The Gathering card
#[derive(Default, Debug, Clone, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Card {
    /// String representing the card name
    pub name: String,
    /// Scryfall oracle id
    pub oracle_id: String,
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
    /// Arena id
    pub arena_id: u64,
    /// Card rarity
    pub rarity: Rarity,
    /// Card release set code
    pub set: SetCode,
    /// True if the card is legal in standard
    pub standard_legal: bool,
}

impl Card {
    /// Returns an empy new card
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the converted mana cost of the card
    #[inline]
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

/// ManaColor represents a [color](https://mtg.gamepedia.com/Color)

impl Default for CardKind {
    fn default() -> Self {
        Self::Unknown
    }
}

impl From<Card> for ManaCost {
    fn from(item: Card) -> Self {
        item.mana_cost
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
    /// Returns a new collection of all cards from data/all_cards.landlord
    pub fn all() -> Result<Self, bincode::Error> {
        // NOTE(jshrake): This file is generated!
        // Run scryfall2landlord to generate this file
        // See the `make card-update` task in the top-level Makefile
        let b = include_bytes!("../../data/oracle_cards.landlord");
        let mut gz = GzDecoder::new(&b[..]);
        let mut s: Vec<u8> = Vec::new();
        gz.read_to_end(&mut s).expect("gz decode failed");
        bincode::deserialize(&s)
    }

    /// Returns a new collection of cards
    pub fn from_cards(mut cards: Vec<Card>) -> Self {
        // sort for binary_search used in card_from_name
        // note that Card implements Ord by
        cards.sort();
        Self {
            cards,
            sort: CollectionSort::Name,
        }
    }

    pub fn sort_by_arena_id(mut self) -> Self {
        self.cards.sort_unstable_by_key(|c| c.arena_id);
        self.sort = CollectionSort::ArenaId;
        self
    }

    pub fn sort_by_name(mut self) -> Self {
        self.cards.sort();
        self.sort = CollectionSort::Name;
        self
    }

    /// Returns a card from the card name
    #[inline]
    pub fn card_from_name(&self, name: &str) -> Option<&Card> {
        assert_eq!(self.sort, CollectionSort::Name);
        let name_lowercase = name.to_lowercase();
        let res = self
            .cards
            .binary_search_by(|probe| probe.name.to_lowercase().cmp(&name_lowercase));
        res.map(|idx| &self.cards[idx]).ok()
    }

    /// Returns a card from the arena id
    #[inline]
    pub fn card_from_arena_id(&self, arena_id: u64) -> Option<&Card> {
        assert_eq!(self.sort, CollectionSort::ArenaId);
        let res = self
            .cards
            .binary_search_by(|probe| probe.arena_id.cmp(&arena_id));
        res.map(|idx| &self.cards[idx]).ok()
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

lazy_static! {
    pub static ref ALL_CARDS: Collection = Collection::all().expect("Collection::all() failed");
}

#[macro_export]
macro_rules! card {
    ($card_name:expr) => {
        ALL_CARDS
            .card_from_name($card_name)
            .unwrap_or_else(|| panic!("Cannot find card named \"{}\"", $card_name))
    };
}

#[cfg(test)]
mod tests {
    use crate::card::*;

    #[test]
    fn all_cards_have_non_empty_image_uri() {
        let any_empty_image_uri = ALL_CARDS.cards.iter().any(|c| c.image_uri.is_empty());
        assert_eq!(any_empty_image_uri, false);
    }

    #[test]
    fn all_cards_have_unique_names() {
        let mut deduped = ALL_CARDS.clone();
        deduped.cards.dedup();
        assert_eq!(deduped.cards.len(), ALL_CARDS.cards.len());
    }

    #[test]
    fn card_field_of_ruin() {
        let card = card!("Field of Ruin");
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
        let card = card!("Carnival");
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
        let card = card!("Steam Vents");
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
        let card = card!("Sulfur Falls");
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
        let card = card!("Jungle Shrine");
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
        let card = card!("Arcades, the Strategist");
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
        let card = card!("Gateway Plaza");
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
        let card = card!("Guildmages' Forum");
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
        let card = card!("Unclaimed Territory");
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
        let card = card!("Vivid Crag");
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
        let card = card!("City of Brass");
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
        let card = card!("Ancient Ziggurat");
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
        let card = card!("Mana Confluence");
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
        let card = card!("Unknown Shores");
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
        let card = card!("Rupture Spire");
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
        let card = card!("Ghalta, Primal Hunger");
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
        let card = card!("Nicol Bolas, the Ravager");
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
        let card = card!("Swamp");
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
        let card = card!("Treasure Map");
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
    fn card_search_for_azcanta() {
        let card = card!("Search for Azcanta");
        assert_eq!(card.turn, 2);
        assert_eq!(card.is_land(), false);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.c, 1);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 0);
        assert!(!card.image_uri.is_empty());
    }

    #[test]
    fn card_integrity() {
        let card = card!("Integrity");
        assert_eq!(card.turn, 1);
        assert_eq!(card.is_land(), false);
        assert!(!card.image_uri.is_empty());
    }

    #[test]
    fn card_teferi_hero_of_dominaria() {
        let card = card!("Teferi, Hero of Dominaria");
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
        let card = card!("Syncopate");
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
        let card = card!("Cinder Glade");
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
    fn card_angel_of_sanctions() {
        let card = card!("Angel of Sanctions");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.turn, 5);
        assert_eq!(card.mana_cost.b, 0);
        assert_eq!(card.mana_cost.u, 0);
        assert_eq!(card.mana_cost.c, 3);
        assert_eq!(card.mana_cost.g, 0);
        assert_eq!(card.mana_cost.r, 0);
        assert_eq!(card.mana_cost.w, 2);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn card_discovery() {
        // NOTE(jshrake): This card has mana cost {1}{U/B}
        // Our code does not properly handle mana costs specified
        // in this fashion and treats the {U/B} as {U}
        let card = card!("Discovery");
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
        let card = card!("Find");
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
        let card = card!("Dispersal");
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
        let card = card!("Slayers' Stronghold");
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
        let card = card!("Alchemist's Refuge");
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
        let card = card!("Desolate Lighthouse");
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
}
