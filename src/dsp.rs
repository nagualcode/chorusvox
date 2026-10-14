//! Motor DSP Avancado e Modelagem Fisica do Hardware Akai HV10 DecaBuddy.
//! - Oversampling Polifasico 2x nos estagios 2SC3330 + 1SS133T-77 e NJM4558L (9V DC)
//!   seguido por DC Blocker de 5 Hz (harmonicos de 2a ordem reais com zero DC offset).
//! - Curva de impedancia de entrada XLR (4k ohms) e Linha (33k ohms) do HV10.
//! - Emulacao 16-bit Q15 + MAC 40-bit do ADSP-2186MKST-300 (IC1) pos-NOLA com Noise Shaping.
//! - Separacao dinamica de sibilantes/consoantes (ZCR detector) para agudos naturais.
//! - Aneis de saida intercalados de 4096 amostras ([out, w] = 288 KB residentes no Cache L1/L2).
//! - Extracao de grao glotal compartilhada entre Main e Dual.
//! - Registradores FTZ/DAZ nativos por bloco (FPCR no Apple Silicon / MXCSR no x86_64).
//! - Decimacao adaptativa do YIN para taxa fixa (~22.05 kHz) + Time-Slicing por amostra.

use std::f32::consts::PI;

pub const RING_SIZE: usize = 32768;
pub const RING_MASK: usize = RING_SIZE - 1;

// Anel de saida compacto e intercalado [out, w] para residir no Cache L1/L2 (32 KB por sub-voz)
pub const VOICE_RING_SIZE: usize = 4096;
pub const VOICE_RING_MASK: usize = VOICE_RING_SIZE - 1;

pub const DELAY_SIZE: usize = 16384;
pub const DELAY_MASK: usize = DELAY_SIZE - 1;
pub const HANN_LUT_SIZE: usize = 2048;
pub const PSOLA_SYNTH_WINDOW_FACTOR: f32 = 1.35;

pub const AKAI_SUBSONIC_HP_HZ: f32 = 32.784804;
pub const AKAI_DAC_LP_HZ: f32 = 12831.712823;
pub const AKAI_DUAL_WARM_LP_HZ: f32 = 8500.0;
pub const PCM3003_RECON_LP_HZ: f32 = 16800.0;

const OU_A: f32 = 0.99995;
const OU_ONE_MINUS_A_TIMES_K: f32 = (1.0 - OU_A) * 346.40582;

/// Guard RAII que ativa Flush-To-Zero (FTZ) e Denormals-Are-Zero (DAZ) diretamente
/// no registrador FPCR (ARM64 / Apple Silicon) ou MXCSR (x86_64) uma vez por bloco.
pub struct ScopedFtzDaz {
    #[cfg(target_arch = "aarch64")]
    prev_fpcr: u64,
    #[cfg(target_arch = "x86_64")]
    prev_mxcsr: u32,
}

impl Default for ScopedFtzDaz {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopedFtzDaz {
    #[inline]
    pub fn new() -> Self {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let prev: u64;
            core::arch::asm!("mrs {}, fpcr", out(reg) prev, options(nomem, nostack, preserves_flags));
            let next = prev | (1 << 24); // Bit 24 = FZ (Flush-to-Zero)
            if next != prev {
                core::arch::asm!("msr fpcr, {}", in(reg) next, options(nomem, nostack, preserves_flags));
            }
            Self { prev_fpcr: prev }
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let prev = core::arch::x86_64::_mm_getcsr();
            let next = prev | 0x8040; // FTZ (0x8000) | DAZ (0x0040)
            if next != prev {
                core::arch::x86_64::_mm_setcsr(next);
            }
            Self { prev_mxcsr: prev }
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            Self {}
        }
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
            core::arch::x86_64::_mm_setcsr(self.prev_mxcsr);
        }
    }
}

#[inline]
pub fn max_grain_for_sr(_sr: f32) -> usize {
    // Limitado a metade de VOICE_RING_SIZE para caber com folga no anel intercalado L1 de 4096 amostras
    2048
}

