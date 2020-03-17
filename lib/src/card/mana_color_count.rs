use crate::card::ManaCost;

#[derive(Debug, Serialize, Deserialize)]
pub struct ManaColorCount {
  pub total: usize, // total number of cards counted
  pub c: usize,
  pub w: usize,
  pub u: usize,
  pub b: usize,
  pub r: usize,
  pub g: usize,
  pub wu: usize, // azorius
  pub wb: usize, // orzhov
  pub ub: usize, // dimir
  pub ur: usize, // izzet
  pub br: usize, // rakdos
  pub bg: usize, // golgari
  pub rg: usize, // gruul
  pub rw: usize, // boros
  pub gw: usize, // selesnya
  pub gu: usize, // simic
}

impl ManaColorCount {
  pub fn new() -> Self {
    Self {
      total: 0,

      b: 0,
      u: 0,
      g: 0,
      r: 0,
      w: 0,
      c: 0,

      wu: 0,
      wb: 0,
      ub: 0,
      ur: 0,
      br: 0,
      bg: 0,
      rg: 0,
      rw: 0,
      gw: 0,
      gu: 0,
    }
  }

  pub fn count(&mut self, card: &ManaCost) {
    self.total += 1;
    self.u += card.u as usize;
    self.r += card.r as usize;
    self.b += card.b as usize;
    self.g += card.g as usize;
    self.w += card.w as usize;
    self.c += card.c as usize;
    match (card.r, card.g, card.b, card.u, card.w) {
      (1, 1, 0, 0, 0) => {
        self.rg += 1;
      }
      (1, 0, 1, 0, 0) => {
        self.br += 1;
      }
      (1, 0, 0, 1, 0) => {
        self.ur += 1;
      }
      (1, 0, 0, 0, 1) => {
        self.rw += 1;
      }
      (0, 1, 1, 0, 0) => {
        self.bg += 1;
      }
      (0, 1, 0, 1, 0) => {
        self.gu += 1;
      }
      (0, 1, 0, 0, 1) => {
        self.gw += 1;
      }
      (0, 0, 1, 1, 0) => {
        self.ub += 1;
      }
      (0, 0, 1, 0, 1) => {
        self.wb += 1;
      }
      (0, 0, 0, 1, 1) => {
        self.wu += 1;
      }
      _ => {}
    }
  }
}
