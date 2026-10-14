//! Motor DSP Otimizado e Modelagem do Barramento de Harmonias (Akai HV10 & Ariwa Studio).
//! - Caminho Direct e entrada YIN 100% transparentes e lineares (zero perda de agudos).
//! - Rastreamento YIN sincrono Time-Sliced com janela de 1536 amostras (suporte ate 42 Hz).
//! - PSOLA com periodo `ta` desacoplado de `out_delay` (graves perfeitos mesmo no modo Live)
//!   e janela de Hann unica com soma NOLA exata (zero dips na Low Octave).
//! - ChoirAnalogBus: aplicado exclusivamente no somatorio das harmonias (185 Hz console warmth,
//!   sag de 9V DC / 250mA, saturacao simetrica NJM4558L, crosstalk -62 dB e filtro PCM3003E 16.8 kHz).

use std::f32::consts::PI;

pub const RING_SIZE: usize = 32768;
pub const RING_MASK: usize = RING_SIZE - 1;
pub const VOICE_RING_SIZE: usize = 4096;
pub const VOICE_RING_MASK: usize = VOICE_RING_SIZE - 1;
pub const DELAY_SIZE: usize = 16384;
pub const DELAY_MASK: usize = DELAY_SIZE - 1;
pub const HANN_LUT_SIZE: usize = 2048;
pub const PSOLA_SYNTH_WINDOW_FACTOR: f32 = 1.35;

pub const AKAI_SUBSONIC_HP_HZ: f32 = 32.784804;
pub const AKAI_MAIN_VOICE_LP_HZ: f32 = 14500.0;
pub const AKAI_DUAL_WARM_LP_HZ: f32 = 9000.0;
pub const PCM3003_RECON_LP_HZ: f32 = 16800.0;

const OU_A: f32 = 0.99995;
const OU_ONE_MINUS_A_TIMES_K: f32 = (1.0 - OU_A) * 346.40582;

/// Guard RAII que ativa FTZ/DAZ diretamente via Assembly Inline sem funcoes depreciadas.
pub struct ScopedFtzDaz {
    #[cfg(target_arch = "aarch64")]
    prev_fpcr: u64,
    #[cfg(target_arch = "x86_64")]
    prev_mxcsr: u32,
}

impl Default for ScopedFtzDaz {
    fn default() -> Self { Self::new() }
}

impl ScopedFtzDaz {
    #[inline]
    pub fn new() -> Self {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let prev: u64;
            core::arch::asm!("mrs {}, fpcr", out(reg) prev, options(nomem, nostack, preserves_flags));
            let next = prev | (1 << 24);
            if next != prev {
                core::arch::asm!("msr fpcr, {}", in(reg) next, options(nomem, nostack, preserves_flags));
            }
            Self { prev_fpcr: prev }
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let mut prev: u32 = 0;
            core::arch::asm!("stmxcsr [{}]", in(reg) &mut prev, options(nostack, preserves_flags));
            let next = prev | 0x8040;
            if next != prev {
                core::arch::asm!("ldmxcsr [{}]", in(reg) &next, options(nostack, preserves_flags));
            }
            Self { prev_mxcsr: prev }
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        { Self {} }
    }
}

impl Drop for ScopedFtzDaz {
    #[inline]
    fn drop(&mut self) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("msr fpcr, {}", in(reg) self.prev_fpcr, options(nomem, nostack, preserves_flags));
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("ldmxcsr [{}]", in(reg) &self.prev_mxcsr, options(nostack, preserves_flags));
        }
    }
}

#[inline]
pub fn max_grain_for_sr(_sr: f32) -> usize { 2048 }

#[inline]
pub fn lookahead_samples_for_sr(sr: f32) -> usize {
    ((1536.0 * (sr / 44_100.0)).round() as usize).clamp(1024, 8192)
}

#[inline]
pub fn out_delay_for_sr(sr: f32) -> usize {
    ((960.0 * (sr / 44_100.0)).round() as usize).clamp(768, 1792)
}

/// Modo Live: Direct com 0 ms de atraso e harmonias com 384 amostras (~8.7 ms a 44.1 kHz)
#[inline]
pub fn live_lookahead_for_sr(sr: f32) -> usize {
    ((384.0 * (sr / 44_100.0)).round() as usize).clamp(256, 1536)
}

#[inline]
pub fn live_out_delay_for_sr(sr: f32) -> usize {
    ((384.0 * (sr / 44_100.0)).round() as usize).clamp(256, 1536)
}

