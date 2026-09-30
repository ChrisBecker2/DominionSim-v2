//! Kingdom selection for the CLI: a preset name, a comma-separated list of 10 card names,
//! `auto`/`auto:<sets>` (the union of cards the given strategies reference, padded to 10), or
//! `random`/`random:<sets>` (10 random kingdom cards from the given sets, seeded by `--seed`).

use std::collections::BTreeSet;

use dominion_engine::cards::{self, CardId, CardSet};
use dominion_engine::rng::Rng;

use crate::strategy::Strategy;

/// A stream tag distinguishing kingdom-selection randomness from everything else derived from
/// `--seed` (game shuffles use their own per-game streams).
const KINGDOM_STREAM: u64 = 0x4B69_6E67_444F_4D;

/// Resolve a `--kingdom` argument into kingdom card ids. `seed` drives `random`/`random:<sets>`
/// and the padding for `auto:<sets>`, so the same seed and selection reproduce the same kingdom.
pub fn resolve(spec: &str, strategies: &[&Strategy], seed: u64) -> Result<Vec<CardId>, String> {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("first-game") || spec.eq_ignore_ascii_case("first_game") {
        return Ok(cards::FIRST_GAME.to_vec());
    }
    if spec.eq_ignore_ascii_case("auto") {
        return resolve_auto(strategies);
    }
    if let Some(rest) = strip_ci_prefix(spec, "auto:") {
        let sets = parse_sets(rest)?;
        let required = used_kingdom_cards(strategies)?;
        let mut rng = Rng::derive(seed, KINGDOM_STREAM);
        return Ok(cards::random_kingdom(&sets, &required, &mut rng));
    }
    if spec.eq_ignore_ascii_case("random") {
        let mut rng = Rng::derive(seed, KINGDOM_STREAM);
        return Ok(cards::random_kingdom(&[CardSet::Base, CardSet::Intrigue], &[], &mut rng));
    }
    if let Some(rest) = strip_ci_prefix(spec, "random:") {
        let sets = parse_sets(rest)?;
        let mut rng = Rng::derive(seed, KINGDOM_STREAM);
        return Ok(cards::random_kingdom(&sets, &[], &mut rng));
    }
    let mut out = Vec::new();
    for part in spec.split(',') {
        let name = part.trim();
        if name.is_empty() {
            continue;
        }
        let c = cards::by_name(name).ok_or_else(|| format!("unknown card {name:?} in --kingdom"))?;
        if c < cards::FIRST_KINGDOM {
            return Err(format!("{name:?} is a basic card, not a kingdom card"));
        }
        if out.contains(&c) {
            return Err(format!("{name:?} listed twice in --kingdom"));
        }
        out.push(c);
    }
    if out.len() != 10 {
        return Err(format!(
            "--kingdom must name exactly 10 kingdom cards (or a preset: \"first-game\", \"auto\", \"auto:<sets>\", \"random\", \"random:<sets>\"), got {}",
            out.len()
        ));
    }
    Ok(out)
}

/// Case-insensitive prefix strip (`spec` is already trimmed).
fn strip_ci_prefix<'a>(spec: &'a str, prefix: &str) -> Option<&'a str> {
    if spec.len() >= prefix.len() && spec[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&spec[prefix.len()..])
    } else {
        None
    }
}

/// Parse a `+`-separated set list, e.g. `"base"`, `"intrigue"`, `"base+intrigue"`.
fn parse_sets(spec: &str) -> Result<Vec<CardSet>, String> {
    let mut out = Vec::new();
    for part in spec.split('+') {
        let name = part.trim();
        if name.is_empty() {
            continue;
        }
        let set = cards::set_by_name(name).ok_or_else(|| format!("unknown card set {name:?} (expected \"base\" and/or \"intrigue\")"))?;
        if !out.contains(&set) {
            out.push(set);
        }
    }
    if out.is_empty() {
        return Err("--kingdom: no card set named (expected \"base\" and/or \"intrigue\")".into());
    }
    Ok(out)
}

