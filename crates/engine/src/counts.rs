use crate::cards::{CardId, NUM_CARDS};

/// Slots in a `Counts`: one per card id, padded to 64 so a `Counts` is exactly one cache line
/// and whole-set operations are fixed-width (vectorizable). Unused slots are always zero.
pub const LANES: usize = 64;
const _: () = assert!(NUM_CARDS <= LANES, "more cards than Counts lanes: widen LANES");

/// A multiset of cards stored as per-card counts. `Copy`, 64 bytes, never allocates.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(align(64))]
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
        self.0.iter().map(|&x| x as u32).sum()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
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
    pub fn count_type(&self, flag: u8) -> u32 {
        self.iter().filter(|&(c, _)| crate::cards::is(c, flag)).map(|(_, n)| n as u32).sum()
    }
    pub fn any_type(&self, flag: u8) -> bool {
        self.iter().any(|(c, _)| crate::cards::is(c, flag))
    }
    /// Iterate (card, count) for nonzero counts, in card-id order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (CardId, u8)> + '_ {
        self.0.iter().enumerate().filter(|(_, &n)| n > 0).map(|(i, &n)| (i as CardId, n))
    }
    /// Each count raised to at least `floor(card)`.
    pub fn max_with(mut self, floor: impl Fn(CardId) -> u8) -> Counts {
        for i in 0..NUM_CARDS {
            self.0[i] = self.0[i].max(floor(i as CardId));
        }
        self
    }

    /// The card at position `idx` (0-based) when the multiset is laid out in id order.
    /// Used to sample uniformly: `nth(rng.below(total))`.
    #[inline]
    pub fn nth(&self, mut idx: u32) -> CardId {
        for (i, &n) in self.0.iter().enumerate() {
            if idx < n as u32 {
                return i as CardId;
            }
            idx -= n as u32;
        }
        panic!("Counts::nth out of range")
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
