#[macro_use]
extern crate criterion;

use criterion::Criterion;
use landlord::deck::Deck;
use landlord::mulligan::London;
use landlord::simulation::{Simulation, SimulationConfig};

fn criterion_function(c: &mut Criterion) {
    let code = "
        3 Wildgrowth Walker (XLN) 216
        3 Carnage Tyrant (XLN) 179
        4 Merfolk Branchwalker (XLN) 197
        5 Swamp (XLN) 268
        8 Forest (XLN) 276
        3 Vraska's Contempt (XLN) 129
        4 Jadelight Ranger (RIX) 136
        2 Ravenous Chupacabra (RIX) 82
        2 Karn, Scion of Urza (DAR) 1
        2 Memorial to Folly (DAR) 242
        4 Woodland Cemetery (DAR) 248
        4 Llanowar Elves (DAR) 168
        1 Detection Tower (M19) 249
        2 Druid of the Cowl (M19) 177
        3 Vivien Reid (M19) 208
        1 Assassin's Trophy (GRN) 152
        4 Overgrown Tomb (GRN) 253
        3 Find // Finality (GRN) 225
        2 Midnight Reaper (GRN) 77

        1 Wildgrowth Walker (XLN) 216
        4 Duress (XLN) 105
        1 Golden Demise (RIX) 73
        3 Cast Down (DAR) 81
        1 The Eldest Reborn (DAR) 90
        1 Reclamation Sage (M19) 196
        1 Assassin's Trophy (GRN) 152
        2 Plaguecrafter (GRN) 82
        1 Midnight Reaper (GRN) 77
    ";
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