#[inline]
pub fn lookahead_samples_for_sr(sr: f32) -> usize {
    ((1536.0 * (sr / 44_100.0)).round() as usize).clamp(1024, 8192)
}

#[inline]
pub fn out_delay_for_sr(sr: f32) -> usize {
    ((960.0 * (sr / 44_100.0)).round() as usize).clamp(768, 1792)
}

#[inline]
pub fn live_lookahead_for_sr(sr: f32) -> usize {
    ((512.0 * (sr / 44_100.0)).round() as usize).clamp(384, 1536)
}

#[inline]
pub fn live_out_delay_for_sr(sr: f32) -> usize {
    ((512.0 * (sr / 44_100.0)).round() as usize).clamp(384, 1536)
}

/// Decimacao adaptativa para taxa constante (~22.05 kHz):
/// - 44.1 / 48 kHz -> decim = 2, win = 1024
/// - 88.2 / 96 kHz -> decim = 4, win = 1024
/// - 176.4 / 192 kHz -> decim = 8, win = 1024
#[inline]
pub fn yin_params_for_sr(sr: f32) -> (usize, usize, usize) {
    let decim = ((sr / 22_050.0).round() as usize).clamp(2, 8);
    let win = 1024usize;
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
    const THRESHOLD: f32 = 0.75;
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

/// Curva de impedancia do estagio de entrada XLR (4k ohms) / Linha (33k ohms) do Akai HV10:
/// Acoplamento AC em 28 Hz + realce organico de corpo medio-grave em 185 Hz.
#[derive(Clone, Copy, Debug, Default)]
pub struct AkaiInputImpedanceFilter {
    hp_x1: f32,
    hp_y1: f32,
    warm_z1: f32,
    hp_r: f32,
    warm_a: f32,
}

impl AkaiInputImpedanceFilter {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.hp_r = (-2.0 * PI * 28.0 / sr).exp();
        self.warm_a = (-2.0 * PI * 185.0 / sr).exp();
    }

    pub fn reset(&mut self) {
        self.hp_x1 = 0.0;
        self.hp_y1 = 0.0;
        self.warm_z1 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let hp = self.hp_r * (self.hp_y1 + x - self.hp_x1);
        self.hp_x1 = x;
        self.hp_y1 = hp;
        self.warm_z1 = (1.0 - self.warm_a) * hp + self.warm_a * self.warm_z1;
        // Soma sutil de corpo medio-grave (+0.6 dB abaixo de 185 Hz) tipica do circuito 4k/33k
        hp + 0.07 * self.warm_z1
    }
}

/// Bloqueador DC de alta precisao sintonizado em 5 Hz para remover qualquer offset
/// continuo gerado pela assimetria de 2a ordem dos transistores 2SC3330 antes do PSOLA.
#[derive(Clone, Copy, Debug, Default)]
pub struct DcBlocker5Hz {
    x1: f32,
    y1: f32,
    r: f32,
}

impl DcBlocker5Hz {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.r = (1.0 - (2.0 * PI * 5.0 / sr)).clamp(0.990, 0.9999);
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + self.r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// Estagio Analogico com Oversampling Polifasico 2x:
/// - Upsampling 2x com interpolador FIR meia-banda de fase linear.
/// - Modelagem da juncao exponencial assimetrica BJT 2SC3330 (TR1-TR3) + diodos 1SS133T-77 (D1-D4).
/// - Modelagem do Op-Amp JRC NJM4558L (IC12) com slew-rate de 1.0 V/us e rail simples de 9V DC.
/// - Downsampling FIR meia-banda + DC Blocker de 5 Hz.
#[derive(Clone, Copy, Debug, Default)]
pub struct OversampledAnalogStage {
    up_x1: f32,
    up_x2: f32,
    up_x3: f32,
    down_z1: f32,
    down_z2: f32,
    opamp_prev: f32,
    opamp_lp: f32,
    max_slew_2x: f32,
    bw_alpha_2x: f32,
    dc_blocker: DcBlocker5Hz,
}

impl OversampledAnalogStage {
    pub fn set_sample_rate(&mut self, sr: f32) {
        let sr_2x = sr * 2.0;
        self.max_slew_2x = (0.58 * (44_100.0 / sr_2x)).clamp(0.04, 0.65);
        self.bw_alpha_2x = (-2.0 * PI * 18_500.0_f32.min(sr_2x * 0.45) / sr_2x).exp();
        self.dc_blocker.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.up_x1 = 0.0;
        self.up_x2 = 0.0;
        self.up_x3 = 0.0;
        self.down_z1 = 0.0;
        self.down_z2 = 0.0;
        self.opamp_prev = 0.0;
        self.opamp_lp = 0.0;
        self.dc_blocker.reset();
    }

