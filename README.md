# Toposynth

**Toposynth** is a chaotic topology synthesizer VST3 plugin built with Rust and `nih_plug`. 

Instead of traditional oscillators, Toposynth relies on a continuous integration of non-linear differential equations (Chaotic Attractors) to generate sound and modulate itself. The core engine mathematically morphs between **Lorenz**, **Rössler**, and **Chua's** strange attractors to create wildly unpredictable rhythms, screaming metal tones, and infinitely evolving drone textures.

By embracing chaos theory, Toposynth guarantees a "once-never-same" sonic evolution. No two notes will ever trigger exactly the same microscopic waveform.

## Features

- **RK4 Chaos Engine:** Real-time 4th-order Runge-Kutta numerical integration of 3 different mathematical chaotic systems (Lorenz, Rössler, Chua).
- **SIMD Audio Processing:** Up to 16 polyphonic voices processed concurrently using `wide` f32x8 SIMD vectors.
- **Dynamic Attractor Morphing:** Seamlessly blend between the topologies of the three chaotic systems on the fly.
- **Chaos Modulation Matrix:** Route the $X$ and $Y$ chaotic state variables directly to the SVG Filter Cutoff and Phase Modulation (FM). 
- **Macro Controls:**
  - `Organic`: Enhances drift and softens the chaos.
  - `Metal`: Pushes Chua's circuit parameters to their absolute extremes for harsh digital screeches and feedback.
  - `Drift`: Destabilizes the Rössler attractor bounds.
  - `Unstable`: Increases Lorenz system scale and heat.
- **Built-in Presets:** Jumpstart your sound design with strictly macro'd global presets (Init, Metal, Rhythmic).
- **Hardware-Accelerated GUI:** Custom UI built on Vizia with OpenGL/Wgpu acceleration, featuring a real-time trajectory visualizer for the chaotic states.

## Installation

### Pre-built Binaries
Download the `.vst3` file from the [Releases](#) tab and place it in your system's VST3 directory:
- **Windows:** `C:\Program Files\Common Files\VST3\`
- **macOS:** `/Library/Audio/Plug-Ins/VST3/`
- **Linux:** `~/.vst3/`

### Building from Source

**Requirements:**
- [Rust toolchain](https://rustup.rs/) (1.70+)
- A compatible C++ compiler (for `nih_plug` and GUI dependencies)

1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/toposynth.git
   cd toposynth
   ```

2. Test the build:
   ```bash
   cargo check
   ```

3. Build and bundle the VST3 plugin:
   ```bash
   # Use the xtask build system to bundle the plugin
   cd xtask
   cargo run -- bundle toposynth --release
   ```

The compiled `Toposynth.vst3` bundle will be located in `target/bundled/`.

## Under the Hood: The Physics Fix

Synthesizers require bounded audio signals `[-1.0, 1.0]`. However, chaotic attractors naturally grow to large values (e.g. Lorenz $X \approx \pm 50.0$). 

Toposynth safely manages this mathematical violence:
1. **Unbounded RK4 States**: The internal state variables ($x, y, z$) are allowed to roam freely between `±100.0`. If they are heavily restricted, the restoring force of the equations is destroyed, paralyzing the chaos. 
2. **Safe Signal Extraction**: Toposynth applies an analog-style soft-saturation (`fast_tanh`) *only* when the chaos variables are extracted for Phase Modulation or Filter Cutoff.
3. **SVF Saturation**: The 12dB/oct State Variable Filter integrators feature soft-saturation to absorb wild resonance sweeps without blowing out the audio.

## License

This project is licensed under the MIT License.

## Acknowledgements

- Built using the amazing [nih_plug](https://github.com/robbert-vdh/nih-plug) audio plugin framework by Robbert van der Helm.
- GUI powered by [Vizia](https://github.com/vizia/vizia).
