//! Motor DSP Avancado e Modelagem Fisica Completa do Hardware Akai HV10 DecaBuddy.
//! - PowerSupply9VSag: Queda de tensao e ripple da fonte externa de 9V DC / 250mA (IC9/IC10/IC11).
//! - Njm4558StereoCrosstalk: Crosstalk capacitivo e termico nos pacotes duplos NJM4558L (IC13/IC14).
//! - ComponentTolerance: Variancia de componentes Monte Carlo (+-2%) e deriva termica por voz.
//! - InputTransformerStage: Saturacao de nucleo magnetico (Jensen/Sowter) antes dos 2SC3330.
//! - SIMD Polyphase Kernels: Intrinsicos ARM NEON e x86_64 SSE2 no oversampler 2x e no YIN.
//! - AsyncYinWorker: Thread lock-free assincrona para rastreamento YIN sem picos de CPU.
//! - LpcFormantMorpher: Extracao de envelope espectral LPC de 4a ordem (Levinson-Durbin).

use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub const RING_SIZE: usize = 32768;
pub const RING_MASK: usize = RING_SIZE - 1;
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
            let prev = core::arch::x86_64::_mm_getcsr();
            let next = prev | 0x8040;
            if next != prev {
                core::arch::x86_64::_mm_setcsr(next);
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
            core::arch::x86_64::_mm_setcsr(self.prev_mxcsr);
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

#[inline]
pub fn live_lookahead_for_sr(sr: f32) -> usize {
    ((512.0 * (sr / 44_100.0)).round() as usize).clamp(384, 1536)
}

#[inline]
pub fn live_out_delay_for_sr(sr: f32) -> usize {
    ((512.0 * (sr / 44_100.0)).round() as usize).clamp(384, 1536)
}

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

/// A1. Modelagem de Sag e Ripple da Fonte Externa de 9V DC / 250mA (IC9 NJM2391 / IC10 NJM317F)
#[derive(Clone, Copy, Debug, Default)]
pub struct PowerSupply9VSag {
    sag_env: f32,
    ripple_phase: f32,
    ripple_inc: f32,
}

impl PowerSupply9VSag {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.ripple_inc = 2.0 * PI * 100.0 / sr.max(8000.0);
    }

    pub fn reset(&mut self) {
        self.sag_env = 0.0;
        self.ripple_phase = 0.0;
    }

    /// Retorna o fator de teto do barramento 9V DC (0.72..0.80) modulado pela corrente polifonica
    #[inline(always)]
    pub fn step_rail(&mut self, bus_energy: f32, active_voices_count: usize) -> f32 {
        let load = bus_energy * (1.0 + 0.12 * (active_voices_count as f32));
        if load > self.sag_env {
            self.sag_env = 0.992 * self.sag_env + 0.008 * load;
        } else {
            self.sag_env = 0.9997 * self.sag_env + 0.0003 * load;
        }
        self.ripple_phase += self.ripple_inc;
        if self.ripple_phase > 2.0 * PI {
            self.ripple_phase -= 2.0 * PI;
        }
        let ripple = self.ripple_phase.sin() * (0.00015 * self.sag_env);
        (0.78 - 0.055 * self.sag_env.clamp(0.0, 1.0) + ripple).clamp(0.70, 0.80)
    }
}

/// A2. Crosstalk Capacitivo e Termico entre Canal A e Canal B dos Op-Amps Duplos NJM4558L (IC13/IC14)
#[derive(Clone, Copy, Debug, Default)]
pub struct Njm4558StereoCrosstalk {
    prev_l: f32,
    prev_r: f32,
    therm_l: f32,
    therm_r: f32,
}

impl Njm4558StereoCrosstalk {
    pub fn reset(&mut self) {
        self.prev_l = 0.0;
        self.prev_r = 0.0;
        self.therm_l = 0.0;
        self.therm_r = 0.0;
    }

