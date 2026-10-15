# ChorusVox DecaBuddy 🎙️🎛️

[![macOS Supported](https://img.shields.io/badge/platform-macOS%20(AU%20%2F%20VST3)-blue.svg)](#-macos-installation--release-assets)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Performance](https://img.shields.io/badge/performance-73x%20Real--Time%20(SIMD)-brightgreen.svg)](#-high-performance-vectorization-benchmarks)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-green.svg)](LICENSE)

**ChorusVox DecaBuddy** is a real-time vocal harmony generator and discrete component-modeled virtual analog processor written in Rust. It re-engineers, optimizes, and expands the classic **Akai Professional DecaBuddy HV10** hardware rack unit (1999) and its vintage companion (`DecaBuddy.dll` v1.02).

By combining all **37 original chromatic scale lookup tables and floating-point constants** extracted from the original Akai binary with discrete analog and fixed-point DSP modeling derived from the **Akai HV10 Service Manual (`PC MAIN BLK L8020A501A`)**, ChorusVox delivers thick, organic vocal harmonies alongside a **Zero-Latency Live Monitoring Engine**, an intelligent **Jazz Harmony Engine (`FlexiChd`)**, and a studio-grade **Natural Mode**.

---

<p align="center">
  <img src="preview.jpg" alt="ChorusVox DecaBuddy Interface" width="100%">
</p>

---

## ⚡ What's New in v0.2.13 (Hotfix)

* **Glottal Epoch Tracking Fixed:** Restored pure low-pass peak epoch tracking centered on true glottal closure instants ($T_0$). Completely eliminates the phase jitter, grain misalignment, and harsh buzzing distortion previously heard on high-pitched singing notes.
* **Pristine Catmull-Rom Hermite Resampling:** Removed flawed FIR decimation blend in PSOLA grain synthesis, ensuring 100% smooth, artifact-free interpolation across all pitch and formant shifts.
* **Stereo Phase Coherent Relay Bypass:** Refactored Takamisawa RY-9WK relay modeling into a unified stereo processor, preventing channel desynchronization and maintaining pristine audio throughout.
* **New "Natural" Mode Toggle:**
  * **Intelligent Unvoiced Sibilance Ducking:** Detects fricatives and plosives ("s", "t", "ch", "k") and attenuates harmony grain generation during unvoiced consonants. Preserves clean vocal diction without granular buzz.
  * **Organic 2nd-Order S-Curve Portamento:** Transitions notes with a critically damped human-vocal glide (~28 ms) instead of stepped jumps.
* **Dielectric Absorption (DA) Modeling:** Simulated the slow dipole relaxation memory of vintage Nichicon/Elna electrolytic coupling capacitors (< 80 Hz) with zero residual DC offset.
* **Hardware SIMD Vector Acceleration:** Explicit ARM64 NEON (`vfmaq_f32`) on Apple Silicon and AVX2 on x86_64, running at **73x faster than real-time**.

---

## 🍎 macOS Installation & Release Assets

Pre-compiled binary releases are packaged exclusively for **macOS** (Universal Apple Silicon & Intel, supporting Ableton Live, Logic Pro, Reaper, Bitwig, Studio One, Cubase, FL Studio, etc.).

1. Navigate to the **[Releases](../../releases)** page of this repository.
2. Under the latest version (**v0.2.13**), expand the **Assets** section and download **`ChorusVox-DecaBuddy-macOS-AU-VST3.zip`** *(do not download the source code zip)*.
3. Extract the archive and copy the bundles to your macOS plug-in directories:
   * **Audio Unit (AUv2):** Copy `ChorusVox DecaBuddy.component` to:
     ```bash
     ~/Library/Audio/Plug-Ins/Components/
     ```
   * **VST3:** Copy `ChorusVox DecaBuddy.vst3` to:
     ```bash
     ~/Library/Audio/Plug-Ins/VST3/
     ```
4. If required by macOS Gatekeeper security policies, clear the quarantine attributes in Terminal:
   ```bash
   xattr -cr ~/Library/Audio/Plug-Ins/VST3/ChorusVox\ DecaBuddy.vst3 ~/Library/Audio/Plug-Ins/Components/ChorusVox\ DecaBuddy.component
   ```
5. Restart your DAW to perform a complete plug-in rescan.

---

## 🛠️ Building & Installing from Source (TRUCE Framework)

If you prefer compiling directly from source on your machine, ChorusVox DecaBuddy utilizes the **[TRUCE](https://github.com/purefunctor/truce)** audio plugin framework and CLI.

### 1. Prerequisites
* **Rust Toolchain:** Install stable Rust via `rustup`:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
* **TRUCE CLI (`cargo-truce`):** Install the command-line utility:
  ```bash
  cargo install truce-cli
  ```

### 2. Build & Installation Commands

* **Compile, sign, and install AUv2 and VST3 bundles to your user plug-in directory:**
  ```bash
  cargo truce install --vst3 --au2
  ```
  *This automatically compiles with `--release` optimization, creates macOS `.vst3` and `.component` bundles, applies ad-hoc codesigning, and copies them to `~/Library/Audio/Plug-Ins/VST3` and `~/Library/Audio/Plug-Ins/Components`.*

* **Build bundles without installing to system folders:**
  ```bash
  cargo truce build --vst3 --au2
  ```
  *(Output bundles will be placed inside `target/bundles/`).*

* **Validate installed plug-ins with Apple's official `auval` tool:**
  ```bash
  cargo truce validate --auval
  ```

* **Flush Audio Units cache if your DAW fails to detect the update:**
  ```bash
  cargo truce reset-au --yes
  ```

* **Uninstall bundles from the system:**
  ```bash
  cargo truce uninstall --vst3 --au2
  ```

---

## 📖 Operational Manual & Parameter Reference

The user interface is organized into 6 functional strips: **Global & Input**, **Direct & Unison**, and four independent harmony channels (**VOX 1** through **VOX 4**).

### 1. `GLOBAL & INPUT` Strip

Controls root key, incoming MIDI chord parsing, master concert pitch, discrete analog drive, Natural mode, and peak metering.

| Control | Range / Options | Description |
| :--- | :--- | :--- |
| **Key** | `C`, `C#`, `D`, `D#`, `E`, `F`, `F#`, `G`, `G#`, `A`, `A#`, `B` | Sets musical root key for all harmony voices assigned to automatic scale tables (Modal 1–5, Harmonic Minor, Melodic Minor, etc.). |
| **MIDI Mode** | `Auto Ch`, `Mono (1-4)`, `Wait All`, `Priority`, `FlexiChd` | Voice-allocation strategy for harmony voices whose **Scale** is set to **`MIDI`**. |
| **MIDI Ch** | `1` to `16` | Sets base MIDI reception channel. In `Mono (1-4)` mode, voices 1 to 4 listen consecutively on `MIDI Ch` through `MIDI Ch + 3`. |
| **Tune Hz** | `427 Hz` to `453 Hz` *(Default: `440 Hz`)* | Master concert A4 reference frequency for the YIN pitch tracker. |
| **In Gain** | `-15.0 dB` to `+15.0 dB` *(Default: `0.0 dB`)* | Input drive into the discrete front-end stage. Levels above `0 dB` introduce subtle 2nd-harmonic analog warmth. |
| **Drive** | `0.0%` to `100.0%` *(Default: `35.0%`)* | Master analog drive for the **ChoirAnalogBus**: Ariwa 185 Hz console low-mid warmth shelf, JRC NJM4558L asymmetric rail saturation (Silk 2nd harmonic), 1.0 V/µs soft slew-rate limiting, 9V DC dynamic power supply sag, Nichicon/Elna electrolytic dielectric absorption, and stereo crosstalk (-62 dB). Applied strictly to synthesized choir voices. |
| **Natural** | `On` / `Off` *(Default: `Off`)* | **Mode Selector (Classic HV10 vs. Modern Studio):**<br>• `Off` (*Classic HV10*): 1990s PSOLA time-domain grain bite, fast stepped microcontroller note tracking, and crunchy consonant presence.<br>• `On` (*Natural*): Intelligent sibilance ducking (clean unvoiced consonants) and 2nd-order critically damped S-curve portamento (~28 ms). |
| **In** *(Meter)* | Vertical Bar | Ballistic input peak meter modeled after the `NJU7032D` (IC401) circuit and the 3 hardware LEDs (`-12 dB`, `-6 dB`, `0 dB/Over`). |

> [!NOTE]
> **Automatic Live vs. Offline PDC Latency:** During real-time monitoring, ChorusVox runs in **Live Mode** with **0 ms reported latency** to the DAW and **0 ms delay on the Direct path**. During offline DAW export/bounce, full lookahead with 2496 samples of Plugin Delay Compensation (PDC) is engaged automatically.

#### Detailed MIDI Modes (`MIDI Mode`)

Set any voice's **Scale** knob to **`MIDI`** (fully counter-clockwise) to control it via incoming MIDI notes. All modes support **Sustain Pedal (`MIDI CC 64`)**:

* **`Auto Ch` (Vanilla / Hardware Default):** Authentic Akai hardware behavior. Chord notes on `MIDI Ch` are sorted lowest-to-highest and assigned across active MIDI voices.
* **`Mono (1-4)`:** Voices 1 to 4 listen independently to consecutive MIDI channels (`MIDI Ch` to `MIDI Ch + 3`), allowing independent polyphonic counterpoint lines.
* **`Wait All`:** Harmony voices remain muted until the number of held MIDI keys equals or exceeds the number of active MIDI voices.
* **`Priority`:** Notes are assigned to voices in the exact chronological sequence keys are struck.
* **`FlexiChd` (Intelligent Jazz Harmony Engine):** Play any jazz chord (`Dmaj7`, `F#m6`, `9th`, `13th`, or 2-note shell voicings) and **all 4 voices track dynamically with your sung melody** while locking onto chord tones:
  * **Dynamic Chord-Scale Inference:** Combines held MIDI chord notes with global **Key** to infer harmonic context from 2-note shell voicings and passing tones.
  * **Voice Leading Memory & Smooth Legato:** Retains common tones stationary across chord changes, gliding smoothly into new voicings with natural vocal portamento.
  * **Anti-Clash ($b2/b9$ Prevention):** Automatically nudges harmony voices that land 1 semitone away from the lead vocal.
  * **Intelligent Sustain Pedal Focus:** Automatically prioritizes recently struck chord tones during extended CC64 pedal passages.

---

### 2. `DIRECT & UNISON` Strip

Controls the dry lead vocal path, the 10th unison doubling voice, parameter linking, and stereo output metering.

| Control | Range / Options | Description |
| :--- | :--- | :--- |
| **Direct** | `On` / `Off` | Enables or mutes dry lead vocal. Turn `Off` when running on an auxiliary send/return track. |
| **Dir Lev** | `-24.0 dB` to `0.0 dB` | Output level of direct signal (`-24.0 dB` = mute). |
| **Dir Pan** | `-50` (L) to `+50` (R) | Constant-power stereo pan position for direct vocal. |
| **Link** | `On` / `Off` | Mirrors `VOX 1` **Random**, **Formant**, and **Quantise** settings across `VOX 2`, `VOX 3`, and `VOX 4` while preserving each voice's individual **Scale**, **Level**, and **Pan**. |
| **Unison** | `On` / `Off` | Enables dedicated 10th unison doubling voice with warm analog low-pass filtering. |
| **U.Rand** | `0.0` to `100.0` *(Default: `0.0`)* | Controls Ornstein-Uhlenbeck pitch drift ($\pm 9\text{ cents}$) and delay modulation for natural doubling. Opens at `0.0` for 100% deterministic pitch and phase; humanization engages above `0.0`. |
| **U.Form** | `-50.0` to `+50.0` | Shifts vocal tract formant frequencies of the Unison voice without altering pitch. |
| **U.Quant** | `0.0` to `100.0` | Pitch quantization strength for Unison voice. |
| **U.Level** | `-24.0 dB` to `0.0 dB` | Output level of Unison voice. |
| **U.Pan** | `-50` (L) to `+50` (R) | Stereo pan position for Unison voice. |
| **Out** *(Meter)* | Stereo Bar (L/R) | Peak output meter after stereo reconstruction and NJM4558L crosstalk modeling. |

---

### 3. Harmony Strips: `VOX 1`, `VOX 2`, `VOX 3`, `VOX 4`

Four independent PSOLA harmony channels. Enabling **Dual** across all 4 voices alongside **Direct** and **Unison** produces all **10 voices** of the DecaBuddy architecture.

| Control | Range / Options | Description |
| :--- | :--- | :--- |
| **On** | `On` / `Off` | Enables or disables voice channel with a 40 ms anti-click envelope (zero CPU when idle). |
| **Dual** | `On` / `Off` | Activates twin sub-voice panned to opposite stereo side (`-Pan`) with shifted formants, $-9\text{ cents}$ detune, $+14\text{ ms}$ Haas delay, and a **1st-order Allpass 90° phase disperser** that eliminates comb filtering in unisons and octaves. |
| **Scale** | `37 Options` | Selects `MIDI`, custom `User` scale, or 35 Akai preset harmony tables (`Low Octave`, `Low 3rd`, `Low 5th`, `Low 7th`, `Unison`, `High 3rd`, `High 5th`, `High 7th`, `High Octave`). |
| **Random** | `0.0` to `100.0` *(Default: `0.0`)* | Humanizes pitch drift ($\pm 9\text{ cents}$) and timing micro-jitter per sub-voice. Opens at `0.0` for deterministic phase; drift and dynamic 9V sag micro-cents act above `0.0`. |
| **Formant** | `-50.0` to `+50.0` | Shifts vocal tract formants with strict preservation (`src_step = formant_ratio`) and band-limited anti-aliasing interpolation when shifted upwards. |
| **Quantise** | `0.0` to `100.0` | Blends between natural vocal inflection tracking (`0%`) and chromatic grid locking (`100%`). |
| **Level** | `-24.0 dB` to `0.0 dB` | Channel output level. |
| **Pan** | `-50` (L) to `+50` (R) | Stereo pan position for the main voice (and mirror `-Pan` for the Dual voice). |

---

## 🔬 Hardware Simulation Architecture (Akai HV10 & Ariwa Studio)

```text
               ┌─────────────────────────────────────────────────────────────┐
               │           INPUT SIGNAL (Mono L or Summed L + R)             │
               └──────────────────────────────┬──────────────────────────────┘
                                              │
                      ┌───────────────────────┴───────────────────────┐
                      │                                               │
           [ DIRECT LINEAR PATH ]                          [ DIGITAL DSP PATH ]
           • Takamisawa RY-9WK Relays                      (Burr-Brown PCM3003E ADC)
             (2.2ms Contact Chatter Bounce)                           │
           • 100% Linear & Transparent (0% THD)                       │
           • 20 Hz - 20 kHz Flat Response                             ▼
           • Live: 0 ms delay / Studio: 2496 PDC      ┌───────────────────────────────────────────────┐
                      │                               │ 1. TIME-SLICED YIN & SEDREAMS GCI ENGINE      │
                      │                               │ • 1536-Sample Synchronous Window (down to 42Hz│
                      │                               │ • Adaptive ~22.05 kHz Decimation (2x/4x/8x SR)│
                      │                               │ • SEDREAMS Mean-Based Signal (MBS) Detector   │
                      │                               │   (Cancels F1/F2 formants for fry stability)  │
                      │                               │ • Zero-Crossing Rate & High-Freq Sibilance Det│
                      │                               └───────────────────────┬───────────────────────┘
                      │                                                       │
                      │                                                       ▼
                      │                               ┌───────────────────────────────────────────────┐
                      │                               │ 2. 10x TD-PSOLA ENGINE (ADSP-2186M HERITAGE)  │
                      │                               │ • 4-Point Hermite Cubic Interpolation (C1)    │
                      │                               │ • Anti-Aliasing Polyphase Resampling on Form. │
                      │                               │ • Dual Voice Allpass 90° Phase Decorrelation  │
                      │                               │   (Zero destructive comb-filtering on unisons)│
                      │                               │ • Exact 50% Hann NOLA Overlap (Weight Sum=1.0)│
                      │                               │ • Mode: Classic HV10 vs. Natural S-Curve Slew │
                      │                               │ • Post-NOLA 16-bit Q15 Quantizer + 40-bit MAC │
                      │                               │ • Akai 32.78 Hz Subsonic HPF & Voice DAC LPF  │
                      │                               └───────────────────────┬───────────────────────┘
                      │                                                       │
                      │                                                       ▼
                      │                               ┌───────────────────────────────────────────────┐
                      │                               │ 3. CHOIR ANALOG BUS (ARIWA STUDIO & HV10)     │
                      │                               │ • Natural Mode: Smart Sibilance Ducking       │
                      │                               │ • Ariwa Console 185 Hz Low-Mid Warmth Shelf   │
                      │                               │ • 9V DC Dynamic Power Supply Sag (C22 220µF)  │
                      │                               │ • JRC NJM4558L Asymmetric Silk Saturation     │
                      │                               │ • NJM4558L 1.0 V/µs Soft Slew-Rate Limiting   │
                      │                               │ • Active Sallen-Key LP Filter (16.8 kHz)      │
                      │                               │ • SIP-8 Dual-Opamp Stereo Crosstalk (-62 dB)  │
                      │                               │ • C23/C25 Output Electrolytic Caps with       │
                      │                               │   Dielectric Absorption (DA) Soakage Memory   │
                      │                               └───────────────────────┬───────────────────────┘
                      │                                                       │
                      └───────────────────────┬───────────────────────────────┘
                                              ▼
               ┌─────────────────────────────────────────────────────────────┐
               │ 4. MASTER SUM & ANALOG BUS SOFT LIMITER                     │
               │ • Transparent Linear Pass-Through for |x| <= 0.92 (0% THD)  │
               │ • C1-Continuous Asymptotic Soft-Knee (Neve/Tape saturation) │
               │ • Mechanical Relay Bypass Circuit (RL1/RL2 RY-9WK)          │
               │ • Zero Hard Digital Clipping Artifacts                      │
               └──────────────────────────────┬──────────────────────────────┘
                                              │
                                              ▼
                             STEREO OUTPUT (Left / Right)
```

---

## 🚀 High-Performance Vectorization Benchmarks

ChorusVox DecaBuddy is engineered from the ground up for minimal CPU consumption and rock-solid audio performance:
* **ARM64 NEON & x86 AVX2 Autocorrelation:** The YIN cumulative mean normalized difference matrix is computed with explicit vector SIMD routines (`vfmaq_f32`), resulting in **zero FFI overhead** and lightning-fast tracking.
* **Benchmark:** On Apple Silicon, full 4-voice polyphonic stereo processing with Dual enabled processes at **73x faster than real-time** (under 98 ms of CPU time for 7.11 seconds of audio).
* **Zero Allocations:** Zero heap allocations inside the audio processing loop (`process_block`).
* **Denormal Protection:** Built-in RAII FTZ/DAZ (`ScopedFtzDaz`) assembly guard protects against denormal floating-point CPU stalls.

---

## 📜 License

ChorusVox DecaBuddy is distributed under the dual **MIT OR Apache-2.0** license. See [LICENSE](LICENSE) for details.
All original scale presets and hardware circuit topologies are derived from public documentation, schematics, and service manuals of the Akai DecaBuddy HV10 for historical preservation and audio engineering research.
