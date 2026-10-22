//! The id capacity: `Counts` has 576 lanes (18 blocks, a `u32` block bitmap), `SupplyMask` five words (ids 0-639), and the Events are a 128-bit set whose hash is the
//! old `u64`'s while no Event past the 64th is in it (see `docs/card-id-capacity.md`).

use dominion_engine::cards::{self, CardId};
use dominion_engine::counts::{Counts, LANES};
use dominion_engine::state::{EvMask, SupplyCards, SupplyMask};
use std::hash::{Hash, Hasher};

fn hash_of<T: Hash>(t: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

#[test]
fn counts_has_576_lanes_and_a_wide_bitmap() {
    assert_eq!(LANES, 576);
    assert_eq!(std::mem::size_of::<Counts>(), 576);
    assert!(cards::NUM_ZONE_IDS <= LANES - 6, "ids must stay below the six bookkeeping lanes");
    // Every block of ids is usable, the block bitmap included (blocks 16 and 17 do not fit a u16).
    let mut c = Counts::EMPTY;
    for id in [0, 100, 415, 416, 480, 511, 512, 543, 544, 569] {
        c.add(id as CardId, 3);
    }
    assert_eq!(c.total(), 30);
    assert_eq!(c.iter().map(|(id, n)| (id as usize, n)).collect::<Vec<_>>(), [(0, 3), (100, 3), (415, 3), (416, 3), (480, 3), (511, 3), (512, 3), (543, 3), (544, 3), (569, 3)]);
    let mut d = c;
    d.add_all(&c);
    assert_eq!(d.get(569), 6);
    assert_eq!(d.total(), 60);
    d.clear();
    assert!(d.is_empty() && d.iter().next().is_none());
    // Equality and hashing look at the card lanes only.
    let mut e = Counts::EMPTY;
    e.add(569, 1);
    e.add(3, 1);
    assert!(e.remove(569));
    let mut f = Counts::EMPTY;
    f.add(3, 1);
    assert!(e == f);
    assert_eq!(hash_of(&e), hash_of(&f));
}

/// A `Counts` whose upper blocks (the 14th on) are empty hashes as the plain 13 legacy blocks did; a card in an upper block changes the hash.
#[test]
fn the_counts_hash_ignores_empty_upper_blocks() {
    let mut a = Counts::EMPTY;
    a.add(5, 2);
    a.add(300, 1);
    let before = hash_of(&a);
    // Touch an upper block and take the card away again: the bitmap keeps a stale bit, the hash does not change.
    a.add(540, 1);
    assert_ne!(hash_of(&a), before);
    assert!(a.remove(540));
    assert_eq!(hash_of(&a), before);
    let mut b = Counts::EMPTY;
    b.add(5, 2);
    b.add(300, 1);
    assert_eq!(hash_of(&b), before);
}

#[test]
fn the_supply_mask_has_five_words() {
    let mut m = SupplyMask::default();
    let ids = [0u16, 127, 128, 255, 256, 383, 384, 511, 512, 558, 639];
    for &id in &ids {
        assert!(!m.has(id));
        m.insert(id);
        assert!(m.has(id), "id {id}");
    }
    assert_eq!(m.ext2, (1u128 << 0) | (1u128 << 46) | (1u128 << 127));
    assert_eq!(m.ext, (1u128 << 0) | (1u128 << 127));
    let got: Vec<CardId> = SupplyCards::from_words(m.lo, m.hi, m.top, m.ext, m.ext2).collect();
    assert_eq!(got, ids);
    // Ids from 640 up (the pseudo-cards) are never in the mask.
    assert!(!m.has(640) && !m.has(700) && !m.has(u16::MAX));
    assert!(m.any(0, 0, 0, 0, 1u128 << 46));
    assert!(!m.any(0, 0, 0, 0, 1u128 << 45));
    m.remove(558);
    assert!(!m.has(558) && m.has(512) && m.has(639));
}

#[test]
fn events_are_a_128_bit_set_that_hashes_like_the_old_u64_below_the_65th() {
    let mut e = EvMask::EMPTY;
    assert!(e.is_empty());
    e.insert(3);
    e.insert(63);
    assert_eq!(hash_of(&e), hash_of(&((1u64 << 3) | (1u64 << 63))));
    assert_eq!(e.bits(), (1u128 << 3) | (1u128 << 63));
    assert!(e.has(3) && e.has(63) && !e.has(64));
    e.insert(70);
    e.insert(127);
    assert!(e.has(70) && e.has(127));
    assert_ne!(hash_of(&e), hash_of(&((1u64 << 3) | (1u64 << 63))));
    assert_eq!(e.events().collect::<Vec<_>>(), [3, 63, 70, 127].map(|i| cards::FIRST_EVENT + i));
}
