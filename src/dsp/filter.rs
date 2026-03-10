use wide::f32x8;

/// A SIMD-optimized 12dB/oct State Variable Filter (SVF).
/// Provides LP, BP, and HP outputs simultaneously.
#[derive(Clone, Copy)]
pub struct SimdSvf {
    pub ic1eq: f32x8,
    pub ic2eq: f32x8,
}

impl SimdSvf {
    pub fn new() -> Self {
        Self {
            ic1eq: f32x8::splat(0.0),
            ic2eq: f32x8::splat(0.0),
        }
    }

    /// Reset the filter state.
    pub fn reset(&mut self) {
        self.ic1eq = f32x8::splat(0.0);
        self.ic2eq = f32x8::splat(0.0);
    }

    /// Process one sample for 8 voices.
    /// cutoff: normalized cutoff frequency [0.0, 1.0] (will be mapped to Hz)
    /// res: resonance [0.0, 1.0] (maps to Q)
    /// sample_rate: the current sample rate
    pub fn process(&mut self, input: f32x8, cutoff_hz: f32x8, res: f32x8, sample_rate: f32) -> (f32x8, f32x8, f32x8) {
        let pi = std::f32::consts::PI;
        
        // g = tan(pi * cutoff / fs), using linear approximation (valid for cutoff << fs/2)
        let g = f32x8::splat(pi) * cutoff_hz / f32x8::splat(sample_rate);
        
        // k = 2.0 - 2.0 * res (where res 1.0 is high resonance)
        // Let's map res to Q or damping.
        let k = f32x8::splat(2.0) - f32x8::splat(2.0) * res.min(f32x8::splat(0.99));
        
        let a1 = f32x8::splat(1.0) / (f32x8::splat(1.0) + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a3 * self.ic1eq + a2 * v3;

        self.ic1eq = f32x8::splat(2.0) * v1 - self.ic1eq;
        self.ic2eq = f32x8::splat(2.0) * v2 - self.ic2eq;

        // hp = input - k*v1 - v2
        let hp = input - k * v1 - v2;
        let bp = v1;
        let lp = v2;

        (lp, bp, hp)
    }
}
