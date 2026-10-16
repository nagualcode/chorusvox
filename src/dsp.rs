//! Motor DSP da emulacao do Akai DecaBuddy HV10.
//!
//! Cadeia do hardware (manual de servico, PC MAIN L8020A501A): entrada MIC/LINE -> NJM4558L ->
//! codec Burr-Brown PCM3003E (cristal 11.2896 MHz = 256 x 44.1 kHz) -> DSP ADSP-2186M (16 bits
//! ponto fixo) -> PCM3003E -> NJM4558L -> saidas. Nao ha circuito de "saturacao" dedicado: as
//! nao-linearidades reais sao o ceifamento do ADC, a saturacao da aritmetica de 16 bits do DSP e
//! a quantizacao de 16 bits.
//!
//! Transposicao: o motor de transposicao do plugin Akai DecaBuddy v1.02 (portado do firmware
//! do DSP: osciladores Q15 a 1 kHz, constantes em amostras) segue a regra da patente IVL
//! US 5,567,901:
//! - fonte travada numa grade periodica (marca anterior + 1 periodo), sem busca de pico;
//! - janela triangular de 2*Ts para cima e 2*P para baixo (na oitava abaixo as janelas so se
//!   tocam, gerando a modulacao no f0 de saida tipica desses aparelhos);
//! - soma das cabecas sem normalizacao, interpolacao linear, formant linear +-25%;
//! - em trechos nao vozeados as harmonias passam a entrada em uníssono (sibilantes audiveis).

use std::f32::consts::PI;

pub const RING_SIZE: usize = 1 << 15;
pub const RING_MASK: usize = RING_SIZE - 1;

/// Constantes de hardware/algoritmo, com a fonte de cada valor.
pub mod hw {
    /// Taxa nativa do HV10 (cristal X1 11.2896 MHz = 256 fs). As constantes em amostras do DSP
    /// sao escaladas por sr / NATIVE_SR.
    pub const NATIVE_SR: f32 = 44_100.0;
    /// Periodo usado em trechos nao vozeados (DSP: 128 amostras, graos de 256 em uníssono).
    pub const UNVOICED_PERIOD_44K: f32 = 128.0;
    /// Ganho da voz unison (sub-voz "sec") relativo ao LEVEL da parte (DSP: 0.7).
    pub const UNISON_GAIN: f32 = 0.7;
    /// Abertura de pan entre voz principal e unison: d = 0.35*s*(0.5-|pan|) (DSP).
    pub const UNISON_PAN_SPREAD: f32 = 0.35;
    /// Faixa de transposicao por MIDI (DSP: +-12 semitons).
    pub const MIDI_SHIFT_RANGE: f32 = 12.0;
}

/// Guard RAII que ativa FTZ/DAZ.
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

/// Coeficiente de um polo para uma constante de tempo em ms.
#[inline]
pub fn one_pole_coef(ms: f32, sr: f32) -> f32 {
    (-1.0 / (ms.max(1e-3) * 0.001 * sr)).exp()
}

#[inline]
pub fn hz_to_midi(f_hz: f32, tune_hz: f32) -> f32 {
    69.0 + 12.0 * (f_hz.max(1e-3) / tune_hz).log2()
}

#[inline]
pub fn semitones_to_ratio(st: f32) -> f32 {
    (st * (std::f32::consts::LN_2 / 12.0)).exp()
}

/// Quantizacao simetrica de 16 bits com saturacao (aritmetica saturada do ADSP-2186M / codec).
#[inline(always)]
pub fn quantize_16(x: f32) -> f32 {
    (x.clamp(-1.0, 32_767.0 / 32_768.0) * 32_768.0).round() * (1.0 / 32_768.0)
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

    /// Uniforme em [-1, 1].
    #[inline(always)]
    pub fn next_bipolar(&mut self) -> f32 {
        (self.next_u32() as f32) * (2.0 / (u32::MAX as f32)) - 1.0
    }

    /// Uniforme em [0, 1].
    #[inline(always)]
    pub fn next_unit(&mut self) -> f32 {
        (self.next_u32() as f32) * (1.0 / (u32::MAX as f32))
    }
}

/// Biquad (RBJ) em forma transposta direta II.
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, z1: f32, z2: f32,
}

