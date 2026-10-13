# ChorusVox DecaBuddy 🎙️✨

A real-time polyphonic vocal harmony generator written in **Rust**, faithful to the reverse-engineered DSP architecture of the vintage **Akai HV10 / DecaBuddy** (v1.02).

Built with the `truce` framework, delivering native **VST3**, **CLAP**, and **AUv2** (Audio Units) builds for macOS and Linux.

---

## ⚡ How Does the DSP Engine Work?

Standard pitch shifters resample or stretch audio linearly, introducing artificial formant shifts (the "chipmunk" or "monster" effect). **ChorusVox** uses a **TD-PSOLA** (*Time-Domain Pitch-Synchronous Overlap-Add*) pipeline:

1. **Pitch Tracking (YIN Engine)**: Continuously tracks the fundamental frequency ($f_0$) of the incoming dry voice.
2. **Epoch / Glottal Pulse Alignment**: Marks the precise peak excitation instances of vocal fold closures.
3. **Grain Splatting & Overlap-Add**: Hann-windowed pitch periods are extracted around each epoch mark and rearranged along the target pitch timeline ($T_s$), preserving the throat and vocal tract acoustic characteristics.
4. **Normalized Overlap-Add (NOLA)**: Re-normalizes overlapping grains dynamically by $\sum w[n]$, preventing amplitude modulation hum (*buzz*) in pitch-down scenarios.

---

## 🎛️ Parameter Guide

### 1. Global & Input
* **Key**: Sets the master musical key (C, C#, D, ..., B). Intelligent scale intervals automatically map to this reference.
* **MIDI Mode**:
  * `Auto Ch`: Play chords from a single MIDI track; voices are automatically distributed from lowest to highest note across active harmony channels.
  * `Mono (1-4)`: Voice channels respond strictly to independent MIDI channels (Channel 1 $\rightarrow$ H1, Channel 2 $\rightarrow$ H2, etc.).
  * `Wait All`: Harmonizer mutes until all keys corresponding to active voices are pressed simultaneously.
  * `Priority`: Allocates voice channels based on key-press timestamp order.
  * `FlexiChd`: Analyzes a sustained MIDI chord and automatically shifts harmony intervals to the nearest valid chord tones as your sung pitch moves.
* **MIDI Ch**: Base MIDI reception channel (1 to 16).
* **Tune Hz**: Master reference tuning frequency (default: 440.0 Hz). Supports alternate tunings (e.g., 432 Hz).
* **In Gain (dB)**: Pre-detector input audio gain adjustment (-15 dB to +15 dB).
* **In / Out Meters**: Peak meters displaying input headroom and stereo output levels.

---

### 2. Direct & Unison
* **Direct**:
  * `Direct (Toggle)`: Enables or mutes the original dry vocal track.
  * `Dir Lev (dB)`: Dry vocal gain level (-24 dB to 0 dB).
  * `Dir Pan`: Stereo placement for the dry vocal (-50 Left to +50 Right).
* **Link**: Mirrors `Random`, `Formant`, and `Quantise` values from Harmony 1 across Harmonias 2, 3, and 4.
* **Unison**: Generates a natural vocal double tracking layer:
  * `U.Rand`: Applies humanizing micro-pitch drift and micro-delay timing variations.
  * `U.Form`: Formant shift applied strictly to the unison voice.
  * `U.Quant`: Pitch-quantization intensity for the unison layer.
  * `U.Level (dB)` / `U.Pan`: Volume and stereo panning.

---

### 3. Harmony Channels (H1 to H4)

Each of the four harmony layers includes:

* **On**: Engages or bypasses the specific harmony voice.
* **Dual**: Adds a second detuned, asymmetric sub-voice with stereo offset, expanding single-voice harmonies into a dense choir texture.
* **Scale**: Defines pitch selection behavior:
  * `MIDI`: Target pitch is driven directly by incoming MIDI note events.
  * `User`: Reads pitch offsets from user-definable custom lookup tables.
  * `Low Octave / High Octave`: Fixed octave transposition (-12 or +12 semitones).
  * `Unison`: Tracks incoming pitch identically.
  * `3rd / 5th / 7th (Modal & Minor Variations)`: Musical intervals that automatically shift between major and minor steps according to the selected `Key` and the sung pitch.
* **Random (0 to 100)**: Humanization engine. Injects natural pitch drift (in cents) and temporal timing jitter (in ms) driven by an Ornstein-Uhlenbeck stochastic process, preventing sterile, static harmonies.
* **Formant (-50 to +50)**: Modifies apparent vocal tract length:
  * Negative values (-): Simulates a longer vocal tract and wider throat (darker, masculine timbre).
  * Positive values (+): Simulates a shorter vocal tract (brighter, lighter, feminine timbre).
  * `0.00`: Preserves original vocal throat characteristics without color alteration.
* **Quantise (0 to 100)**: Pitch stabilization:
  * `0.00`: Voice preserves all human inflections, vibratos, and micro-glides.
  * `100.00`: Hard-quantizes output pitch to the mathematical center of the target chromatic note.
* **Level (dB)**: Channel output level (-24 dB to 0 dB).
* **Pan**: Stereo panning placement (-50 Left to +50 Right).

---

## 🛠️ Build & Installation

### Requirements
* [Rust toolchain](https://rustup.rs/) (latest stable release)
* Cargo `truce` CLI runner:
  ```bash
  cargo install truce-cli
