//! Per-channel vested-leaky integrators — 32 independent channels of vest+decay
//! coupling. Each channel maintains a vesting accumulator, a decay state, and a cap.
//! The `compose` operation yields the per-channel maximum, ensuring vest floor monotonicity.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Per-channel vested-leaky integrators: 32 independent channels.
pub struct VestedLeaky32 {
    /// Decay accumulator for each of 32 channels.
    pub decay: [u64; 32],
    /// Vest accumulator for each of 32 channels.
    pub vest: [u64; 32],
    /// Per-channel ceiling, clamping the composed output.
    pub caps: [u64; 32],
}

impl VestedLeaky32 {
    /// Create a new vested-leaky with zero decay/vest, uncapped.
    pub const fn new() -> Self {
        Self {
            decay: [0u64; 32],
            vest: [0u64; 32],
            caps: [u64::MAX; 32],
        }
    }

    /// Per-channel floor: max(vest[i], decay[i]), clamped to caps[i].
    /// Vest never decreases; the standing output is always >= the decay state.
    pub fn compose(&mut self) -> [u64; 32] {
        let mut out = [0u64; 32];
        let mut i = 0;
        while i < 32 {
            let raw_max = if self.vest[i] >= self.decay[i] {
                self.vest[i]
            } else {
                self.decay[i]
            };
            out[i] = if raw_max > self.caps[i] {
                self.caps[i]
            } else {
                raw_max
            };
            i += 1;
        }
        out
    }
}

const _: () = assert!(core::mem::size_of::<VestedLeaky32>() == 768);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vested_floor_monotonic() {
        let mut vl = VestedLeaky32::new();
        vl.vest[0] = 5_000;
        vl.decay[0] = 3_000;
        vl.caps[0] = 10_000;

        let out1 = vl.compose();
        assert_eq!(out1[0], 5_000, "max(vest, decay) should be 5000");

        vl.vest[0] = 6_000;
        vl.decay[0] = 2_000;

        let out2 = vl.compose();
        assert_eq!(out2[0], 6_000, "vest increased, output should be 6000");
        assert!(out2[0] >= out1[0], "vest never decreases across compose calls");

        vl.decay[0] = 8_000;
        vl.vest[0] = 6_000;

        let out3 = vl.compose();
        assert_eq!(out3[0], 8_000, "decay exceeds vest, take decay");
        assert!(out3[0] >= vl.decay[0], "standing >= decay state");

        vl.vest[1] = 15_000;
        vl.caps[1] = 12_000;

        let out4 = vl.compose();
        assert_eq!(out4[1], 12_000, "capped at max(vest, decay)");
    }
}