impl Biquad {
    pub fn lowpass(fc: f32, q: f32, sr: f32) -> Self {
        let w0 = 2.0 * PI * fc.clamp(10.0, sr * 0.49) / sr;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - c) * 0.5 / a0,
            b1: (1.0 - c) / a0,
            b2: (1.0 - c) * 0.5 / a0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn highpass(fc: f32, q: f32, sr: f32) -> Self {
        let w0 = 2.0 * PI * fc.clamp(1.0, sr * 0.49) / sr;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 + c) * 0.5 / a0,
            b1: -(1.0 + c) / a0,
            b2: (1.0 + c) * 0.5 / a0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
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

/// Passa-baixas Butterworth de 4a ordem (duas secoes).
#[derive(Clone, Copy, Debug, Default)]
pub struct Butter4 {
    s1: Biquad,
    s2: Biquad,
}

impl Butter4 {
    pub fn lowpass(fc: f32, sr: f32) -> Self {
        Self { s1: Biquad::lowpass(fc, 0.541_196_1, sr), s2: Biquad::lowpass(fc, 1.306_563, sr) }
    }

    pub fn reset(&mut self) { self.s1.reset(); self.s2.reset(); }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 { self.s2.process(self.s1.process(x)) }
}

// ---------------------------------------------------------------------------------------------
// Transpositor de duas cabecas sincronizado ao periodo (motor DSP do DecaBuddy, 0x10006100)
// ---------------------------------------------------------------------------------------------

/// Pico da janela triangular (constante Q15 0.99997 do DSP).
const TRI_PEAK: f32 = 0.999_97;

#[derive(Clone, Copy, Debug, Default)]
struct ShiftHead {
    pos: f64,
    g: f32,
    slope: f32,
    active: bool,
}

/// Parametros de uma sub-voz, recalculados a cada leitura de pitch (em degrau, como no DSP).
#[derive(Clone, Copy, Debug)]
pub struct ShiftParams {
    /// Periodo da fonte P (amostras).
    pub period: f32,
    /// Intervalo entre graos Ts = P * 2^(-shift/12).
    pub ts: f32,
    /// Inclinacao da janela triangular: 1/Ts para cima, 1/P para baixo.
    pub slope: f32,
    /// Taxa de leitura dentro do grao (formant).
    pub fr: f32,
}

impl ShiftParams {
    pub fn new(period: f32, shift_semitones: f32, fr: f32) -> Self {
        let p = period.max(8.0);
        let ts = (p * semitones_to_ratio(-shift_semitones)).max(4.0);
        let slope = if shift_semitones >= 0.0 { 1.0 / ts } else { 1.0 / p };
        Self { period: p, ts, slope, fr }
    }
}

/// Sub-voz transpositora. A cada Ts amostras de saida dispara um grao triangular que comeca na
/// grade de periodo A (A avanca so em multiplos inteiros de P, mantendo A + atraso em
/// (w - 1.5P, w - 0.5P]) e le a fonte com interpolacao linear a taxa `fr`. Como no DSP, duas
/// cabecas se alternam (flag de toggle) e sao somadas sem normalizacao; cada grao guarda a
/// inclinacao do momento em que disparou.
///
/// Unico desvio: quando Ts encurta bruscamente (fim de nota, voz -> sem pitch, salto de nota),
/// o DSP reinicia a cabeca mais antiga mesmo que ela ainda soe, o que gera um click audivel.
/// Aqui esse grao termina numa cabeca reserva (duas: na passagem para "sem pitch" as duas
/// cabecas antigas sao substituidas em seguida). Fora desses momentos a saida e a do DSP.
#[derive(Clone, Debug)]
pub struct PitchShifter {
    /// Duas cabecas alternadas (0, 1) e duas reservas (2, 3).
    heads: [ShiftHead; 4],
    toggle: usize,
    since: f32,
    grid: f64,
    grid_valid: bool,
}

impl Default for PitchShifter {
    fn default() -> Self {
        Self { heads: [ShiftHead::default(); 4], toggle: 0, since: f32::MAX, grid: 0.0, grid_valid: false }
    }
}

impl PitchShifter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `w` = posicao absoluta da amostra mais recente gravada em `ring`.
    /// `delay` = atraso adicional da fonte em amostras (RAND.DELAY / unison).
    #[inline]
    pub fn step(&mut self, ring: &[f32], w: u64, p: &ShiftParams, delay: f32) -> f32 {
        let wf = w as f64;
        self.since += 1.0;
        if self.since >= p.ts {
            self.since = if self.since >= 2.0 * p.ts { 0.0 } else { self.since - p.ts };
            self.trigger(wf, p, delay);
        }

        let mut y = 0.0_f32;
        let limit = wf - 1.0;
        for h in &mut self.heads {
            if !h.active { continue; }
            let pos = h.pos.min(limit);
            let i0 = pos.floor();
            let f = (pos - i0) as f32;
            let i0 = i0 as i64 as usize;
            let a = ring[i0 & RING_MASK];
            let b = ring[i0.wrapping_add(1) & RING_MASK];
            let weight = TRI_PEAK - h.g.abs();
            y += (a + (b - a) * f) * weight;
            h.pos += p.fr as f64;
            h.g += h.slope;
            if h.g >= TRI_PEAK { h.active = false; }
        }
        y
    }