/// Decimacao adaptativa para ~22.05 kHz com janela de 1536 amostras (cobre >2 periodos a 42 Hz)
#[inline]
pub fn yin_params_for_sr(sr: f32) -> (usize, usize, usize) {
    let decim = ((sr / 22_050.0).round() as usize).clamp(2, 8);
    let win = 1536usize;
    let hop = ((256.0 * (sr / 44_100.0)).round() as usize).clamp(128, 1024);
    (decim, win, hop)
}

#[inline]
pub fn hz_to_midi(f_hz: f32, tune_hz: f32) -> f32 {
    69.0 + 12.0 * (f_hz.max(1e-3) / tune_hz.clamp(427.0, 453.0)).log2()
}

#[inline(always)]
pub fn fast_exp2_small(x: f32) -> f32 {
    1.0 + x * (0.6931472 + x * (0.2402265 + x * 0.0555041))
}

#[inline]
pub fn voice_db_to_lin(db: f32) -> f32 {
    if db <= -23.95 { 0.0 } else { 10.0_f32.powf(db * 0.05) }
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

#[inline(always)]
pub fn soft_saturate(x: f32) -> f32 {
    const THRESHOLD: f32 = 0.78;
    if x > THRESHOLD {
        let over = x - THRESHOLD;
        THRESHOLD + (1.0 - THRESHOLD) * (over / (1.0 + over))
    } else if x < -THRESHOLD {
        let under = -x - THRESHOLD;
        -(THRESHOLD + (1.0 - THRESHOLD) * (under / (1.0 + under)))
    } else {
        x
    }
}

/// Barramento Analogico Estereo do Coro (Akai HV10 IC13/IC14 NJM4558L + PCM3003E + Ariwa Warmth).
/// Aplicado EXCLUSIVAMENTE nas vozes sintetizadas (Unison + Harmonias), mantendo o Direct 100% puro.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChoirAnalogBus {
    warm_l_z1: f32,
    warm_r_z1: f32,
    warm_a: f32,
    sag_env: f32,
    prev_l: f32,
    prev_r: f32,
    therm_l: f32,
    therm_r: f32,
    recon_l: BiquadLowpass,
    recon_r: BiquadLowpass,
}

