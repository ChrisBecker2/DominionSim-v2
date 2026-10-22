use crate::cards::{CardId, NUM_ZONE_IDS};

/// Slots in a `Counts`: one per card id, padded to a multiple of 32 so whole-set operations are
/// fixed-width (vectorizable). Unused slots are always zero. (128 lanes since Seaside and
/// Prosperity took the id count past 64; 160 since Cornucopia & Guilds took it past 128; 192 since the Promo cards took it past 160: 176 or 168
/// lanes, with a smaller alignment, made whole games 5-7% slower than a multiple of 32.)
pub const LANES: usize = 576;
/// The last six lanes are bookkeeping, not card lanes (`NUM_ZONE_IDS` leaves them free): a little-endian `u32` bitmap of the 32-lane blocks that
/// may hold a card (bit `b` clear means block `b` is all zero), then the multiset's total count (`u16`). Every mutation keeps them up to date, so
/// `total` and `is_empty` are a load and `iter`, `add_all` and `clear` visit only the blocks that can have cards (a hand or a discard pile has
/// cards in a few of the 18 blocks). The bitmap is a superset: `remove` does not clear a bit (except when the last card goes), and a stale bit
/// costs one more block to look at. `Eq` and `Hash` ignore the bookkeeping lanes. (The bitmap was a `u16` and the bookkeeping four lanes while
/// `LANES` was at most 512.)
const BLOCKS_LANE: usize = LANES - 6;
const TOTAL_LANE: usize = LANES - 2;
/// The card lanes of the last 32-lane block (all but the six bookkeeping lanes), as a mask of `nonzero_mask`.
const LAST_BLOCK_CARDS: u32 = (1 << (BLOCKS_LANE - (LANES - 32))) - 1;
const _: () = assert!(crate::cards::NUM_ZONE_IDS <= BLOCKS_LANE, "more card ids that can sit in a zone than Counts lanes: widen LANES");
const _: () = assert!(LANES / 32 <= 32, "the block bitmap is a u32");

/// A multiset of cards stored as per-card counts. `Copy`, `LANES` bytes, never allocates. Big (384 bytes): a zone that is empty in nearly every game belongs in a
/// `state::SmallMultiset` instead (see "Keeping GameState small" in `docs/adding-a-set.md`).
#[derive(Clone, Copy)]
#[repr(align(32))]
pub struct Counts([u8; LANES]);

/// A set of cards as one byte per lane (0xFF in the lanes of its cards, 0 elsewhere, the bookkeeping lanes 0): the question `Counts::intersects` asks.
/// Built at compile time from the card table (`sets::ACTION_LANES` ...).
#[derive(Clone, Copy)]
#[repr(align(32))]
pub struct LaneMask(pub(crate) [u8; LANES]);

impl LaneMask {
    pub const EMPTY: LaneMask = LaneMask([0; LANES]);
}

/// Equal when the card lanes are (the bookkeeping lanes can differ: the block bitmap is a superset).
impl PartialEq for Counts {
    fn eq(&self, other: &Counts) -> bool {
        self.0[..BLOCKS_LANE] == other.0[..BLOCKS_LANE]
    }
}

impl Eq for Counts {}

/// The blocks that are always hashed, empty or not: the 13 of the 416-lane layout (`LANES` when the hash keys of the search were first stored).
/// A wider `Counts` hashes the blocks past them only when they hold a card, so the cost of a hash follows the cards present, not `LANES`.
const LEGACY_BLOCKS: usize = 13;
const _: () = assert!(LANES / 32 >= LEGACY_BLOCKS, "Counts narrower than the legacy hash layout");

/// Hashes as the plain `[u8; 416]` of the first 13 blocks of card lanes (the bookkeeping lanes as zero; a test pins the equality), so the hash of a state
/// (`stable_seed`, the search's table key) does not depend on how `Counts` caches its total and blocks. Blocks past the 13th (none while `LANES` is 416) add
/// their index and 32 bytes each, and only when they hold a card.
impl std::hash::Hash for Counts {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        hash_blocks(&self.0, BLOCKS_LANE, self.blocks(), state);
    }
}