    fn trigger(&mut self, wf: f64, p: &ShiftParams, delay: f32) {
        let period = p.period as f64;
        let anchor = wf - delay as f64;
        if !self.grid_valid {
            self.grid = anchor - period;
            self.grid_valid = true;
        }
        // Mantem a fonte na janela (w - 1.5P, w - 0.5P] deslocada pelo atraso.
        let x = anchor - self.grid;
        if x >= 1.5 * period {
            let k = ((x - 0.5 * period) / period).floor();
            self.grid += k * period;
        } else if x < 0.5 * period {
            let k = ((0.5 * period - x) / period).ceil();
            self.grid -= k * period;
        }

        let old = self.heads[self.toggle];
        if old.active && TRI_PEAK - old.g.abs() > 0.0 {
            let weight = |h: &ShiftHead| if h.active { TRI_PEAK - h.g.abs() } else { -1.0 };
            let spare = if weight(&self.heads[2]) <= weight(&self.heads[3]) { 2 } else { 3 };
            self.heads[spare] = old;
        }
        self.heads[self.toggle] = ShiftHead { pos: self.grid, g: -TRI_PEAK, slope: p.slope, active: true };
        self.toggle ^= 1;
    }
}

// ---------------------------------------------------------------------------------------------
// Moduladores de atraso aleatorio (3 osciladores senoidais Q15 a 1 kHz, 0x100067a0)
// ---------------------------------------------------------------------------------------------

/// Frequencias dos tres osciladores a 44.1 kHz (coeficientes proporcionais a 1/fs no DSP).
const LFO_HZ: [f64; 3] = [0.42, 0.74, 1.50];
/// Amplitude Q15 dos osciladores (raio 23170 = 0.707 * 32768).
const LFO_AMP: f64 = 23_170.0;

#[derive(Clone, Debug)]
pub struct RandomLfo {
    phase: [f64; 3],
    inc: [f64; 3],
    counter: usize,
    period: usize,
    /// Saidas x0, x1, x2 (escala Q15).
    pub x: [f32; 3],
}

impl RandomLfo {
    pub fn new(sr: f32) -> Self {
        let mut l = Self { phase: [0.0, 2.1, 4.2], inc: [0.0; 3], counter: 0, period: 44, x: [0.0; 3] };
        l.set_sample_rate(sr);
        l
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.period = ((sr / 1000.0).round() as usize).max(1);
        for (inc, hz) in self.inc.iter_mut().zip(LFO_HZ) {
            *inc = 2.0 * std::f64::consts::PI * hz * self.period as f64 / sr as f64;
        }
        self.update_outputs();
    }

    pub fn reset(&mut self) {
        self.phase = [0.0, 2.1, 4.2];
        self.counter = 0;
        self.update_outputs();
    }

    fn update_outputs(&mut self) {
        for k in 0..3 {
            self.x[k] = (LFO_AMP * self.phase[k].sin()) as f32;
        }
    }

    /// Avanca uma amostra; os osciladores atualizam na taxa de controle de 1 kHz.
    #[inline]
    pub fn tick(&mut self) {
        self.counter += 1;
        if self.counter >= self.period {
            self.counter = 0;
            for k in 0..3 {
                self.phase[k] = (self.phase[k] + self.inc[k]) % (2.0 * std::f64::consts::PI);
            }
            self.update_outputs();
        }
    }
}

