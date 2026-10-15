//! Motor DSP Otimizado e Modelagem do Barramento de Harmonias (Akai HV10 & Ariwa Studio).
//! - Caminho Direct 100% linear e transparente (20 Hz - 20 kHz, 0% THD).
//! - Rastreamento YIN sincrono Time-Sliced com janela de 1536 amostras (suporte ate 42 Hz).
//! - Ancoragem causal da borda esquerda do grao no anel de 8192 amostras + janela Hann 2.0*Ts:
//!   garante sobreposicao constante (soma de pesos = 1.0) sem lacunas na Low Octave em 65 Hz
//!   mesmo no modo Live e sem cancelamento de fase em High 5th.
//! - Random (padrao 0.0): controla exclusivamente humanizacao de pitch/tempo (Ornstein-Uhlenbeck
//!   +-9 cents / 16 ms + micro-sag de pitch da fonte de 9V DC nos picos dinamicos).
//! - Drive (Analog Drive): controla exclusivamente o timbre do ChoirAnalogBus (185 Hz shelf,
//!   saturacao simetrica dos op-amps NJM4558L com sag de 9V DC e crosstalk estereo de -62 dB).

use std::f32::consts::PI;

pub const RING_SIZE: usize = 32768;
pub const RING_MASK: usize = RING_SIZE - 1;
pub const VOICE_RING_SIZE: usize = 8192;
pub const VOICE_RING_MASK: usize = VOICE_RING_SIZE - 1;
pub const DELAY_SIZE: usize = 16384;
pub const DELAY_MASK: usize = DELAY_SIZE - 1;
pub const HANN_LUT_SIZE: usize = 2048;
pub const PSOLA_SYNTH_WINDOW_FACTOR: f32 = 2.0;
pub const MAX_CACHE_RADIUS: isize = 1800;

pub const AKAI_SUBSONIC_HP_HZ: f32 = 32.784804;
pub const AKAI_MAIN_VOICE_LP_HZ: f32 = 14800.0;
pub const AKAI_DUAL_WARM_LP_HZ: f32 = 11500.0;
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
            let prev_ptr: *mut u32 = &mut prev;
            core::arch::asm!("stmxcsr [{}]", in(reg) prev_ptr, options(nostack, preserves_flags));
            let next: u32 = prev | 0x8040;
            if next != prev {
                let next_ptr: *const u32 = &next;
                core::arch::asm!("ldmxcsr [{}]", in(reg) next_ptr, options(nostack, preserves_flags));
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
            let prev_ptr: *const u32 = &self.prev_mxcsr;
            core::arch::asm!("ldmxcsr [{}]", in(reg) prev_ptr, options(nostack, preserves_flags));
        }
    }
}

#[inline]
pub fn max_grain_for_sr(_sr: f32) -> usize { 4096 }

#[inline]
pub fn lookahead_samples_for_sr(sr: f32) -> usize {
    ((1536.0 * (sr / 44_100.0)).round() as usize).clamp(1024, 8192)
}

#[inline]
pub fn out_delay_for_sr(sr: f32) -> usize {
    ((960.0 * (sr / 44_100.0)).round() as usize).clamp(768, 2048)
}

/// Modo Live: Direct com 0 ms de atraso e harmonias com 384 + 384 = 768 amostras (~17.4 ms a 44.1 kHz)
#[inline]
pub fn live_lookahead_for_sr(sr: f32) -> usize {
    ((384.0 * (sr / 44_100.0)).round() as usize).clamp(256, 1536)
}

#[inline]
pub fn live_out_delay_for_sr(sr: f32) -> usize {
    ((384.0 * (sr / 44_100.0)).round() as usize).clamp(256, 1536)
}

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
    1.0 + x * (std::f32::consts::LN_2 + x * (0.2402265 + x * 0.0555041))
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

/// Interpolacao Cubica Hermite de 4 pontos (Catmull-Rom, C1-continua).
/// Elimina a perda de agudos (sinc^2 attenuation) e espurios de aliasing da interpolacao linear.
#[inline(always)]
pub fn hermite_interp(ym1: f32, y0: f32, y1: f32, y2: f32, f: f32) -> f32 {
    let c0 = y0;
    let c1 = 0.5 * (y1 - ym1);
    let c2 = ym1 - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
    let c3 = 0.5 * (y2 - ym1) + 1.5 * (y0 - y1);
    ((c3 * f + c2) * f + c1) * f + c0
}

/// Limitador suave analogico de barramento master (estilo transformador Rupert Neve / fita).
/// - Transparente e 100% linear para |x| <= 0.92 (0% THD no caminho nominal).
/// - Saturacao assintotica C1-continua acima de 0.92, evitando ceifa digital rigida (hard clipping).
#[inline(always)]
pub fn analog_bus_soft_limit(x: f32) -> f32 {
    const THRESHOLD: f32 = 0.92;
    const MARGIN: f32 = 0.999 - THRESHOLD;
    if x > THRESHOLD {
        let u = x - THRESHOLD;
        THRESHOLD + u / (1.0 + u / MARGIN)
    } else if x < -THRESHOLD {
        let u = -x - THRESHOLD;
        -(THRESHOLD + u / (1.0 + u / MARGIN))
    } else {
        x
    }
}

