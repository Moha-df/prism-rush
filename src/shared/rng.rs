//! Deterministic random number generation.
//!
//! The whole game must produce *exactly* the same world on every machine from
//! the same seed, so we never use a thread-local or OS-seeded RNG for anything
//! that matters. Instead we use tiny, well-known integer hash functions
//! (SplitMix64) that are fully deterministic and platform independent.

/// SplitMix64 finalizer: turns any u64 into a well-mixed u64.
/// `const fn` means it can even be evaluated at compile time.
pub const fn hash64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Hashes a seed together with two integer coordinates (e.g. a chunk position).
pub fn hash_2d(seed: u64, x: i32, y: i32) -> u64 {
    // `as u32 as u64` keeps the bit pattern of negative numbers stable.
    let a = hash64(seed ^ (x as u32 as u64));
    hash64(a ^ ((y as u32 as u64) << 1).rotate_left(17))
}

/// Combines several values into one hash (order matters).
pub fn hash_combine(values: &[u64]) -> u64 {
    values
        .iter()
        .fold(0xA076_1D64_78BD_642F, |acc, v| hash64(acc ^ v))
}

/// A small deterministic random generator (SplitMix64 stream).
///
/// `#[derive(Clone)]` lets us copy the generator to "replay" a sequence.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        hash64(self.state)
    }

    /// Uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // Use the top 24 bits: exactly representable in an f32 mantissa.
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform float in `[min, max)`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// Uniform integer in `[min, max)` (max exclusive). Panics if `min >= max`.
    pub fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        assert!(min < max, "empty range");
        min + (self.next_u64() % u64::from(max - min)) as u32
    }

    /// Returns `true` with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// Picks a random element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range_u32(0, items.len() as u32) as usize]
    }

    /// Picks an index according to the given weights (all >= 0, sum > 0).
    pub fn weighted_index(&mut self, weights: &[f32]) -> usize {
        let total: f32 = weights.iter().sum();
        let mut roll = self.next_f32() * total;
        for (i, w) in weights.iter().enumerate() {
            if roll < *w {
                return i;
            }
            roll -= w;
        }
        weights.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn floats_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
            let g = r.range(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&g));
        }
    }

    #[test]
    fn hash_2d_differs_by_coordinate() {
        assert_ne!(hash_2d(1, 0, 1), hash_2d(1, 1, 0));
        assert_ne!(hash_2d(1, -1, 0), hash_2d(1, 1, 0));
        assert_eq!(hash_2d(9, 3, -4), hash_2d(9, 3, -4));
    }

    #[test]
    fn weighted_index_respects_zero_weights() {
        let mut r = Rng::new(3);
        for _ in 0..1000 {
            let i = r.weighted_index(&[0.0, 1.0, 0.0]);
            assert_eq!(i, 1);
        }
    }
}