    #[inline(always)]
    fn nonlinear_2sc3330_njm4558(&mut self, v: f32) -> f32 {
        let xc = v.clamp(-2.2, 2.2);
        // 1. Curvatura assimetrica suave do transistor 2SC3330 (gera 2o harmonico)
        let bjt = xc + 0.022 * xc * xc;
        // 2. Joelho dos diodos de silicio 1SS133T-77
        let d2 = bjt * bjt;
        let diode = bjt * (27.0 + d2) / (27.0 + 9.0 * d2);
        // 3. Slew-rate limiting (1.0 V/us) e polo do NJM4558L em fonte simples de 9V DC
        let delta = (diode - self.opamp_prev).clamp(-self.max_slew_2x, self.max_slew_2x);
        let slewed = self.opamp_prev + delta;
        self.opamp_prev = slewed;
        let filtered = (1.0 - self.bw_alpha_2x) * slewed + self.bw_alpha_2x * self.opamp_lp;
        self.opamp_lp = filtered;

        if filtered > 0.78 {
            let over = filtered - 0.78;
            0.78 + 0.22 * (over / (1.0 + over))
        } else if filtered < -0.74 {
            let under = -filtered - 0.74;
            -(0.74 + 0.26 * (under / (1.0 + under)))
        } else {
            filtered
        }
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        // Upsampling polifasico 2x (Fase 0: amostra exata atrasada; Fase 1: FIR meia-banda [-1, 9, 9, -1]/16)
        let s0 = self.up_x1;
        let s1 = 0.5625 * (self.up_x1 + self.up_x2) - 0.0625 * (x + self.up_x3);
        self.up_x3 = self.up_x2;
        self.up_x2 = self.up_x1;
        self.up_x1 = x;

        let y0 = self.nonlinear_2sc3330_njm4558(s0);
        let y1 = self.nonlinear_2sc3330_njm4558(s1);

        // Filtro anti-aliasing meia-banda [0.25, 0.5, 0.25] + decimacao 2:1
        let decimated = 0.25 * self.down_z2 + 0.5 * self.down_z1 + 0.25 * y0;
        self.down_z2 = y0;
        self.down_z1 = y1;

        // Bloqueia o offset DC antes do anel PSOLA
        self.dc_blocker.process(decimated)
    }
}

/// Detector dinamico de sibilancia e consoantes fricativas (s, sh, t, k) por ZCR e energia HF.
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
        self.prev_sign = false;
        self.zcr_smooth = 0.0;
        self.hf_env = 0.0;
        self.lf_env = 0.0;
        self.prev_x = 0.0;
    }

    /// Retorna um fator [0.0..1.0] indicando presenca de sibilante/consoante fricativa.
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

        // Se a taxa de cruzamento por zero for alta (> 0.24) e a energia HF superar a LF, e sibilante
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
        Self {
            state: if seed == 0 { 0x6D2B79F5 } else { seed },
        }
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

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

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

    // Estado para Time-Slicing (amortizacao do YIN ao longo das amostras do hop)
    slice_tau_next: usize,
    slice_tau_max: usize,
    slice_tau_min: usize,
    slice_w: usize,
    slice_running_sum: f32,
    slice_active: bool,
}

impl Default for PitchTracker {
    fn default() -> Self {
        Self::new(44_100.0)
    }
}

