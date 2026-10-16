# ChorusVox DecaBuddy 🎙️🎛️

[![macOS Supported](https://img.shields.io/badge/platform-macOS%20(AU%20%2F%20VST3)-blue.svg)](#-macos-installation--release-assets)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-green.svg)](LICENSE)

**ChorusVox DecaBuddy** is a Rust emulation of the **Akai DecaBuddy HV10** vocal harmony processor (1 original voice + 4 harmonies + 5 unison voices = 10 voices). It reproduces the unit's controls, harmony modes and processing chain, and adds one modern extra: the **SMART** MIDI mode.

<p align="center">
  <img src="preview.jpg" alt="ChorusVox DecaBuddy Interface" width="100%">
</p>

---

## ⚡ What's New in v0.3.1

* **Fixed upper harmonies jumping one or two octaves too high:**
  * the pitch detector sometimes locked onto the 2nd or 3rd harmonic of vowels like "o" and "u" (≈19% of the time on a test phrase). The period-locked engine then put the whole harmony an octave or more up;
  * a sub-harmonic check now rejects weak harmonic dips in favor of the true period. Detection accuracy on the test phrase went from 81% to 99.7%;
  * a new regression test covers three vowels across C3–C5.
* **USER STYLES tab:**
  * **RESET** restores the selected U1–U9 to its defaults;
  * **COPY FROM** copies any factory style (above or below the voice) into the selected user style for editing, like the HV10's "Copy Style".

## v0.3.0: HV10 emulation

v0.3.0 is a rewrite. Controls and DSP now follow the HV10 documentation and the DSP engine of Akai's own DecaBuddy v1.02 software.

* **HV10 front panel:**
  * parts **ORIGIN, L2, L1, H1, H2**;
  * **BUDDIES** (`--` / `ON` / `ON+UN` / `FX`);
  * **FORMANT −9…+9**, **LEVEL 0…9**, **PAN L9…R9**;
  * **P.CORRECT `--`, 1…9** and **RAND.DELAY `--`, D1…D9, R1…R9**;
  * **MASTER**, **TUNE 427–453 Hz**, **Sub** Key/Style.
* **HV10 harmony modes:** STYLE (`UN`, `8v`, `3A`–`3H`, `5A`–`5H`, `U1`–`U9`), MODAL (template + voicing `L8`…`H8`), FLEXICHORD, MONO CHN., WAIT ALL and PRIORITY, plus the modern **SMART** mode.
* **Pitch-shifting engine** from the Akai DSP code:
  * two-head shifter locked to the pitch period, with triangular windows (2·Ts up, 2·P down);
  * linear interpolation and plain overlap-add;
  * formant ±25%;
  * random delay from three Q15 LFOs;
  * unvoiced sounds pass through every harmony in unison.
* **Real hardware chain only:** 16-bit ADC/DAC quantization, clipping in the 16-bit DSP mix, coupling capacitors, and the PCM3003E bandwidth.
* **New interface:** a compact, flat, Ableton Live-inspired editor (868×346):
  * each voice is a color-coded strip: ORIGIN gray, L2 blue, L1 yellow, H1 green, H2 orange (the harmony colors of Akai's DecaBuddy software);
  * only the selectors for the active mode are shown (Style/Sub Style, Modal voicing, Priority);
  * a **USER STYLES** tab edits U1–U9 note by note.
* **Removed** (they are not on the HV10): the `Drive` / "analog bus", `Natural`, `Link` and `Dual` controls, plus the invented relay, crosstalk, dielectric-absorption and "console shelf" models.
* **Latency:** the original voice is undelayed. Harmonies lag by 0.5–1.5 pitch periods plus RAND.DELAY, like the hardware. The plugin reports 0 latency, and offline bounces sound the same as realtime playback.
* **Performance:** about 240× faster than realtime on Apple Silicon (CLI benchmark: ORIGIN + H1/H2 + L2 octave with unison).

> [!WARNING]
> v0.3.0 changes the parameter set. Sessions saved with v0.2.x will not recall their settings.

---

## 🍎 macOS Installation & Release Assets

Pre-compiled binaries are packaged for **macOS on Apple Silicon (arm64)** (Ableton Live, Logic Pro, Reaper, Bitwig, Studio One, Cubase, FL Studio, etc.).

1. Open the **[Releases](../../releases)** page of this repository.
2. Under the latest version, expand **Assets** and download **`ChorusVox-DecaBuddy-macOS-AU-VST3.zip`**.
3. Extract the archive and copy the bundles:
   * **Audio Unit (AUv2):** `ChorusVox DecaBuddy.component` → `~/Library/Audio/Plug-Ins/Components/`
   * **VST3:** `ChorusVox DecaBuddy.vst3` → `~/Library/Audio/Plug-Ins/VST3/`
4. If macOS Gatekeeper blocks the plug-in:
   ```bash
   xattr -cr ~/Library/Audio/Plug-Ins/VST3/ChorusVox\ DecaBuddy.vst3 ~/Library/Audio/Plug-Ins/Components/ChorusVox\ DecaBuddy.component
   ```
5. Restart your DAW to rescan.

---

## 🛠️ Building & Installing from Source (TRUCE Framework)

1. Install Rust (`rustup`) and the TRUCE CLI: `cargo install truce-cli`.
2. Then:
   ```bash
   cargo truce install --vst3 --au2     # build, sign and install AUv2 + VST3
   cargo truce build --vst3 --au2       # bundles only, in target/bundles/
   cargo truce validate --auval         # validate the installed AU
   cargo truce reset-au --yes           # flush the AU cache
   cargo truce uninstall --vst3 --au2
   cargo test --release                 # DSP / harmony test suite
   cargo run --release --bin decabuddy_cli [in.wav] [out.wav]   # offline render + benchmark
   ```

---

## 🎨 Interface

* **Top bar:** title, **VOICES / USER STYLES** tabs, **BYPASS**, and the input/output meters.
* **Global row:** Mode, Key, Sub and Sub Key, then the Input, Master, Tune and MIDI Ch knobs. The right side shows controls for the active mode: the MODAL template (1–7, with `m`/`M` on 3 and 7), the FLEXICHORD voicing, or a short hint.
* **Voice strips:** ORIGIN, L2, L1, H1 and H2, each with its own color. A strip has BUDDIES buttons, the selector for the current mode, and knobs for Level, Pan, Formant, P.Correct and R.Delay. Strips that are switched off are dimmed.
* **Knobs:** drag vertically; hold **Shift** for fine steps; **double-click** resets to the default.
* **USER STYLES tab:**
  * pick U1–U9;
  * drag each note's bar to set its harmony interval (−24…+24 semitones; the resulting note is shown in the current Key);
  * double-click a bar to leave that note unassigned (`--`);
  * **RESET** restores the style's defaults;
  * **COPY FROM** loads a factory style (3A…5H, above or below) as a starting point.

---

## 📖 Operation (follows the HV10 Operator's Manual)

### GLOBAL

| Control | Range | Description |
| :--- | :--- | :--- |
| **Mode** | STYLE, MODAL, FLEXICHORD, MONO CHN., WAIT ALL, PRIORITY, SMART | How harmonies are generated (see below). |
| **Key** | C/Am … B/G#m | Key used by STYLE and MODAL. |
| **Sub** / **Sub Key** | On/Off, key | HV10 Sub Parameter. While Sub is on, Sub Key and each part's Sub Style replace Key and Style. MIDI CC80/81 toggle it like the PROGRAM footswitches. |
| **Tune** | 427–453 Hz | TUNE ADJUST: reference pitch for detection and P.CORRECT. |
| **MIDI Ch** | 1–16 | Base channel. ORIGINAL = base, L2 = base+1, L1 = base+2, H1 = base+3, H2 = base+4. |
| **Input** | −15…+15 dB | INPUT LEVEL. The ADC clips at 0 dBFS, as on the hardware. |
| **Master** | 0–9 | MASTER LVL. |
| **Bypass** | On/Off | HV10 Normal Bypass: the dry signal on both outputs, through the codec. |

### Voice parameters (ORIGIN, L2, L1, H1, H2)

| Control | Range | Description |
| :--- | :--- | :--- |
| **Buddies** | `--`, `ON`, `ON+UN`; ORIGIN adds `FX` | Part off/on; `ON+UN` adds that part's unison voice; `FX` = the original's unison only. |
| **Style** | `UN`, `8v`, `3A`–`3H`, `5A`–`5H`, `U1`–`U9` | STYLE mode table. In STYLE mode L2/L1 always harmonize below and H1/H2 above. |
| **Sub Style** | same | Style used while Sub is on. |
| **Modal** | `UN`, `8L`, `8H`, `L8`…`L1`, `H1`…`H8` | MODAL voicing: the n-th template chord tone below (L) or above (H) the sung note. |
| **Formant** | −9…+9 | Formant shift without pitch change (±25%). |
| **Level** | 0–9 | 0 = off; 3 dB steps, 9 = 0 dB. |
| **Pan** | L9 … C … R9 | Pan law of the DSP (−3 dB center). |
| **P.Correct** | `--`, 1–9 | Pitch correction of the harmony toward the tempered grid. `--` follows the singer's inflection; 9 locks to the grid. |
| **R.Delay** | `--`, D1–D9, R1–R9 | D: fixed delay. R: random delay, three slow LFOs, plus ±1.1 cents of detune per step between a part and its unison. |
| **Priority** | 1–4 | PRIORITY mode: which received note (1st…4th) this part plays. |

### Harmony modes

* **STYLE:** Key + Style per part. Notes left unassigned in a table follow the DSP rule: a gap of 2+ notes is filled half from each neighbor; a single-note gap uses the neighbor in the direction of the pitch deviation.
* **MODAL:** Key + **Template** (degrees 1–7; 3 and 7 accept `m`/`M`) + Modal voicing per part. The chord root comes from a MIDI note on the base channel and is latched.
* **FLEXICHORD:** play a 3+ note chord in any inversion. Harmonies take the nearest chord tones below/above the sung note, with voicing `LO LO LO HI`, `LO LO HI HI` or `LO HI HI HI`. Notes that don't form a known chord are ignored. The chord is latched.
* **MONO CHN.:** each part plays the note received on its own channel (base+1 … base+4).
* **WAIT ALL:** harmonies sound once as many notes are held as there are parts switched on. Notes are assigned by pitch order.
* **PRIORITY:** notes are assigned in the order they are played, following each part's Priority.
* **SMART** (modern, not on the HV10):
  * infers the chord from held notes plus Key (handles 2-note shells and extensions);
  * keeps common tones between chords;
  * avoids b9 clashes against the melody;
  * honors the sustain pedal (CC64).

In the MIDI note modes (MONO CHN., WAIT ALL, PRIORITY) the harmony is tuned exactly to the received note, as on the DSP, and folded into ±1 octave of the singer.

### MIDI implementation

| Message | Function |
| :--- | :--- |
| Note On/Off | MODAL root, FLEXICHORD/SMART chords, MONO CHN./WAIT ALL/PRIORITY notes |
| CC7 / CC10 / CC12 / CC13 | LEVEL / PAN / FORMANT / P.CORRECT of the part on that channel (base = ORIGIN, base+1…4 = L2…H2) |
| CC11 | MASTER LVL |
| CC80 / CC81 | Sub Parameter (PROGRAM A/B footswitch) |
| CC82 | BYPASS |
| CC64 | Sustain (SMART and chord modes) |
| CC120 / CC123 | All notes off |

CC changes stay in effect until the matching on-screen control is moved.

---

## 🔬 What is emulated, and where it comes from

```text
INPUT ─► coupling cap ─► ADC PCM3003E (16-bit, clips at 0 dBFS, ~-98 dBFS noise floor)
            │
            ├─► ORIGIN (level / pan) ─────────────────────────────────────────────┐
            │                                                                     │
            └─► pitch detection ─► harmony mode ─► P.CORRECT ─► for each part:    │
                                                   two-head period-locked shifter │
                                                   (+ unison shifter)             │
                                                   RAND.DELAY / LFOs / formant ───┤
                                                                                  ▼
                       DSP mix ─► MASTER ─► 16-bit saturation ─► DAC ─► coupling cap ─► L/R
```

| Element | Source |
| :--- | :--- |
| Chips (ADSP-2186M, PCM3003E, H8/3644, NJM4558L, RY-9WK) and 44.1 kHz rate (11.2896 MHz crystal) | HV10 service manual, `reference/78b793.pdf` |
| Controls, ranges, modes, MIDI channels and CCs, Normal Bypass, factory program 01A (default) | HV10 operator's manual, `reference/a420d5.pdf` |
| STYLE tables (16 lower + 16 upper) | Text tables inside `DecaBuddy.dll` v1.02; checked against the manual's 3A/3E examples |
| Shifter, window shape and length, formant law, random-delay LFOs, unison gain/pan/detune, P.CORRECT formula, unvoiced behavior, pan law, 50 ms voicing hold, 2.5-semitone jump confirmation | Reverse engineering of the DSP engine in `DecaBuddy.dll` v1.02 (Akai's software version, ported from DSP code) |
| Pitch-period rule (window 2× period, shortened only for upward shifts) | IVL Technologies patent US 5,567,901 |

**Known approximations:**
* **Pitch detection:** uses YIN, a more robust detector. The original uses a zero-crossing/lobe-similarity tracker with a 500 Hz pre-filter. Pitch updates are still stepwise and per analysis, like the DSP.
* **Style names:** mapping 3H → "7th Major" and 5H → "7th Minor" is inferred. 3A–3G and 5A–5G follow the DLL order.
* **Fixed-delay length:** the D1–D9 length is scaled from the DSP's random-delay centers. The manual only says "the higher, the longer".
* **LEVEL steps:** the 3 dB LEVEL steps are an estimate.
* **CHORD mode:** not implemented. The list of 50 factory chords is not in the available documentation.
* **Factory programs:** the 100 factory programs are not included. Only program 01A is the default.
* **Third shifter head:** one extra head avoids cutting an audible grain when the pitch jumps. The original alternates two.

---

## 📜 License

Dual **MIT OR Apache-2.0**. Scale tables and hardware topology are derived from public Akai documentation and from Akai's DecaBuddy v1.02 software, for historical preservation and audio-engineering research.
