/// xoshiro256++ — small, fast, `Copy`, so it lives inside `GameState` and games replay exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rng {
    s: [u64; 4],
}

#[inline]
fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        Rng { s: [splitmix(&mut x), splitmix(&mut x), splitmix(&mut x), splitmix(&mut x)] }
    }

    /// Derive an independent stream, e.g. `Rng::derive(base_seed, game_index)`.
    pub fn derive(seed: u64, stream: u64) -> Self {
        let mut x = seed ^ stream.wrapping_mul(0xD605_BBB5_8C8A_BBFD);
        splitmix(&mut x);
        Rng::new(x)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform in `0..n` (n > 0). Lemire multiply-shift; bias is negligible for our n.
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        (((self.next_u64() >> 32) * n as u64) >> 32) as u32
    }

    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64) < p
    }
}
