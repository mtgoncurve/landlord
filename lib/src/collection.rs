//! # Collection
//!
use crate::card::{Card, SetCode};
use std::collections::HashMap;
use std::ops::Deref;

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

impl Collection {
  pub fn group_by_name<'a>(&'a self) -> HashMap<&'a String, Vec<&'a Card>> {
    let mut m = HashMap::new();
    for card in &self.cards {
      let cards = m.entry(&card.name).or_insert(Vec::new());
      cards.push(card);
    }
    m
  }

  pub fn group_by_oracle_id<'a>(&'a self) -> HashMap<&'a String, Vec<&'a Card>> {
    let mut m = HashMap::new();
    for card in &self.cards {
      // Ignore card faces, which duplicate the oracle id of the parent card object
      if card.is_face {
        continue;
      }
      let cards = m.entry(&card.oracle_id).or_insert(Vec::new());
      cards.push(card);
    }
    m
  }

  pub fn group_by_set<'a>(&'a self) -> HashMap<SetCode, Vec<&'a Card>> {
    let mut m = HashMap::new();
    for card in &self.cards {
      // Ignore card faces, which duplicate the set of the parent card object
      if card.is_face {
        continue;
      }
      let cards = m.entry(card.set).or_insert(Vec::new());
      cards.push(card);
    }
    m
  }

  pub fn group_by_id<'a>(&'a self) -> HashMap<&'a String, &'a Card> {
    let mut m = HashMap::new();
    for card in &self.cards {
      // Ignore card faces, which duplicate the id of the parent card object
      if card.is_face {
        continue;
      }
      m.insert(&card.id, card);
    }
    m
  }

  pub fn group_by_arena_id<'a>(&'a self) -> HashMap<u64, &'a Card> {
    let mut m = HashMap::new();
    for card in &self.cards {
      m.insert(card.arena_id, card);
    }
    m
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
}

impl Deref for Collection {
  type Target = [Card];

  fn deref(&self) -> &Self::Target {
    &self.cards
  }
}

#[cfg(test)]
mod tests {}
