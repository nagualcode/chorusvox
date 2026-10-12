//! Motor DSP em tempo real do ChorusVox / DecaBuddy.

use std::f32::consts::PI;

pub const RING_SIZE: usize = 16384;
pub const RING_MASK: usize = RING_SIZE - 1;
pub const MAX_GRAIN: usize = 2048;

pub const OUT_DELAY: usize = MAX_GRAIN / 2;
pub const PSOLA_SYNTH_WINDOW_FACTOR: f32 = 1.25;

#[inline]
pub fn yin_window_and_hop(sr: f32) -> (usize, usize) {
    let scale = (sr / 44_100.0).clamp(0.5, 4.0);
    let win = ((2048.0 * scale).round() as usize)
        .next_power_of_two()
        .clamp(1024, 8192);
    let hop = ((256.0 * scale).round() as usize).clamp(128, 1024);
    (win, hop)
}

#[inline]
pub fn hz_to_midi(f_hz: f32, tune_hz: f32) -> f32 {
    69.0 + 12.0 * (f_hz.max(1e-3) / tune_hz.clamp(427.0, 453.0)).log2()
}

#[inline]
pub fn voice_db_to_lin(db: f32) -> f32 {
    if db <= -23.95 {
        0.0
    } else {
        10.0_f32.powf(db * 0.05)
    }
}

#[inline]
pub fn gain_db_to_lin(db: f32) -> f32 {
    10.0_f32.powf(db * 0.05)
}

#[inline]
pub fn pan50_to_gains(pan_50: f32) -> (f32, f32) {
    let p = (pan_50 / 50.0).clamp(-1.0, 1.0);
    let theta = (p + 1.0) * PI * 0.25;
    (theta.cos(), theta.sin())
}

#[derive(Clone, Copy, Debug)]
pub struct FastRng {
    state: u32,
}