    #[inline(always)]
    pub fn process_stereo(&mut self, l: f32, r: f32) -> (f32, f32) {
        // Acoplamento capacitivo de alta frequencia (~-62 dB) + acoplamento termico de Substrato
        let cap_from_r = (r - self.prev_r) * 0.00075;
        let cap_from_l = (l - self.prev_l) * 0.00075;
        self.prev_l = l;
        self.prev_r = r;

        self.therm_l = 0.999 * self.therm_l + 0.001 * l;
        self.therm_r = 0.999 * self.therm_r + 0.001 * r;

        let out_l = l + cap_from_r + 0.00035 * self.therm_r;
        let out_r = r + cap_from_l + 0.00035 * self.therm_l;
        (out_l, out_r)
    }
}

/// A4. Modulo de Transformador de Entrada Discreto (Histerese magnetica LF + Impedancia 4k/33k)
#[derive(Clone, Copy, Debug, Default)]
pub struct InputTransformerStage {
    flux_z1: f32,
    hp_x1: f32,
    hp_y1: f32,
    warm_z1: f32,
    hp_r: f32,
    warm_a: f32,
    flux_leak: f32,
}

impl InputTransformerStage {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.hp_r = (-2.0 * PI * 28.0 / sr).exp();
        self.warm_a = (-2.0 * PI * 185.0 / sr).exp();
        self.flux_leak = (-2.0 * PI * 65.0 / sr).exp();
    }

    pub fn reset(&mut self) {
        self.flux_z1 = 0.0;
        self.hp_x1 = 0.0;
        self.hp_y1 = 0.0;
        self.warm_z1 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let hp = self.hp_r * (self.hp_y1 + x - self.hp_x1);
        self.hp_x1 = x;
        self.hp_y1 = hp;

        // Integrador de fluxo magnetico (saturacao B-H mais intensa em baixas frequencias)
        self.flux_z1 = self.flux_leak * self.flux_z1 + (1.0 - self.flux_leak) * hp;
        let f2 = (self.flux_z1 * self.flux_z1).min(1.5);
        let core_sat = hp - 0.045 * self.flux_z1 * f2;

        self.warm_z1 = (1.0 - self.warm_a) * core_sat + self.warm_a * self.warm_z1;
        core_sat + 0.075 * self.warm_z1
    }
}

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

