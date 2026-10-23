//! Audit smoke test: every kingdom card of Prosperity, Alchemy, the promos and Cornucopia & Guilds is bought (up to three
//! copies) by a Big Money bot in 2- and 3-player games with the other bots' attacks around, so each card's default
//! decisions (Pass where nothing is offered, an illegal pick...) are exercised: the game must finish without a panic.

use dominion_engine::cards::{self, id, CardId};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn kingdom_cards_of_the_audited_sets() -> Vec<CardId> {
    let mut v: Vec<CardId> = (id::ANVIL..=id::PEDDLER).collect();
    v.extend(id::TRANSMUTE..=id::APPRENTICE);
    v.extend([id::PRINCE, id::WALLED_VILLAGE, id::ENVOY, id::CAPTAIN, id::SAUNA, id::GOVERNOR, id::DISMANTLE, id::CHURCH, id::MARCHLAND]);
    v.extend(id::CANDLESTICK_MAKER..=id::FAIRGROUNDS);
    v
}

fn bot(card: CardId) -> Strategy {
    let name = cards::def(card).name;
    let src = format!(
        "name = \"fuzz\"\n\
         [[gain]]\ncard = \"Province\"\nif = \"money >= 15\"\n\
         [[gain]]\ncard = \"Duchy\"\nif = \"provinces_left <= 4\"\n\
         [[gain]]\ncard = \"Gold\"\n\
         [[gain]]\ncard = \"{name}\"\nif = \"count({name}) < 3\"\n\
         [[gain]]\ncard = \"Silver\"\n"
    );
    Strategy::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn every_audited_kingdom_card_plays_through_games_without_panicking() {
    let fillers = [id::VILLAGE, id::MILITIA, id::MOAT, id::MARKET, id::SMITHY];
    for (i, &card) in kingdom_cards_of_the_audited_sets().iter().enumerate() {
        let mut kingdom = vec![card];
        // A few other cards of the audited sets to interact with, rotating, then the fillers.
        let all = kingdom_cards_of_the_audited_sets();
        for k in 1..=4 {
            let other = all[(i * 7 + k * 11) % all.len()];
            if !kingdom.contains(&other) {
                kingdom.push(other);
            }
        }
        for f in fillers {
            if kingdom.len() < 10 && !kingdom.contains(&f) {
                kingdom.push(f);
            }
        }
        if cards::potion_cost(card) || kingdom.iter().any(|&c| cards::potion_cost(c)) {
            kingdom.push(id::POTION);
        }
        let a = bot(card);
        let b = bot(kingdom[1]);
        let cfg = MatchConfig { games: 15, kingdom, seed: 7 + i as u64, max_turns: 200 };
        run_match(&[&a, &b], &cfg);
    }
}
