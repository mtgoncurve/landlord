use std::collections::HashSet;

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
    self.bits = Self::calculate_signature_rgbuwc(self.r, self.g, self.b, self.u, self.w, self.c);
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

pub fn mana_costs_from_str(mana_cost_str: &str) -> Vec<ManaCost> {
  let symbol_stack = mana_cost_symbols_from_str(mana_cost_str);
  // NOTE: The hashset ensures that we do not double count
  // the same mana cost multiple times. This is important for cards
  // that have multiple split costs, like Find {B/G}{B/G} -- i.e. we
  // want combinations (dont care about order) rather than all permutations
  let mut results = HashSet::new();
  mana_costs_from_str_recur(&mut results, ManaCost::new(), &symbol_stack, 0);
  // Guarantee the resulting order by sorting
  let mut results_as_vec: Vec<_> = results.into_iter().collect();
  results_as_vec.sort();
  results_as_vec
}

fn mana_costs_from_str_recur(
  results: &mut HashSet<ManaCost>,
  current: ManaCost,
  symbol_stack: &[(ManaCost, Option<ManaCost>)],
  idx: usize,
) {
  if symbol_stack.len() <= idx {
    let current = current.update_bits();
    results.insert(current);
    return;
  }
  let mut left = symbol_stack[idx].0;
  left.r += current.r;
  left.g += current.g;
  left.b += current.b;
  left.u += current.u;
  left.w += current.w;
  left.c += current.c;
  mana_costs_from_str_recur(results, left, symbol_stack, idx + 1);
  if let Some(mut right) = symbol_stack[idx].1 {
    right.r += current.r;
    right.g += current.g;
    right.b += current.b;
    right.u += current.u;
    right.w += current.w;
    right.c += current.c;
    mana_costs_from_str_recur(results, right, symbol_stack, idx + 1);
  }
}

fn mana_cost_symbols_from_str(mana_cost_str: &str) -> Vec<(ManaCost, Option<ManaCost>)> {
  let mut sigil = String::new();
  let mut symbol_stack: Vec<(ManaCost, Option<ManaCost>)> = Vec::new();
  let mut should_push_right = false;
  let mut idx = 0;

  for c in mana_cost_str.chars() {
    match c {
      '{' => {
        sigil.clear();
        symbol_stack.push((ManaCost::new(), None));
        idx = symbol_stack.len() - 1;
        should_push_right = false;
      }
      '/' | '\\' => {
        let color = ManaColor::from_str(&sigil);
        let count = sigil.parse::<u8>().unwrap_or(1);
        let mut cost = ManaCost::new();
        match color {
          ManaColor::Black => cost.b += count,
          ManaColor::Blue => cost.u += count,
          ManaColor::Green => cost.g += count,
          ManaColor::Red => cost.r += count,
          ManaColor::White => cost.w += count,
          ManaColor::Colorless => cost.c += count,
        }
        symbol_stack[idx].0 = cost;
        should_push_right = true;
        sigil.clear();
      }
      '}' => {
        let color = ManaColor::from_str(&sigil);
        let count = sigil.parse::<u8>().unwrap_or(1);
        let mut cost = ManaCost::new();
        match color {
          ManaColor::Black => cost.b += count,
          ManaColor::Blue => cost.u += count,
          ManaColor::Green => cost.g += count,
          ManaColor::Red => cost.r += count,
          ManaColor::White => cost.w += count,
          ManaColor::Colorless => cost.c += count,
        }
        if should_push_right {
          symbol_stack[idx].1 = Some(cost);
        } else {
          symbol_stack[idx].0 = cost;
        }
      }
      c => {
        sigil.push(c);
      }
    }
  }
  symbol_stack
}

#[cfg(test)]
mod tests {
  use crate::card::mana_cost::*;

  #[test]
  fn empty_string() {
    let res = mana_costs_from_str("");
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].c, 0);
    assert_eq!(res[0].r, 0);
    assert_eq!(res[0].w, 0);
    assert_eq!(res[0].b, 0);
    assert_eq!(res[0].u, 0);
    assert_eq!(res[0].g, 0);
    assert_eq!(res[0].bits, 0);
  }

  #[test]
  fn simple_test_0() {
    let res = mana_costs_from_str("{1}{U}");
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].c, 1);
    assert_eq!(res[0].r, 0);
    assert_eq!(res[0].w, 0);
    assert_eq!(res[0].b, 0);
    assert_eq!(res[0].u, 1);
    assert_eq!(res[0].g, 0);
  }

  #[test]
  fn x_test_0() {
    let res = mana_costs_from_str("{X}{U}");
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].c, 1);
    assert_eq!(res[0].r, 0);
    assert_eq!(res[0].w, 0);
    assert_eq!(res[0].b, 0);
    assert_eq!(res[0].u, 1);
    assert_eq!(res[0].g, 0);
  }

  // Hybrid mana is of the for {B/R}
  #[test]
  fn hybrid_test_0() {
    let res = mana_costs_from_str("{B/R}");
    assert_eq!(res.len(), 2);
    //
    assert_eq!(res[0].c, 0);
    assert_eq!(res[0].r, 1);
    assert_eq!(res[0].w, 0);
    assert_eq!(res[0].b, 0);
    assert_eq!(res[0].u, 0);
    assert_eq!(res[0].g, 0);
    //
    assert_eq!(res[1].c, 0);
    assert_eq!(res[1].r, 0);
    assert_eq!(res[1].w, 0);
    assert_eq!(res[1].b, 1);
    assert_eq!(res[1].u, 0);
    assert_eq!(res[1].g, 0);
  }

  // NOTE: Split cards are not handled correctly
  // Split cards are those that have multiple card faces, such as Carnival // Carnage
  // The mana cost of this card looks like "{B/R} // {2}{B}{R}", which the code currently
  // doesn't parse correctly. In practice, this isn't an issue since we only ever evaluate
  // the individual card faces
  #[test]
  #[should_panic]
  fn split_test_0() {
    let res = mana_costs_from_str("{B} // {2}{B}{R}");
    assert_eq!(res.len(), 2);
    //
    assert_eq!(res[0].c, 0);
    assert_eq!(res[0].r, 0);
    assert_eq!(res[0].w, 0);
    assert_eq!(res[0].b, 1);
    assert_eq!(res[0].u, 0);
    assert_eq!(res[0].g, 0);
    //
    assert_eq!(res[1].c, 2);
    assert_eq!(res[1].r, 1);
    assert_eq!(res[1].w, 0);
    assert_eq!(res[1].b, 1);
    assert_eq!(res[1].u, 0);
    assert_eq!(res[1].g, 0);
  }
}
