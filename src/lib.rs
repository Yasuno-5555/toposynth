pub mod dsp;
pub mod editor;

use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;
use wide::{f32x8, CmpEq};

const MAX_VOICES: usize = 16;

#[derive(Clone, Copy)]
struct Voice {
    state: (f32, f32, f32),
    phase: f32,
    freq: f32,
    envelope: f32,
    filter_z: [f32; 4],
    filters: [crate::dsp::filter::SimdSvf; 1], // Placeholder for scalar storage if needed, but we'll use SIMD in process
    active: bool,
    note: u8,
}

pub(crate) const TRAJECTORY_SIZE: usize = 512;
pub(crate) struct Trajectory {
    pub(crate) points: [(f32, f32, f32); TRAJECTORY_SIZE],
    pub(crate) write_pos: usize,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            state: (0.1, 0.0, 0.0), // Initialize with small non-zero to jumpstart chaos
            phase: 0.0,
            freq: 0.0,
            envelope: 0.0,
            filter_z: [0.0; 4],
            filters: [crate::dsp::filter::SimdSvf::new(); 1],
            active: false,
            note: 0,
        }
    }
}

pub struct Toposynth {
    params: Arc<ToposynthParams>,
    voices: [Voice; MAX_VOICES],
    sample_rate: f32,
    trajectory: Arc<std::sync::RwLock<Trajectory>>,
    trajectory_counter: usize,
}

#[derive(Params)]
pub struct ToposynthParams {
    /// The editor's state, used for persisting size and position.
    #[persist = "editor_state"]
    pub editor_state: Arc<ViziaState>,

    // Performance Layer
    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "sustain"]
    pub sustain: FloatParam,
    #[id = "release"]
    pub release: FloatParam,

    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "resonance"]
    pub resonance: FloatParam,

    #[id = "chaos_to_cutoff"]
    pub chaos_to_cutoff: FloatParam,
    #[id = "chaos_to_fm"]
    pub chaos_to_fm: FloatParam,

    // Sound Design Layer
    #[id = "morph"]
    pub morph: FloatParam,
    #[id = "drift"]
    pub drift: FloatParam,
    #[id = "chaos_depth"]
    pub chaos_depth: FloatParam,
    #[id = "interaction"]
    pub interaction: FloatParam,

    #[id = "shaper_mode"]
    pub shaper_mode: IntParam,

    // Physics Layer
    #[id = "sigma"]
    pub sigma: FloatParam,
    #[id = "rho"]
    pub rho: FloatParam,
    #[id = "beta"]
    pub beta: FloatParam,
    #[id = "substeps"]
    pub substeps: IntParam,

    // Macro Knobs
    #[id = "macro_organic"]
    pub macro_organic: FloatParam,
    #[id = "macro_metal"]
    pub macro_metal: FloatParam,
    #[id = "macro_drift"]
    pub macro_drift: FloatParam,
    #[id = "macro_unstable"]
    pub macro_unstable: FloatParam,
}

impl Default for Toposynth {
    fn default() -> Self {
        Self {
            params: Arc::new(ToposynthParams::default()),
            voices: [Voice::default(); MAX_VOICES],
            sample_rate: 44100.0,
            trajectory: Arc::new(std::sync::RwLock::new(Trajectory {
                points: [(0.0, 0.0, 0.0); TRAJECTORY_SIZE],
                write_pos: 0,
            })),
            trajectory_counter: 0,
        }
    }
}