impl ChoirAnalogBus {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.warm_a = (-2.0 * PI * 185.0 / sr.max(8000.0)).exp();
        self.recon_l.set_cutoff(PCM3003_RECON_LP_HZ, sr);
        self.recon_r.set_cutoff(PCM3003_RECON_LP_HZ, sr);
    }

    pub fn reset(&mut self) {
        self.warm_l_z1 = 0.0;
        self.warm_r_z1 = 0.0;
        self.sag_env = 0.0;
        self.prev_l = 0.0;
        self.prev_r = 0.0;
        self.therm_l = 0.0;
        self.therm_r = 0.0;
        self.recon_l.reset();
        self.recon_r.reset();
    }

    #[inline(always)]
    fn njm4558_rail_saturate(v: f32, ceiling: f32) -> f32 {
        let x = v.clamp(-2.5, 2.5);
        // Curva Padé tanh simetrica suave (zero DC offset, apenas harmonicos impares suaves)
        let d = x / ceiling;
        let d2 = d * d;
        ceiling * d * (27.0 + d2) / (27.0 + 9.0 * d2)
    }

    #[inline(always)]
    pub fn process_stereo(&mut self, wet_l: f32, wet_r: f32, active_voices: usize) -> (f32, f32) {
        // 1. Ariwa Console Low-Mid Warmth Shelf em 185 Hz (+1.2 dB de corpo nas harmonias)
        self.warm_l_z1 = (1.0 - self.warm_a) * wet_l + self.warm_a * self.warm_l_z1;
        self.warm_r_z1 = (1.0 - self.warm_a) * wet_r + self.warm_a * self.warm_r_z1;
        let w_l = wet_l + 0.14 * self.warm_l_z1;
        let w_r = wet_r + 0.14 * self.warm_r_z1;

        // 2. Sag dinamico da fonte de 9V DC / 250mA sob carga polifonica
        let energy = 0.5 * (w_l.abs() + w_r.abs()) * (1.0 + 0.08 * (active_voices as f32));
        if energy > self.sag_env {
            self.sag_env = 0.992 * self.sag_env + 0.008 * energy;
        } else {
            self.sag_env = 0.9997 * self.sag_env + 0.0003 * energy;
        }
        let ceiling = (0.96 - 0.08 * self.sag_env.clamp(0.0, 1.0)).clamp(0.84, 0.96);

        // 3. Saturacao suave do estagio de saida NJM4558L (IC13/IC14)
        let sat_l = Self::njm4558_rail_saturate(w_l, ceiling);
        let sat_r = Self::njm4558_rail_saturate(w_r, ceiling);

        // 4. Filtro de reconstrucao analogico Burr-Brown PCM3003E (IC3) + indutores LAP02TA (16.8 kHz)
        let rec_l = self.recon_l.process(sat_l);
        let rec_r = self.recon_r.process(sat_r);

        // 5. Crosstalk capacitivo e termico inter-canal do pacote duplo NJM4558L (-62 dB)
        let cap_r = (rec_r - self.prev_r) * 0.00075;
        let cap_l = (rec_l - self.prev_l) * 0.00075;
        self.prev_l = rec_l;
        self.prev_r = rec_r;
        self.therm_l = 0.999 * self.therm_l + 0.001 * rec_l;
        self.therm_r = 0.999 * self.therm_r + 0.001 * rec_r;

        (
            rec_l + cap_r + 0.00035 * self.therm_r,
            rec_r + cap_l + 0.00035 * self.therm_l,
        )
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SibilanceDetector {
    prev_sign: bool,
    zcr_smooth: f32,
    hf_env: f32,
    lf_env: f32,
    prev_x: f32,
}

impl SibilanceDetector {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline(always)]
    pub fn step(&mut self, x: f32, lp_x: f32) -> f32 {
        let sign = x >= 0.0;
        let zc = if sign != self.prev_sign { 1.0 } else { 0.0 };
        self.prev_sign = sign;
        self.zcr_smooth = 0.985 * self.zcr_smooth + 0.015 * zc;

        let diff_hf = (x - self.prev_x).abs();
        self.prev_x = x;
        self.hf_env = 0.98 * self.hf_env + 0.02 * diff_hf;
        self.lf_env = 0.98 * self.lf_env + 0.02 * lp_x.abs();

        let ratio = self.hf_env / (self.lf_env + 1e-4);
        if self.zcr_smooth > 0.22 && ratio > 0.85 {
            ((self.zcr_smooth - 0.22) * 4.0).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

pub fn build_hann_lut() -> Vec<f32> {
    let mut lut = vec![0.0_f32; HANN_LUT_SIZE];
    let inv = 1.0 / (HANN_LUT_SIZE as f32);
    for (k, slot) in lut.iter_mut().enumerate() {
        *slot = 0.5 * (1.0 - (2.0 * PI * (k as f32) * inv).cos());
    }
    lut
}

/// Kernel de diferenca quadratica com 8 acumuladores independentes (auto-vetorizacao NEON/AVX sem stalls)
#[inline(always)]
fn sq_diff_simd(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0.0_f32; 8];
    let ca = a.chunks_exact(8);
    let cb = b.chunks_exact(8);
    let tail: f32 = ca
        .remainder()
        .iter()
        .zip(cb.remainder())
        .map(|(&x, &y)| {
            let d = x - y;
            d * d
        })
        .sum();

    for (x, y) in ca.zip(cb) {
        for k in 0..8 {
            let d = x[k] - y[k];
            acc[k] += d * d;
        }
    }
    acc.iter().sum::<f32>() + tail
}

#[derive(Clone, Copy, Debug)]
pub struct FastRng {
    state: u32,
}

impl FastRng {
    pub fn new(seed: u32) -> Self {
        Self { state: if seed == 0 { 0x6D2B79F5 } else { seed } }
    }

    #[inline(always)]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    #[inline(always)]
    pub fn next_bipolar(&mut self) -> f32 {
        (self.next_u32() as f32) * (2.0 / (u32::MAX as f32)) - 1.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BiquadLowpass {
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, z1: f32, z2: f32,
}

impl BiquadLowpass {
    pub fn set_cutoff(&mut self, cutoff_hz: f32, sr: f32) {
        let fc = cutoff_hz.clamp(60.0, sr * 0.45);
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

    pub fn reset(&mut self) { self.z1 = 0.0; self.z2 = 0.0; }

    #[inline(always)]
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
    pub decim_factor: usize,
    pub yin_win: usize,
    pub yin_hop: usize,
    cmnd: Vec<f32>,
    oct_bias_lut: Vec<f32>,
    decim_buf: Vec<f32>,
    smooth_midi: f32,
    pub current_f0: f32,
    pub current_midi: f32,
    pub current_note: i32,
    pub voiced: bool,
    unvoiced_hold: usize,

    slice_tau_next: usize,
    slice_tau_max: usize,
    slice_tau_min: usize,
    slice_w: usize,
    slice_running_sum: f32,
    slice_active: bool,
}

impl Default for PitchTracker {
    fn default() -> Self { Self::new(44_100.0) }
}

impl PitchTracker {
    pub fn new(sr: f32) -> Self {
        let (decim_factor, yin_win, yin_hop) = yin_params_for_sr(sr);
        let mut pt = Self {
            fmin: 42.0,
            fmax: 900.0,
            threshold: 0.33,
            hysteresis_semi: 0.65,
            decim_factor,
            yin_win,
            yin_hop,
            cmnd: vec![1.0; yin_win / 2],
            oct_bias_lut: vec![0.0; yin_win / 2],
            decim_buf: vec![0.0; yin_win],
            smooth_midi: 60.0,
            current_f0: 0.0,
            current_midi: 60.0,
            current_note: -1,
            voiced: false,
            unvoiced_hold: 0,
            slice_tau_next: 1,
            slice_tau_max: 0,
            slice_tau_min: 2,
            slice_w: 0,
            slice_running_sum: 0.0,
            slice_active: false,
        };
        pt.rebuild_luts(sr);
        pt
    }

    fn rebuild_luts(&mut self, sr: f32) {
        let sr_decim = sr / (self.decim_factor as f32);
        let tau_min = ((sr_decim / self.fmax) as usize).max(2);
        let max_len = self.yin_win / 2;
        self.oct_bias_lut.resize(max_len, 0.0);
        for tau in tau_min..max_len {
            self.oct_bias_lut[tau] = 0.018 * ((tau as f32) / (tau_min as f32)).log2();
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        let (decim_factor, yin_win, yin_hop) = yin_params_for_sr(sr);
        self.decim_factor = decim_factor;
        self.yin_win = yin_win;
        self.yin_hop = yin_hop;
        self.cmnd.resize(yin_win / 2, 1.0);
        self.decim_buf.resize(yin_win, 0.0);
        self.rebuild_luts(sr);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.cmnd.fill(1.0);
        self.current_f0 = 0.0;
        self.current_midi = 60.0;
        self.current_note = -1;
        self.voiced = false;
        self.unvoiced_hold = 0;
        self.slice_active = false;
    }

    pub fn begin_sliced_frame(&mut self, ring: &[f32], write_pos: usize, sr: f32) {
        let n_decim = self.yin_win;
        let d = self.decim_factor;
        let sr_decim = sr / (d as f32);
        let tau_min = ((sr_decim / self.fmax) as usize).max(2);
        let tau_max = ((sr_decim / self.fmin) as usize).min(n_decim / 2 - 2);
        let w = n_decim - tau_max;
        let start = write_pos.wrapping_sub(n_decim * d);

        let mut sum_sq = 0.0_f32;
        for i in 0..n_decim {
            let c = start.wrapping_add(i * d);
            let s = 0.25 * ring[c.wrapping_sub(1) & RING_MASK]
                + 0.5 * ring[c & RING_MASK]
                + 0.25 * ring[c.wrapping_add(1) & RING_MASK];
            self.decim_buf[i] = s;
            if i < w { sum_sq += s * s; }
        }
        if (sum_sq / (w as f32)).sqrt() < 0.0015 {
            self.slice_active = false;
            self.mark_unvoiced();
            return;
        }

        self.cmnd[0] = 1.0;
        self.slice_tau_min = tau_min;
        self.slice_tau_max = tau_max;
        self.slice_w = w;
        self.slice_tau_next = 1;
        self.slice_running_sum = 0.0;
        self.slice_active = true;
    }

    #[inline]
    pub fn step_time_slice(&mut self, sr: f32, tune_hz: f32) -> bool {
        if !self.slice_active { return false; }
        let taus_per_sample = ((self.slice_tau_max / self.yin_hop.max(1)) + 2).max(2);
        let end_tau = (self.slice_tau_next + taus_per_sample - 1).min(self.slice_tau_max);
        let w = self.slice_w;
        let base_slice = &self.decim_buf[0..w];

        for tau in self.slice_tau_next..=end_tau {
            let shifted_slice = &self.decim_buf[tau..tau + w];
            let diff = sq_diff_simd(base_slice, shifted_slice);
            self.slice_running_sum += diff;
            self.cmnd[tau] = if self.slice_running_sum > 1e-12 {
                diff * (tau as f32) / self.slice_running_sum
            } else {
                1.0
            };
        }

        self.slice_tau_next = end_tau + 1;
        if self.slice_tau_next > self.slice_tau_max {
            self.slice_active = false;
            self.finalize_frame(sr, tune_hz);
            return true;
        }
        false
    }

    fn finalize_frame(&mut self, sr: f32, tune_hz: f32) {
        let sr_decim = sr / (self.decim_factor as f32);
        let tau_min = self.slice_tau_min;
        let tau_max = self.slice_tau_max;

        let mut best_tau = 0usize;
        let mut best_cost = 1e9_f32;
        let prev_f0 = self.current_f0;
        let was_voiced = self.voiced && prev_f0 > 0.0;
        let active_thresh = if was_voiced { self.threshold + 0.09 } else { self.threshold };

        for tau in tau_min..tau_max {
            let c = self.cmnd[tau];
            if c < active_thresh && c < self.cmnd[tau - 1] && c <= self.cmnd[tau + 1] {
                let oct_bias = self.oct_bias_lut[tau];
                let jump_cost = if was_voiced {
                    let cand_f0 = sr_decim / (tau as f32);
                    0.04 * (12.0 * (cand_f0 / prev_f0).log2()).abs().min(12.0)
                } else { 0.0 };
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
        let frac = if den.abs() > 1e-9 { 0.5 * (y0 - y2) / den } else { 0.0 };
        let refined_tau = (best_tau as f32 + frac.clamp(-0.5, 0.5)).max(1.0);
        let f0 = sr_decim / refined_tau;
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

    pub fn analyze_frame(&mut self, ring: &[f32], write_pos: usize, sr: f32, tune_hz: f32) {
        self.begin_sliced_frame(ring, write_pos, sr);
        while self.slice_active {
            self.step_time_slice(sr, tune_hz);
        }
    }

    fn mark_unvoiced(&mut self) {
        self.unvoiced_hold += 1;
        if self.unvoiced_hold >= 6 {
            self.voiced = false;
            self.current_f0 = 0.0;
            self.current_note = -1;
        }
    }
}

/// Cache de amostras brutas do periodo glotal para compartilhamento entre Main e Dual (sem janela dupla!)
#[derive(Clone, Debug)]
pub struct SharedGrainCache {
    pub cached_mark: usize,
    pub cached_reach: isize,
    pub buf: Vec<f32>,
}

impl Default for SharedGrainCache {
    fn default() -> Self { Self::new() }
}

impl SharedGrainCache {
    pub fn new() -> Self {
        Self { cached_mark: usize::MAX, cached_reach: 0, buf: vec![0.0; 3073] }
    }

    pub fn reset(&mut self) {
        self.cached_mark = usize::MAX;
        self.cached_reach = 0;
    }

    #[inline]
    pub fn ensure_extracted(&mut self, dry_ring: &[f32], safe_mark: usize, reach: isize) {
        let r = reach.clamp(16, 1500);
        if self.cached_mark == safe_mark && self.cached_reach == r { return; }
        self.cached_mark = safe_mark;
        self.cached_reach = r;

        let len = (2 * r + 1) as usize;
        for idx in 0..len {
            let offset = idx as isize - r;
            let p = (safe_mark as isize + offset) as usize & RING_MASK;
            self.buf[idx] = dry_ring[p];
        }
    }
}

#[derive(Clone, Debug)]
pub struct PsolaSubVoice {
    out_w_ring: Vec<[f32; 2]>,
    delay_line: Vec<f32>,
    delay_write: usize,
    synth_phase: f32,
    drift_cents: f32,
    delay_rand_norm: f32,
    delay_mod_samples: f32,
    rng_pitch: FastRng,
    rng_delay: FastRng,
    hp_x1: f32,
    hp_y1: f32,
    dac_lp_z1: f32,
    is_dual_warm: bool,
}

impl PsolaSubVoice {
    pub fn new(seed: u32) -> Self {
        Self {
            out_w_ring: vec![[0.0; 2]; VOICE_RING_SIZE],
            delay_line: vec![0.0; DELAY_SIZE],
            delay_write: 0,
            synth_phase: 0.0,
            drift_cents: 0.0,
            delay_rand_norm: 0.0,
            delay_mod_samples: 0.0,
            rng_pitch: FastRng::new(seed),
            rng_delay: FastRng::new(seed.wrapping_mul(0x85EBCA6B).wrapping_add(17)),
            hp_x1: 0.0,
            hp_y1: 0.0,
            dac_lp_z1: 0.0,
            is_dual_warm: false,
        }
    }

    pub fn with_dual_warmth(mut self) -> Self {
        self.is_dual_warm = true;
        self
    }

    pub fn reset(&mut self) {
        self.out_w_ring.fill([0.0; 2]);
        self.delay_line.fill(0.0);
        self.synth_phase = 0.0;
        self.drift_cents = 0.0;
        self.delay_rand_norm = 0.0;
        self.delay_mod_samples = 0.0;
        self.hp_x1 = 0.0;
        self.hp_y1 = 0.0;
        self.dac_lp_z1 = 0.0;
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn step_sample(
        &mut self,
        dry_ring: &[f32],
        hann_lut: &[f32],
        write_pos: usize,
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
        max_grain: usize,
    ) -> f32 {
        self.step_sample_with_cache(
            dry_ring, hann_lut, None, 0.0,
            write_pos, read_pos, emit_pos, latest_mark,
            voiced_mask, emit_mask, f0_hz, pitch_ratio, formant_ratio,
            random_norm, extra_cents, extra_delay_ms, unvoiced_gain, sr, max_grain,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn step_sample_with_cache(
        &mut self,
        dry_ring: &[f32],
        hann_lut: &[f32],
        mut shared_cache: Option<&mut SharedGrainCache>,
        sibilance_factor: f32,
        write_pos: usize,
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
        max_grain: usize,
    ) -> f32 {
        let n_pitch = self.rng_pitch.next_bipolar();
        let n_delay = self.rng_delay.next_bipolar();

        self.drift_cents =
            OU_A * self.drift_cents + OU_ONE_MINUS_A_TIMES_K * n_pitch * (9.0 * random_norm);
        self.delay_rand_norm =
            OU_A * self.delay_rand_norm + OU_ONE_MINUS_A_TIMES_K * n_delay;

        let jitter_ms = (self.delay_rand_norm * 0.5 + 0.5).clamp(0.0, 1.5) * (16.0 * random_norm);
        let target_delay_ms = jitter_ms + extra_delay_ms;
        let target_delay_samp =
            (target_delay_ms * 0.001 * sr).clamp(0.0, (DELAY_SIZE - 16) as f32);
        self.delay_mod_samples = 0.999 * self.delay_mod_samples + 0.001 * target_delay_samp;

        let ring_cap = max_grain.min(VOICE_RING_SIZE / 2);
        let active_out_delay = read_pos.wrapping_sub(emit_pos).clamp(64, ring_cap / 2);
        let max_safe_grain = ((active_out_delay * 2).saturating_sub(8) | 1).min(ring_cap - 1);

        if voiced_mask > 0.01 && f0_hz > 40.0 {
            let cents_oct = (self.drift_cents + extra_cents) * (1.0 / 1200.0);
            let cents_factor = fast_exp2_small(cents_oct);
            let ratio = (pitch_ratio * cents_factor).clamp(0.25, 4.0);
            // `ta` e desacoplado de `max_safe_grain`: suporta fundamentais graves ate ~42 Hz mesmo em modo Live!
            let max_ta = (1050.0 * (sr / 44_100.0)).clamp(512.0, 4096.0);
            let ta = (sr / f0_hz).clamp(20.0, max_ta);
            let ts = (ta / ratio).clamp(12.0, max_ta * 2.0);

            self.synth_phase -= 1.0;
            if self.synth_phase <= 0.0 {
                self.synth_phase += ts;
                let grain_len = ((2.0 * ta).max(PSOLA_SYNTH_WINDOW_FACTOR * ts) as usize | 1)
                    .clamp(31, max_safe_grain);
                self.splat_grain_interleaved(
                    dry_ring,
                    hann_lut,
                    shared_cache.as_deref_mut(),
                    write_pos,
                    latest_mark,
                    read_pos,
                    grain_len,
                    ta,
                    formant_ratio,
                    max_safe_grain,
                );
            }
        } else {
            self.synth_phase = 0.0;
        }

        let v_idx = emit_pos & VOICE_RING_MASK;
        let pair = self.out_w_ring[v_idx];
        self.out_w_ring[v_idx] = [0.0, 0.0];

        let wet_nola = pair[0] / pair[1].max(0.25);

        // Quantizacao simetrica pura de 16 bits do barramento ADSP-2186MKST-300 (IC1) pos-NOLA
        let wet_adsp2186 = if emit_mask > 0.01 {
            let mac40 = wet_nola.clamp(-3.99, 3.99);
            (mac40 * 32_768.0).round() * (1.0 / 32_768.0)
        } else {
            wet_nola
        };

        let sib_att = if pitch_ratio > 1.15 { 1.0 - 0.65 * sibilance_factor } else { 1.0 };
        let dry = dry_ring[emit_pos & RING_MASK];
        let nat_sib_blend = if pitch_ratio > 1.15 { 0.22 * sibilance_factor * emit_mask } else { 0.0 };
        let combined = wet_adsp2186 * emit_mask * sib_att
            + dry * ((1.0 - emit_mask) * unvoiced_gain + nat_sib_blend);

        // 1. Filtro Passa-Altas Subsonico Akai HV10 (32.7848 Hz)
        let hp_r = (-2.0 * PI * AKAI_SUBSONIC_HP_HZ / sr).exp();
        let hp_out = hp_r * (self.hp_y1 + combined - self.hp_x1);
        self.hp_x1 = combined;
        self.hp_y1 = hp_out;

        // 2. Filtro DAC por voz (14.5 kHz Main / 9.0 kHz Dual & Unison Warmth)
        let lp_fc = if self.is_dual_warm { AKAI_DUAL_WARM_LP_HZ } else { AKAI_MAIN_VOICE_LP_HZ };
        let lp_a = (-2.0 * PI * lp_fc.min(sr * 0.45) / sr).exp();
        let dac_out = (1.0 - lp_a) * hp_out + lp_a * self.dac_lp_z1;
        self.dac_lp_z1 = dac_out;

        let w = self.delay_write & DELAY_MASK;
        self.delay_line[w] = dac_out;

        let read_f = (w + DELAY_SIZE) as f32 - self.delay_mod_samples;
        let r0i = (read_f as usize) & DELAY_MASK;
        let r1i = (r0i + 1) & DELAY_MASK;
        let frac = read_f.fract();
        let delayed = self.delay_line[r0i] * (1.0 - frac) + self.delay_line[r1i] * frac;

        self.delay_write = self.delay_write.wrapping_add(1);
        delayed
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn splat_grain_interleaved(
        &mut self,
        dry_ring: &[f32],
        hann_lut: &[f32],
        shared_cache: Option<&mut SharedGrainCache>,
        write_pos: usize,
        mark_center: usize,
        out_center: usize,
        l_base: usize,
        ta: f32,
        formant_ratio: f32,
        max_safe_grain: usize,
    ) {
        let fr = formant_ratio.clamp(0.5, 2.0);
        let l_Desired = (((l_base as f32) / fr).round() as usize | 1).clamp(17, max_safe_grain);
        // Garante que a leitura na fonte nunca ultrapasse 1.35 * ta e que a janela de Hann
        // seja calculada exatamente sobre o comprimento efetivo (zero bordas mudas com peso Hann!)
        let max_src_reach = (ta * 1.35).clamp(24.0, 1480.0);
        let max_l_by_src = (((2.0 * max_src_reach) / fr).floor() as usize | 1).max(17);
        let l_out = l_Desired.min(max_l_by_src).min(max_safe_grain);
        let half_out = (l_out / 2) as isize;

        let actual_src_reach = ((half_out as f32) * fr).ceil() as isize + 2;
        let min_dist_from_write = (actual_src_reach as usize) + 4;
        let dist_to_write = write_pos.wrapping_sub(mark_center);
        let safe_mark = if dist_to_write < min_dist_from_write {
            write_pos.wrapping_sub(min_dist_from_write)
        } else {
            mark_center
        };

        let lut_step = ((HANN_LUT_SIZE << 16) / l_out).max(1);
        let mut lut_phase = 0usize;

        if let Some(cache) = shared_cache {
            cache.ensure_extracted(dry_ring, safe_mark, actual_src_reach);
            let rad_f = cache.cached_reach as f32;
            let cache_slice = &cache.buf;

            for k in 0..l_out {
                let offset_out = k as isize - half_out;
                let src_off = (offset_out as f32) * fr;
                let pos = (src_off + rad_f).clamp(0.0, (2 * cache.cached_reach - 1) as f32);
                let i0 = pos.floor() as usize;
                let frac = pos - (i0 as f32);
                let sample = cache_slice[i0] + frac * (cache_slice[i0 + 1] - cache_slice[i0]);

                let w = hann_lut[(lut_phase >> 16) & (HANN_LUT_SIZE - 1)];
                lut_phase += lut_step;

                let dst = (out_center as isize + offset_out) as usize & VOICE_RING_MASK;
                self.out_w_ring[dst][0] += sample * w;
                self.out_w_ring[dst][1] += w;
            }
        } else {
            for k in 0..l_out {
                let offset_out = k as isize - half_out;
                let src_off = (offset_out as f32) * fr;
                let src_i0 = src_off.floor() as isize;
                let frac = src_off - (src_i0 as f32);
                let p0 = (safe_mark as isize + src_i0) as usize & RING_MASK;
                let p1 = (p0 + 1) & RING_MASK;
                let sample = dry_ring[p0] + frac * (dry_ring[p1] - dry_ring[p0]);

                let w = hann_lut[(lut_phase >> 16) & (HANN_LUT_SIZE - 1)];
                lut_phase += lut_step;

                let dst = (out_center as isize + offset_out) as usize & VOICE_RING_MASK;
                self.out_w_ring[dst][0] += sample * w;
                self.out_w_ring[dst][1] += w;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct HarmonyChannelDsp {
    pub main: PsolaSubVoice,
    pub dual: PsolaSubVoice,
    pub shared_grain: SharedGrainCache,
}

impl HarmonyChannelDsp {
    pub fn new(seed_base: u32) -> Self {
        Self {
            main: PsolaSubVoice::new(seed_base),
            dual: PsolaSubVoice::new(seed_base.wrapping_add(0x9E3779B9)).with_dual_warmth(),
            shared_grain: SharedGrainCache::new(),
        }
    }

    pub fn reset(&mut self) {
        self.main.reset();
        self.dual.reset();
        self.shared_grain.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psola_ring_no_ghost_echo_and_clean_low_octave() {
        let _ftz = ScopedFtzDaz::new();
        let sr = 44_100.0;
        let hann_lut = build_hann_lut();
        let mut voice = PsolaSubVoice::new(1);
        let mut dry_ring = vec![0.0_f32; RING_SIZE];
        let mut write_pos = 2048usize;
        // Testa nota grave masculina (65 Hz = C2) no modo Live (out_delay = 384) em Low Octave (0.5)
        let f0 = 65.0_f32;
        let period = (sr / f0) as usize;
        let mut latest_mark = write_pos - 384;
        let max_grain = max_grain_for_sr(sr);

        let mut silence_rms_acc = 0.0_f32;
        let mut silence_count = 0usize;
        let mut active_energy = 0.0_f32;

        for step in 0..16_000 {
            let x = if step < 6000 { (2.0 * PI * f0 * (step as f32) / sr).sin() * 0.8 } else { 0.0 };
            let voiced = if step < 6000 { 1.0 } else { 0.0 };
            dry_ring[write_pos & RING_MASK] = x;

            let read_pos = write_pos.wrapping_sub(384);
            let emit_pos = read_pos.wrapping_sub(384);
            if step < 6000 && read_pos.wrapping_sub(latest_mark) >= period {
                latest_mark = read_pos;
            }
            let emit_voiced = if step >= 768 && step < 6000 + 768 { 1.0 } else { 0.0 };

            let y = voice.step_sample(
                &dry_ring, &hann_lut, write_pos, read_pos, emit_pos, latest_mark,
                voiced, emit_voiced, f0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, sr, max_grain,
            );

            if step > 2000 && step < 5000 {
                active_energy += y * y;
            }
            if step > 10_000 {
                silence_rms_acc += y * y;
                silence_count += 1;
            }
            write_pos = write_pos.wrapping_add(1);
        }

        let active_rms = (active_energy / 3000.0).sqrt();
        let ghost_rms = (silence_rms_acc / silence_count as f32).sqrt();
        assert!(active_rms > 0.25, "Low Octave em 65 Hz deve manter nivel forte sem dips: RMS = {}", active_rms);
        assert!(ghost_rms < 1e-6, "Ruido residual detectado no anel PSOLA: RMS = {}", ghost_rms);
    }

    #[test]
    fn test_direct_path_is_transparent_at_10khz() {
        let sr = 44_100.0;
        let mut proc = super::super::DecaBuddyProcessor::new(sr);
        proc.params.direct.enabled = true;
        proc.params.direct.level_db = 0.0;
        proc.params.direct.pan = 0.0;
        for h in &mut proc.params.harmonies {
            h.enabled = false;
        }
        proc.params.unison.enabled = false;

        let n = 4096;
        let mut input = vec![0.0_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];
        for i in 0..n {
            input[i] = (2.0 * PI * 10_000.0 * (i as f32) / sr).sin() * 0.5;
        }
        proc.process_block(&input, &mut out_l, &mut out_r);

        // No centro (pan = 0), ganho = cos(pi/4) = 0.70710678. A soma L + R deve recuperar ~1.4142 * input
        let mut in_sq = 0.0_f32;
        let mut out_sq = 0.0_f32;
        for i in 2048..n {
            in_sq += input[i] * input[i];
            let mono_sum = (out_l[i] + out_r[i]) * std::f32::consts::FRAC_1_SQRT_2;
            out_sq += mono_sum * mono_sum;
        }
        let gain_10k_db = 10.0 * (out_sq / in_sq).log10();
        assert!(
            gain_10k_db.abs() < 0.05,
            "O caminho Direct deve ser plano em 10 kHz (desvio = {:.4} dB)",
            gain_10k_db
        );
    }
}