/// The 32 lanes of block `b` with the bookkeeping lanes (`card_lanes` on) as zero.
#[inline(always)]
fn piece_of(lanes: &[u8], card_lanes: usize, b: usize) -> [u8; 32] {
    let mut piece = [0u8; 32];
    piece.copy_from_slice(&lanes[b * 32..b * 32 + 32]);
    if card_lanes < b * 32 + 32 {
        piece[card_lanes.saturating_sub(b * 32)..].fill(0);
    }
    piece
}

/// The hash step of one legacy block: its 32 lanes, or a constant piece when the bitmap says it holds no card (a hasher that goes word by
/// word, the search's FxHasher, folds a constant piece into a cheaper step, and a stream hasher takes in one copy).
#[inline(always)]
fn hash_legacy_block<H: std::hash::Hasher>(lanes: &[u8], card_lanes: usize, blocks: u32, b: usize, state: &mut H) {
    if blocks & (1 << b) == 0 {
        state.write(&[0u8; 32]);
    } else {
        state.write(&piece_of(lanes, card_lanes, b));
    }
}

/// The blocks past the legacy ones that hold a card, each by index and content.
#[inline(always)]
fn hash_upper_blocks<H: std::hash::Hasher>(lanes: &[u8], card_lanes: usize, blocks: u32, state: &mut H) {
    let mut upper = blocks >> LEGACY_BLOCKS << LEGACY_BLOCKS;
    while upper != 0 {
        let b = upper.trailing_zeros() as usize;
        upper &= upper - 1;
        let piece = piece_of(lanes, card_lanes, b);
        // A stale bit (no card left in the block) hashes as no block.
        if piece != [0u8; 32] {
            state.write_u8(b as u8);
            state.write(&piece);
        }
    }
}

/// `Counts::hash` over any lane array (`lanes.len()` a multiple of 32, the lanes from `card_lanes` on bookkeeping) so that a test can use a wider one: the
/// length prefix of the legacy `[u8; 416]`, its blocks in order, then the upper blocks that hold a card.
#[inline(always)]
fn hash_blocks<H: std::hash::Hasher>(lanes: &[u8], card_lanes: usize, blocks: u32, state: &mut H) {
    state.write_usize(LEGACY_BLOCKS * 32);
    for b in 0..LEGACY_BLOCKS {
        hash_legacy_block(lanes, card_lanes, blocks, b, state);
    }
    hash_upper_blocks(lanes, card_lanes, blocks, state);
}

impl Default for Counts {
    fn default() -> Self {
        Counts::EMPTY
    }
}

impl Counts {
    pub const EMPTY: Counts = Counts([0; LANES]);


