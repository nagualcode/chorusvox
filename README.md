# ChorusVox

**Vocal harmonizer and pitch corrector for macOS (Apple Silicon), Windows and Linux, as AU and
VST3, written in Rust with the [truce](https://github.com/truce-audio/truce) framework.**

ChorusVox puts up to 4 harmony voices (8 with **Double**) on top of your voice and sounds like
real singers right away, without hours of tweaking: few options, all automatable in Ableton
Live. It also corrects the pitch of the lead vocal, with or without the harmonies.

Formerly known as Heaven.

![Interface](docs/gui.png)

---

## Installation

Download the package for your system from the [Releases](../../releases) page.

| System | Package | Formats | Copy to |
|---|---|---|---|
| macOS (Apple Silicon) | `ChorusVox-macOS-AppleSilicon.zip` | AU, VST3 | `~/Library/Audio/Plug-Ins/Components/` (AU) and `~/Library/Audio/Plug-Ins/VST3/` (VST3) |
| Windows (x64) | `ChorusVox-Windows-x64.zip` | VST3 | `C:\Program Files\Common Files\VST3\` |
| Linux (x64) | `ChorusVox-Linux-x64.tar.gz` | VST3 | `~/.vst3/` |

On macOS, if Gatekeeper blocks the plugin:

```bash
xattr -cr ~/Library/Audio/Plug-Ins/Components/ChorusVox.component ~/Library/Audio/Plug-Ins/VST3/ChorusVox.vst3
```

Restart your DAW. In Live: *Plug-Ins → Audio Units → Jardim Sonoro → ChorusVox* (or *VST3*).

## Using it in Ableton Live

1. Put **ChorusVox** as an effect on the vocal track.
2. Choose where the harmony comes from (buttons at the top):
   * **CHORD**: pick a **Root** and a **Chord**. To change chords through the song, automate
     *Root* and *Chord* on the timeline. The change happens exactly at the drawn point. If you
     sing ahead of the downbeat, a note starting up to 80 ms before the change already uses the
     next chord.
   * **SCALE**: just the key (**Key** + **Scale**). The voices move in diatonic thirds, with no
     chords needed.
   * **MIDI CHORD**: play or record chords on a MIDI track. On that track, under *MIDI To*, pick
     the vocal track and then *ChorusVox*. The voices choose the best notes of the chord.
   * **MIDI NOTES**: each held MIDI note becomes a voice (up to 4), at that exact pitch.
3. **VOICES** (1–4) and **VOICING**:
   * *Above*: close harmony above the lead.
   * *Below*: below it.
   * *Around*: above and below.
   * *Wide*: open chord.
4. **MIX**:
   * **Harmony**: level of the voice group. Changing the number of voices does not change the
     volume.
   * **Lead**: level of the lead vocal. At minimum, only the harmony comes out.
   * **Humanize**: how much each virtual singer differs: delay, pitch drift, timbre and level.
   * **Width**: stereo spread.
   * **Character**: longer throat (darker voice) or shorter (brighter).
   * **Pure Tuning**: tunes the intervals with the lead in just intonation, like an a cappella
     group.
   * **Double**: two singers per voice.

### Lead vocal pitch correction

In the **TUNE** panel:

* **LEAD TUNE** turns the correction on. When it is off, the lead vocal passes untouched.
* **RETUNE** is the speed. At 0 ms the voice locks to the note and vibrato disappears (the
  robotic effect). Around 20 ms (the default) the correction sounds natural and slower vibrato
  passes through. Above that, it only fixes the average offset of the note.
* **AMOUNT** is how much of the correction is applied.
* **CHROMATIC** accepts any semitone as a target. When it is off, the targets follow the HARMONY
  panel:
  * *CHORD*: the scale of the current chord, which can be automated on the timeline;
  * *SCALE*: the chosen key;
  * *MIDI CHORD* and *MIDI NOTES*: the held MIDI notes (with none held, any semitone).

**Pitch correction only, no harmonies:** turn on **LEAD TUNE** and turn off **HARMONIES** at the
top. Then choose **SCALE**, the key and the scale. ChorusVox then works as a lead vocal
auto-tune. With both on, the harmonies are built from the corrected note.

The formants stay where they are, so the correction does not change the timbre of the voice.
Bypass turns both off, without clicks.

**Latency:** 80 ms, reported to the host. Live compensates for it automatically on playback and
mixdown. To record while hearing the harmony live, the delay is noticeable: record the dry vocal
and use ChorusVox at mixdown. The 80 ms is the price of looking 30 ms ahead and knowing which
note the singer will reach *before* choosing the harmony.

---

## Sound lineage

ChorusVox inherits five decades of harmonizers. Each generation solved one problem and left
another, and each left an idea that lives on here.

| Era | Milestone | What it brought | What was missing | What ChorusVox inherited |
|---|---|---|---|---|
| 1975 | **Eventide H910 Harmonizer** | The first commercial digital pitch shifter: harmony in real time. | Shifting through a variable delay line: "chipmunk" voice, displaced formants, periodic glitches. | The idea that a voice can sing with itself. |
| 1989–1990 | **Lent** (*Computer Music Journal*) and **TD-PSOLA** (Moulines & Charpentier) | Pitch shifting *synchronized to the glottal cycle*: each grain is one cycle of the voice, so the spectral envelope (the formants, the "face" of the voice) stays put. | Needs to know exactly where each cycle starts. | The whole shifting engine: two-period grains with a Hann window, re-placed at the target note's period. |
| 1990s | **IVL Technologies → DigiTech Vocalist** (patents US 5,231,671 and 5,567,901) | Intelligent harmony in hardware: Lent's method simplified, hysteresis against vibrato, sibilants passing unshifted, grain resampling to change the "gender" of the voice. | Notes from a fixed interval table, with no notion of chords or voice leading. | Note hysteresis, consonant handling and throat control (Character) by grain resampling. |
| 2000s | **TC-Helicon** (VoicePrism, VoiceOne, VoiceLive) | "Hybrid" shifting with formant correction, humanization and harmony guided by the accompaniment. In the line of the polyphonic harmony patents (US 8,168,877): ~0.2 semitone hysteresis, a note counts as stable only after ~100 ms, choosing a minor or major third by consonance with the chord. | Mandatory minimum latency in live hardware: it decides the note without looking ahead. | Choosing notes by consonance with the chord, and the idea of waiting for the note to "settle". |
| 2000s–today | **Antares Harmony Engine** (and Auto-Tune, US 5,973,252) | 4 voices, *Throat Modeling*, *Naturalize*, pitch and timing variation, Chord/Scale/MIDI modes, Choir. | In a side-by-side test (below), harmonies collapsing into unison, flat and robotic tuning, and amplitude modulation visible in the spectrum. | The interaction model (harmony source + voices + humanization), reduced to the essentials. |

What ChorusVox adds to this lineage:

* **Real lookahead:** it chooses the harmony note knowing where the singer will arrive.
* **Proportional transitions:** the harmony leaves the old note for the new one in the *same
  proportion* as the lead's glide, and a shared note stays still, as in a real arrangement.
* **Cost-based voice leading:** no minor second against the lead, no parallel fifths, a complete
  chord, short shifts.
* **Just intonation relative to the lead:** the tuning of a vocal group.
* **Humanization based on choir data**, not arbitrary noise.

---

## How we researched it

ChorusVox was built in a research and engineering session with Claude (Anthropic), under a
curious constraint: **whoever wrote the DSP cannot hear**. So everything that "sounds good" here
first had to *measure* well.

### 1. State of the art

* **Manuals.** We read the Antares *Harmony Engine User Guide* and *Harmony Engine Evo* manual
  from end to end. They gave us the list of ideas that work (*Naturalize*, pitch and timing
  variation, *Throat Length*, *Glide*, *Choir*) and the layout of the interface (harmony source
  → voices → humanization).
* **Patents.**
  * IVL/DigiTech (US 5,231,671 and US 5,567,901): two-period window, hysteresis "so the singer
    can do vibrato without the harmony oscillating", unshifted sibilants, a window that shrinks
    on upward shifts.
  * Harmony from polyphonic accompaniment (US 8,168,877): 0.2 semitone hysteresis, 100 ms to
    consider a note stable, minor or major third decided by the chord.
  * Antares (US 5,973,252): pitch detection by autocorrelation, the basis of Auto-Tune.
* **Academic research.**
  * TD-PSOLA and Lent's method, for formant-preserving shifting.
  * YIN (de Cheveigné & Kawahara, 2002), for detection.
  * Ternström's studies on pitch dispersion in choirs: listeners prefer 0–5 cents of deviation
    between singers and tolerate up to ~14. Measurements in real choirs show 25–30 cents. This
    calibrated **Humanize**.
* **Forums and production.** Discussions on Gearspace and backing vocal mixing guides converge on
  three points:
  * multiplied sibilants make harmonies seem "out of time" and artificial;
  * harmonies sound better slightly thinner and quieter than the lead;
  * too much uniformity (identical tuning, identical timing) gives the effect away.

  Hence the attenuated consonants in the harmonies, the high-pass filter and the timbre
  differences between singers.

### 2. The competitor on the bench

The installed Harmony Engine 4x was loaded through `pedalboard` (Spotify), with all parameters
inspected and renders made by script. We measured its latency (zero: the lead passes straight
through and the harmonies lag) and rendered references with the same settings for comparison.

### 3. A ruler instead of an ear

We built an objective evaluation suite:

* **Tuning:** harmony−lead interval frame by frame, measured with pyin.
* **Stability:** excess jitter relative to the lead and pitch glitches.
* **Periodicity:** autocorrelation at the period; misaligned grains lower this value.
* **Clicks:** high-frequency transients above the local envelope.
* **Formants:** envelope error measured *on the harmonics* of the output, with the intentional
  formant scaling discounted.
* **Spectrograms:** inspected visually at each change.

The test material was:

* real vocals (stems from *Frederico Flores – The Bedroom Sessions* and a bossa vocal);
* a **synthetic soprano and baritone with known f0**, to measure the detector's error in absolute
  cents.

### 4. What the ruler found (and what we fixed)

* **High soprano read an octave (or a third) too low.** In a periodic signal every multiple of
  the period is a YIN valley, and the candidate list had only 8 slots. It now has 32 in period
  order, with generic sub-harmonic correction.
* **"Creaky" voice turning into the octave below.** Alternating pulses double the apparent
  period. The downward octave move now respects continuity, unless the evidence is overwhelming.
* **Octave error persisting after a transition.** Both valleys are mediocre in the transition
  frame. The sub-multiple now only counts if it is truly periodic.
* **73 Hz readings on breaths and plosives.** We created a "range gate" based on the singer's
  recent median.
* **Octave error changing the harmony note.** A guard folds ±12 semitone excursions back to the
  current octave and only accepts the jump if it lasts 120 ms.
* **Voice-leading memory locking the register** (harmony a tenth below after an attack error).
  Movement now has a bounded cost, and shifts above an octave became much more expensive.
* **"Rough" humanization.** The Ornstein-Uhlenbeck process is continuous but not smooth at short
  scales. Smoothing at 3 Hz brought the harmony jitter *below* the lead's jitter.
* **Breathy tails turning into a weak unison.** We created a "weakly voiced" class: the harmony
  stays on the note, quieter, until the tail dies.
* **Allocations on the audio thread.** Two buffers per block and a list per candidate voicing.
  A test now counts allocations on the audio thread in every mode: there are none.

**Final result:** the detector's median error is 0.6 cent on the synthetic soprano. After
discounting the intentional formant scaling, the envelope error stays under 1 dB from −12 to +12
semitones. The plugin runs at ~50× real time in the worst case (4 voices + Double at 96 kHz) and
passes `auval` and `pluginval` at strictness 10.

### Lead correction on real voices

Measured on 60 s of each of three real vocals, with chromatic targets. The deviation is the
distance to the nearest semitone, measured at the output. The 20 ms RETUNE preserves vibrato on
purpose, which keeps its deviation higher. The level column is the p1–p99 range relative to the
dry signal, in 10 ms windows.

| | median deviation | level |
|---|---|---|
| original voice | 11–21 cents | |
| RETUNE 20 ms | 6–8 cents | −1.9 to +1.6 dB |
| RETUNE 0 ms | 1.3–2.4 cents | −1.9 to +1.5 dB |

With the harmony off, the correction runs at more than 130× real time.

### Side by side with Harmony Engine

Same vocal passage, one voice a diatonic third above:

| | Harmony Engine 4x | ChorusVox |
|---|---|---|
| frames with a wrong note / unison (0–2 semitones) | 543 | 130 |
| abrupt pitch jumps | 27 | 2 |
| spectral envelope distance (lower = truer timbre) | 12.1 | 8.6 |
| pitch jitter | 12.9 cents | 8.3 cents |

---

## How it works

* **Lead pitch:** YIN via FFT every 4 ms, with:
  * candidate choice by continuity;
  * octave and sub-harmonic tests;
  * a range gate;
  * a 3-frame median.
* **Epoch marks:** one per glottal cycle. The first of each passage goes on the peak of the
  cycle; the next ones come from normalized cross-correlation with the previous cycle. Grains sum
  in phase: no *phasing* or roughness.
* **PSOLA shifter:**
  * 2-period Hann window and fractional cubic interpolation;
  * power normalization by grain density;
  * humanized delays repeat or skip whole cycles, with no Doppler effect.
* **Lead correction:**
  * the result is a shift in semitones over the period of each mark, so a detector octave error
    does not make the voice jump;
  * the target note has hysteresis and only changes without reaching the neighbor note after
    150 ms, which holds wide and off-center vibratos;
  * grains are normalized by the sum of the windows, with no holes at consonant boundaries;
  * the L and R channels use the same marks, preserving the stereo image.
* **Harmonic plan:**
  * 30 ms of lookahead and hysteresis against vibrato;
  * voice notes by exhaustive search with a vocal arrangement cost;
  * chord and MIDI context with a sample timestamp (automation stays aligned to the audio).
* **Expression:** the harmony follows the lead's intonation (85%) and part of its vibrato (55%).
  In scoops and glissandos it follows everything.
* **Virtual singers:** each has a base delay, slow pitch and timing drift, and its own level and
  "throat". The throat also follows 10% of the shift (higher voices have shorter vocal tracts).
* **Consonants:** −10 dB in the harmonies, sibilants at −15 dB.
* **Mix:**
  * high-pass at 75 Hz and a slight high-frequency cut on the harmonies;
  * constant-power panning;
  * the group level is normalized by the number of voices.

## Development

```bash
cargo truce install --au2 --vst3            # build, sign and install
cargo test --no-default-features            # music theory, engine and real-time tests
cargo run --release --no-default-features --bin chorusvox_cli -- voice.wav out.wav \
    mode=chord voices=3 voicing=around chords="0:Am,4:F,8:C,12:G" double=1
cargo run --release --no-default-features --bin chorusvox_cli -- voice.wav out.wav \
    tune=1 harmonies=0 retune=20 mode=scale root=A scale=minor
cargo truce screenshot --out docs/gui.png
```

`chorusvox_cli` runs the same engine as the plugin offline. It accepts a chord timeline (like Live
automation), MIDI notes by interval and `dump=file.csv` with the detected pitch and the target
notes of each voice.

To publish a version, update `version` in `Cargo.toml` and push a tag:

```bash
git tag v0.2.1 && git push origin v0.2.1
```

The `.github/workflows/release.yml` workflow builds on each system, validates with `pluginval`
(and `auval` on macOS) and publishes the packages to the release. On pull requests it builds and
validates the same packages without publishing.

License: MIT or Apache-2.0.
