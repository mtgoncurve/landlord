#[macro_use]
extern crate criterion;

use criterion::Criterion;
use landlord::deck::Deck;
use landlord::mulligan::London;
use landlord::simulation::{Simulation, SimulationConfig};

fn criterion_function(c: &mut Criterion) {
    let code = include_str!("../src/decks/48388");
    let deck = Deck::from_list(code).expect("Bad deckcode");
    let mulligan = London::never();
    let highest_cmc = deck
        .cards
        .iter()
        .fold(0, |max, cc| std::cmp::max(max, cc.card.turn as usize));
    let sim = Simulation::from_config(&SimulationConfig {
        run_count: 1000,
        draw_count: highest_cmc,
        mulligan: &mulligan,
        deck: &deck,
        on_the_play: false,
    });
    c.bench_function("48388 card_observations", |b| {
        b.iter(|| {
            deck.cards
                .iter()
                .filter(|cc| !cc.card.is_land())
                .for_each(|cc| {
                    sim.observations_for_card(&cc.card);
                });
        })
    });
}

criterion_group!(benches, criterion_function);
criterion_main!(benches);