impl FastRng {
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x6D2B79F5 } else { seed },
        }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    #[inline]
    pub fn next_bipolar(&mut self) -> f32 {
        (self.next_u32() as f32 / (u32::MAX as f32)) * 2.0 - 1.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BiquadLowpass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl BiquadLowpass {
    pub fn set_cutoff(&mut self, cutoff_hz: f32, sr: f32) {
        let fc = cutoff_hz.clamp(80.0, sr * 0.45);
        let w0 = 2.0 * PI * fc / sr;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / std::f32::consts::SQRT_2;
        let a0 = 1.0 + alpha;
        self.b0 = ((1.0 - cos_w0) * 0.5) / a0;
        self.b1 = (1.0 - cos_w0) / a0;
        self.b2 = self.b0;
        self.a1 = (-2.0 * cos_w0) / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

#[derive(Clone, Debug)]
pub struct PitchTracker {
    pub fmin: f32,
    pub fmax: f32,
    pub threshold: f32,
    pub hysteresis_semi: f32,
    pub yin_win: usize,
    pub yin_hop: usize,
    cmnd: Vec<f32>,
    smooth_midi: f32,
    pub current_f0: f32,
    pub current_midi: f32,
    pub current_note: i32,
    pub voiced: bool,
    unvoiced_hold: usize,
}

impl Default for PitchTracker {
    fn default() -> Self {
        Self::new(44_100.0)
    }
}

impl PitchTracker {
    pub fn new(sr: f32) -> Self {
        let (yin_win, yin_hop) = yin_window_and_hop(sr);
        Self {
            fmin: 70.0,
            fmax: 900.0,
            threshold: 0.32,
            hysteresis_semi: 0.65,
            yin_win,
            yin_hop,
            cmnd: vec![1.0; yin_win / 2],
            smooth_midi: 60.0,
            current_f0: 0.0,
            current_midi: 60.0,
            current_note: -1,
            voiced: false,
            unvoiced_hold: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        let (yin_win, yin_hop) = yin_window_and_hop(sr);
        self.yin_win = yin_win;
        self.yin_hop = yin_hop;
        self.cmnd.resize(yin_win / 2, 1.0);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.cmnd.fill(1.0);
        self.current_f0 = 0.0;
        self.current_midi = 60.0;
        self.current_note = -1;
        self.voiced = false;
        self.unvoiced_hold = 0;
    }

    pub fn analyze_frame(&mut self, ring: &[f32], write_pos: usize, sr: f32, tune_hz: f32) {
        let win = self.yin_win;
        let tau_min = ((sr / self.fmax) as usize).max(2);
        let tau_max = ((sr / self.fmin) as usize).min(win / 2 - 2);
        let w = win - tau_max;
        let start = write_pos.wrapping_sub(win);

        let mut sum_sq = 0.0_f32;
        for i in 0..w {
            let s = ring[(start.wrapping_add(i)) & RING_MASK];
            sum_sq += s * s;
        }
        let rms = (sum_sq / w as f32).sqrt();
        if rms < 0.004 {
            self.mark_unvoiced();
            return;
        }

        self.cmnd[0] = 1.0;
        let mut running_sum = 0.0_f32;
        for tau in 1..=tau_max {
            let mut diff = 0.0_f32;
            for i in 0..w {
                let a = ring[(start.wrapping_add(i)) & RING_MASK];
                let b = ring[(start.wrapping_add(i + tau)) & RING_MASK];
                let d = a - b;
                diff += d * d;
            }
            running_sum += diff;
            self.cmnd[tau] = if running_sum > 1e-12 {
                diff * (tau as f32) / running_sum
            } else {
                1.0
            };
        }

        let mut best_tau = 0usize;
        let mut best_cost = 1e9_f32;
        for tau in tau_min..tau_max {
            let c = self.cmnd[tau];
            if c < self.threshold && c < self.cmnd[tau - 1] && c <= self.cmnd[tau + 1] {
                let oct_bias = 0.02 * ((tau as f32) / (tau_min as f32)).log2();
                let cand_f0 = sr / (tau as f32);
                let jump_cost = if self.voiced && self.current_f0 > 0.0 {
                    0.04 * (12.0 * (cand_f0 / self.current_f0).log2()).abs().min(12.0)
                } else {
                    0.0
                };
                let total_cost = c + oct_bias + jump_cost;
                if total_cost < best_cost {
                    best_cost = total_cost;
                    best_tau = tau;
                }
            }
        }

        if best_tau == 0 {
            self.mark_unvoiced();
            return;
        }

        let y0 = self.cmnd[best_tau - 1];
        let y1 = self.cmnd[best_tau];
        let y2 = self.cmnd[best_tau + 1];
        let den = y0 - 2.0 * y1 + y2;
        let frac = if den.abs() > 1e-9 {
            0.5 * (y0 - y2) / den
        } else {
            0.0
        };
        let refined_tau = (best_tau as f32 + frac.clamp(-0.5, 0.5)).max(1.0);
        let f0 = sr / refined_tau;
        let midi = hz_to_midi(f0, tune_hz);

        if !self.voiced {
            self.smooth_midi = midi;
            self.current_note = midi.round() as i32;
        } else {
            self.smooth_midi = 0.72 * self.smooth_midi + 0.28 * midi;
            if (self.smooth_midi - self.current_note as f32).abs() > self.hysteresis_semi {
                self.current_note = self.smooth_midi.round() as i32;
            }
        }

        self.current_f0 = f0;
        self.current_midi = midi;
        self.voiced = true;
        self.unvoiced_hold = 0;
    }

    fn mark_unvoiced(&mut self) {
        self.unvoiced_hold += 1;
        if self.unvoiced_hold >= 2 {
            self.voiced = false;
            self.current_f0 = 0.0;
            self.current_note = -1;
        }
    }
}

const DELAY_SIZE: usize = 8192;
const DELAY_MASK: usize = DELAY_SIZE - 1;

#[derive(Clone, Debug)]
pub struct PsolaSubVoice {
    out_ring: Vec<f32>,
    w_ring: Vec<f32>,
    delay_line: Vec<f32>,
    delay_write: usize,
    synth_phase: f32,
    drift_cents: f32,
    delay_rand_norm: f32,
    delay_mod_samples: f32,
    rng_pitch: FastRng,
    rng_delay: FastRng,
}

impl PsolaSubVoice {
    pub fn new(seed: u32) -> Self {
        Self {
            out_ring: vec![0.0; RING_SIZE],
            w_ring: vec![0.0; RING_SIZE],
            delay_line: vec![0.0; DELAY_SIZE],
            delay_write: 0,
            synth_phase: 0.0,
            drift_cents: 0.0,
            delay_rand_norm: 0.0,
            delay_mod_samples: 0.0,
            rng_pitch: FastRng::new(seed),
            rng_delay: FastRng::new(seed.wrapping_mul(0x85EBCA6B).wrapping_add(17)),
        }
    }

    pub fn reset(&mut self) {
        self.out_ring.fill(0.0);
        self.w_ring.fill(0.0);
        self.delay_line.fill(0.0);
        self.synth_phase = 0.0;
        self.drift_cents = 0.0;
        self.delay_rand_norm = 0.0;
        self.delay_mod_samples = 0.0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step_sample(
        &mut self,
        dry_ring: &[f32],
        read_pos: usize,
        emit_pos: usize,
        latest_mark: usize,
        voiced_mask: f32,
        emit_mask: f32,
        f0_hz: f32,
        pitch_ratio: f32,
        formant_ratio: f32,
        random_norm: f32,
        extra_cents: f32,
        extra_delay_ms: f32,
        unvoiced_gain: f32,
        sr: f32,
    ) -> f32 {
        const A: f32 = 0.99995;
        let k_norm = ((1.0 + A) / (1.0 - A)).sqrt() * 3.0_f32.sqrt();
        let n_pitch = self.rng_pitch.next_bipolar();
        let n_delay = self.rng_delay.next_bipolar();

        self.drift_cents = A * self.drift_cents + (1.0 - A) * n_pitch * k_norm * (9.0 * random_norm);
        self.delay_rand_norm = A * self.delay_rand_norm + (1.0 - A) * n_delay * k_norm;

        let jitter_ms = (self.delay_rand_norm * 0.5 + 0.5).clamp(0.0, 1.5) * (16.0 * random_norm);
        let target_delay_ms = jitter_ms + extra_delay_ms;
        let target_delay_samp =
            (target_delay_ms * 0.001 * sr).clamp(0.0, (DELAY_SIZE - 8) as f32);
        self.delay_mod_samples = 0.999 * self.delay_mod_samples + 0.001 * target_delay_samp;

        if voiced_mask > 0.01 && f0_hz > 50.0 {
            let cents_factor = 2.0_f32.powf((self.drift_cents + extra_cents) / 1200.0);
            let ratio = (pitch_ratio * cents_factor).clamp(0.25, 4.0);
            let ta = (sr / f0_hz).clamp(20.0, (MAX_GRAIN / 2 - 4) as f32);
            let ts = (ta / ratio).clamp(12.0, (MAX_GRAIN - 8) as f32);

            self.synth_phase -= 1.0;
            if self.synth_phase <= 0.0 {
                self.synth_phase += ts;
                let grain_len = ((2.0 * ta).max(PSOLA_SYNTH_WINDOW_FACTOR * ts) as usize | 1)
                    .clamp(31, MAX_GRAIN - 1);
                self.splat_grain(dry_ring, latest_mark, read_pos, grain_len, formant_ratio);
            }
        } else {
            self.synth_phase = 0.0;
        }

        let idx = emit_pos & RING_MASK;
        let raw_out = self.out_ring[idx];
        let raw_w = self.w_ring[idx];
        self.out_ring[idx] = 0.0;
        self.w_ring[idx] = 0.0;

        let wet = if raw_w > 0.05 { raw_out / raw_w } else { raw_out };
        let dry = dry_ring[idx];
        let combined = wet * emit_mask + dry * (1.0 - emit_mask) * unvoiced_gain;

        self.delay_line[self.delay_write & DELAY_MASK] = combined;
        let read_f = self.delay_write as f32 - self.delay_mod_samples;
        let r0 = (read_f.floor() as isize) & (DELAY_MASK as isize);
        let r1 = (r0 + 1) & (DELAY_MASK as isize);
        let frac = read_f - read_f.floor();
        let delayed =
            self.delay_line[r0 as usize] * (1.0 - frac) + self.delay_line[r1 as usize] * frac;
        self.delay_write = self.delay_write.wrapping_add(1);
        delayed
    }

    fn splat_grain(
        &mut self,
        dry_ring: &[f32],
        mark_center: usize,
        out_center: usize,
        l_base: usize,
        formant_ratio: f32,
    ) {
        let fr = formant_ratio.clamp(0.5, 2.0);
        let l_out = (((l_base as f32) / fr).round() as usize | 1).clamp(17, MAX_GRAIN - 1);
        let half_out = (l_out / 2) as isize;
        let inv_l = 1.0 / (l_out as f32);

        for k in 0..l_out {
            let offset_out = k as isize - half_out;
            let src_off = (offset_out as f32) * fr;
            let src_i0 = src_off.floor() as isize;
            let frac = src_off - (src_i0 as f32);

            let p0 = (mark_center as isize + src_i0) as usize & RING_MASK;
            let p1 = (p0 + 1) & RING_MASK;
            let sample = dry_ring[p0] * (1.0 - frac) + dry_ring[p1] * frac;

            let w = 0.5 * (1.0 - (2.0 * PI * (k as f32) * inv_l).cos());
            let dst = (out_center as isize + offset_out) as usize & RING_MASK;
            self.out_ring[dst] += sample * w;
            self.w_ring[dst] += w;
        }
    }
}

#[derive(Clone, Debug)]
pub struct HarmonyChannelDsp {
    pub main: PsolaSubVoice,
    pub dual: PsolaSubVoice,
}

impl HarmonyChannelDsp {
    pub fn new(seed_base: u32) -> Self {
        Self {
            main: PsolaSubVoice::new(seed_base),
            dual: PsolaSubVoice::new(seed_base.wrapping_add(0x9E3779B9)),
        }
    }

    pub fn reset(&mut self) {
        self.main.reset();
        self.dual.reset();
    }
}
