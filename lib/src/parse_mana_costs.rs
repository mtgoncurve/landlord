use crate::card::{ManaColor, ManaCost};
use std::collections::HashSet;

pub fn parse_mana_costs(mana_cost_str: &str) -> Vec<ManaCost> {
    let symbol_stack = mana_cost_symbols_from_str(mana_cost_str);
    // NOTE: The hashset ensures that we do not double count
    // the same mana cost multiple times. This is important for cards
    // that have multiple split costs, like Find {B/G}{B/G} -- i.e. we
    // want combinations (dont care about order) rather than all permutations
    let mut results = HashSet::new();
    parse_mana_costs_recur(&mut results, ManaCost::new(), &symbol_stack, 0);
    // Guarantee the resulting order by sorting
    let mut results_as_vec: Vec<_> = results.into_iter().collect();
    results_as_vec.sort();
    results_as_vec
}

fn parse_mana_costs_recur(
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
    parse_mana_costs_recur(results, left, symbol_stack, idx + 1);
    if let Some(mut right) = symbol_stack[idx].1 {
        right.r += current.r;
        right.g += current.g;
        right.b += current.b;
        right.u += current.u;
        right.w += current.w;
        right.c += current.c;
        parse_mana_costs_recur(results, right, symbol_stack, idx + 1);
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
    use crate::parse_mana_costs::*;

    #[test]
    fn empty_string() {
        let res = parse_mana_costs("");
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
        let res = parse_mana_costs("{1}{U}");
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
        let res = parse_mana_costs("{X}{U}");
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
        let res = parse_mana_costs("{B/R}");
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
        let res = parse_mana_costs("{B} // {2}{B}{R}");
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