    #[inline(always)]
    pub fn get(&self, c: CardId) -> u8 {
        self.0[c as usize]
    }
    #[inline(always)]
    pub fn has(&self, c: CardId) -> bool {
        self.0[c as usize] > 0
    }
    #[inline(always)]
    fn blocks(&self) -> u32 {
        u32::from_le_bytes([self.0[BLOCKS_LANE], self.0[BLOCKS_LANE + 1], self.0[BLOCKS_LANE + 2], self.0[BLOCKS_LANE + 3]])
    }
    #[inline(always)]
    fn set_blocks(&mut self, b: u32) {
        let b = b.to_le_bytes();
        self.0[BLOCKS_LANE] = b[0];
        self.0[BLOCKS_LANE + 1] = b[1];
        self.0[BLOCKS_LANE + 2] = b[2];
        self.0[BLOCKS_LANE + 3] = b[3];
    }
    #[inline(always)]
    fn cached_total(&self) -> u16 {
        u16::from_le_bytes([self.0[TOTAL_LANE], self.0[TOTAL_LANE + 1]])
    }
    #[inline(always)]
    fn set_cached_total(&mut self, t: u16) {
        let b = t.to_le_bytes();
        self.0[TOTAL_LANE] = b[0];
        self.0[TOTAL_LANE + 1] = b[1];
    }
    #[inline(always)]
    pub fn add(&mut self, c: CardId, n: u8) {
        self.0[c as usize] += n;
        self.set_cached_total(self.cached_total().wrapping_add(n as u16));
        self.set_blocks(self.blocks() | 1 << (c as usize / 32));
    }
    #[inline(always)]
    pub fn set(&mut self, c: CardId, n: u8) {
        let old = std::mem::replace(&mut self.0[c as usize], n);
        self.set_cached_total(self.cached_total().wrapping_add(n as u16).wrapping_sub(old as u16));
        self.set_blocks(self.blocks() | 1 << (c as usize / 32));
    }
    /// Removes one copy; returns false (and changes nothing) if absent.
    #[inline(always)]
    pub fn remove(&mut self, c: CardId) -> bool {
        let v = &mut self.0[c as usize];
        if *v == 0 {
            return false;
        }
        *v -= 1;
        let total = self.cached_total().wrapping_sub(1);
        self.set_cached_total(total);
        // Nothing left: no block has a card.
        if total == 0 {
            self.set_blocks(0);
        }
        true
    }
    #[inline(always)]
    pub fn total(&self) -> u32 {
        self.cached_total() as u32
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.cached_total() == 0
    }
    /// The total by summing the lanes (what `total` caches) and whether every block with a card is in the bitmap: for tests.
    #[cfg(test)]
    fn check_bookkeeping(&self) {
        let by_lanes: u32 = self.0[..BLOCKS_LANE].iter().map(|&x| x as u32).sum();
        assert_eq!(self.total(), by_lanes, "cached total");
        for b in 0..LANES / 32 {
            if self.nonzero_mask(b) != 0 {
                assert!(self.blocks() & (1 << b) != 0, "block {b} has cards but is not in the bitmap");
            }
        }
    }
    #[inline]
    pub fn add_all(&mut self, other: &Counts) {
        let (blocks, total) = (self.blocks() | other.blocks(), self.cached_total().wrapping_add(other.cached_total()));
        let mut todo = other.blocks();
        while todo != 0 {
            let b = todo.trailing_zeros() as usize * 32;
            todo &= todo - 1;
            // The last block's bookkeeping lanes are added too and written again below.
            for i in b..b + 32 {
                self.0[i] = self.0[i].wrapping_add(other.0[i]);
            }
        }
        self.set_blocks(blocks);
        self.set_cached_total(total);
    }
    #[inline]
    pub fn clear(&mut self) {
        let mut todo = self.blocks();
        while todo != 0 {
            let b = todo.trailing_zeros() as usize * 32;
            todo &= todo - 1;
            self.0[b..b + 32].fill(0);
        }
        self.set_blocks(0);
        self.set_cached_total(0);
    }
    /// Count of cards matching a type flag (see `cards::ACTION` etc.).
    pub fn count_type(&self, flag: u32) -> u32 {
        self.iter().filter(|&(c, _)| crate::cards::is(c, flag)).map(|(_, n)| n as u32).sum()
    }
    pub fn any_type(&self, flag: u32) -> bool {
        self.iter().any(|(c, _)| crate::cards::is(c, flag))
    }
    /// Iterate (card, count) for nonzero counts, in card-id order. Hands, decks and piles hold
    /// few distinct cards, so this looks at 32 lanes at a time, only in the blocks that can have a card.
    #[inline]
    pub fn iter(&self) -> NonZero<'_> {
        NonZero { counts: self, block: 0, blocks: self.blocks(), mask: 0 }
    }

    /// Bit `i` set when lane `32 * block + i` is nonzero (the bookkeeping lanes are not card lanes).
    #[inline(always)]
    pub(crate) fn nonzero_mask(&self, block: usize) -> u32 {
        #[cfg(target_arch = "x86_64")]
        let m = self.nonzero_mask_simd(block);
        #[cfg(not(target_arch = "x86_64"))]
        let m = self.nonzero_mask_swar(block);
        if block == LANES / 32 - 1 { m & LAST_BLOCK_CARDS } else { m }
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn nonzero_mask_simd(&self, block: usize) -> u32 {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: SSE2 is always available on x86-64; `Counts` is 32-byte aligned, `LANES` is a multiple of 32, and `block < LANES / 32`
        // (callers check), so both 16-byte loads are aligned and in bounds.
        unsafe {
            use std::arch::x86_64::*;
            let p = self.0.as_ptr().add(block * 32) as *const __m128i;
            let zero = _mm_setzero_si128();
            let lo = _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_load_si128(p), zero)) as u32;
            let hi = _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_load_si128(p.add(1)), zero)) as u32;
            !(lo | hi << 16)
        }
    }

    /// `nonzero_mask` without SIMD (other targets; tested against the SIMD version).
    #[inline(always)]
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))]
    fn nonzero_mask_swar(&self, block: usize) -> u32 {
        let mut m = 0u32;
        for k in 0..4 {
            let nz = nonzero_bytes(self.word(block * 4 + k)) >> 7;
            // One bit per byte, gathered into the low byte.
            m |= ((nz.wrapping_mul(0x0102_0408_1020_4080) >> 56) as u32) << (8 * k);
        }
        m
    }

    #[cfg_attr(target_arch = "x86_64", allow(dead_code))]
    #[inline(always)]
    fn word(&self, w: usize) -> u64 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.0[w * 8..w * 8 + 8]);
        u64::from_le_bytes(b)
    }
    /// Whether the multiset holds a card whose lane `mask` marks: looks at the blocks that can have cards, 32 lanes in a few instructions, instead of
    /// asking each card in the hand about its type.
    #[inline]
    pub fn intersects(&self, mask: &LaneMask) -> bool {
        let mut todo = self.blocks();
        while todo != 0 {
            let b = todo.trailing_zeros() as usize;
            todo &= todo - 1;
            if self.block_intersects(mask, b) {
                return true;
            }
        }
        false
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn block_intersects(&self, mask: &LaneMask, block: usize) -> bool {
        // SAFETY: SSE2 is always available on x86-64; `Counts` and `LaneMask` are 32-byte aligned, `LANES` is a multiple of 32 and
        // `block < LANES / 32` (it is a bit of the bitmap), so the four 16-byte loads are aligned and in bounds.
        unsafe {
            use std::arch::x86_64::*;
            let (a, m) = (self.0.as_ptr().add(block * 32) as *const __m128i, mask.0.as_ptr().add(block * 32) as *const __m128i);
            let both = _mm_or_si128(_mm_and_si128(_mm_load_si128(a), _mm_load_si128(m)), _mm_and_si128(_mm_load_si128(a.add(1)), _mm_load_si128(m.add(1))));
            _mm_movemask_epi8(_mm_cmpeq_epi8(both, _mm_setzero_si128())) != 0xFFFF
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    fn block_intersects(&self, mask: &LaneMask, block: usize) -> bool {
        (0..4).any(|k| self.word(block * 4 + k) & u64::from_le_bytes(mask.0[(block * 4 + k) * 8..(block * 4 + k) * 8 + 8].try_into().unwrap()) != 0)
    }

    /// In block `b` (not the last): the lane holding the `idx`-th card when the block has more than `idx` cards, else the block's card total.
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    unsafe fn block_nth(&self, b: usize, idx: u32) -> (Option<usize>, u32) {
        use std::arch::x86_64::*;
        let p = self.0.as_ptr().add(b * 32) as *const __m128i;
        let (mut lo, mut hi) = (_mm_load_si128(p), _mm_load_si128(p.add(1)));
        lo = _mm_add_epi8(lo, _mm_slli_si128::<1>(lo));
        lo = _mm_add_epi8(lo, _mm_slli_si128::<2>(lo));
        lo = _mm_add_epi8(lo, _mm_slli_si128::<4>(lo));
        lo = _mm_add_epi8(lo, _mm_slli_si128::<8>(lo));
        hi = _mm_add_epi8(hi, _mm_slli_si128::<1>(hi));
        hi = _mm_add_epi8(hi, _mm_slli_si128::<2>(hi));
        hi = _mm_add_epi8(hi, _mm_slli_si128::<4>(hi));
        hi = _mm_add_epi8(hi, _mm_slli_si128::<8>(hi));
        // The low half's total added to every lane of the high half.
        let t = _mm_srli_si128::<15>(lo);
        let carry = _mm_shuffle_epi32::<0>(_mm_unpacklo_epi8(_mm_unpacklo_epi8(t, t), _mm_unpacklo_epi8(t, t)));
        hi = _mm_add_epi8(hi, carry);
        let total = _mm_cvtsi128_si32(_mm_srli_si128::<15>(hi)) as u32 & 0xFF;
        let v = _mm_set1_epi8(idx.min(255) as i8);
        // Lanes whose prefix sum is not above idx (unsigned bytes): a run from lane 0.
        let le = |x: __m128i| _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_min_epu8(x, v), x)) as u32;
        let below = le(lo) | le(hi) << 16;
        if idx < total { (Some(below.count_ones() as usize), total) } else { (None, total) }
    }

    /// Each count raised to at least the other's (the union of two multisets).
    #[inline]
    pub fn max_assign(&mut self, other: &Counts) {
        for i in 0..BLOCKS_LANE {
            self.0[i] = self.0[i].max(other.0[i]);
        }
        self.set_blocks(self.blocks() | other.blocks());
        self.recount();
    }
    /// Each count raised to at least `floor(card)`.
    pub fn max_with(mut self, floor: impl Fn(CardId) -> u8) -> Counts {
        for i in 0..NUM_ZONE_IDS {
            self.0[i] = self.0[i].max(floor(i as CardId));
        }
        self.set_blocks((1 << (LANES / 32)) - 1);
        self.recount();
        self
    }
    /// Sets the cached total from the lanes (after lanes were changed directly).
    fn recount(&mut self) {
        let t: u32 = self.0[..BLOCKS_LANE].iter().map(|&x| x as u32).sum();
        self.set_cached_total(t as u16);
    }

    /// The card at position `idx` (0-based) when the multiset is laid out in id order.
    /// Used to sample uniformly: `nth(rng.below(total))`.
    #[inline]
    pub fn nth(&self, idx: u32) -> CardId {
        #[cfg(target_arch = "x86_64")]
        if self.cached_total() <= 255 {
            // Prefix sums of a block's 32 lanes in bytes (no wrap: the total fits), the lane is the number of prefix sums not above `idx`: no branch per card.
            let mut todo = self.blocks();
            let mut left = idx;
            while todo != 0 {
                let b = todo.trailing_zeros() as usize;
                todo &= todo - 1;
                // The last block holds the bookkeeping lanes: the generic walk below, from the start.
                if b == LANES / 32 - 1 {
                    break;
                }
                // SAFETY: SSE2 is baseline on x86-64; the block is in bounds and 32-byte aligned.
                let (lane, total) = unsafe { self.block_nth(b, left) };
                if let Some(l) = lane {
                    return (b * 32 + l) as CardId;
                }
                left -= total;
            }
        }
        let mut idx = idx;
        for (c, n) in self.iter() {
            if idx < n as u32 {
                return c;
            }
            idx -= n as u32;
        }
        panic!("Counts::nth out of range")
    }
}