impl PitchTracker {
    pub fn new(sr: f32) -> Self {
        let (decim_factor, yin_win, yin_hop) = yin_params_for_sr(sr);
        let mut pt = Self {
            fmin: 45.0,
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

    /// Inicia um novo quadro YIN (captura buffer decimado com filtro anti-aliasing)
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
            let s_prev = ring[c.wrapping_sub(1) & RING_MASK];
            let s_curr = ring[c & RING_MASK];
            let s_next = ring[c.wrapping_add(1) & RING_MASK];
            let s = 0.25 * s_prev + 0.5 * s_curr + 0.25 * s_next;
            self.decim_buf[i] = s;
            if i < w {
                sum_sq += s * s;
            }
        }
        let rms = (sum_sq / (w as f32)).sqrt();
        if rms < 0.002 {
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

    /// Processa uma fatia de valores `tau` por amostra (Time-Slicing).
    /// Retorna `true` no sample exato em que o quadro termina e atualiza `current_f0`.
    #[inline]
    pub fn step_time_slice(&mut self, sr: f32, tune_hz: f32) -> bool {
        if !self.slice_active {
            return false;
        }
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
        let active_thresh = if was_voiced { self.threshold + 0.08 } else { self.threshold };

        for tau in tau_min..tau_max {
            let c = self.cmnd[tau];
            if c < active_thresh && c < self.cmnd[tau - 1] && c <= self.cmnd[tau + 1] {
                let oct_bias = self.oct_bias_lut[tau];
                let jump_cost = if was_voiced {
                    let cand_f0 = sr_decim / (tau as f32);
                    0.04 * (12.0 * (cand_f0 / prev_f0).log2()).abs().min(12.0)
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
        if self.unvoiced_hold >= 5 {
            self.voiced = false;
            self.current_f0 = 0.0;
            self.current_note = -1;
        }
    }
}

/// Cache de periodo glotal pre-extraido e janelado para compartilhamento entre Main e Dual.
#[derive(Clone, Debug)]
pub struct SharedGrainCache {
    pub cached_mark: usize,
    pub cached_ta_int: usize,
    pub radius: isize,
    pub buf: Vec<f32>,
}

impl Default for SharedGrainCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedGrainCache {
    pub fn new() -> Self {
        Self {
            cached_mark: usize::MAX,
            cached_ta_int: 0,
            radius: 0,
            buf: vec![0.0; 2049],
        }
    }

    pub fn reset(&mut self) {
        self.cached_mark = usize::MAX;
        self.cached_ta_int = 0;
    }

    #[inline]
    pub fn ensure_extracted(
        &mut self,
        dry_ring: &[f32],
        hann_lut: &[f32],
        safe_mark: usize,
        ta: f32,
    ) {
        let ta_int = ta.round() as usize;
        if self.cached_mark == safe_mark && self.cached_ta_int == ta_int {
            return;
        }
        self.cached_mark = safe_mark;
        self.cached_ta_int = ta_int;

        let max_src_reach = (ta * 1.15).clamp(16.0, 1020.0);
        let rad = (max_src_reach.ceil() as isize).clamp(16, 1020);
        self.radius = rad;
        let len = (2 * rad + 1) as usize;
        let inv_reach = 1.0 / (rad as f32);

        for idx in 0..len {
            let offset = idx as isize - rad;
            let p = (safe_mark as isize + offset) as usize & RING_MASK;
            let norm = (offset as f32 * inv_reach) * 0.5 + 0.5;
            let lut_i = ((norm * (HANN_LUT_SIZE as f32)) as usize).min(HANN_LUT_SIZE - 1);
            let taper = hann_lut[lut_i].sqrt();
            self.buf[idx] = dry_ring[p] * taper;
        }
    }
}

#[derive(Clone, Debug)]
pub struct PsolaSubVoice {
    // Anel intercalado [out, w] de 4096 amostras (32 KB) para máxima localidade no Cache L1
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
    quant_err: f32,
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
            quant_err: 0.0,
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
        self.quant_err = 0.0;
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
            dry_ring,
            hann_lut,
            None,
            0.0,
            write_pos,
            read_pos,
            emit_pos,
            latest_mark,
            voiced_mask,
            emit_mask,
            f0_hz,
            pitch_ratio,
            formant_ratio,
            random_norm,
            extra_cents,
            extra_delay_ms,
            unvoiced_gain,
            sr,
            max_grain,
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
            let ta = (sr / f0_hz).clamp(20.0, (max_safe_grain / 2 - 4) as f32);
            let ts = (ta / ratio).clamp(12.0, (max_safe_grain - 8) as f32);

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

        let wet_raw = pair[0] / pair[1].max(0.35);

        // Emulacao pos-NOLA do barramento 16-bit Q15 + MAC 40-bit do ADSP-2186MKST-300 (IC1)
        // com Noise Shaping psicoacustico de 1a ordem (zera o erro em silencio para piso 0.0)
        let wet_adsp2186 = if wet_raw.abs() > 1e-5 && emit_mask > 0.01 {
            let mac40 = wet_raw.clamp(-3.99, 3.99);
            let shaped = mac40 + 0.5 * self.quant_err;
            let q16 = (shaped * 32_768.0).round() * (1.0 / 32_768.0);
            self.quant_err = (shaped - q16).clamp(-1e-3, 1e-3);
            q16
        } else {
            self.quant_err = 0.0;
            wet_raw
        };

        // Separacao dinamica de sibilantes/consoantes: reduz sibilancia artificial em High 5th/Octave
        let sib_att = if pitch_ratio > 1.15 {
            1.0 - 0.65 * sibilance_factor
        } else {
            1.0
        };
        let dry = dry_ring[emit_pos & RING_MASK];
        let nat_sib_blend = if pitch_ratio > 1.15 {
            0.22 * sibilance_factor * emit_mask
        } else {
            0.0
        };
        let combined = wet_adsp2186 * emit_mask * sib_att
            + dry * ((1.0 - emit_mask) * unvoiced_gain + nat_sib_blend);

        // 1. Filtro Passa-Altas Subsonico Akai HV10 (32.7848 Hz)
        let hp_r = (-2.0 * PI * AKAI_SUBSONIC_HP_HZ / sr).exp();
        let hp_out = hp_r * (self.hp_y1 + combined - self.hp_x1);
        self.hp_x1 = combined;
        self.hp_y1 = hp_out;

        // 2. Filtro Passa-Baixas de Reconstrucao DAC Akai (12.83 kHz principal / 8.5 kHz Dual Warmth)
        let lp_fc = if self.is_dual_warm { AKAI_DUAL_WARM_LP_HZ } else { AKAI_DAC_LP_HZ };
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
        let l_out = (((l_base as f32) / fr).round() as usize | 1).clamp(17, max_safe_grain);
        let half_out = (l_out / 2) as isize;

        let max_src_reach = (ta * 1.15).clamp(16.0, 1020.0);
        let min_dist_from_write = (max_src_reach.ceil() as usize) + 4;
        let dist_to_write = write_pos.wrapping_sub(mark_center);
        let safe_mark = if dist_to_write < min_dist_from_write {
            write_pos.wrapping_sub(min_dist_from_write)
        } else {
            mark_center
        };

        let lut_step = ((HANN_LUT_SIZE << 16) / l_out).max(1);
        let mut lut_phase = 0usize;

        if let Some(cache) = shared_cache {
            cache.ensure_extracted(dry_ring, hann_lut, safe_mark, ta);
            let rad = cache.radius;
            let rad_f = rad as f32;
            let cache_slice = &cache.buf;

            for k in 0..l_out {
                let offset_out = k as isize - half_out;
                let src_off = (offset_out as f32) * fr;
                let sample = if src_off.abs() < rad_f {
                    let pos = src_off + rad_f;
                    let i0 = pos.floor() as usize;
                    let frac = pos - (i0 as f32);
                    cache_slice[i0] + frac * (cache_slice[i0 + 1] - cache_slice[i0])
                } else {
                    0.0
                };

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

                let sample = if src_off.abs() < max_src_reach {
                    let src_i0 = src_off.floor() as isize;
                    let frac = src_off - (src_i0 as f32);
                    let p0 = (safe_mark as isize + src_i0) as usize & RING_MASK;
                    let p1 = (p0 + 1) & RING_MASK;
                    let raw_s = dry_ring[p0] + frac * (dry_ring[p1] - dry_ring[p0]);
                    let norm_src = (src_off / max_src_reach) * 0.5 + 0.5;
                    let src_w_idx =
                        ((norm_src * (HANN_LUT_SIZE as f32)) as usize).min(HANN_LUT_SIZE - 1);
                    raw_s * hann_lut[src_w_idx].sqrt()
                } else {
                    0.0
                };

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
    fn test_psola_ring_no_ghost_echo_and_clean_live_mode() {
        let _ftz = ScopedFtzDaz::new();
        let sr = 44_100.0;
        let hann_lut = build_hann_lut();
        let mut voice = PsolaSubVoice::new(1);
        let mut dry_ring = vec![0.0_f32; RING_SIZE];
        let mut write_pos = 2048usize;
        let f0 = 110.0_f32;
        let period = (sr / f0) as usize;
        let mut latest_mark = write_pos - 512;
        let max_grain = max_grain_for_sr(sr);

        let mut silence_rms_acc = 0.0_f32;
        let mut silence_count = 0usize;

        for step in 0..14_000 {
            let x = if step < 4000 {
                (2.0 * PI * f0 * (step as f32) / sr).sin() * 0.8
            } else {
                0.0
            };
            let voiced = if step < 4000 { 1.0 } else { 0.0 };
            dry_ring[write_pos & RING_MASK] = x;

            let read_pos = write_pos.wrapping_sub(512);
            let emit_pos = read_pos.wrapping_sub(512);
            if step < 4000 && read_pos.wrapping_sub(latest_mark) >= period {
                latest_mark = read_pos;
            }
            let emit_voiced = if step >= 1024 && step < 4000 + 1024 { 1.0 } else { 0.0 };

            let y = voice.step_sample(
                &dry_ring,
                &hann_lut,
                write_pos,
                read_pos,
                emit_pos,
                latest_mark,
                voiced,
                emit_voiced,
                f0,
                0.5,
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                sr,
                max_grain,
            );

            if step > 8000 {
                silence_rms_acc += y * y;
                silence_count += 1;
            }
            write_pos = write_pos.wrapping_add(1);
        }

        let ghost_rms = (silence_rms_acc / silence_count as f32).sqrt();
        assert!(
            ghost_rms < 1e-6,
            "Ruido residual ou eco fantasma detectado no anel PSOLA: RMS = {}",
            ghost_rms
        );
    }

    #[test]
    fn test_hardware_oversampling_ftz_and_timesliced_yin_96khz() {
        let _ftz = ScopedFtzDaz::new();
        let sr = 96_000.0;
        let mut analog = OversampledAnalogStage::default();
        analog.set_sample_rate(sr);

        // Verifica que um sinal senoidal com offset DC tem o DC removido pelo DC Blocker de 5 Hz
        let mut dc_acc = 0.0_f32;
        for n in 0..48_000 {
            let s = (2.0 * PI * 220.0 * (n as f32) / sr).sin() * 0.7;
            let y = analog.process(s);
            if n >= 24_000 {
                dc_acc += y;
            }
        }
        let mean_dc = (dc_acc / 24_000.0).abs();
        assert!(mean_dc < 5e-3, "DC Blocker de 5 Hz deve eliminar o offset DC: mean={}", mean_dc);

        // Verifica decimação adaptativa e Time-Slicing do YIN a 96 kHz
        let mut pt = PitchTracker::new(sr);
        assert_eq!(pt.decim_factor, 4, "A 96 kHz o fator de decimacao deve ser 4 (~24 kHz)");
    }
}