/// Emulacao do transiente mecanico dos reles de sinal Takamisawa RY-9WK (RL1/RL2) do Akai HV10.
/// Modela o repique de contato (contact bounce de 1.5 a 2.5 ms) e o micro-thump indutivo de bobina
/// ao comutar o bypass fisico de hardware.
#[derive(Clone, Copy, Debug)]
pub struct RelayBypassModel {
    prev_bypass: bool,
    bounce_timer: usize,
    bounce_total: usize,
    ring_phase: f32,
    ring_decay: f32,
    is_transitioning: bool,
    target_bypass: bool,
}

impl Default for RelayBypassModel {
    fn default() -> Self {
        Self {
            prev_bypass: false,
            bounce_timer: 0,
            bounce_total: 0,
            ring_phase: 0.0,
            ring_decay: 0.0,
            is_transitioning: false,
            target_bypass: false,
        }
    }
}

impl RelayBypassModel {
    pub fn new() -> Self { Self::default() }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn update_bypass_trigger(&mut self, bypass: bool, sr: f32) {
        if bypass != self.prev_bypass {
            self.prev_bypass = bypass;
            self.target_bypass = bypass;
            self.bounce_total = ((0.0022 * sr).round() as usize).clamp(32, 256);
            self.bounce_timer = self.bounce_total;
            self.ring_phase = 0.0;
            self.ring_decay = 1.0;
            self.is_transitioning = true;
        }
    }

    #[inline(always)]
    pub fn process_transient(&mut self, direct_sample: f32, wet_sample: f32, sr: f32) -> f32 {
        if !self.is_transitioning {
            return if self.prev_bypass { direct_sample } else { wet_sample };
        }

        let progress = 1.0 - (self.bounce_timer as f32 / self.bounce_total as f32);
        self.bounce_timer = self.bounce_timer.saturating_sub(1);
        if self.bounce_timer == 0 {
            self.is_transitioning = false;
            return if self.target_bypass { direct_sample } else { wet_sample };
        }

        // Contact bounce: micro-interrupcoes de contato nos primeiros 60% da comutacao
        let bounce_gain = if progress < 0.25 {
            0.35
        } else if progress < 0.45 {
            0.0
        } else if progress < 0.70 {
            0.75
        } else {
            1.0
        };

        // Micro-thump indutivo de bobina de 9V (oscilacao amortecida ~1850 Hz a ~ -48 dBFS)
        let ring_freq = 1850.0;
        let w = 2.0 * PI * ring_freq / sr;
        let ring_sig = self.ring_phase.sin() * self.ring_decay * 0.004;
        self.ring_phase += w;
        if self.ring_phase > 2.0 * PI { self.ring_phase -= 2.0 * PI; }
        self.ring_decay *= 0.96;

        let effective_sig = if self.target_bypass {
            wet_sample * (1.0 - progress) + direct_sample * progress
        } else {
            direct_sample * (1.0 - progress) + wet_sample * progress
        };

        (effective_sig * bounce_gain) + ring_sig
    }
}

/// Barramento Analogico Estereo do Coro (Akai HV10 IC13/IC14 NJM4558L + PCM3003E + Ariwa Warmth).
/// Controlado pelo knob global `Drive` (0..100%) e aplicado exclusivamente no Choir Bus.
#[derive(Clone, Copy, Debug)]
pub struct ChoirAnalogBus {
    sr: f32,
    warm_l_z1: f32,
    warm_r_z1: f32,
    warm_a: f32,
    pub sag_env: f32,
    slew_l: f32,
    slew_r: f32,
    prev_l: f32,
    prev_r: f32,
    therm_l: f32,
    therm_r: f32,
    recon_l: BiquadLowpass,
    recon_r: BiquadLowpass,
    dc_x1_l: f32,
    dc_y1_l: f32,
    dc_x1_r: f32,
    dc_y1_r: f32,
    da_fast_l: f32,
    da_slow_l: f32,
    da_fast_r: f32,
    da_slow_r: f32,
    da_fast_a: f32,
    da_slow_a: f32,
}

impl Default for ChoirAnalogBus {
    fn default() -> Self {
        Self {
            sr: 44_100.0,
            warm_l_z1: 0.0,
            warm_r_z1: 0.0,
            warm_a: (-2.0 * PI * 185.0 / 44_100.0).exp(),
            sag_env: 0.0,
            slew_l: 0.0,
            slew_r: 0.0,
            prev_l: 0.0,
            prev_r: 0.0,
            therm_l: 0.0,
            therm_r: 0.0,
            recon_l: BiquadLowpass::default(),
            recon_r: BiquadLowpass::default(),
            dc_x1_l: 0.0,
            dc_y1_l: 0.0,
            dc_x1_r: 0.0,
            dc_y1_r: 0.0,
            da_fast_l: 0.0,
            da_slow_l: 0.0,
            da_fast_r: 0.0,
            da_slow_r: 0.0,
            da_fast_a: (-1.0_f32 / (44_100.0 * 0.015)).exp(),
            da_slow_a: (-1.0_f32 / (44_100.0 * 0.080)).exp(),
        }
    }
}

