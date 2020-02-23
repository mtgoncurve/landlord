#[macro_use]
extern crate criterion;

use criterion::Criterion;
use landlord::deck::Deck;
use landlord::mulligan::London;
use landlord::simulation::{Simulation, SimulationConfig};

fn criterion_function(c: &mut Criterion) {
    let code = include_str!("../src/decks/48388");
    let deck = Deck::from_list(code).expect("Bad deckcode");
    c.bench_function_over_inputs(
        "48388 card_observations",
        move |b, runs| {
            let mulligan = London::never();
            let highest_cmc = deck
                .cards
                .iter()
                .fold(0, |max, (c, _)| std::cmp::max(max, c.turn as usize));
            let sim = Simulation::from_config(&SimulationConfig {
                run_count: **runs,
                draw_count: highest_cmc,
                mulligan: &mulligan,
                deck: &deck,
                on_the_play: false,
            });
            b.iter(|| {
                deck.cards.iter().for_each(|(c, _)| {
                    sim.observations_for_card(&c);
                });
            })
        },
        &[1000],
    );
}

criterion_group!(benches, criterion_function);
criterion_main!(benches);