/// The high bit of every nonzero byte of `w` (and no other bits).
#[inline(always)]
fn nonzero_bytes(w: u64) -> u64 {
    const LOW7: u64 = 0x7f7f_7f7f_7f7f_7f7f;
    (((w & LOW7) + LOW7) | w) & !LOW7
}

/// Iterator over a `Counts`' nonzero entries (see `Counts::iter`).
pub struct NonZero<'a> {
    counts: &'a Counts,
    /// The block `mask` is of.
    block: usize,
    /// The blocks not looked at yet that may have cards (bit `b` for block `b`).
    blocks: u32,
    /// Nonzero lanes of the current block not yet returned.
    mask: u32,
}

impl Iterator for NonZero<'_> {
    type Item = (CardId, u8);

    #[inline]
    fn next(&mut self) -> Option<(CardId, u8)> {
        loop {
            if self.mask != 0 {
                let i = self.block * 32 + self.mask.trailing_zeros() as usize;
                self.mask &= self.mask - 1;
                return Some((i as CardId, self.counts.0[i]));
            }
            if self.blocks == 0 {
                return None;
            }
            self.block = self.blocks.trailing_zeros() as usize;
            self.blocks &= self.blocks - 1;
            self.mask = self.counts.nonzero_mask(self.block);
        }
    }
}

