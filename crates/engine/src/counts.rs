use crate::cards::{CardId, NUM_ZONE_IDS};

/// Slots in a `Counts`: one per card id, padded to a multiple of 32 so whole-set operations are
/// fixed-width (vectorizable). Unused slots are always zero. (128 lanes since Seaside and
/// Prosperity took the id count past 64; 160 since Cornucopia & Guilds took it past 128; 192 since the Promo cards took it past 160: 176 or 168
/// lanes, with a smaller alignment, made whole games 5-7% slower than a multiple of 32.)
pub const LANES: usize = 416;
/// The last four lanes are bookkeeping, not card lanes (`NUM_ZONE_IDS` leaves them free): a little-endian `u16` bitmap of the 32-lane blocks that
/// may hold a card (bit `b` clear means block `b` is all zero), then the multiset's total count (`u16`). Every mutation keeps them up to date, so
/// `total` and `is_empty` are a load and `iter`, `add_all` and `clear` visit only the blocks that can have cards (a hand or a discard pile has
/// cards in a few of the 12 blocks). The bitmap is a superset: `remove` does not clear a bit (except when the last card goes), and a stale bit
/// costs one more block to look at. `Eq` and `Hash` ignore the bookkeeping lanes.
const BLOCKS_LANE: usize = LANES - 4;
const TOTAL_LANE: usize = LANES - 2;
/// The card lanes of the last 32-lane block (all but the four bookkeeping lanes), as a mask of `nonzero_mask`.
const LAST_BLOCK_CARDS: u32 = (1 << (BLOCKS_LANE - (LANES - 32))) - 1;
const _: () = assert!(crate::cards::NUM_ZONE_IDS <= BLOCKS_LANE, "more card ids that can sit in a zone than Counts lanes: widen LANES");
const _: () = assert!(LANES / 32 <= 16, "the block bitmap is a u16");

/// A multiset of cards stored as per-card counts. `Copy`, `LANES` bytes, never allocates. Big (384 bytes): a zone that is empty in nearly every game belongs in a
/// `state::SmallMultiset` instead (see "Keeping GameState small" in `docs/adding-a-set.md`).
#[derive(Clone, Copy)]
#[repr(align(32))]
pub struct Counts([u8; LANES]);

/// Equal when the card lanes are (the bookkeeping lanes can differ: the block bitmap is a superset).
impl PartialEq for Counts {
    fn eq(&self, other: &Counts) -> bool {
        self.0[..BLOCKS_LANE] == other.0[..BLOCKS_LANE]
    }
}

impl Eq for Counts {}

/// Hashes as the plain `[u8; LANES]` of the card lanes (the bookkeeping lanes as zero), so the hash of a state (`stable_seed`, the search's table
/// key) does not depend on how `Counts` caches its total and blocks.
impl std::hash::Hash for Counts {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut lanes = self.0;
        lanes[BLOCKS_LANE..].fill(0);
        lanes.hash(state);
    }
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
        u16::from_le_bytes([self.0[BLOCKS_LANE], self.0[BLOCKS_LANE + 1]]) as u32
    }
    #[inline(always)]
    fn set_blocks(&mut self, b: u32) {
        let b = (b as u16).to_le_bytes();
        self.0[BLOCKS_LANE] = b[0];
        self.0[BLOCKS_LANE + 1] = b[1];
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
    pub fn nth(&self, mut idx: u32) -> CardId {
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
        let mut plain = [0u8; LANES];
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