/// Combinacoes dos osciladores e centros (amostras*16 a 44.1 kHz) do DSP para cada voz.
/// Indice 0 = unison da voz original, 1..4 = L2, L1, H1, H2.
struct DelayRecipe {
    center: f32,
    sign: f32,
    coef: [f32; 3],
}

const PRI_DELAY: [DelayRecipe; 5] = [
    DelayRecipe { center: 14_111.0, sign: 1.0, coef: [6_545.0, 8_276.0, -3_111.0] },
    DelayRecipe { center: 15_360.0, sign: 1.0, coef: [10_471.0, 5_485.0, 1_828.0] },
    DelayRecipe { center: 16_731.0, sign: 1.0, coef: [8_932.0, 5_215.0, -2_419.0] },
    DelayRecipe { center: 13_549.0, sign: -1.0, coef: [10_471.0, 5_485.0, 1_828.0] },
    DelayRecipe { center: 12_345.0, sign: -1.0, coef: [-11_328.0, 1_865.0, -3_639.0] },
];

/// Atrasos sempre ativos da voz unison de cada harmonia (sub-voz "sec" do DSP).
const UNISON_DELAY: [DelayRecipe; 4] = [
    DelayRecipe { center: 12_934.0, sign: 1.0, coef: [-7_828.0, 6_765.0, 2_125.0] },
    DelayRecipe { center: 12_465.0, sign: 1.0, coef: [-11_328.0, 1_865.0, -3_639.0] },
    DelayRecipe { center: 11_977.0, sign: -1.0, coef: [8_932.0, 5_215.0, -2_419.0] },
    DelayRecipe { center: 13_123.0, sign: -1.0, coef: [-7_828.0, 6_765.0, 2_125.0] },
];

#[inline]
fn recipe_samples(r: &DelayRecipe, x: &[f32; 3], modulated: bool, sr_scale: f32) -> f32 {
    let m = if modulated { (r.coef[0] * x[0] + r.coef[1] * x[1] + r.coef[2] * x[2]) / 32_768.0 } else { 0.0 };
    ((r.center + r.sign * m) / 16.0) * sr_scale
}

/// Atraso (amostras) da voz principal: `voice` 0 = unison da original, 1..4 = L2..H2.
/// R1..R9 escala o atraso modulado (rnd = n/9); D1..D9 fixa o atraso no centro * n/9.
pub fn primary_delay(voice: usize, mode: RandDelay, lfo: &RandomLfo, sr: f32) -> f32 {
    let scale = sr / hw::NATIVE_SR;
    let r = &PRI_DELAY[voice.min(4)];
    match mode {
        RandDelay::Off => 0.0,
        RandDelay::Random(n) => (n as f32 / 9.0) * recipe_samples(r, &lfo.x, true, scale),
        RandDelay::Fixed(n) => (n as f32 / 9.0) * recipe_samples(r, &lfo.x, false, scale),
    }
}

/// Atraso (amostras) da voz unison de uma harmonia (`part` 0..4 = L2..H2), sempre modulado.
pub fn unison_delay(part: usize, lfo: &RandomLfo, sr: f32) -> f32 {
    recipe_samples(&UNISON_DELAY[part.min(3)], &lfo.x, true, sr / hw::NATIVE_SR)
}

