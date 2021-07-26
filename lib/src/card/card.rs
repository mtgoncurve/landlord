//! # Internal card representation
//!
pub use crate::card::mana_cost::*;
pub use crate::scryfall::{GameFormat, Legality, Object, Rarity, SetCode};
use std::hash::{Hash, Hasher};

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
    /// True if this card is a sub face
    pub is_face: bool,
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

impl Card {
    /// Returns an empy new card
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the converted mana cost of the card
    pub fn cmc(&self) -> u8 {
        self.mana_cost.cmc()
    }

    /// Returns true if the card type is a land
    pub fn is_land(&self) -> bool {
        self.kind.is_land()
    }

    pub fn in_standard(&self) -> bool {
        self.set.in_standard()
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

#[macro_export]
macro_rules! card {
    ($card_name:expr) => {
        $crate::data::ALL_CARDS
            .card_from_name($card_name)
            .unwrap_or_else(|| panic!("Cannot find card named \"{}\"", $card_name))
    };
}

#[cfg(test)]
mod tests {
    use crate::card::*;

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
    fn card_fabled_passage() {
        let card = card!("Fabled Passage");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.mana_cost.c, 0);
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

    #[test]
    fn card_evolving_wilds() {
        let card = card!("Evolving Wilds");
        assert_eq!(card.is_land(), true);
        assert_eq!(card.kind, CardKind::OtherLand);
        assert_eq!(card.mana_cost.r, 1);
        assert_eq!(card.mana_cost.g, 1);
        assert_eq!(card.mana_cost.b, 1);
        assert_eq!(card.mana_cost.u, 1);
        assert_eq!(card.mana_cost.w, 1);
        assert_eq!(card.mana_cost.c, 0);
    }

    #[test]
    fn card_narset_of_the_ancient_way() {
        let card = card!("Narset of the Ancient Way");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn card_nexus_of_fate() {
        let card = card!("Nexus of Fate");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn c20_card_call_the_coppercoats() {
        let card = card!("Call the Coppercoats");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn m21_card_terror_of_the_peaks() {
        let card = card!("Terror of the Peaks");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn m21_card_brash_taunter() {
        let card = card!("Brash Taunter");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn iko_card_companion_lurrus() {
        let card = card!("Lurrus of the Dream-Den");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn jump_card_supply_runners() {
        let card = card!("Supply Runners");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn znr_zareth_san() {
        let card = card!("Zareth San, the Trickster");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn khm_rise_of_the_dread_marn() {
        let card = card!("Rise of the Dread Marn");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn khm_issue_33() {
        // https://github.com/mtgoncurve/landlord/issues/33
        // https://scryfall.com/search?as=grid&order=name&q=type%3Aland+set%3Akhm+rarity%3Au
        {
            let card = card!("Axgard Armory");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 1);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Gates of Istfell");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 1);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Bretagard Stronghold");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 1);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Gnottvold Slumbermound");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 1);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Great Hall of Starnheim");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 1);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Immersturm Skullcairn");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 1);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Littjara Mirrorlake");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 1);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Port of Karfell");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 1);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Skemfar Elderhall");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 0);
            assert_eq!(card.mana_cost.g, 1);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
        {
            let card = card!("Surtland Frostpyre");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost.r, 1);
            assert_eq!(card.mana_cost.g, 0);
            assert_eq!(card.mana_cost.b, 0);
            assert_eq!(card.mana_cost.u, 0);
            assert_eq!(card.mana_cost.w, 0);
            assert_eq!(card.mana_cost.c, 0);
        }
    }

    #[test]
    fn stx_divide_by_zero() {
        let card = card!("Divide by Zero");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn pathway_lands() {
        {
            let card = card!("Barkchannel Pathway // Tidechannel Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 0, 1, 0, 0));
        }
        {
            let card = card!("Barkchannel Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 0, 0, 0, 0));
        }
        {
            let card = card!("Tidechannel Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 1, 0, 0));
        }
        {
            let card = card!("Blightstep Pathway // Searstep Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(1, 0, 1, 0, 0, 0));
        }
        {
            let card = card!("Blightstep Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 1, 0, 0, 0));
        }
        {
            let card = card!("Searstep Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 1, 0, 0));
        }
        {
            let card = card!("Branchloft Pathway // Boulderloft Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 0, 0, 1, 0));
        }
        {
            let card = card!("Branchloft Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 0, 0, 0, 0));
        }
        {
            let card = card!("Boulderloft Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 0, 1, 0));
        }
        {
            let card = card!("Brightclimb Pathway // Grimclimb Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 1, 0, 1, 0));
        }
        {
            let card = card!("Brightclimb Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 0, 1, 0));
        }
        {
            let card = card!("Grimclimb Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 1, 0, 0, 0));
        }
        {
            let card = card!("Clearwater Pathway // Murkwater Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 1, 1, 0, 0));
        }
        {
            let card = card!("Clearwater Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 1, 0, 0));
        }
        {
            let card = card!("Murkwater Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 1, 0, 0, 0));
        }
        {
            let card = card!("Cragcrown Pathway // Timbercrown Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(1, 1, 0, 0, 0, 0));
        }
        {
            let card = card!("Cragcrown Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(1, 0, 0, 0, 0, 0));
        }
        {
            let card = card!("Timbercrown Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 0, 0, 0, 0));
        }
        {
            let card = card!("Darkbore Pathway // Slitherbore Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 1, 0, 0, 0));
        }
        {
            let card = card!("Darkbore Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 1, 0, 0, 0));
        }
        {
            let card = card!("Slitherbore Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 1, 0, 0, 0, 0));
        }
        {
            let card = card!("Hengegate Pathway // Mistgate Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 1, 1, 0));
        }
        {
            let card = card!("Hengegate Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 0, 1, 0));
        }
        {
            let card = card!("Mistgate Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 1, 0, 0));
        }
        {
            let card = card!("Needleverge Pathway // Pillarverge Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 0, 1, 0));
        }
        {
            let card = card!("Needleverge Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(1, 0, 0, 0, 0, 0));
        }
        {
            let card = card!("Pillarverge Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 0, 1, 0));
        }
        {
            let card = card!("Riverglide Pathway // Lavaglide Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(1, 0, 0, 1, 0, 0));
        }
        {
            let card = card!("Riverglide Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(0, 0, 0, 1, 0, 0));
        }
        {
            let card = card!("Lavaglide Pathway");
            assert_eq!(card.is_land(), true);
            assert_eq!(card.mana_cost, ManaCost::from_rgbuwc(1, 0, 0, 0, 0, 0));
        }
    }

    #[test]
    fn c21_card_osgir() {
        let card = card!("Osgir, the Reconstructor");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn mh2_card_solitude() {
        let card = card!("Solitude");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }

    #[test]
    fn afr_book_of_exalted_deeds() {
        let card = card!("The Book of Exalted Deeds");
        assert_eq!(card.is_land(), false);
        assert_eq!(card.kind, CardKind::Unknown);
    }
}