impl Default for ToposynthParams {
    fn default() -> Self {
        Self {
            editor_state: ViziaState::new(|| (800, 500)),
            gain: FloatParam::new("Gain", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Logarithmic(50.0)),

            morph: FloatParam::new("Morph", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            drift: FloatParam::new("Drift", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            chaos_depth: FloatParam::new("Chaos Depth", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            interaction: FloatParam::new("Interaction", 0.0, FloatRange::Linear { min: 0.0, max: 0.5 })
                .with_smoother(SmoothingStyle::Linear(50.0)),

            sigma: FloatParam::new("Sigma", 10.0, FloatRange::Linear { min: 0.0, max: 50.0 }),
            rho: FloatParam::new("Rho", 28.0, FloatRange::Linear { min: 0.0, max: 100.0 }),
            beta: FloatParam::new("Beta", 8.0 / 3.0, FloatRange::Linear { min: 0.0, max: 10.0 }),

            substeps: IntParam::new("Substeps", 4, IntRange::Linear { min: 1, max: 16 }),

            attack: FloatParam::new("Attack", 0.1, FloatRange::Skewed { min: 0.001, max: 10.0, factor: 0.2 }),
            decay: FloatParam::new("Decay", 0.1, FloatRange::Skewed { min: 0.001, max: 10.0, factor: 0.2 }),
            sustain: FloatParam::new("Sustain", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            release: FloatParam::new("Release", 0.2, FloatRange::Skewed { min: 0.001, max: 10.0, factor: 0.2 }),

            cutoff: FloatParam::new("Cutoff", 1000.0, FloatRange::Skewed { min: 20.0, max: 20000.0, factor: 0.2 })
                .with_unit(" Hz"),
            resonance: FloatParam::new("Resonance", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(50.0)),
            chaos_to_cutoff: FloatParam::new("Chaos > Cutoff", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }),
            chaos_to_fm: FloatParam::new("Chaos > FM", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }),

            shaper_mode: IntParam::new("Shaper Mode", 0, IntRange::Linear { min: 0, max: 3 }),

            macro_organic: FloatParam::new("Macro Organic", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            macro_metal: FloatParam::new("Macro Metal", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            macro_drift: FloatParam::new("Macro Drift", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            macro_unstable: FloatParam::new("Macro Unstable", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
        }
    }
}

impl Plugin for Toposynth {
    const NAME: &'static str = "Toposynth";
    const VENDOR: &'static str = "Antigravity";
    const URL: &'static str = "https://example.com";
    const EMAIL: &'static str = "info@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create_editor(self.params.clone(), self.trajectory.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn { note, .. } => {
                    // Find free voice
                    if let Some(voice) = self.voices.iter_mut().find(|v| !v.active) {
                        voice.active = true;
                        voice.note = note;
                        voice.freq = util::midi_note_to_freq(note);
                        voice.phase = 0.0;
                    }
                }
                NoteEvent::NoteOff { note, .. } => {
                    if let Some(voice) = self.voices.iter_mut().find(|v| v.active && v.note == note) {
                        voice.active = false;
                    }
                }
                _ => (),
            }
        }

        let _num_samples = buffer.samples();
        let sample_rate = self.sample_rate;
        let dt = 1.0 / sample_rate;

        let shaper_mode = self.params.shaper_mode.value();
        let macro_organic = self.params.macro_organic.value();
        let macro_metal = self.params.macro_metal.value();
        let macro_drift = self.params.macro_drift.value();
        let macro_unstable = self.params.macro_unstable.value();

        // Macro mapping
        let drift_mod = (self.params.drift.value() + macro_organic * 0.5).min(1.0);
        let sigma_val = self.params.sigma.value() * (1.0 + macro_unstable * 2.0);
        let rho_val = self.params.rho.value() * (1.0 + macro_unstable * 1.5);

        // Prep SIMD parameters
        let sigma = f32x8::splat(sigma_val);
        let rho = f32x8::splat(rho_val);
        let beta = f32x8::splat(self.params.beta.value());
        
        let lorenz_p = dsp::attractor::LorenzParams { sigma, rho, beta };
        
        // Add drift to Rossler/Chua for "instability"
        let drift_f = f32x8::splat(drift_mod + macro_drift);
        let rossler_p = dsp::attractor::RosslerParams { 
            a: f32x8::splat(0.2) + drift_f * 0.1, 
            b: f32x8::splat(0.2), 
            c: f32x8::splat(5.7) 
        };
        let chua_p = dsp::attractor::ChuaParams {
            alpha: f32x8::splat(15.6) * (1.0 + macro_metal), 
            beta: f32x8::splat(28.0), 
            m0: f32x8::splat(-1.143), 
            m1: f32x8::splat(-0.714)
        };

        let interaction_val = self.params.interaction.value();
        
        // Calculate global mean X for coupling
        let mut active_count = 0.001; // Avoid div by zero
        let mut sum_x = 0.0;
        for v in self.voices.iter() {
            if v.active {
                sum_x += v.state.0;
                active_count += 1.0;
            }
        }
        let mean_x = f32x8::splat(sum_x / active_count * interaction_val);
        
        // Prepare SIMD constants
        let dt_simd = f32x8::splat(dt as f32);

        // Load voice states into SIMD (two batches of 8)
        let mut block1_state = dsp::attractor::SimdState {
            x: f32x8::from([
                self.voices[0].state.0, self.voices[1].state.0, self.voices[2].state.0, self.voices[3].state.0,
                self.voices[4].state.0, self.voices[5].state.0, self.voices[6].state.0, self.voices[7].state.0,
            ]),
            y: f32x8::from([
                self.voices[0].state.1, self.voices[1].state.1, self.voices[2].state.1, self.voices[3].state.1,
                self.voices[4].state.1, self.voices[5].state.1, self.voices[6].state.1, self.voices[7].state.1,
            ]),
            z: f32x8::from([
                self.voices[0].state.2, self.voices[1].state.2, self.voices[2].state.2, self.voices[3].state.2,
                self.voices[4].state.2, self.voices[5].state.2, self.voices[6].state.2, self.voices[7].state.2,
            ]),
        };
        let mut block2_state = dsp::attractor::SimdState {
            x: f32x8::from([
                self.voices[8].state.0, self.voices[9].state.0, self.voices[10].state.0, self.voices[11].state.0,
                self.voices[12].state.0, self.voices[13].state.0, self.voices[14].state.0, self.voices[15].state.0,
            ]),
            y: f32x8::from([
                self.voices[8].state.1, self.voices[9].state.1, self.voices[10].state.1, self.voices[11].state.1,
                self.voices[12].state.1, self.voices[13].state.1, self.voices[14].state.1, self.voices[15].state.1,
            ]),
            z: f32x8::from([
                self.voices[8].state.2, self.voices[9].state.2, self.voices[10].state.2, self.voices[11].state.2,
                self.voices[12].state.2, self.voices[13].state.2, self.voices[14].state.2, self.voices[15].state.2,
            ]),
        };

        let mut block1_phase = f32x8::from([
            self.voices[0].phase, self.voices[1].phase, self.voices[2].phase, self.voices[3].phase,
            self.voices[4].phase, self.voices[5].phase, self.voices[6].phase, self.voices[7].phase,
        ]);
        let mut block2_phase = f32x8::from([
            self.voices[8].phase, self.voices[9].phase, self.voices[10].phase, self.voices[11].phase,
            self.voices[12].phase, self.voices[13].phase, self.voices[14].phase, self.voices[15].phase,
        ]);

        let block1_freq = f32x8::from([
            self.voices[0].freq, self.voices[1].freq, self.voices[2].freq, self.voices[3].freq,
            self.voices[4].freq, self.voices[5].freq, self.voices[6].freq, self.voices[7].freq,
        ]);
        let block2_freq = f32x8::from([
            self.voices[8].freq, self.voices[9].freq, self.voices[10].freq, self.voices[11].freq,
            self.voices[12].freq, self.voices[13].freq, self.voices[14].freq, self.voices[15].freq,
        ]);

        let block1_mask = f32x8::from([
            if self.voices[0].active { 1.0 } else { 0.0 }, if self.voices[1].active { 1.0 } else { 0.0 },
            if self.voices[2].active { 1.0 } else { 0.0 }, if self.voices[3].active { 1.0 } else { 0.0 },
            if self.voices[4].active { 1.0 } else { 0.0 }, if self.voices[5].active { 1.0 } else { 0.0 },
            if self.voices[6].active { 1.0 } else { 0.0 }, if self.voices[7].active { 1.0 } else { 0.0 },
        ]);
        let block2_mask = f32x8::from([
            if self.voices[8].active { 1.0 } else { 0.0 }, if self.voices[9].active { 1.0 } else { 0.0 },
            if self.voices[10].active { 1.0 } else { 0.0 }, if self.voices[11].active { 1.0 } else { 0.0 },
            if self.voices[12].active { 1.0 } else { 0.0 }, if self.voices[13].active { 1.0 } else { 0.0 },
            if self.voices[14].active { 1.0 } else { 0.0 }, if self.voices[15].active { 1.0 } else { 0.0 },
        ]);

        let attack = self.params.attack.value();
        let release = self.params.release.value();

        for channel_samples in buffer.iter_samples() {
            let gain_val = self.params.gain.smoothed.next();
            let chaos_depth_val = self.params.chaos_depth.smoothed.next();
            let chaos_depth = f32x8::splat((chaos_depth_val + macro_metal * 0.5).min(1.0));
            let morph_val = self.params.morph.smoothed.next();
            let morph = f32x8::splat(morph_val);
            let cutoff = self.params.cutoff.smoothed.next();
            let resonance = self.params.resonance.smoothed.next();

            // Process ADSR
            for i in 0..8 {
                let v = &mut self.voices[i];
                if v.active { v.envelope = (v.envelope + dt / attack).min(1.0); }
                else { v.envelope = (v.envelope - dt / release).max(0.0); }
            }
            for i in 8..16 {
                let v = &mut self.voices[i];
                if v.active { v.envelope = (v.envelope + dt / attack).min(1.0); }
                else { v.envelope = (v.envelope - dt / release).max(0.0); }
            }

            let block1_env = f32x8::from([
                self.voices[0].envelope, self.voices[1].envelope, self.voices[2].envelope, self.voices[3].envelope,
                self.voices[4].envelope, self.voices[5].envelope, self.voices[6].envelope, self.voices[7].envelope,
            ]);
            let block2_env = f32x8::from([
                self.voices[8].envelope, self.voices[9].envelope, self.voices[10].envelope, self.voices[11].envelope,
                self.voices[12].envelope, self.voices[13].envelope, self.voices[14].envelope, self.voices[15].envelope,
            ]);

            // RK4 substeps
            let substeps = self.params.substeps.value().max(1) as usize;
            let dt_substep = dt_simd / f32x8::splat(substeps as f32);

            for _ in 0..substeps {
                block1_state.x += (mean_x - block1_state.x) * dt_substep * f32x8::splat(interaction_val);
                block2_state.x += (mean_x - block2_state.x) * dt_substep * f32x8::splat(interaction_val);

                let next1 = dsp::engine::rk4_step(&block1_state, &lorenz_p, &rossler_p, &chua_p, morph, dt_substep);
                let next2 = dsp::engine::rk4_step(&block2_state, &lorenz_p, &rossler_p, &chua_p, morph, dt_substep);
                
                let safe = |v: f32x8| {
                    let zero = f32x8::splat(0.0);
                    let clean = v.cmp_eq(v).blend(v, zero); // NaN guard
                    clean.max(f32x8::splat(-100.0)).min(f32x8::splat(100.0))
                };

                block1_state.x += (safe(next1.x) - block1_state.x) * block1_mask;
                block1_state.y += (safe(next1.y) - block1_state.y) * block1_mask;
                block1_state.z += (safe(next1.z) - block1_state.z) * block1_mask;

                block2_state.x += (safe(next2.x) - block2_state.x) * block2_mask;
                block2_state.y += (safe(next2.y) - block2_state.y) * block2_mask;
                block2_state.z += (safe(next2.z) - block2_state.z) * block2_mask;
            }

            // Load SVF states for block 1
            let mut ic1_1 = f32x8::from([
                self.voices[0].filters[0].ic1eq.as_array_ref()[0], self.voices[1].filters[0].ic1eq.as_array_ref()[0],
                self.voices[2].filters[0].ic1eq.as_array_ref()[0], self.voices[3].filters[0].ic1eq.as_array_ref()[0],
                self.voices[4].filters[0].ic1eq.as_array_ref()[0], self.voices[5].filters[0].ic1eq.as_array_ref()[0],
                self.voices[6].filters[0].ic1eq.as_array_ref()[0], self.voices[7].filters[0].ic1eq.as_array_ref()[0],
            ]);
            let mut ic2_1 = f32x8::from([
                self.voices[0].filters[0].ic2eq.as_array_ref()[0], self.voices[1].filters[0].ic2eq.as_array_ref()[0],
                self.voices[2].filters[0].ic2eq.as_array_ref()[0], self.voices[3].filters[0].ic2eq.as_array_ref()[0],
                self.voices[4].filters[0].ic2eq.as_array_ref()[0], self.voices[5].filters[0].ic2eq.as_array_ref()[0],
                self.voices[6].filters[0].ic2eq.as_array_ref()[0], self.voices[7].filters[0].ic2eq.as_array_ref()[0],
            ]);

            // Modulation Matrix logic
            let mod_cutoff = self.params.chaos_to_cutoff.smoothed.next();
            let mod_fm = self.params.chaos_to_fm.smoothed.next();
            
            let norm_x1 = dsp::engine::fast_tanh(block1_state.x * f32x8::splat(0.1));
            let norm_y1 = dsp::engine::fast_tanh(block1_state.y * f32x8::splat(0.1));

            let cutoff_hz_1 = (f32x8::splat(cutoff) * (norm_x1 * f32x8::splat(mod_cutoff) * f32x8::splat(5.0) * f32x8::splat(std::f32::consts::LN_2)).exp()).max(f32x8::splat(20.0)).min(f32x8::splat(20000.0));
            let fm_mod_1 = norm_y1 * f32x8::splat(mod_fm) * f32x8::splat(2000.0);

            let audio1 = dsp::engine::process_chaos_to_audio(
                block1_state.x,
                &mut block1_phase,
                block1_freq + fm_mod_1,
                dt_simd,
                chaos_depth,
                cutoff_hz_1,
                f32x8::splat(resonance),
                sample_rate as f32,
                &mut ic1_1,
                &mut ic2_1,
            ) * block1_env * block1_mask;

            // Load SVF states for block 2
            let mut ic1_2 = f32x8::from([
                self.voices[8].filters[0].ic1eq.as_array_ref()[0], self.voices[9].filters[0].ic1eq.as_array_ref()[0],
                self.voices[10].filters[0].ic1eq.as_array_ref()[0], self.voices[11].filters[0].ic1eq.as_array_ref()[0],
                self.voices[12].filters[0].ic1eq.as_array_ref()[0], self.voices[13].filters[0].ic1eq.as_array_ref()[0],
                self.voices[14].filters[0].ic1eq.as_array_ref()[0], self.voices[15].filters[0].ic1eq.as_array_ref()[0],
            ]);
            let mut ic2_2 = f32x8::from([
                self.voices[8].filters[0].ic2eq.as_array_ref()[0], self.voices[9].filters[0].ic2eq.as_array_ref()[0],
                self.voices[10].filters[0].ic2eq.as_array_ref()[0], self.voices[11].filters[0].ic2eq.as_array_ref()[0],
                self.voices[12].filters[0].ic2eq.as_array_ref()[0], self.voices[13].filters[0].ic2eq.as_array_ref()[0],
                self.voices[14].filters[0].ic2eq.as_array_ref()[0], self.voices[15].filters[0].ic2eq.as_array_ref()[0],
            ]);

            let norm_x2 = dsp::engine::fast_tanh(block2_state.x * f32x8::splat(0.1));
            let norm_y2 = dsp::engine::fast_tanh(block2_state.y * f32x8::splat(0.1));

            let cutoff_hz_2 = (f32x8::splat(cutoff) * (norm_x2 * f32x8::splat(mod_cutoff) * f32x8::splat(5.0) * f32x8::splat(std::f32::consts::LN_2)).exp()).max(f32x8::splat(20.0)).min(f32x8::splat(20000.0));
            let fm_mod_2 = norm_y2 * f32x8::splat(mod_fm) * f32x8::splat(2000.0);

            let audio2 = dsp::engine::process_chaos_to_audio(
                block2_state.x,
                &mut block2_phase,
                block2_freq + fm_mod_2,
                dt_simd,
                chaos_depth,
                cutoff_hz_2,
                f32x8::splat(resonance),
                sample_rate as f32,
                &mut ic1_2,
                &mut ic2_2,
            ) * block2_env * block2_mask;

            // Sum audio and save states
            let mut sum = 0.0f32;
            for i in 0..8 {
                sum += audio1.as_array_ref()[i] + audio2.as_array_ref()[i];

                self.voices[i].state = (block1_state.x.as_array_ref()[i], block1_state.y.as_array_ref()[i], block1_state.z.as_array_ref()[i]);
                self.voices[i].phase = block1_phase.as_array_ref()[i];
                self.voices[i].filters[0].ic1eq = f32x8::splat(ic1_1.as_array_ref()[i]);
                self.voices[i].filters[0].ic2eq = f32x8::splat(ic2_1.as_array_ref()[i]);

                self.voices[i+8].state = (block2_state.x.as_array_ref()[i], block2_state.y.as_array_ref()[i], block2_state.z.as_array_ref()[i]);
                self.voices[i+8].phase = block2_phase.as_array_ref()[i];
                self.voices[i+8].filters[0].ic1eq = f32x8::splat(ic1_2.as_array_ref()[i]);
                self.voices[i+8].filters[0].ic2eq = f32x8::splat(ic2_2.as_array_ref()[i]);
            }

            for sample in channel_samples { *sample = sum * gain_val; }
        }

        let captured_state = self.voices[0].state;
        let n_samples = buffer.samples();
        self.trajectory_counter += n_samples;
        if self.trajectory_counter >= 32 {
            self.trajectory_counter = 0;
            if let Ok(mut traj) = self.trajectory.try_write() {
                let wp = traj.write_pos;
                traj.points[wp] = captured_state;
                traj.write_pos = (wp + 1) % TRAJECTORY_SIZE;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Toposynth {
    const CLAP_ID: &'static str = "com.antigravity.toposynth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Chaotic Topology Synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for Toposynth {
    const VST3_CLASS_ID: [u8; 16] = *b"ToposyntAntigrav";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Toposynth);
nih_export_vst3!(Toposynth);