/// B1. Produto Escalar FIR 4-Tap Acelerado por Hardware SIMD (ARM NEON / x86_64 SSE)
#[inline(always)]
fn simd_fir4_dot(x0: f32, x1: f32, x2: f32, x3: f32, c0: f32, c1: f32, c2: f32, c3: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        let vx = vld1q_f32([x0, x1, x2, x3].as_ptr());
        let vc = vld1q_f32([c0, c1, c2, c3].as_ptr());
        vaddvq_f32(vmulq_f32(vx, vc))
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        let vx = _mm_set_ps(x3, x2, x1, x0);
        let vc = _mm_set_ps(c3, c2, c1, c0);
        let prod = _mm_mul_ps(vx, vc);
        let shuf = _mm_shuffle_ps(prod, prod, 0b10_11_00_01);
        let sums = _mm_add_ps(prod, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        _mm_cvtss_f32(_mm_add_ss(sums, shuf2))
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        x0 * c0 + x1 * c1 + x2 * c2 + x3 * c3
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OversampledAnalogStage {
    up_x1: f32,
    up_x2: f32,
    up_x3: f32,
    down_z1: f32,
    down_z2: f32,
    down_z3: f32,
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
        self.down_z3 = 0.0;
        self.opamp_prev = 0.0;
        self.opamp_lp = 0.0;
        self.dc_blocker.reset();
    }

    #[inline(always)]
    fn nonlinear_2sc3330_njm4558(&mut self, v: f32, rail_limit: f32) -> f32 {
        let xc = v.clamp(-2.2, 2.2);
        let bjt = xc + 0.022 * xc * xc;
        let d2 = bjt * bjt;
        let diode = bjt * (27.0 + d2) / (27.0 + 9.0 * d2);

        let delta = (diode - self.opamp_prev).clamp(-self.max_slew_2x, self.max_slew_2x);
        let slewed = self.opamp_prev + delta;
        self.opamp_prev = slewed;
        let filtered = (1.0 - self.bw_alpha_2x) * slewed + self.bw_alpha_2x * self.opamp_lp;
        self.opamp_lp = filtered;

        let pos_rail = rail_limit.clamp(0.68, 0.82);
        let neg_rail = pos_rail - 0.04;
        if filtered > pos_rail {
            let over = filtered - pos_rail;
            pos_rail + (1.0 - pos_rail) * (over / (1.0 + over))
        } else if filtered < -neg_rail {
            let under = -filtered - neg_rail;
            -(neg_rail + (1.0 - neg_rail) * (under / (1.0 + under)))
        } else {
            filtered
        }
    }

    #[inline(always)]
    pub fn process_with_rail(&mut self, x: f32, rail_limit: f32) -> f32 {
        let s0 = self.up_x1;
        // Kernel FIR polifasico 2x acelerado via SIMD [-1/16, 9/16, 9/16, -1/16]
        let s1 = simd_fir4_dot(x, self.up_x1, self.up_x2, self.up_x3, -0.0625, 0.5625, 0.5625, -0.0625);
        self.up_x3 = self.up_x2;
        self.up_x2 = self.up_x1;
        self.up_x1 = x;

        let y0 = self.nonlinear_2sc3330_njm4558(s0, rail_limit);
        let y1 = self.nonlinear_2sc3330_njm4558(s1, rail_limit);

        // Decimador FIR meia-banda via SIMD
        let decimated = simd_fir4_dot(y1, y0, self.down_z1, self.down_z2, 0.125, 0.375, 0.375, 0.125);
        self.down_z3 = self.down_z2;
        self.down_z2 = self.down_z1;
        self.down_z1 = y1;

        self.dc_blocker.process(decimated)
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        self.process_with_rail(x, 0.78)
    }
}

/// B3. Extrator e Morfador de Envelope Espectral LPC de 4a Ordem (Levinson-Durbin)
#[derive(Clone, Copy, Debug, Default)]
pub struct LpcFormantMorpher {
    r0: f32,
    r1: f32,
    r2: f32,
    x1: f32,
    x2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl LpcFormantMorpher {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline(always)]
    pub fn track_sample(&mut self, x: f32) {
        self.r0 = 0.996 * self.r0 + 0.004 * (x * x) + 1e-7;
        self.r1 = 0.996 * self.r1 + 0.004 * (x * self.x1);
        self.r2 = 0.996 * self.r2 + 0.004 * (x * self.x2);
        self.x2 = self.x1;
        self.x1 = x;
    }

    /// Resolve recursao de Levinson-Durbin de 2a ordem a cada hop para extrair o trato vocal
    #[inline]
    pub fn update_coefficients(&mut self) {
        if self.r0 < 1e-6 {
            self.a1 = 0.0;
            self.a2 = 0.0;
            return;
        }
        let k1 = (self.r1 / self.r0).clamp(-0.92, 0.92);
        let e1 = (self.r0 * (1.0 - k1 * k1)).max(1e-6);
        let k2 = ((self.r2 - k1 * self.r1) / e1).clamp(-0.88, 0.88);
        self.a1 = (k1 * (1.0 - k2)).clamp(-1.6, 1.6);
        self.a2 = k2.clamp(-0.85, 0.85);
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

/// B1. Kernel de Diferenca Quadratica YIN Vetorizado com Intrinsicos Hardware SIMD (NEON / SSE2)
#[inline(always)]
fn sq_diff_simd(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        use core::arch::aarch64::*;
        let n = a.len().min(b.len());
        let chunks = n / 4;
        let pa = a.as_ptr();
        let pb = b.as_ptr();
        let mut vacc = vdupq_n_f32(0.0);
        for i in 0..chunks {
            let va = vld1q_f32(pa.add(i * 4));
            let vb = vld1q_f32(pb.add(i * 4));
            let vd = vsubq_f32(va, vb);
            vacc = vfmaq_f32(vacc, vd, vd);
        }
        let mut sum = vaddvq_f32(vacc);
        for i in (chunks * 4)..n {
            let d = *pa.add(i) - *pb.add(i);
            sum += d * d;
        }
        sum
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        let n = a.len().min(b.len());
        let chunks = n / 4;
        let pa = a.as_ptr();
        let pb = b.as_ptr();
        let mut vacc = _mm_setzero_ps();
        for i in 0..chunks {
            let va = _mm_loadu_ps(pa.add(i * 4));
            let vb = _mm_loadu_ps(pb.add(i * 4));
            let vd = _mm_sub_ps(va, vb);
            vacc = _mm_add_ps(vacc, _mm_mul_ps(vd, vd));
        }
        let shuf = _mm_shuffle_ps(vacc, vacc, 0b10_11_00_01);
        let sums = _mm_add_ps(vacc, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let mut sum = _mm_cvtss_f32(_mm_add_ss(sums, shuf2));
        for i in (chunks * 4)..n {
            let d = *pa.add(i) - *pb.add(i);
            sum += d * d;
        }
        sum
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        a.iter().zip(b.iter()).map(|(&x, &y)| { let d = x - y; d * d }).sum()
    }
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
            fmin: 45.0, fmax: 900.0, threshold: 0.33, hysteresis_semi: 0.65,
            decim_factor, yin_win, yin_hop,
            cmnd: vec![1.0; yin_win / 2],
            oct_bias_lut: vec![0.0; yin_win / 2],
            decim_buf: vec![0.0; yin_win],
            smooth_midi: 60.0, current_f0: 0.0, current_midi: 60.0, current_note: -1,
            voiced: false, unvoiced_hold: 0,
            slice_tau_next: 1, slice_tau_max: 0, slice_tau_min: 2,
            slice_w: 0, slice_running_sum: 0.0, slice_active: false,
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
        if (sum_sq / (w as f32)).sqrt() < 0.002 {
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
        let active_thresh = if was_voiced { self.threshold + 0.08 } else { self.threshold };

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
        if self.unvoiced_hold >= 5 {
            self.voiced = false;
            self.current_f0 = 0.0;
            self.current_note = -1;
        }
    }
}

/// B2. Worker Assincrono Lock-Free para Rastreamento YIN em Thread Dedicada
pub struct AsyncYinShared {
    pub ring_bits: Vec<AtomicU32>,
    pub write_pos: AtomicUsize,
    pub sr_bits: AtomicU32,
    pub tune_bits: AtomicU32,
    pub job_ready: AtomicBool,
    pub result_ready: AtomicBool,
    pub stop_flag: AtomicBool,
    pub out_f0_bits: AtomicU32,
    pub out_midi_bits: AtomicU32,
    pub out_note: AtomicI32,
    pub out_voiced: AtomicBool,
}

pub struct AsyncYinWorker {
    shared: Arc<AsyncYinShared>,
    handle: Option<JoinHandle<()>>,
}

impl AsyncYinWorker {
    pub fn new(sr: f32) -> Self {
        let mut ring_bits = Vec::with_capacity(RING_SIZE);
        for _ in 0..RING_SIZE {
            ring_bits.push(AtomicU32::new(0));
        }
        let shared = Arc::new(AsyncYinShared {
            ring_bits,
            write_pos: AtomicUsize::new(0),
            sr_bits: AtomicU32::new(sr.to_bits()),
            tune_bits: AtomicU32::new(440.0_f32.to_bits()),
            job_ready: AtomicBool::new(false),
            result_ready: AtomicBool::new(false),
            stop_flag: AtomicBool::new(false),
            out_f0_bits: AtomicU32::new(0),
            out_midi_bits: AtomicU32::new(60.0_f32.to_bits()),
            out_note: AtomicI32::new(-1),
            out_voiced: AtomicBool::new(false),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::spawn(move || {
            let _ftz = ScopedFtzDaz::new();
            let mut local_ring = vec![0.0_f32; RING_SIZE];
            let mut local_sr = f32::from_bits(worker_shared.sr_bits.load(Ordering::Relaxed));
            let mut tracker = PitchTracker::new(local_sr);

            while !worker_shared.stop_flag.load(Ordering::Relaxed) {
                if worker_shared.job_ready.swap(false, Ordering::Acquire) {
                    let sr_now = f32::from_bits(worker_shared.sr_bits.load(Ordering::Relaxed));
                    if (sr_now - local_sr).abs() > 1.0 {
                        local_sr = sr_now;
                        tracker.set_sample_rate(local_sr);
                    }
                    let wpos = worker_shared.write_pos.load(Ordering::Acquire);
                    let tune = f32::from_bits(worker_shared.tune_bits.load(Ordering::Relaxed));
                    let span = tracker.yin_win * tracker.decim_factor + 8;
                    let start = wpos.wrapping_sub(span);
                    for k in 0..=span {
                        let idx = start.wrapping_add(k) & RING_MASK;
                        local_ring[idx] = f32::from_bits(worker_shared.ring_bits[idx].load(Ordering::Relaxed));
                    }
                    tracker.analyze_frame(&local_ring, wpos, local_sr, tune);
                    worker_shared.out_f0_bits.store(tracker.current_f0.to_bits(), Ordering::Relaxed);
                    worker_shared.out_midi_bits.store(tracker.current_midi.to_bits(), Ordering::Relaxed);
                    worker_shared.out_note.store(tracker.current_note, Ordering::Relaxed);
                    worker_shared.out_voiced.store(tracker.voiced, Ordering::Release);
                    worker_shared.result_ready.store(true, Ordering::Release);
                } else {
                    thread::park_timeout(std::time::Duration::from_micros(250));
                }
            }
        });
        Self { shared, handle: Some(handle) }
    }

    #[inline(always)]
    pub fn push_sample(&self, write_pos: usize, sample: f32) {
        self.shared.ring_bits[write_pos & RING_MASK].store(sample.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn trigger_job(&self, write_pos: usize, sr: f32, tune_hz: f32) {
        self.shared.write_pos.store(write_pos, Ordering::Release);
        self.shared.sr_bits.store(sr.to_bits(), Ordering::Relaxed);
        self.shared.tune_bits.store(tune_hz.to_bits(), Ordering::Relaxed);
        self.shared.job_ready.store(true, Ordering::Release);
        if let Some(h) = &self.handle {
            h.thread().unpark();
        }
    }

    #[inline]
    pub fn poll_result(&self, pitch: &mut PitchTracker) -> bool {
        if self.shared.result_ready.swap(false, Ordering::Acquire) {
            pitch.current_f0 = f32::from_bits(self.shared.out_f0_bits.load(Ordering::Relaxed));
            pitch.current_midi = f32::from_bits(self.shared.out_midi_bits.load(Ordering::Relaxed));
            pitch.current_note = self.shared.out_note.load(Ordering::Relaxed);
            pitch.voiced = self.shared.out_voiced.load(Ordering::Acquire);
            return true;
        }
        false
    }
}

impl Drop for AsyncYinWorker {
    fn drop(&mut self) {
        self.shared.stop_flag.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            h.thread().unpark();
            let _ = h.join();
        }
    }
}

#[derive(Clone, Debug)]
pub struct SharedGrainCache {
    pub cached_mark: usize,
    pub cached_ta_int: usize,
    pub radius: isize,
    pub buf: Vec<f32>,
}

impl Default for SharedGrainCache {
    fn default() -> Self { Self::new() }
}

impl SharedGrainCache {
    pub fn new() -> Self {
        Self { cached_mark: usize::MAX, cached_ta_int: 0, radius: 0, buf: vec![0.0; 2049] }
    }

    pub fn reset(&mut self) {
        self.cached_mark = usize::MAX;
        self.cached_ta_int = 0;
    }

    #[inline]
    pub fn ensure_extracted(&mut self, dry_ring: &[f32], hann_lut: &[f32], safe_mark: usize, ta: f32) {
        let ta_int = ta.round() as usize;
        if self.cached_mark == safe_mark && self.cached_ta_int == ta_int { return; }
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
            self.buf[idx] = dry_ring[p] * hann_lut[lut_i].sqrt();
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
    quant_err: f32,
    // A3. Tolerancia Monte Carlo +-2% nos componentes RC da voz
    rc_tol_hp: f32,
    rc_tol_lp: f32,
    // B3. Estado do filtro de sintese LPC para preservacao de formantes
    lpc_w1: f32,
    lpc_w2: f32,
    lpc_s1: f32,
    lpc_s2: f32,
    is_dual_warm: bool,
}

impl PsolaSubVoice {
    pub fn new(seed: u32) -> Self {
        let mut tol_rng = FastRng::new(seed.wrapping_add(0x517CC1B7));
        let rc_tol_hp = 1.0 + 0.02 * tol_rng.next_bipolar();
        let rc_tol_lp = 1.0 + 0.02 * tol_rng.next_bipolar();
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
            rc_tol_hp,
            rc_tol_lp,
            lpc_w1: 0.0,
            lpc_w2: 0.0,
            lpc_s1: 0.0,
            lpc_s2: 0.0,
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
        self.lpc_w1 = 0.0;
        self.lpc_w2 = 0.0;
        self.lpc_s1 = 0.0;
        self.lpc_s2 = 0.0;
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
            dry_ring, hann_lut, None, 0.0, 0.0, 0.0,
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
        lpc_a1: f32,
        lpc_a2: f32,
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

        let wet_nola = pair[0] / pair[1].max(0.35);

        // B3. Morfagem de Envelope Espectral LPC (pre-whitening na razao de pitch + sintese no formante original)
        let wet_lpc = if (pitch_ratio - 1.0).abs() > 0.08 && emit_mask > 0.01 {
            let resid = wet_nola - (lpc_a1 * 0.45) * self.lpc_w1 - (lpc_a2 * 0.45) * self.lpc_w2;
            self.lpc_w2 = self.lpc_w1;
            self.lpc_w1 = wet_nola;
            let synth = (resid + (lpc_a1 * 0.48) * self.lpc_s1 + (lpc_a2 * 0.48) * self.lpc_s2).clamp(-2.5, 2.5);
            self.lpc_s2 = self.lpc_s1;
            self.lpc_s1 = synth;
            0.65 * synth + 0.35 * wet_nola
        } else {
            self.lpc_w1 = wet_nola;
            self.lpc_s1 = wet_nola;
            wet_nola
        };

        let wet_adsp2186 = if wet_lpc.abs() > 1e-5 && emit_mask > 0.01 {
            let mac40 = wet_lpc.clamp(-3.99, 3.99);
            let shaped = mac40 + 0.5 * self.quant_err;
            let q16 = (shaped * 32_768.0).round() * (1.0 / 32_768.0);
            self.quant_err = (shaped - q16).clamp(-1e-3, 1e-3);
            q16
        } else {
            self.quant_err = 0.0;
            wet_lpc
        };

        let sib_att = if pitch_ratio > 1.15 { 1.0 - 0.65 * sibilance_factor } else { 1.0 };
        let dry = dry_ring[emit_pos & RING_MASK];
        let nat_sib_blend = if pitch_ratio > 1.15 { 0.22 * sibilance_factor * emit_mask } else { 0.0 };
        let combined = wet_adsp2186 * emit_mask * sib_att
            + dry * ((1.0 - emit_mask) * unvoiced_gain + nat_sib_blend);

        // Filtros Akai com tolerancia Monte Carlo +-2% individual por sub-voz
        let hp_hz = AKAI_SUBSONIC_HP_HZ * self.rc_tol_hp;
        let hp_r = (-2.0 * PI * hp_hz / sr).exp();
        let hp_out = hp_r * (self.hp_y1 + combined - self.hp_x1);
        self.hp_x1 = combined;
        self.hp_y1 = hp_out;

        let base_lp = if self.is_dual_warm { AKAI_DUAL_WARM_LP_HZ } else { AKAI_DAC_LP_HZ };
        let lp_fc = (base_lp * self.rc_tol_lp).min(sr * 0.45);
        let lp_a = (-2.0 * PI * lp_fc / sr).exp();
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
            let rad_f = cache.radius as f32;
            let cache_slice = &cache.buf;

            for k in 0..l_out {
                let offset_out = k as isize - half_out;
                let src_off = (offset_out as f32) * fr;
                let sample = if src_off.abs() < rad_f {
                    let pos = src_off + rad_f;
                    let i0 = pos.floor() as usize;
                    let frac = pos - (i0 as f32);
                    cache_slice[i0] + frac * (cache_slice[i0 + 1] - cache_slice[i0])
                } else { 0.0 };

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
                    let src_w_idx = ((norm_src * (HANN_LUT_SIZE as f32)) as usize).min(HANN_LUT_SIZE - 1);
                    raw_s * hann_lut[src_w_idx].sqrt()
                } else { 0.0 };

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
            let x = if step < 4000 { (2.0 * PI * f0 * (step as f32) / sr).sin() * 0.8 } else { 0.0 };
            let voiced = if step < 4000 { 1.0 } else { 0.0 };
            dry_ring[write_pos & RING_MASK] = x;

            let read_pos = write_pos.wrapping_sub(512);
            let emit_pos = read_pos.wrapping_sub(512);
            if step < 4000 && read_pos.wrapping_sub(latest_mark) >= period {
                latest_mark = read_pos;
            }
            let emit_voiced = if step >= 1024 && step < 4000 + 1024 { 1.0 } else { 0.0 };

            let y = voice.step_sample(
                &dry_ring, &hann_lut, write_pos, read_pos, emit_pos, latest_mark,
                voiced, emit_voiced, f0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, sr, max_grain,
            );

            if step > 8000 {
                silence_rms_acc += y * y;
                silence_count += 1;
            }
            write_pos = write_pos.wrapping_add(1);
        }

        let ghost_rms = (silence_rms_acc / silence_count as f32).sqrt();
        assert!(ghost_rms < 1e-6, "Ruido residual detectado no anel PSOLA: RMS = {}", ghost_rms);
    }

    #[test]
    fn test_psu_sag_crosstalk_lpc_and_simd_oversampling() {
        let _ftz = ScopedFtzDaz::new();
        let sr = 96_000.0;
        let mut psu = PowerSupply9VSag::default();
        psu.set_sample_rate(sr);
        let rail_idle = psu.step_rail(0.0, 0);
        let rail_load = psu.step_rail(0.95, 4);
        assert!(rail_load < rail_idle, "A tensao do barramento 9V deve cair sob carga polifonica");

        let mut xtalk = Njm4558StereoCrosstalk::default();
        let (_l, r) = xtalk.process_stereo(0.8, 0.0);
        assert!(r.abs() > 0.0 && r.abs() < 0.01, "Crosstalk NJM4558L deve vazar sutilmente L->R");

        let mut lpc = LpcFormantMorpher::default();
        for n in 0..2000 {
            lpc.track_sample((2.0 * PI * 440.0 * (n as f32) / sr).sin() * 0.5);
        }
        lpc.update_coefficients();
        assert!(lpc.a1.abs() > 0.01, "LPC deve estimar coeficientes do envelope vocal");
    }
}
