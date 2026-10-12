use crate::cards::{CardId, NUM_ZONE_IDS};

/// Slots in a `Counts`: one per card id, padded to a multiple of 32 so whole-set operations are
/// fixed-width (vectorizable). Unused slots are always zero. (128 lanes since Seaside and
/// Prosperity took the id count past 64; 160 since Cornucopia & Guilds took it past 128; 192 since the Promo cards took it past 160: 176 or 168
/// lanes, with a smaller alignment, made whole games 5-7% slower than a multiple of 32.)
pub const LANES: usize = 352;
const _: () = assert!(crate::cards::NUM_ZONE_IDS <= LANES, "more card ids that can sit in a zone than Counts lanes: widen LANES");

/// A multiset of cards stored as per-card counts. `Copy`, `LANES` bytes, never allocates. Big (288 bytes): a zone that is empty in nearly every game belongs in a
/// `state::SmallMultiset` instead (see "Keeping GameState small" in `docs/adding-a-set.md`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(align(32))]
pub struct Counts(pub [u8; LANES]);

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
    pub fn add(&mut self, c: CardId, n: u8) {
        self.0[c as usize] += n;
    }
    #[inline(always)]
    pub fn set(&mut self, c: CardId, n: u8) {
        self.0[c as usize] = n;
    }
    /// Removes one copy; returns false (and changes nothing) if absent.
    #[inline(always)]
    pub fn remove(&mut self, c: CardId) -> bool {
        let v = &mut self.0[c as usize];
        if *v == 0 {
            return false;
        }
        *v -= 1;
        true
    }
    #[inline]
    pub fn total(&self) -> u32 {
        // Sum of absolute differences against zero is a horizontal byte sum: 16 lanes per
        // instruction (SSE2 is part of the x86-64 baseline; other targets sum the bytes).
        #[cfg(target_arch = "x86_64")]
        // SAFETY: SSE2 is always available on x86-64; `Counts` is 32-byte aligned and `LANES` is a
        // multiple of 16, so every 16-byte load below is aligned and in bounds.
        unsafe {
            use std::arch::x86_64::*;
            let zero = _mm_setzero_si128();
            let mut acc = zero;
            let p = self.0.as_ptr() as *const __m128i;
            for i in 0..LANES / 16 {
                acc = _mm_add_epi64(acc, _mm_sad_epu8(_mm_load_si128(p.add(i)), zero));
            }
            (_mm_cvtsi128_si64(acc) + _mm_cvtsi128_si64(_mm_unpackhi_epi64(acc, acc))) as u32
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            self.0.iter().map(|&x| x as u32).sum()
        }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        // OR every word together: branch-free and vectorized, without a library compare call.
        let mut acc = 0u64;
        for w in 0..LANES / 8 {
            acc |= self.word(w);
        }
        acc == 0
    }
    #[inline]
    pub fn add_all(&mut self, other: &Counts) {
        for i in 0..LANES {
            self.0[i] += other.0[i];
        }
    }
    #[inline]
    pub fn clear(&mut self) {
        self.0 = [0; LANES];
    }
    /// Count of cards matching a type flag (see `cards::ACTION` etc.).
    pub fn count_type(&self, flag: u32) -> u32 {
        self.iter().filter(|&(c, _)| crate::cards::is(c, flag)).map(|(_, n)| n as u32).sum()
    }
    pub fn any_type(&self, flag: u32) -> bool {
        self.iter().any(|(c, _)| crate::cards::is(c, flag))
    }
    /// Iterate (card, count) for nonzero counts, in card-id order. Hands, decks and piles hold
    /// few distinct cards, so this looks at 32 lanes at a time and skips empty blocks.
    #[inline]
    pub fn iter(&self) -> NonZero<'_> {
        NonZero { counts: self, block: 0, mask: self.nonzero_mask(0) }
    }

    /// Bit `i` set when lane `32 * block + i` is nonzero.
    #[inline(always)]
    pub(crate) fn nonzero_mask(&self, block: usize) -> u32 {
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
        #[cfg(not(target_arch = "x86_64"))]
        {
            self.nonzero_mask_swar(block)
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

    #[inline(always)]
    fn word(&self, w: usize) -> u64 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.0[w * 8..w * 8 + 8]);
        u64::from_le_bytes(b)
    }
    /// Each count raised to at least `floor(card)`.
    pub fn max_with(mut self, floor: impl Fn(CardId) -> u8) -> Counts {
        for i in 0..NUM_ZONE_IDS {
            self.0[i] = self.0[i].max(floor(i as CardId));
        }
        self
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
    block: usize,
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
            self.block += 1;
            if self.block >= LANES / 32 {
                return None;
            }
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
        for i in (0..LANES).step_by(7) {
            c.0[i] = (i % 250 + 1) as u8;
        }
        c.0[LANES - 1] = 255;
        for b in 0..LANES / 32 {
            assert_eq!(c.nonzero_mask(b), c.nonzero_mask_swar(b), "block {b}");
        }
        let got: Vec<_> = c.iter().collect();
        let want: Vec<_> = (0..LANES).filter(|&i| c.0[i] != 0).map(|i| (i as CardId, c.0[i])).collect();
        assert_eq!(got, want);
    }
}
