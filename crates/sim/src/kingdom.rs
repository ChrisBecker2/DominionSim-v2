//! Kingdom selection for the CLI: a preset name, a comma-separated list of 10 card names, or
//! `auto` (the union of cards the given strategies reference, padded to 10).

use std::collections::BTreeSet;

use dominion_engine::cards::{self, CardId};

use crate::strategy::Strategy;

/// Resolve a `--kingdom` argument into exactly 10 kingdom card ids.
pub fn resolve(spec: &str, strategies: &[&Strategy]) -> Result<Vec<CardId>, String> {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("first-game") || spec.eq_ignore_ascii_case("first_game") {
        return Ok(cards::FIRST_GAME.to_vec());
    }
    if spec.eq_ignore_ascii_case("auto") {
        return resolve_auto(strategies);
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
        return Err(format!("--kingdom must name exactly 10 kingdom cards (or \"first-game\" / \"auto\"), got {}", out.len()));
    }
    Ok(out)
}

fn resolve_auto(strategies: &[&Strategy]) -> Result<Vec<CardId>, String> {
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
    let mut out: Vec<CardId> = used.into_iter().collect();
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
        let k = resolve("first-game", &[]).unwrap();
        assert_eq!(k.len(), 10);
    }

    #[test]
    fn explicit_list() {
        let k = resolve("Village,Smithy,Witch,Chapel,Market,Gardens,Militia,Moat,Bandit,Cellar", &[]).unwrap();
        assert_eq!(k.len(), 10);
    }

    #[test]
    fn rejects_wrong_count() {
        assert!(resolve("Village,Smithy", &[]).is_err());
    }
}
