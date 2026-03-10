use wide::f32x8;

#[derive(Clone, Copy, Debug)]
pub struct SimdState {
    pub x: f32x8,
    pub y: f32x8,
    pub z: f32x8,
}

impl SimdState {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x: f32x8::splat(x),
            y: f32x8::splat(y),
            z: f32x8::splat(z),
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

pub struct LorenzParams {
    pub sigma: f32x8,
    pub rho: f32x8,
    pub beta: f32x8,
}

pub struct RosslerParams {
    pub a: f32x8,
    pub b: f32x8,
    pub c: f32x8,
}

pub struct ChuaParams {
    pub alpha: f32x8,
    pub beta: f32x8,
    pub m0: f32x8,
    pub m1: f32x8,
}

#[inline(always)]
pub fn lorenz_derivative(state: &SimdState, params: &LorenzParams) -> SimdState {
    let dx = params.sigma * (state.y - state.x);
    let dy = state.x * (params.rho - state.z) - state.y;
    let dz = state.x * state.y - params.beta * state.z;

    SimdState { x: dx, y: dy, z: dz }
}

#[inline(always)]
pub fn rossler_derivative(state: &SimdState, params: &RosslerParams) -> SimdState {
    let dx = -state.y - state.z;
    let dy = state.x + params.a * state.y;
    let dz = params.b + state.z * (state.x - params.c);

    SimdState { x: dx, y: dy, z: dz }
}

#[inline(always)]
pub fn chua_derivative(state: &SimdState, params: &ChuaParams) -> SimdState {
    //  h(x) = m1 * x + 0.5 * (m0 - m1) * (|x + 1| - |x - 1|)
    let one = f32x8::splat(1.0);
    let abs_plus_1 = (state.x + one).abs();
    let abs_min_1 = (state.x - one).abs();
    let hx = params.m1 * state.x + f32x8::splat(0.5) * (params.m0 - params.m1) * (abs_plus_1 - abs_min_1);

    let dx = params.alpha * (state.y - state.x - hx);
    let dy = state.x - state.y + state.z;
    let dz = -params.beta * state.y;

    SimdState { x: dx, y: dy, z: dz }
}
