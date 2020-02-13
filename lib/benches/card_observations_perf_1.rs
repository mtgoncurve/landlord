#[macro_use]
extern crate criterion;

use criterion::Criterion;
use landlord::card::Collection;
use landlord::mulligan::London;
use landlord::simulation::{Simulation, SimulationConfig};
use std::collections::HashSet;

fn criterion_function(c: &mut Criterion) {
    let code = "
    2 Hostage Taker
    4 Nicol Bolas, the Ravager
    4 Rekindling Phoenix
    4 Rix Maadi Reveler
    2 Siege-Gang Commander
    4 Thief of Sanity
    4 Bedevil
    2 Cast Down
    1 Vraska's Contempt
    4 Lava Coil
    4 Thought Erasure
    2 The Immortal Sun
    4 Blood Crypt
    4 Dragonskull Summit
    4 Drowned Catacomb
    4 Steam Vents
    3 Sulfur Falls
    4 Watery Grave
    ";
    let collection = Collection::all().expect("Collection::all failed");
    let deck = collection.from_deck_list(code).expect("Bad deckcode").0;
    c.bench_function_over_inputs(
        "reddit_deck card_observations",
        move |b, runs| {
            let mulligan = London::never();
            let highest_cmc = deck
                .cards
                .iter()
                .fold(0, |max, c| std::cmp::max(max, c.turn as usize));
            let sim = Simulation::from_config(&SimulationConfig {
                run_count: **runs,
                draw_count: highest_cmc,
                mulligan: &mulligan,
                deck: &deck,
                on_the_play: false,
            });
            let uniques: HashSet<_> = deck.cards.iter().filter(|c| !c.is_land()).collect();
            b.iter(|| {
                uniques.iter().for_each(|c| {
                    sim.observations_for_card(&c);
                });
            })
        },
        &[1000],
    );
}

criterion_group!(benches, criterion_function);
criterion_main!(benches);
