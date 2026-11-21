//! Deterministic FNV-1a / SplitMix64 state & replay hashing for Proof-of-Flight verification.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetHasher {
    state: u64,
}

impl Default for DetHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl DetHasher {
    pub const fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }

    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            self.state ^= bytes[i] as u64;
            self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
            i += 1;
        }
    }

    #[inline]
    pub fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    pub fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Canonicalizes -0.0 to +0.0 before hashing IEEE-754 bits.
    #[inline]
    pub fn write_f64(&mut self, v: f64) {
        let bits = if v == 0.0 { 0u64 } else { v.to_bits() };
        self.write_u64(bits);
    }

    #[inline]
    pub fn finish(&self) -> u64 {
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}
