//! 2D value noise and fractal Brownian motion (fBm).
//!
//! Value noise: we put a pseudo-random value on every integer lattice point
//! and smoothly interpolate between them. Summing several "octaves" of it at
//! growing frequencies and shrinking amplitudes gives natural-looking terrain.

use super::rng::hash_2d;

/// Random value in `[0, 1)` attached to the lattice point `(x, y)`.
fn lattice(seed: u64, x: i32, y: i32) -> f32 {
    (hash_2d(seed, x, y) >> 40) as f32 / (1u64 << 24) as f32
}

/// Quintic smoothstep: zero first *and* second derivatives at 0 and 1,
/// which avoids visible creases at lattice boundaries.
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Smooth value noise in `[0, 1]`.
pub fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let (ix, iy) = (x0 as i32, y0 as i32);
    let (tx, ty) = (fade(x - x0), fade(y - y0));

    let a = lattice(seed, ix, iy);
    let b = lattice(seed, ix + 1, iy);
    let c = lattice(seed, ix, iy + 1);
    let d = lattice(seed, ix + 1, iy + 1);
    lerp(lerp(a, b, tx), lerp(c, d, tx), ty)
}

/// Fractal noise: `octaves` layers of value noise. Result roughly in `[0, 1]`.
pub fn fbm(seed: u64, x: f32, y: f32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        // Each octave uses a different seed so they are not correlated.
        sum += amp * value_noise(seed.wrapping_add(u64::from(o) * 1013), x * freq, y * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.03; // not exactly 2 to avoid aligned artifacts
    }
    sum / norm
}

/// "Ridged" noise: sharp crests, good for mountain ranges. Result in `[0, 1]`.
pub fn ridged(seed: u64, x: f32, y: f32, octaves: u32) -> f32 {
    let n = fbm(seed, x, y, octaves);
    1.0 - (n * 2.0 - 1.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_and_deterministic() {
        for i in 0..2000 {
            let x = i as f32 * 0.37 - 300.0;
            let y = i as f32 * -0.11 + 50.0;
            let n = fbm(5, x, y, 4);
            assert!((0.0..=1.0).contains(&n), "{n}");
            assert_eq!(n, fbm(5, x, y, 4));
        }
    }

    #[test]
    fn noise_is_continuous() {
        // Tiny input change => tiny output change.
        let a = value_noise(1, 10.5, 3.25);
        let b = value_noise(1, 10.5001, 3.25);
        assert!((a - b).abs() < 0.01);
    }
}
