use crate::format::AgeEntry;

/// xoshiro128++ — fast, deterministic, pass BigCrush
pub struct TilePrng {
    state: [u32; 4],
}

fn splitmix32(x: &mut u64) -> u32 {
    *x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) as u32
}

impl TilePrng {
    /// Seed from tile's `noise_seed` XOR tile coords, with splitmix expansion
    pub fn from_tile(age: &AgeEntry) -> Self {
        let s0 = u32::from_le_bytes(age.noise_seed);
        let s1 = (age.tx as u32) | ((age.ty as u32) << 16);
        let seed = (s0 ^ s1) as u64 | ((s0 as u64).wrapping_add(1) << 32);

        let mut mix = seed;
        let a = splitmix32(&mut mix) as u64;
        let b = splitmix32(&mut mix) as u64;
        let c = splitmix32(&mut mix) as u64;
        let d = splitmix32(&mut mix) as u64;

        TilePrng {
            state: [
                s0 ^ (a as u32),
                s1 ^ (b as u32),
                (c as u32).wrapping_add(s0),
                (d as u32).wrapping_add(s1),
            ],
        }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .rotate_left(7)
            .wrapping_add(self.state[0]);
        let t = self.state[1] << 9;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(11);
        result
    }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        // Upper 24 bits → mantissa of f32, range [0, 1)
        (self.next_u32() >> 8) as f32 * (1.0 / (1u32 << 24) as f32)
    }
}
