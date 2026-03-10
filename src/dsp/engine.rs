use wide::*;
use super::attractor::*;

#[inline(always)]
pub fn interpolated_derivative(
    state: &SimdState,
    lorenz_p: &LorenzParams,
    rossler_p: &RosslerParams,
    chua_p: &ChuaParams,
    morph: f32x8,
) -> SimdState {
    let dl = lorenz_derivative(state, lorenz_p);
    let dr = rossler_derivative(state, rossler_p);
    let dc = chua_derivative(state, chua_p);

    // morph: 0.0 -> Lorenz, 0.5 -> Rossler, 1.0 -> Chua
    let half = f32x8::splat(0.5);
    let two = f32x8::splat(2.0);

    let is_lower = half.cmp_gt(morph);
    
    let t_lower = morph * two;
    let t_upper = (morph - half) * two;

    let mix_x_lower = dl.x + t_lower * (dr.x - dl.x);
    let mix_y_lower = dl.y + t_lower * (dr.y - dl.y);
    let mix_z_lower = dl.z + t_lower * (dr.z - dl.z);

    let mix_x_upper = dr.x + t_upper * (dc.x - dr.x);
    let mix_y_upper = dr.y + t_upper * (dc.y - dr.y);
    let mix_z_upper = dr.z + t_upper * (dc.z - dr.z);

    SimdState {
        x: is_lower.blend(mix_x_lower, mix_x_upper),
        y: is_lower.blend(mix_y_lower, mix_y_upper),
        z: is_lower.blend(mix_z_lower, mix_z_upper),
    }
}

#[inline(always)]
fn add_scaled_state(base: &SimdState, d: &SimdState, scale: f32x8) -> SimdState {
    SimdState {
        x: base.x + d.x * scale,
        y: base.y + d.y * scale,
        z: base.z + d.z * scale,
    }
}

#[inline(always)]
pub fn rk4_step(
    state: &SimdState,
    lorenz_p: &LorenzParams,
    rossler_p: &RosslerParams,
    chua_p: &ChuaParams,
    morph: f32x8,
    dt: f32x8,
) -> SimdState {
    let half = f32x8::splat(0.5);
    let two = f32x8::splat(2.0);
    let sixth = f32x8::splat(1.0 / 6.0);

    let dt_half = dt * half;

    let k1 = interpolated_derivative(state, lorenz_p, rossler_p, chua_p, morph);
    
    let state_k2 = add_scaled_state(state, &k1, dt_half);
    let k2 = interpolated_derivative(&state_k2, lorenz_p, rossler_p, chua_p, morph);
    
    let state_k3 = add_scaled_state(state, &k2, dt_half);
    let k3 = interpolated_derivative(&state_k3, lorenz_p, rossler_p, chua_p, morph);
    
    let state_k4 = add_scaled_state(state, &k3, dt);
    let k4 = interpolated_derivative(&state_k4, lorenz_p, rossler_p, chua_p, morph);

    let dx = dt * sixth * (k1.x + two * k2.x + two * k3.x + k4.x);
    let dy = dt * sixth * (k1.y + two * k2.y + two * k3.y + k4.y);
    let dz = dt * sixth * (k1.z + two * k2.z + two * k3.z + k4.z);

    SimdState {
        x: state.x + dx,
        y: state.y + dy,
        z: state.z + dz,
    }
}
#[inline(always)]
pub fn fast_tanh(x: f32x8) -> f32x8 {
    let x2 = x * x;
    let num = x * (f32x8::splat(27.0) + x2);
    let den = f32x8::splat(27.0) + f32x8::splat(9.0) * x2;
    num / den
}

/// Absolutely bounded soft-clipper for extreme divergence protection
#[inline(always)]
pub fn fast_tanh_bounded(x: f32x8) -> f32x8 {
    let one = f32x8::splat(1.0);
    x / (one + x.abs())
}

#[inline(always)]
pub fn process_chaos_to_audio(
    chaos: f32x8,
    phase: &mut f32x8,
    freq: f32x8,
    dt: f32x8,
    depth: f32x8,
    cutoff_hz: f32x8,
    res: f32x8,
    sample_rate: f32,
    ic1eq: &mut f32x8,
    ic2eq: &mut f32x8,
) -> f32x8 {
    let two_pi = f32x8::splat(std::f32::consts::TAU);
    
    // Update phase
    *phase += freq * dt * two_pi;
    let wraps = (*phase / two_pi).floor();
    *phase -= wraps * two_pi;

    // Chaos-driven phase modulation
    let chaos_norm = fast_tanh(chaos * f32x8::splat(0.1));
    let phase_mod = *phase + chaos_norm * depth;
    let voice_signal = phase_mod.sin();

    // 12dB/oct SVF
    let pi = std::f32::consts::PI;
    let g = f32x8::splat(pi) * cutoff_hz / f32x8::splat(sample_rate);
    let k = f32x8::splat(2.0) - f32x8::splat(2.0) * res.min(f32x8::splat(0.99));
    
    let a1 = f32x8::splat(1.0) / (f32x8::splat(1.0) + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;

    let v3 = voice_signal - *ic2eq;
    let v1 = a1 * *ic1eq + a2 * v3;
    let v2 = *ic2eq + a3 * *ic1eq + a2 * v3;

    let saturate = |v: f32x8| fast_tanh(v * f32x8::splat(0.1)) * f32x8::splat(10.0);
    *ic1eq = saturate(f32x8::splat(2.0) * v1 - *ic1eq);
    *ic2eq = saturate(f32x8::splat(2.0) * v2 - *ic2eq);

    v2 // LP output
}