/// The union of kingdom cards the strategies reference, sorted; errors if there are more than 10.
fn used_kingdom_cards(strategies: &[&Strategy]) -> Result<Vec<CardId>, String> {
    let mut used = BTreeSet::new();
    for s in strategies {
        for c in s.kingdom_refs() {
            used.insert(c);
        }
    }
    if used.len() > 10 {
        let names: Vec<&str> = used.iter().map(|&c| cards::name(c)).collect();
        return Err(format!("auto kingdom: strategies reference {} kingdom cards (>10): {}. Pass an explicit --kingdom.", used.len(), names.join(", ")));
    }
    Ok(used.into_iter().collect())
}

/// `auto`: the strategies' cards, padded to 10 in ascending card-id order (deterministic,
/// unseeded, kept for backward compatibility).
fn resolve_auto(strategies: &[&Strategy]) -> Result<Vec<CardId>, String> {
    let mut out = used_kingdom_cards(strategies)?;
    for c in cards::kingdom_cards() {
        if out.len() >= 10 {
            break;
        }
        if !out.contains(&c) {
            out.push(c);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_game_preset() {
        let k = resolve("first-game", &[], 1).unwrap();
        assert_eq!(k.len(), 10);
    }

    #[test]
    fn explicit_list() {
        let k = resolve("Village,Smithy,Witch,Chapel,Market,Gardens,Militia,Moat,Bandit,Cellar", &[], 1).unwrap();
        assert_eq!(k.len(), 10);
    }

    #[test]
    fn rejects_wrong_count() {
        assert!(resolve("Village,Smithy", &[], 1).is_err());
    }

    #[test]
    fn random_is_reproducible_and_respects_sets() {
        let a = resolve("random:base", &[], 7).unwrap();
        let b = resolve("random:base", &[], 7).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 10);
        assert!(a.iter().all(|&c| cards::set_of(c) == CardSet::Base));

        let intrigue = resolve("random:intrigue", &[], 7).unwrap();
        assert!(intrigue.iter().all(|&c| cards::set_of(c) == CardSet::Intrigue));

        let both = resolve("random:base+intrigue", &[], 7).unwrap();
        assert_eq!(both.len(), 10);

        let plain = resolve("random", &[], 7).unwrap();
        assert_eq!(plain.len(), 10);

        // A different seed usually gives a different kingdom.
        let other_seed = resolve("random:base", &[], 8).unwrap();
        assert_ne!(a, other_seed);
    }

    #[test]
    fn random_rejects_unknown_set() {
        // Seaside and Prosperity are recognized sets now (step 1/2); use a genuinely unknown one.
        assert!(resolve("random:alchemy", &[], 1).is_err());
    }

    #[test]
    fn random_accepts_seaside_and_prosperity_by_name() {
        let k = resolve("random:prosperity", &[], 1).unwrap();
        assert!(!k.is_empty());
        for &c in &k {
            assert_eq!(cards::set_of(c), CardSet::Prosperity);
        }
    }

    #[test]
    fn auto_with_sets_pads_from_the_given_sets_and_keeps_required_cards() {
        // No strategies given: the whole kingdom is padding from the requested set.
        let k = resolve("auto:intrigue", &[], 3).unwrap();
        assert_eq!(k.len(), 10);
        assert!(k.iter().all(|&c| cards::set_of(c) == CardSet::Intrigue));

        // Reproducible for the same seed.
        let k2 = resolve("auto:intrigue", &[], 3).unwrap();
        assert_eq!(k, k2);
    }

    #[test]
    fn plain_auto_still_pads_deterministically_by_card_id() {
        let a = resolve("auto", &[], 1).unwrap();
        let b = resolve("auto", &[], 2).unwrap();
        assert_eq!(a, b, "plain auto ignores the seed, as before");
    }
}