/// Desafinacao simetrica (semitons) do par principal/unison: random/1024, random = n*100/9.
pub fn rand_detune_semitones(mode: RandDelay) -> f32 {
    match mode {
        RandDelay::Random(n) => (n as f32 * 100.0 / 9.0) / 1024.0,
        _ => 0.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandDelay {
    Off,
    Fixed(u8),
    Random(u8),
}

impl RandDelay {
    /// Indice 0..18: --, D1..D9, R1..R9.
    pub fn from_index(i: usize) -> Self {
        match i {
            1..=9 => RandDelay::Fixed(i as u8),
            10..=18 => RandDelay::Random((i - 9) as u8),
            _ => RandDelay::Off,
        }
    }
}

/// FORMANT do HV10 (-9..9) -> taxa de leitura do grao. O DSP usa 1 + 0.005*v, v em -50..50,
/// limitado a 0.75..1.25 (linear, +-25%).
#[inline]
pub fn formant_ratio(step: i32) -> f32 {
    let v = step.clamp(-9, 9) as f32 * (50.0 / 9.0);
    (1.0 + 0.005 * v).clamp(0.75, 1.25)
}

/// Lei de pan do DSP: g(x) = x / sqrt(x^2 + (1-x)^2), -3 dB no centro. `pos` em 0..1.
#[inline]
pub fn pan_gains(pos: f32) -> (f32, f32) {
    let g = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        let d = (x * x + (1.0 - x) * (1.0 - x)).sqrt();
        if d > 0.0 { x / d } else { 0.0 }
    };
    (g(1.0 - pos), g(pos))
}

/// LEVEL 0..9 do HV10 -> ganho linear (0 = mudo, passos de 3 dB, 9 = 0 dB).
#[inline]
pub fn level_gain(level: f32) -> f32 {
    if level <= 0.0 { 0.0 } else { 10.0_f32.powf((level.min(9.0) - 9.0) * 3.0 / 20.0) }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_16_saturates() {
        assert_eq!(quantize_16(2.0), 32_767.0 / 32_768.0);
        assert_eq!(quantize_16(-2.0), -1.0);
        assert!((quantize_16(0.25) - 0.25).abs() < 1e-7);
    }

    #[test]
    fn shifter_unison_reproduces_input_and_octave_down_halves_pitch() {
        let sr = 44_100.0;
        let f0 = 220.0_f32;
        let n = 16_384;
        let mut ring = vec![0.0; RING_SIZE];
        let period = sr / f0;
        for &(shift, expect) in &[(0.0_f32, f0), (-12.0, f0 * 0.5), (7.0, f0 * 1.498_307)] {
            let mut sh = PitchShifter::default();
            let p = ShiftParams::new(period, shift, 1.0);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                ring[i & RING_MASK] = (2.0 * PI * f0 * i as f32 / sr).sin() * 0.5
                    + (2.0 * PI * 2.0 * f0 * i as f32 / sr).sin() * 0.2;
                out.push(sh.step(&ring, i as u64, &p, 0.0));
            }
            let seg = &out[4096..12_288];
            let f = crate::pitch::measure_f0(seg, sr);
            let cents = 1200.0 * (f / expect).log2();
            assert!(cents.abs() < 8.0, "shift {shift}: esperado {expect} Hz, medido {f} Hz");
        }
    }

    #[test]
    fn voice_to_unvoiced_switch_does_not_cut_sounding_grains() {
        // Fim de glissando: harmonia +4 st num periodo longo vira unison com P = 128 (como o
        // DSP faz ao perder o pitch). Nenhum grao ainda audivel pode ser cortado.
        let sr = 44_100.0;
        let f0 = 110.0_f32;
        let mut ring = vec![0.0; RING_SIZE];
        let mut sh = PitchShifter::default();
        let voiced = ShiftParams::new(sr / f0, 4.0, 1.0);
        let unvoiced = ShiftParams::new(128.0, 0.0, 1.0);
        let mut out = Vec::new();
        for i in 0..12_000usize {
            ring[i & RING_MASK] = (2.0 * PI * f0 * i as f32 / sr).sin() * 0.5;
            let p = if i < 8_000 { &voiced } else { &unvoiced };
            out.push(sh.step(&ring, i as u64, p, 0.0));
        }
        let jump = |r: std::ops::Range<usize>| r.map(|i| (out[i + 1] - out[i]).abs()).fold(0.0_f32, f32::max);
        let steady = jump(4_000..8_000);
        let switch = jump(8_000..9_000);
        assert!(switch < steady * 1.5, "salto na troca {switch} vs regime {steady}");
    }

    #[test]
    fn dsp_constants_match_reverse_engineered_ranges() {
        assert!((formant_ratio(9) - 1.25).abs() < 1e-6);
        assert!((formant_ratio(-9) - 0.75).abs() < 1e-6);
        let (l, r) = pan_gains(0.5);
        assert!((l - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6 && (r - l).abs() < 1e-6);
        assert!((rand_detune_semitones(RandDelay::Random(9)) * 100.0 - 9.765_625).abs() < 1e-3);
        let lfo = RandomLfo::new(44_100.0);
        // R9 na voz L2: ~22 ms +- 18 ms
        for _ in 0..1 {
            let d = primary_delay(1, RandDelay::Random(9), &lfo, 44_100.0);
            assert!((170.0..1750.0).contains(&d), "{d}");
        }
    }
}
