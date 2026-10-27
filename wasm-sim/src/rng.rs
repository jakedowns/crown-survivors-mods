//! Deterministic PRNG.
//!
//! This is bit-for-bit compatible with the `mulberry32` generator used by the
//! browser game's seeded analysis mode, so results can be compared during the
//! migration. Unlike the legacy JS code, the core has *no* other source of
//! randomness: rendering, audio and particles must never consume this stream.

#[derive(Clone, Debug)]
pub struct Rng {
    pub state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // JS treats 0 as "unseeded". Headless runs are always seeded; map 0 to 1
        // so simulations remain reproducible.
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let s = self.state;
        let t = (s ^ (s >> 15)).wrapping_mul(1 | s);
        let t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        (t ^ (t >> 14)) & 0xFFFF_FFFF
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4_294_967_296.0
    }

    #[inline]
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next_f64()
    }

    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            ((self.next_f64() * n as f64) as usize).min(n - 1)
        }
    }

    #[inline]
    pub fn sign(&mut self) -> f64 {
        if self.chance(0.5) {
            1.0
        } else {
            -1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn mulberry32_reference_values() {
        // Reference values produced by the JS implementation used by index.html.
        let mut r = Rng::new(12345);
        let a = r.next_u32();
        let b = r.next_u32();
        let c = r.next_u32();
        assert_eq!(a, 4_207_900_869);
        assert_eq!(b, 1_317_490_944);
        assert_eq!(c, 2_079_646_450);
    }
}