impl std::fmt::Debug for Counts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        write!(f, "[")?;
        for (c, n) in self.iter() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            if n == 1 {
                write!(f, "{}", crate::cards::name(c))?;
            } else {
                write!(f, "{}x {}", n, crate::cards::name(c))?;
            }
        }
        write!(f, "]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Hash` is that of the plain card-lane array (bookkeeping lanes as zero), however the bitmap and the total stand, for a stream hasher and a word-by-word one.
    #[test]
    fn hash_is_that_of_the_lane_array() {
        use std::hash::{Hash, Hasher};
        struct Words(u64);
        impl Hasher for Words {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for chunk in bytes.chunks(8) {
                    let mut w = [0u8; 8];
                    w[..chunk.len()].copy_from_slice(chunk);
                    self.0 = (self.0.rotate_left(5) ^ u64::from_le_bytes(w)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
                }
            }
        }
        fn check(c: &Counts) {
            // (Blocks past the legacy 13 are hashed only when they hold a card, by index: `upper_blocks_are_hashed_only_when_they_hold_a_card`.)
            let mut lanes = [0u8; LEGACY_BLOCKS * 32];
            lanes.copy_from_slice(&c.0[..LEGACY_BLOCKS * 32]);
            let (mut a, mut b) = (std::collections::hash_map::DefaultHasher::new(), std::collections::hash_map::DefaultHasher::new());
            c.hash(&mut a);
            lanes.hash(&mut b);
            assert_eq!(a.finish(), b.finish());
            let (mut a, mut b) = (Words(0), Words(0));
            c.hash(&mut a);
            lanes.hash(&mut b);
            assert_eq!(a.0, b.0);
        }
        let mut c = Counts::EMPTY;
        check(&c);
        for id in [0, 5, 31, 32, 100, 351, 352, LEGACY_BLOCKS * 32 - 1] {
            c.add(id as CardId, 3);
            check(&c);
        }
        // Removing leaves a stale bit in the bitmap: still the same hash.
        for id in [5, 100, 351] {
            c.set(id as CardId, 0);
            check(&c);
        }
        c.clear();
        check(&c);
    }

    /// `nth` (the SIMD walk for totals up to 255) is the card at that position of the id-ordered multiset.
    #[test]
    fn nth_is_the_card_at_that_position() {
        let mut seed = 99u64;
        let mut rnd = move |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        for round in 0..3000 {
            let mut c = Counts::EMPTY;
            let kinds = 1 + rnd(12);
            let top = if round % 3 == 0 { crate::cards::NUM_ZONE_IDS as u64 } else { 70 };
            for _ in 0..kinds {
                c.add(rnd(top) as CardId, 1 + rnd(if round % 5 == 0 { 40 } else { 6 }) as u8);
            }
            let mut flat = Vec::new();
            for (id, n) in c.iter() {
                flat.extend(std::iter::repeat(id).take(n as usize));
            }
            for (i, want) in flat.iter().enumerate() {
                assert_eq!(c.nth(i as u32), *want, "position {i} of {c:?}");
            }
        }
    }

    /// `intersects` a card-type lane mask is the question about each card's type that it replaces, for one card and for sets of cards (also with stale bitmap bits).
    #[test]
    fn intersects_matches_the_type_of_the_cards() {
        use crate::cards::{self, NUM_ZONE_IDS as NUM_CARDS};
        use crate::sets::{ACTION_LANES, CHOICE_TREASURE_LANES, OTHER_TREASURE_LANES, TREASURE_LANES};
        let by_type = |c: &Counts, flag: u32| c.iter().any(|(x, _)| cards::is(x, flag));
        let mut seed = 12345u64;
        let mut rnd = move |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        for c in 0..NUM_CARDS as CardId {
            let mut one = Counts::EMPTY;
            one.add(c, 2);
            assert_eq!(one.intersects(&ACTION_LANES), by_type(&one, cards::ACTION), "{}", cards::name(c));
            assert_eq!(one.intersects(&TREASURE_LANES), by_type(&one, cards::TREASURE), "{}", cards::name(c));
            assert_eq!(one.intersects(&OTHER_TREASURE_LANES), by_type(&one, cards::TREASURE) && c > crate::cards::id::GOLD, "{}", cards::name(c));
            assert_eq!(one.intersects(&CHOICE_TREASURE_LANES), by_type(&one, cards::TREASURE) && !cards::is_choice_free(c), "{}", cards::name(c));
        }
        for _ in 0..2000 {
            let mut set = Counts::EMPTY;
            for _ in 0..rnd(8) {
                set.add(rnd(NUM_CARDS as u64) as CardId, 1);
            }
            // A stale bitmap bit: a card added and removed again.
            let ghost = rnd(NUM_CARDS as u64) as CardId;
            set.add(ghost, 1);
            set.remove(ghost);
            assert_eq!(set.intersects(&ACTION_LANES), by_type(&set, cards::ACTION));
            assert_eq!(set.intersects(&TREASURE_LANES), by_type(&set, cards::TREASURE));
        }
    }

    /// Blocks past the legacy 13 (a wider `Counts`, exercised on a plain lane array) are hashed only when they hold a card, by index and content: no card in them is the
    /// legacy hash, a card in different blocks or lanes is different, and a stale bitmap bit changes nothing.
    #[test]
    fn upper_blocks_are_hashed_only_when_they_hold_a_card() {
        use std::hash::{Hash, Hasher};
        const WIDE: usize = 640;
        let hash = |lanes: &[u8; WIDE], blocks: u32| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            hash_blocks(lanes, WIDE - 6, blocks, &mut h);
            h.finish()
        };
        let legacy = |lanes: &[u8; WIDE]| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            let mut plain = [0u8; LEGACY_BLOCKS * 32];
            plain.copy_from_slice(&lanes[..LEGACY_BLOCKS * 32]);
            plain.hash(&mut h);
            h.finish()
        };
        let mut a = [0u8; WIDE];
        a[5] = 2;
        a[300] = 1;
        let low_blocks = (1 << 0) | (1 << 9);
        assert_eq!(hash(&a, low_blocks), legacy(&a), "nothing above the 13th block: the plain legacy hash");
        assert_eq!(hash(&a, low_blocks | 1 << 15), legacy(&a), "a stale bit in an empty upper block changes nothing");
        // The bookkeeping lanes at the end are not cards.
        let mut b = a;
        b[WIDE - 5] = 77;
        assert_eq!(hash(&b, low_blocks), hash(&a, low_blocks));
        let mut seen = vec![hash(&a, low_blocks)];
        for lane in [416, 417, 447, 448, 500, 511, 512, 545, WIDE - 7] {
            let mut x = a;
            x[lane] = 1;
            let h = hash(&x, low_blocks | 1 << (lane / 32));
            assert!(!seen.contains(&h), "lane {lane} hashes like an earlier state");
            seen.push(h);
        }
        // The same piece in two different upper blocks differs (the index is hashed).
        let (mut x, mut y) = (a, a);
        x[448 + 3] = 1;
        y[480 + 3] = 1;
        assert_ne!(hash(&x, low_blocks | 1 << 14), hash(&y, low_blocks | 1 << 15));
        // A card in the last lane of the 13th block is a card (it was a bookkeeping lane in the 416-lane layout).
        let mut z = a;
        z[411] = 1;
        assert_ne!(hash(&z, low_blocks | 1 << 12), hash(&a, low_blocks));
    }

    /// The layout of the 576-lane `Counts`: 18 blocks, a `u32` bitmap in the lanes before the `u16` total, card lanes up to 570 (the old 512-lane layout's
    /// bookkeeping lanes 508..512 are card lanes now).
    #[test]
    fn the_layout_has_eighteen_blocks_and_a_u32_bitmap() {
        assert_eq!(LANES, 576);
        assert_eq!(BLOCKS_LANE, 570);
        assert_eq!(TOTAL_LANE, 574);
        assert_eq!(std::mem::size_of::<Counts>(), 576);
        let mut c = Counts::EMPTY;
        for id in [0usize, 31, 32, 415, 416, 480, 507, 508, 511, 512, 543, 544, 569] {
            c.add(id as CardId, 2);
            c.check_bookkeeping();
            assert_eq!(c.get(id as CardId), 2);
        }
        // The bitmap reaches block 17 (bit 17 is beyond a u16) and the total is not disturbed by it.
        assert!(c.blocks() & (1 << 17) != 0 && c.blocks() > u16::MAX as u32);
        assert_eq!(c.total(), 26);
        assert_eq!(c.iter().map(|(id, _)| id as usize).collect::<Vec<_>>(), vec![0, 31, 32, 415, 416, 480, 507, 508, 511, 512, 543, 544, 569]);
        let mut d = Counts::EMPTY;
        d.add_all(&c);
        d.add_all(&c);
        d.check_bookkeeping();
        assert_eq!(d.get(569), 4);
        assert_eq!(d.total(), 52);
        assert!(d == { let mut e = c; e.add_all(&c); e });
        d.clear();
        assert!(d.is_empty() && d.blocks() == 0);
        // Removing the last card of the multiset empties the bitmap.
        let mut s = Counts::EMPTY;
        s.add(569, 1);
        assert!(s.remove(569) && s.blocks() == 0 && s.is_empty());
        // The state the bookkeeping lanes are in never shows as a card: the last block iterates only its 26 card lanes.
        let full = Counts::EMPTY.max_with(|_| 1);
        assert_eq!(full.iter().count(), NUM_ZONE_IDS);
    }

    #[test]
    fn simd_and_swar_masks_agree() {
        let mut c = Counts::EMPTY;
        for i in (0..BLOCKS_LANE).step_by(7) {
            c.set(i as CardId, (i % 250 + 1) as u8);
        }
        c.set((BLOCKS_LANE - 1) as CardId, 255);
        for b in 0..LANES / 32 {
            let swar = c.nonzero_mask_swar(b) & if b == LANES / 32 - 1 { LAST_BLOCK_CARDS } else { !0 };
            assert_eq!(c.nonzero_mask(b), swar, "block {b}");
        }
        let got: Vec<_> = c.iter().collect();
        let want: Vec<_> = (0..BLOCKS_LANE).filter(|&i| c.0[i] != 0).map(|i| (i as CardId, c.0[i])).collect();
        assert_eq!(got, want);
        c.check_bookkeeping();
    }

    #[test]
    fn the_bookkeeping_follows_every_mutation() {
        let mut a = Counts::EMPTY;
        let mut b = Counts::EMPTY;
        a.add(3, 5);
        a.add(300, 200);
        a.check_bookkeeping();
        a.set(3, 2);
        a.set(7, 9);
        a.check_bookkeeping();
        assert!(a.remove(3));
        assert!(!a.remove(100));
        a.check_bookkeeping();
        b.add(7, 1);
        b.add(330, 50);
        b.add_all(&a);
        b.check_bookkeeping();
        assert_eq!(b.total(), 1 + 50 + 1 + 9 + 200);
        let mut u = a;
        u.max_assign(&b);
        u.check_bookkeeping();
        let w = a.max_with(|c| u8::from(c == 5));
        w.check_bookkeeping();
        assert!(!w.is_empty());
        b.clear();
        assert!(b.is_empty());
        assert_eq!(b.iter().count(), 0);
        b.check_bookkeeping();
        // A block that emptied stays in the bitmap until the multiset is empty or cleared, and iterates as nothing.
        let mut c = Counts::EMPTY;
        c.add(300, 1);
        c.add(5, 1);
        assert!(c.remove(300));
        c.check_bookkeeping();
        assert_eq!(c.iter().collect::<Vec<_>>(), vec![(5, 1)]);
        assert!(c.remove(5));
        assert!(c.is_empty() && c.blocks() == 0);
        // Equal contents are equal whatever the bitmaps say, and hash like the plain lanes.
        let mut x = Counts::EMPTY;
        x.add(10, 3);
        x.add(11, 1);
        x.add(200, 1);
        assert!(x.remove(200));
        let mut y = Counts::EMPTY;
        y.add(11, 1);
        y.add(10, 3);
        assert!(x == y);
        use std::hash::{Hash, Hasher};
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        x.hash(&mut h1);
        let mut plain = [0u8; LEGACY_BLOCKS * 32];
        plain[10] = 3;
        plain[11] = 1;
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        plain.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
        // Every id from the first to the last card lane works, including the last block.
        let mut all = Counts::EMPTY;
        for id in 0..NUM_ZONE_IDS {
            all.add(id as CardId, 1);
        }
        all.check_bookkeeping();
        assert_eq!(all.iter().count(), NUM_ZONE_IDS);
        let mut twice = all;
        twice.add_all(&all);
        twice.check_bookkeeping();
        assert_eq!(twice.total(), 2 * NUM_ZONE_IDS as u32);
    }
}