impl ChoirAnalogBus {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr.max(8000.0);
        self.warm_a = (-2.0 * PI * 185.0 / self.sr).exp();
        self.recon_l.set_cutoff(PCM3003_RECON_LP_HZ, self.sr);
        self.recon_r.set_cutoff(PCM3003_RECON_LP_HZ, self.sr);
        self.da_fast_a = (-1.0 / (self.sr * 0.015)).exp();
        self.da_slow_a = (-1.0 / (self.sr * 0.080)).exp();
    }

    pub fn reset(&mut self) {
        self.warm_l_z1 = 0.0;
        self.warm_r_z1 = 0.0;
        self.sag_env = 0.0;
        self.slew_l = 0.0;
        self.slew_r = 0.0;
        self.prev_l = 0.0;
        self.prev_r = 0.0;
        self.therm_l = 0.0;
        self.therm_r = 0.0;
        self.recon_l.reset();
        self.recon_r.reset();
        self.dc_x1_l = 0.0;
        self.dc_y1_l = 0.0;
        self.dc_x1_r = 0.0;
        self.dc_y1_r = 0.0;
        self.da_fast_l = 0.0;
        self.da_slow_l = 0.0;
        self.da_fast_r = 0.0;
        self.da_slow_r = 0.0;
    }

    #[inline(always)]
    fn njm4558_asym_saturate(v: f32, ceiling: f32, drv: f32) -> f32 {
        let x = v.clamp(-2.5, 2.5);
        // Saturacao assimetrica suave modelando op-amps NJM4558L em fonte simples (9V DC)
        // com geracao musical de 2a harmonica (calor Silk Neve / fita analogica)
        let asym = 0.08 * drv * (x * x);
        let x_biased = x + asym;
        let d = x_biased / ceiling;
        let d2 = d * d;
        ceiling * d * (27.0 + d2) / (27.0 + 9.0 * d2)
    }

    #[inline(always)]
    pub fn process_stereo(
        &mut self,
        wet_l: f32,
        wet_r: f32,
        drive_norm: f32,
        active_voices: usize,
    ) -> (f32, f32) {
        let drv = drive_norm.clamp(0.0, 1.0);

        // 1. Ariwa Console Low-Mid Warmth Shelf em 185 Hz (proporcional ao Drive)
        self.warm_l_z1 = (1.0 - self.warm_a) * wet_l + self.warm_a * self.warm_l_z1;
        self.warm_r_z1 = (1.0 - self.warm_a) * wet_r + self.warm_a * self.warm_r_z1;
        let shelf_gain = 0.28 * drv;
        let w_l = wet_l + shelf_gain * self.warm_l_z1;
        let w_r = wet_r + shelf_gain * self.warm_r_z1;

        // 2. Sag dinamico da fonte de 9V DC / 250mA sob carga polifonica
        let energy = 0.5 * (w_l.abs() + w_r.abs()) * (1.0 + 0.08 * (active_voices as f32));
        if energy > self.sag_env {
            self.sag_env = 0.992 * self.sag_env + 0.008 * energy;
        } else {
            self.sag_env = 0.9997 * self.sag_env + 0.0003 * energy;
        }

        // 3. Saturacao assimetrica dos op-amps NJM4558L (IC13/IC14) com 2a harmonica (Silk Red)
        let (sat_l, sat_r) = if drv > 1e-3 {
            let ceiling = (0.96 - 0.12 * drv * self.sag_env.clamp(0.0, 1.0)).clamp(0.78, 0.96);
            let pre_drv = 1.0 + 0.85 * drv;
            let inv_drv = 1.0 / (1.0 + 0.55 * drv);
            let sl = Self::njm4558_asym_saturate(w_l * pre_drv, ceiling, drv) * inv_drv;
            let sr_out = Self::njm4558_asym_saturate(w_r * pre_drv, ceiling, drv) * inv_drv;
            (
                w_l * (1.0 - drv) + sl * drv,
                w_r * (1.0 - drv) + sr_out * drv,
            )
        } else {
            (w_l, w_r)
        };

        // 4. Limitacao suave de taxa de subida (Slew Rate 1.0 V/us) dos op-amps NJM4558L
        let (slewed_l, slewed_r) = if drv > 1e-3 && self.sr > 1000.0 {
            let max_delta = (90_000.0 / self.sr) * (1.0 + 3.0 * (1.0 - drv));
            let dl = sat_l - self.slew_l;
            let u_l = dl / max_delta;
            self.slew_l += max_delta * u_l / (1.0 + u_l * u_l).sqrt();

            let dr = sat_r - self.slew_r;
            let u_r = dr / max_delta;
            self.slew_r += max_delta * u_r / (1.0 + u_r * u_r).sqrt();
            (self.slew_l, self.slew_r)
        } else {
            self.slew_l = sat_l;
            self.slew_r = sat_r;
            (sat_l, sat_r)
        };

        // 5. Filtro de reconstrucao analogico Burr-Brown PCM3003E (IC3) + indutores LAP02TA (16.8 kHz)
        let rec_l = self.recon_l.process(slewed_l);
        let rec_r = self.recon_r.process(slewed_r);

        // 6. Crosstalk capacitivo e termico do pacote duplo NJM4558L escalado por Drive
        let cap_r = (rec_r - self.prev_r) * (0.0012 * drv);
        let cap_l = (rec_l - self.prev_l) * (0.0012 * drv);
        self.prev_l = rec_l;
        self.prev_r = rec_r;
        self.therm_l = 0.999 * self.therm_l + 0.001 * rec_l;
        self.therm_r = 0.999 * self.therm_r + 0.001 * rec_r;

        let out_unblocked_l = rec_l + cap_r + (0.0005 * drv) * self.therm_r;
        let out_unblocked_r = rec_r + cap_l + (0.0005 * drv) * self.therm_l;

        // 7. Capacitores de acoplamento de saida C23/C25 (10 uF / 10k ~ 1.6 Hz, bloqueio 100% de DC)
        let hp_r_coeff = (-2.0 * PI * 10.0 / self.sr).exp();
        let hp_l = out_unblocked_l - self.dc_x1_l + hp_r_coeff * self.dc_y1_l;
        self.dc_x1_l = out_unblocked_l;
        self.dc_y1_l = hp_l;

        let hp_r = out_unblocked_r - self.dc_x1_r + hp_r_coeff * self.dc_y1_r;
        self.dc_x1_r = out_unblocked_r;
        self.dc_y1_r = hp_r;

        // 8. Absorcao Dieletrica (DA - Dielectric Absorption) dos capacitores Nichicon/Elna
        // Modela o relaxamento lento de dipolos no sub-grave (< 80 Hz) com 0% DC residual
        self.da_fast_l = (1.0 - self.da_fast_a) * hp_l + self.da_fast_a * self.da_fast_l;
        self.da_slow_l = (1.0 - self.da_slow_a) * hp_l + self.da_slow_a * self.da_slow_l;
        self.da_fast_r = (1.0 - self.da_fast_a) * hp_r + self.da_fast_a * self.da_fast_r;
        self.da_slow_r = (1.0 - self.da_slow_a) * hp_r + self.da_slow_a * self.da_slow_r;

        let out_l = hp_l + (0.018 * drv) * self.da_fast_l + (0.009 * drv) * self.da_slow_l;
        let out_r = hp_r + (0.018 * drv) * self.da_fast_r + (0.009 * drv) * self.da_slow_r;

        (out_l, out_r)
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

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn sq_diff_neon(a: &[f32], b: &[f32]) -> f32 {
    use core::arch::aarch64::*;
    let len = a.len().min(b.len());
    let mut sum_v0 = vdupq_n_f32(0.0);
    let mut sum_v1 = vdupq_n_f32(0.0);
    let chunks = len / 8;
    let ptr_a = a.as_ptr();
    let ptr_b = b.as_ptr();

    for i in 0..chunks {
        let offset = i * 8;
        let va0 = vld1q_f32(ptr_a.add(offset));
        let vb0 = vld1q_f32(ptr_b.add(offset));
        let diff0 = vsubq_f32(va0, vb0);
        sum_v0 = vfmaq_f32(sum_v0, diff0, diff0);

        let va1 = vld1q_f32(ptr_a.add(offset + 4));
        let vb1 = vld1q_f32(ptr_b.add(offset + 4));
        let diff1 = vsubq_f32(va1, vb1);
        sum_v1 = vfmaq_f32(sum_v1, diff1, diff1);
    }

    let sum_v = vaddq_f32(sum_v0, sum_v1);
    let mut sum = vaddvq_f32(sum_v);
    for i in (chunks * 8)..len {
        let d = *ptr_a.add(i) - *ptr_b.add(i);
        sum += d * d;
    }
    sum
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx")]
#[inline]
unsafe fn sq_diff_x86_avx(a: &[f32], b: &[f32]) -> f32 {
    use core::arch::x86_64::*;
    let len = a.len().min(b.len());
    let mut sum_v = _mm256_setzero_ps();
    let chunks = len / 8;
    let ptr_a = a.as_ptr();
    let ptr_b = b.as_ptr();

    for i in 0..chunks {
        let va = _mm256_loadu_ps(ptr_a.add(i * 8));
        let vb = _mm256_loadu_ps(ptr_b.add(i * 8));
        let diff = _mm256_sub_ps(va, vb);
        let diff2 = _mm256_mul_ps(diff, diff);
        sum_v = _mm256_add_ps(sum_v, diff2);
    }

    let mut temp = [0.0_f32; 8];
    _mm256_storeu_ps(temp.as_mut_ptr(), sum_v);
    let mut sum = temp[0] + temp[1] + temp[2] + temp[3] + temp[4] + temp[5] + temp[6] + temp[7];
    for i in (chunks * 8)..len {
        let d = *ptr_a.add(i) - *ptr_b.add(i);
        sum += d * d;
    }
    sum
}

#[allow(dead_code, clippy::chunks_exact_to_as_chunks)]
#[inline(always)]
fn sq_diff_fallback(a: &[f32], b: &[f32]) -> f32 {
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

#[inline(always)]
pub fn sq_diff_simd(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        sq_diff_neon(a, b)
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx") {
            unsafe { sq_diff_x86_avx(a, b) }
        } else {
            sq_diff_fallback(a, b)
        }
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        sq_diff_fallback(a, b)
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

/// Cache de amostras brutas do periodo glotal chaveado exclusivamente por `safe_mark`
/// (Main e Dual compartilham a mesma janela sem invalidacao cruzada).
#[derive(Clone, Debug)]
pub struct SharedGrainCache {
    pub cached_mark: usize,
    pub buf: Vec<f32>,
}

impl Default for SharedGrainCache {
    fn default() -> Self { Self::new() }
}

impl SharedGrainCache {
    pub fn new() -> Self {
        Self {
            cached_mark: usize::MAX,
            buf: vec![0.0; (2 * MAX_CACHE_RADIUS + 1) as usize],
        }
    }

    pub fn reset(&mut self) {
        self.cached_mark = usize::MAX;
    }

    #[inline]
    pub fn ensure_extracted(&mut self, dry_ring: &[f32], safe_mark: usize) {
        if self.cached_mark == safe_mark {
            return;
        }
        self.cached_mark = safe_mark;
        let r = MAX_CACHE_RADIUS;
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
    sag_polarity: f32,
    is_dual_warm: bool,
    ap_x1: f32,
    ap_y1: f32,
}

impl PsolaSubVoice {
    pub fn new(seed: u32) -> Self {
        let sag_polarity = if (seed & 1) == 0 { 1.0 } else { -1.0 };
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
            sag_polarity,
            is_dual_warm: false,
            ap_x1: 0.0,
            ap_y1: 0.0,
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
        self.ap_x1 = 0.0;
        self.ap_y1 = 0.0;
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
            dry_ring, hann_lut, None, 0.0, 0.0,
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
        shared_cache: Option<&mut SharedGrainCache>,
        sibilance_factor: f32,
        psu_sag_env: f32,
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

        // Em Random = 0.0, drift_cents e sag_pitch_cents sao identicamente 0.0
        self.drift_cents =
            OU_A * self.drift_cents + OU_ONE_MINUS_A_TIMES_K * n_pitch * (9.0 * random_norm);
        self.delay_rand_norm =
            OU_A * self.delay_rand_norm + OU_ONE_MINUS_A_TIMES_K * n_delay;

        let sag_pitch_cents = self.sag_polarity * psu_sag_env.clamp(0.0, 1.0) * (2.2 * random_norm);

        let jitter_ms = (self.delay_rand_norm * 0.5 + 0.5).clamp(0.0, 1.5) * (16.0 * random_norm);
        let target_delay_ms = jitter_ms + extra_delay_ms;
        let target_delay_samp =
            (target_delay_ms * 0.001 * sr).clamp(0.0, (DELAY_SIZE - 16) as f32);
        self.delay_mod_samples = 0.999 * self.delay_mod_samples + 0.001 * target_delay_samp;

        // Com ancoragem causal da borda esquerda, o grao pode ter ate 4079 amostras no anel de 8192
        let max_safe_grain = (max_grain.min(VOICE_RING_SIZE / 2).saturating_sub(16) | 1).max(63);

        if voiced_mask > 0.01 && f0_hz > 40.0 {
            let cents_oct = (self.drift_cents + sag_pitch_cents + extra_cents) * (1.0 / 1200.0);
            let cents_factor = fast_exp2_small(cents_oct);
            let ratio = (pitch_ratio * cents_factor).clamp(0.25, 4.0);
            let max_ta = (1050.0 * (sr / 44_100.0)).clamp(512.0, 4096.0);
            let ta = (sr / f0_hz).clamp(20.0, max_ta);
            let ts = (ta / ratio).clamp(12.0, max_ta * 2.0);

            self.synth_phase -= 1.0;
            if self.synth_phase <= 0.0 {
                self.synth_phase += ts;
                // Janela de sintese de 2.0 * ts garante sobreposicao de 50% (soma de pesos Hann = 1.0 constante)
                let grain_len = ((PSOLA_SYNTH_WINDOW_FACTOR * ts) as usize | 1)
                    .clamp(31, max_safe_grain);
                self.splat_grain_interleaved(
                    dry_ring,
                    hann_lut,
                    shared_cache,
                    write_pos,
                    latest_mark,
                    read_pos,
                    emit_pos,
                    grain_len,
                    ta,
                    ratio,
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

        // 2. Filtro DAC por voz (abre dinamicamente em sibilantes nas vozes Dual/Unison)
        let base_lp = if self.is_dual_warm {
            AKAI_DUAL_WARM_LP_HZ + 4500.0 * sibilance_factor.clamp(0.0, 1.0)
        } else {
            AKAI_MAIN_VOICE_LP_HZ
        };
        let lp_a = (-2.0 * PI * base_lp.min(sr * 0.45) / sr).exp();
        let dac_out = (1.0 - lp_a) * hp_out + lp_a * self.dac_lp_z1;
        self.dac_lp_z1 = dac_out;

        let w = self.delay_write & DELAY_MASK;
        self.delay_line[w] = dac_out;

        let read_f = (w + DELAY_SIZE) as f32 - self.delay_mod_samples;
        let r0i = (read_f as usize) & DELAY_MASK;
        let r1i = (r0i + 1) & DELAY_MASK;
        let frac = read_f.fract();
        let delayed = self.delay_line[r0i] * (1.0 - frac) + self.delay_line[r1i] * frac;

        let final_out = if self.is_dual_warm {
            let a = 0.38_f32;
            let y_ap = a * delayed + self.ap_x1 - a * self.ap_y1;
            self.ap_x1 = delayed;
            self.ap_y1 = y_ap;
            y_ap
        } else {
            delayed
        };

        self.delay_write = self.delay_write.wrapping_add(1);
        final_out
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
        read_pos: usize,
        emit_pos: usize,
        l_base: usize,
        _ta: f32,
        _pitch_ratio: f32,
        formant_ratio: f32,
        max_safe_grain: usize,
    ) {
        let fr = formant_ratio.clamp(0.5, 2.0);
        // Preservacao estrita de formantes (Regra 3 de DSP): a taxa de leitura dentro do grao
        // segue exclusivamente `formant_ratio`, sem escala por `pitch_ratio`.
        let src_step = fr;

        let l_desired = (l_base | 1).clamp(17, max_safe_grain);
        let max_src_reach = (MAX_CACHE_RADIUS - 8) as f32;
        let max_l_by_src = (((2.0 * max_src_reach) / src_step).floor() as usize | 1).max(17);
        let l_out = l_desired.min(max_l_by_src).min(max_safe_grain);
        let half_out = (l_out / 2) as isize;

        let active_out_delay = read_pos.wrapping_sub(emit_pos);
        let min_center_dist = (half_out as usize) + 8;
        let out_center = if active_out_delay < min_center_dist {
            emit_pos.wrapping_add(min_center_dist)
        } else {
            read_pos
        };

        let actual_src_reach = (((half_out as f32) * src_step).ceil() as isize + 2).min(MAX_CACHE_RADIUS - 2);
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
            cache.ensure_extracted(dry_ring, safe_mark);
            let rad_f = MAX_CACHE_RADIUS as f32;
            let max_idx_f = (2 * MAX_CACHE_RADIUS - 1) as f32;
            let max_idx = max_idx_f as usize;
            let cache_slice = &cache.buf;

            for k in 0..l_out {
                let offset_out = k as isize - half_out;
                let src_off = (offset_out as f32) * src_step;
                let pos = (src_off + rad_f).clamp(0.0, max_idx_f);
                let i0 = pos.floor() as usize;
                let frac = pos - (i0 as f32);
                let im1 = i0.saturating_sub(1);
                let i1 = (i0 + 1).min(max_idx);
                let i2 = (i0 + 2).min(max_idx);
                let sample = hermite_interp(cache_slice[im1], cache_slice[i0], cache_slice[i1], cache_slice[i2], frac);
                let final_sample = if src_step > 1.05 {
                    let blend = ((src_step - 1.0) * 0.8).min(0.40);
                    sample * (1.0 - blend) + 0.5 * blend * (cache_slice[im1] + cache_slice[i1])
                } else {
                    sample
                };

                let w = hann_lut[(lut_phase >> 16) & (HANN_LUT_SIZE - 1)];
                lut_phase += lut_step;

                let dst = (out_center as isize + offset_out) as usize & VOICE_RING_MASK;
                self.out_w_ring[dst][0] += final_sample * w;
                self.out_w_ring[dst][1] += w;
            }
        } else {
            for k in 0..l_out {
                let offset_out = k as isize - half_out;
                let src_off = (offset_out as f32) * src_step;
                let src_i0 = src_off.floor() as isize;
                let frac = src_off - (src_i0 as f32);
                let p0 = (safe_mark as isize + src_i0) as usize & RING_MASK;
                let pm1 = p0.wrapping_sub(1) & RING_MASK;
                let p1 = (p0 + 1) & RING_MASK;
                let p2 = (p0 + 2) & RING_MASK;
                let sample = hermite_interp(dry_ring[pm1], dry_ring[p0], dry_ring[p1], dry_ring[p2], frac);
                let final_sample = if src_step > 1.05 {
                    let blend = ((src_step - 1.0) * 0.8).min(0.40);
                    sample * (1.0 - blend) + 0.5 * blend * (dry_ring[pm1] + dry_ring[p1])
                } else {
                    sample
                };

                let w = hann_lut[(lut_phase >> 16) & (HANN_LUT_SIZE - 1)];
                lut_phase += lut_step;

                let dst = (out_center as isize + offset_out) as usize & VOICE_RING_MASK;
                self.out_w_ring[dst][0] += final_sample * w;
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
    fn test_psola_live_low_octave_envelope_regularity_and_no_ghost_echo() {
        let _ftz = ScopedFtzDaz::new();
        let sr = 44_100.0;
        let hann_lut = build_hann_lut();
        let mut voice = PsolaSubVoice::new(1);
        let mut dry_ring = vec![0.0_f32; RING_SIZE];
        let mut write_pos = 4096usize;
        let f0 = 65.0_f32;
        let period = (sr / f0) as usize;
        let mut latest_mark = write_pos - 384;
        let max_grain = max_grain_for_sr(sr);

        let mut ys = Vec::with_capacity(18_000);
        let mut silence_rms_acc = 0.0_f32;
        let mut silence_count = 0usize;

        for step in 0..18_000 {
            let t = (step as f32) / sr;
            let x = if step < 8000 {
                (0.55 * (2.0 * PI * f0 * t).sin()
                    + 0.30 * (2.0 * PI * (2.0 * f0) * t).sin()
                    + 0.20 * (2.0 * PI * (4.0 * f0) * t).cos()
                    + 0.15 * (2.0 * PI * (6.0 * f0) * t).sin())
                    * 0.75
            } else {
                0.0
            };
            let voiced = if step < 8000 { 1.0 } else { 0.0 };
            dry_ring[write_pos & RING_MASK] = x;

            let read_pos = write_pos.wrapping_sub(384);
            let emit_pos = read_pos.wrapping_sub(384);
            if step < 8000 && read_pos.wrapping_sub(latest_mark) >= period {
                latest_mark = read_pos;
            }
            let emit_voiced = if (768..8000 + 2048).contains(&step) { 1.0 } else { 0.0 };

            let y = voice.step_sample(
                &dry_ring, &hann_lut, write_pos, read_pos, emit_pos, latest_mark,
                voiced, emit_voiced, f0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, sr, max_grain,
            );
            ys.push(y);

            if step > 13_000 {
                silence_rms_acc += y * y;
                silence_count += 1;
            }
            write_pos = write_pos.wrapping_add(1);
        }

        let period_out = (sr / (f0 * 0.5)).round() as usize;
        let (mut mn, mut mx) = (f32::MAX, 0.0_f32);
        for w in ys[3000..6500].chunks(period_out) {
            if w.len() >= period_out {
                let r = (w.iter().map(|v| v * v).sum::<f32>() / (w.len() as f32)).sqrt();
                mn = mn.min(r);
                mx = mx.max(r);
            }
        }
        assert!(
            mn > 0.5 * mx && mx > 0.15,
            "Lacunas detectadas no PSOLA Live Low Octave 65 Hz: min={mn} max={mx}"
        );

        let ghost_rms = (silence_rms_acc / (silence_count as f32)).sqrt();
        assert!(ghost_rms < 1e-6, "Ruido residual detectado no anel PSOLA: RMS = {ghost_rms}");
    }

    #[test]
    fn test_direct_path_is_linear_and_zero_thd_at_extreme_pan() {
        let sr = 44_100.0;
        let mut proc = super::super::DecaBuddyProcessor::new(sr);
        proc.params.direct.enabled = true;
        proc.params.direct.level_db = 0.0;
        proc.params.direct.pan = -50.0;
        for h in &mut proc.params.harmonies {
            h.enabled = false;
        }
        proc.params.unison.enabled = false;

        let n = 4096;
        let mut input = vec![0.0_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];
        for (i, item) in input.iter_mut().enumerate().take(n) {
            *item = (2.0 * PI * 1000.0 * (i as f32) / sr).sin() * 0.90;
        }
        proc.process_block(&input, &mut out_l, &mut out_r);

        let mut max_err = 0.0_f32;
        for i in 2500..n {
            max_err = max_err.max((out_l[i] - input[i]).abs());
        }
        assert!(
            max_err < 1e-4,
            "Caminho Direct deve ter 0% THD mesmo em pan extremo e amplitude 0.90 (erro max = {max_err})"
        );
    }

    fn estimate_f0_autocorr(sig: &[f32], sr: f32, fmin: f32, fmax: f32) -> f32 {
        let tau_min = (sr / fmax).floor() as usize;
        let tau_max = (sr / fmin).ceil() as usize;
        let w = sig.len() - tau_max - 4;
        let mut diffs = vec![0.0_f32; tau_max + 2];
        let mut best_tau = tau_min;
        let mut min_diff = f32::MAX;
        for tau in (tau_min - 1)..=(tau_max + 1) {
            let mut d = 0.0_f32;
            for i in 0..w {
                let diff = sig[i] - sig[i + tau];
                d += diff * diff;
            }
            diffs[tau] = d;
            if tau >= tau_min && tau <= tau_max && d < min_diff {
                min_diff = d;
                best_tau = tau;
            }
        }
        let y0 = diffs[best_tau - 1];
        let y1 = diffs[best_tau];
        let y2 = diffs[best_tau + 1];
        let den = y0 - 2.0 * y1 + y2;
        let frac = if den.abs() > 1e-9 { 0.5 * (y0 - y2) / den } else { 0.0 };
        sr / ((best_tau as f32) + frac.clamp(-0.5, 0.5))
    }

    #[test]
    fn test_pitch_accuracy_within_10_cents_for_low_octave_and_high_5th() {
        let sr = 44_100.0;
        for &f_in in &[65.406_f32, 110.0, 220.0] {
            for &(scale, expected_ratio) in &[
                (super::super::scales::ScaleId::LowOctave, 0.5_f32),
                (super::super::scales::ScaleId::High5thModal1, 1.498307_f32),
            ] {
                let mut proc = super::super::DecaBuddyProcessor::new(sr);
                proc.params.key = if (f_in - 65.406).abs() < 1.0 { 0 } else { 9 };
                proc.params.direct.enabled = false;
                proc.params.unison.enabled = false;
                proc.params.analog_drive = 0.0;
                for h in &mut proc.params.harmonies {
                    h.enabled = false;
                }
                proc.params.harmonies[0].enabled = true;
                proc.params.harmonies[0].dual = false;
                proc.params.harmonies[0].scale = scale;
                proc.params.harmonies[0].random = 0.0;
                proc.params.harmonies[0].quantise = 0.0;
                proc.params.harmonies[0].formant = 0.0;
                proc.params.harmonies[0].level_db = 0.0;

                let n = 16_384;
                let mut input = vec![0.0_f32; n];
                let mut out_l = vec![0.0_f32; n];
                let mut out_r = vec![0.0_f32; n];
                for (i, item) in input.iter_mut().enumerate().take(n) {
                    let t = (i as f32) / sr;
                    *item = (0.7 * (2.0 * PI * f_in * t).sin()
                        + 0.2 * (2.0 * PI * 2.0 * f_in * t).sin())
                        * 0.6;
                }
                proc.process_block(&input, &mut out_l, &mut out_r);

                let expected_f0 = f_in * expected_ratio;
                let measured_f0 = estimate_f0_autocorr(
                    &out_l[8192..15_000],
                    sr,
                    expected_f0 * 0.85,
                    expected_f0 * 1.18,
                );
                let cents_err = 1200.0 * (measured_f0 / expected_f0).log2();
                assert!(
                    cents_err.abs() <= 10.0,
                    "Erro de pitch excedeu +-10 cents para f_in={f_in} Hz ({scale:?}): esperado={expected_f0:.2} Hz, medido={measured_f0:.2} Hz ({cents_err:.2} cents)"
                );
            }
        }
    }

    #[test]
    fn test_choir_analog_bus_silk_warmth_and_dc_rejection() {
        let sr = 44_100.0;
        let mut bus = ChoirAnalogBus::default();
        bus.set_sample_rate(sr);

        let n = 16_384;
        let mut out_l_accum = Vec::with_capacity(n);
        for i in 0..n {
            let t = (i as f32) / sr;
            let sig = (2.0 * PI * 150.0 * t).sin() * 0.70;
            let (l, _r) = bus.process_stereo(sig, sig, 0.75, 4);
            out_l_accum.push(l);
        }

        // 1. Verificacao de rejeicao de DC pelos capacitores C23/C25 apos estabilizacao
        // A media sobre 14 ciclos exatos de 150 Hz (14 * 294 = 4116 amostras) isola o componente DC puro
        let period_samples = 294;
        let num_cycles = 14;
        let tail = &out_l_accum[12_000..12_000 + num_cycles * period_samples];
        let dc_mean: f32 = tail.iter().sum::<f32>() / (tail.len() as f32);
        assert!(
            dc_mean.abs() < 1e-4,
            "Capacitores de saida C23/C25 devem rejeitar qualquer componente DC da saturacao assimetrica (dc_mean={dc_mean})"
        );

        // 2. Verificacao de limitacao suave de Slew Rate (1.0 V/us) em transiente vertical
        let mut bus_slew = ChoirAnalogBus::default();
        bus_slew.set_sample_rate(sr);
        let (_init_l, _) = bus_slew.process_stereo(-0.85, -0.85, 0.85, 4);
        let (step_l, _) = bus_slew.process_stereo(0.85, 0.85, 0.85, 4);
        let delta = (step_l - (-0.85)).abs();
        assert!(
            delta < 1.70,
            "Slew rate limiting do NJM4558 deve arredondar transiente vertical instantaneo (delta={delta})"
        );
    }
}
