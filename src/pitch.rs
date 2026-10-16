//! Rastreador de pitch original do DecaBuddy (port de `DecaBuddy.dll` v1.02, 0x10008cd0,
//! 0x10008e60 e 0x100092d0; estilo IVL US 4,688,464).
//!
//! No dominio do tempo, sincrono com o pitch:
//! 1. dois passa-baixas de 2 polos em cascata (fc = 500 Hz) e integral corrida do sinal;
//! 2. cada cruzamento de zero vira um evento (instante interpolado, inclinacao, area do lobo);
//! 3. so cruzamentos "fortes" (inclinacao acima de 0.45 x o maximo anterior, por polaridade)
//!    viram candidatos;
//! 4. num candidato, busca pares de periodos consecutivos (d1 = atual - j1, d2 = j1 - j2) entre
//!    candidatos da mesma polaridade e da uma nota de similaridade (periodo, areas dos lobos,
//!    inclinacoes). Aceita a melhor; prefere o periodo menor quando o 2o melhor tambem e bom;
//! 5. saltos de 2.5 semitons ou mais precisam de um cruzamento mais forte, ou de nota nova;
//! 6. hold de voicing de 50 ms por polaridade; unvoiced = os dois holds expiraram.
//!
//! Aritmetica: o DLL roda em x87 com precisao de 53 bits e grava estados em `float`. Aqui os
//! calculos sao em `f64` e os estados em `f32`, nos mesmos pontos.

const EVENTS: usize = 64;
const EV_MASK: usize = EVENTS - 1;
/// Relogio de amostras do rastreador (2^18, com comparacoes em meio anel).
const CLOCK_MASK: u32 = 0x3_FFFF;
const CLOCK_HALF: u32 = 0x2_0000;
const CLOCK_SPAN: f64 = 262_144.0;

/// O DLL limita cada periodo do par a 1000 amostras (44.1 Hz a 44.1 kHz); escalado pela taxa.
const MAX_PERIOD_44K: f64 = 1000.0;
const MAX_CANDIDATES: u32 = 16;
const MAX_STEPS: u32 = 31;

/// 12/ln 2 e 69 - 12*log2(440): midi = 12*log2(fs/P) - 36.376 (constantes do DLL).
const MIDI_SCALE: f64 = 17.312_340_490_667_56;
const MIDI_OFFSET: f64 = -36.376_316_562_295_91;

#[derive(Clone, Copy, Debug, Default)]
struct ZeroCrossing {
    /// Amostra do cruzamento (relogio de 18 bits).
    sample: u32,
    /// 1.0 se e candidato (no DLL o campo guarda antes a amostra anterior).
    candidate: f32,
    /// Inclinacao no cruzamento (antes de processado: a amostra apos o cruzamento).
    slope: f32,
    /// Instante interpolado do cruzamento.
    time: f64,
    /// Area do lobo que termina neste cruzamento (antes: integral corrida).
    area: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct TwoPole {
    y1: f32,
    y2: f32,
    a1: f32,
    a2: f32,
    g: f32,
}

impl TwoPole {
    /// Passa-baixas de 2 polos em 500 Hz, raio exp(-1.25*pi*500/fs), ganho DC 1 (0x10008ab0).
    fn new(sr: f64) -> Self {
        let w = 500.0 / sr;
        let r = (-std::f64::consts::PI * w * 1.25).exp();
        let a1 = ((2.0 * std::f64::consts::PI * w).cos() * 2.0 * r) as f32;
        let a2 = (-2.0 * std::f64::consts::PI * w * 1.25).exp() as f32;
        let g = ((1.0 - a1 as f64) + a2 as f64) as f32;
        Self { y1: 0.0, y2: 0.0, a1, a2, g }
    }

    /// Devolve a saida em precisao estendida (o DLL encadeia sem arredondar).
    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = (self.a1 as f64 * self.y1 as f64 - self.a2 as f64 * self.y2 as f64) + x * self.g as f64;
        self.y2 = self.y1;
        self.y1 = y as f32;
        y
    }
}

#[inline]
fn clock_reached(now: u32, deadline: u32) -> bool {
    (now.wrapping_sub(deadline) & CLOCK_MASK) < CLOCK_HALF
}

#[inline]
fn wrap_span(mut d: f64) -> f64 {
    if d < 0.0 {
        while d < 0.0 { d += CLOCK_SPAN; }
    } else {
        while d >= CLOCK_SPAN { d -= CLOCK_SPAN; }
    }
    d
}

#[derive(Clone, Debug)]
pub struct PitchTracker {
    lp: [TwoPole; 2],
    events: [ZeroCrossing; EVENTS],
    write: usize,
    current: usize,
    clock: u32,
    integral: f32,
    last_area: f32,
    prev_y: f32,
    slope: f32,

    min_slope: f32,
    min_strength: f32,
    max_period: f32,
    max_pair_period: f64,
    ln_sr: f32,
    hold_samples: u32,

    pos_peak: f32,
    pos_level: f32,
    neg_peak: f32,
    neg_level: f32,
    hold_pos: u32,
    hold_neg: u32,
    lock_until: u32,
    last_strength: f32,
    last_slope: f32,
    last_midi: f32,

    /// MIDI (69 = A 440) da ultima leitura.
    pub midi: f32,
    /// Periodo da ultima leitura, em amostras.
    pub period: f32,
    pub voiced: bool,
}

impl PitchTracker {
    pub fn new(sr: f32) -> Self {
        let mut t = Self {
            lp: [TwoPole::default(); 2],
            events: [ZeroCrossing::default(); EVENTS],
            write: 0, current: 0, clock: 0, integral: 0.0, last_area: 0.0, prev_y: 0.0, slope: 0.0,
            min_slope: 0.0, min_strength: 0.0, max_period: 0.0, max_pair_period: 0.0, ln_sr: 0.0, hold_samples: 0,
            pos_peak: 0.0, pos_level: 0.0, neg_peak: 0.0, neg_level: 0.0,
            hold_pos: 0, hold_neg: 0, lock_until: 0, last_strength: 0.0, last_slope: 0.0, last_midi: 0.0,
            midi: 0.0, period: 0.0, voiced: false,
        };
        t.set_sample_rate(sr);
        t
    }

    /// Setup do DLL (0x10008cd0); limiares de inclinacao proporcionais a 1/fs.
    pub fn set_sample_rate(&mut self, sr: f32) {
        let sr = sr as f64;
        self.lp = [TwoPole::new(sr); 2];
        self.min_slope = (40.374_755_859_375 / sr) as f32;
        self.min_strength = (107.666_015_625 / sr) as f32;
        self.max_period = (0.024_801_587_685_942_65 * sr) as f32;
        self.ln_sr = sr.ln() as f32;
        self.hold_samples = (50.0 * sr * 0.001) as u32;
        self.max_pair_period = MAX_PERIOD_44K * sr / 44_100.0;
        self.reset();
    }

    pub fn reset(&mut self) {
        for f in &mut self.lp {
            f.y1 = 0.0;
            f.y2 = 0.0;
        }
        self.events = [ZeroCrossing::default(); EVENTS];
        self.write = 0;
        self.current = 0;
        self.clock = 0;
        self.integral = 0.0;
        self.last_area = 0.0;
        self.prev_y = 0.0;
        self.slope = 0.0;
        self.pos_peak = 0.0;
        self.pos_level = 0.0;
        self.neg_peak = 0.0;
        self.neg_level = 0.0;
        self.hold_pos = 0;
        self.hold_neg = 0;
        self.lock_until = 0;
        self.last_strength = 0.0;
        self.last_slope = 0.0;
        self.last_midi = 0.0;
        self.midi = 0.0;
        self.period = 0.0;
        self.voiced = false;
    }

    /// Uma amostra (0x10008e60). Devolve true quando ha leitura nova ou o voicing muda.
    #[inline]
    pub fn process(&mut self, x: f32) -> bool {
        let y1 = self.lp[0].process(x as f64);
        let y = self.lp[1].process(y1);
        self.integral = (self.integral as f64 + y) as f32;

        let prev = self.prev_y;
        let crossed = if y < 0.0 { prev >= 0.0 } else { prev < 0.0 };
        if crossed {
            self.write = (self.write + 1) & EV_MASK;
            self.events[self.write] = ZeroCrossing {
                sample: self.clock,
                candidate: prev,
                slope: y as f32,
                time: 0.0,
                area: self.integral,
            };
        }
        self.prev_y = y as f32;

        let mut event = false;
        if self.write != self.current {
            self.current = self.write;
            event = self.on_crossing();
        }

        // Holds de voicing por polaridade; enquanto expirados, acompanham o relogio.
        let next = (self.clock + 1) & CLOCK_MASK;
        let mut unvoiced = true;
        if clock_reached(next, self.hold_pos) { self.hold_pos = self.clock; } else { unvoiced = false; }
        if clock_reached(next, self.hold_neg) { self.hold_neg = self.clock; } else { unvoiced = false; }
        if unvoiced == self.voiced {
            self.voiced = !unvoiced;
            event = true;
        }
        self.clock = next;
        event
    }

    fn on_crossing(&mut self) -> bool {
        let e = &mut self.events[self.current];
        let slope = (e.slope as f64 - e.candidate as f64) as f32;
        self.slope = slope;
        let t = wrap_span((e.sample & CLOCK_MASK) as f64 - e.slope as f64 / slope as f64);
        e.time = t;
        e.slope = slope;
        let area = e.area;
        e.area = (area as f64 - self.last_area as f64) as f32;
        self.last_area = area;
        e.candidate = 0.0;
        let sample = e.sample;

        let s = slope as f64;
        let decay = 0.45_f32 as f64;
        let ninety = 0.9_f32 as f64;
        if s > self.min_slope as f64 {
            let d = self.pos_peak as f64 * decay;
            if d >= s {
                self.pos_peak = d as f32;
                return false;
            }
            self.pos_peak = slope;
            let d = self.pos_level as f64 * decay;
            if s <= d {
                self.pos_level = d as f32;
                return false;
            }
            self.pos_level = slope;
            self.events[self.current].candidate = 1.0;
            if clock_reached(sample, self.hold_neg) || ninety * (self.neg_level as f64) < self.pos_level as f64 {
                return self.estimate();
            }
        } else if s < -(self.min_slope as f64) {
            let d = self.neg_peak as f64 * decay;
            if s >= -d {
                self.neg_peak = d as f32;
                return false;
            }
            self.neg_peak = -slope;
            let d = self.neg_level as f64 * decay;
            if s >= -d {
                self.neg_level = d as f32;
                return false;
            }
            self.neg_level = -slope;
            self.events[self.current].candidate = 1.0;
            if clock_reached(sample, self.hold_pos) || ninety * (self.pos_level as f64) < self.neg_level as f64 {
                return self.estimate();
            }
        }
        false
    }

    /// Volta `i` ate o candidato anterior da mesma polaridade, contando passos.
    #[inline]
    fn step_back(&self, mut i: usize, steps: &mut u32) -> Option<usize> {
        loop {
            i = i.wrapping_sub(2) & EV_MASK;
            *steps += 1;
            if *steps >= MAX_STEPS { return None; }
            if self.events[i].candidate != 0.0 { return Some(i); }
        }
    }

    /// Estimativa de periodo num cruzamento candidato (0x100092d0).
    fn estimate(&mut self) -> bool {
        let s = self.slope as f64;
        let abs_s = s.abs();
        if abs_s <= self.min_strength as f64 { return false; }
        let cur = self.current;
        let ev = |i: usize| self.events[i];

        // Dois candidatos anteriores da mesma polaridade.
        let mut steps = 2u32;
        let mut j2 = cur.wrapping_sub(2) & EV_MASK;
        if ev(j2).candidate == 0.0 {
            match self.step_back(j2, &mut steps) { Some(i) => j2 = i, None => return false }
        }
        let mut j = j2.wrapping_sub(2) & EV_MASK;
        if ev(j).candidate == 0.0 {
            match self.step_back(j, &mut steps) { Some(i) => j = i, None => return false }
        }
        j2 = j;
        let mut j1 = cur;

        let c = ev(cur);
        let slope_scale = 17_179_869_184.0 * s; // 2^34
        let mut best = 0.0_f64;
        let mut second = 0.001_f32 as f64;
        let mut best_pair = 0.0_f64;
        let mut second_pair = 0.0_f64;
        let mut best_k: i32 = -1;
        let (mut d1, mut d2) = (0.0_f64, 1.0_f64);
        let mut k = 0i32;
        loop {
            if d1 > d2 {
                match self.step_back(j2, &mut steps) { Some(i) => j2 = i, None => break }
            } else {
                loop {
                    j1 = j1.wrapping_sub(2) & EV_MASK;
                    if ev(j1).candidate != 0.0 { break; }
                }
                if j1 == j2 {
                    match self.step_back(j2, &mut steps) { Some(i) => j2 = i, None => break }
                }
            }
            let (a, b) = (ev(j1), ev(j2));
            let p1 = wrap_span(c.time - a.time);
            let p2 = wrap_span(a.time - b.time);
            if p1 > self.max_pair_period || p2 > self.max_pair_period { break; }
            d1 = p1;
            d2 = p2;

            let dd = d1 - d2;
            let prod = d1 * d2;
            let r_period = prod / (384.0 * dd * dd + (prod + 1.0));
            let (ac, aj) = (c.area as f64, a.area as f64);
            let x = 64.0 * ac * aj;
            let da = ac - aj;
            let r_area = x / (1280.0 * da * da + (x + 1.0));
            let (b1, b2) = (ev((j1 + 1) & EV_MASK).area as f64, ev((j2 + 1) & EV_MASK).area as f64);
            let y = 64.0 * b1 * b2;
            let db = b1 - b2;
            let r_next = y / (1280.0 * db * db + (y + 1.0));
            let sj = a.slope as f64;
            let z = sj * slope_scale;
            let ds = s - sj;
            let r_slope = z / ((1.0 + z) + 25_769_803_776.0 * ds * ds); // 1.5 * 2^34
            let score = r_period * r_area * r_next * r_slope;

            if score > best {
                second_pair = best_pair;
                second = best;
                best_pair = d1 + d2;
                best = score;
                best_k = k;
            } else if score > second {
                second = score;
                second_pair = d1 + d2;
            }
            if score > 0.6_f32 as f64 && k == 0 { break; }
            if second > 0.5 { break; }
            k += 1;
            if k >= MAX_CANDIDATES as i32 { break; }
        }

        // Decisao final.
        let strength = abs_s * best;
        let mut period = 0.0_f64;
        let mut found = false;
        if strength > self.min_strength as f64 {
            period = 0.5 * best_pair;
            if best_k == 0 && best > 0.6_f32 as f64 {
                found = true;
            } else if best > 0.2_f32 as f64 {
                if best_pair <= second_pair || 0.6_f32 as f64 * best > second {
                    found = true;
                } else if second > 0.5 {
                    found = true;
                    period = 0.5 * second_pair;
                }
            }
        }

        let latest_hold = if clock_reached(self.hold_pos, self.hold_neg) { self.hold_pos } else { self.hold_neg };
        let new_note = clock_reached(c.sample, latest_hold);
        if new_note {
            self.last_strength = 0.0;
            self.last_slope = 0.0;
        }
        if !found || period <= 0.625 || period >= self.max_period as f64 { return false; }

        let midi = (self.ln_sr as f64 - period.ln()) * MIDI_SCALE + MIDI_OFFSET;
        self.last_strength = (self.last_strength as f64 * ninety()) as f32;
        if (self.last_midi as f64 - midi).abs() >= 2.5
            && 0.25 * strength <= self.last_strength as f64
            && !new_note
        {
            return false;
        }
        if !clock_reached(c.sample, self.lock_until) && abs_s <= self.last_slope as f64 {
            return false;
        }
        self.last_slope = abs_s as f32;
        self.lock_until = (c.sample.wrapping_add((period * 0.95) as u32)) & CLOCK_MASK;
        self.last_strength = strength as f32;
        let hold = (c.sample.wrapping_add(self.hold_samples)) & CLOCK_MASK;
        if s > 0.0 { self.hold_pos = hold; } else { self.hold_neg = hold; }
        self.last_midi = midi as f32;
        // tracksmooth = 100 (padrao): sem suavizacao.
        self.midi = midi as f32;
        self.period = period as f32;
        true
    }
}

#[inline]
fn ninety() -> f64 { 0.9_f32 as f64 }

/// f0 (Hz) de um sinal periodico limpo, por autocorrelacao normalizada (ferramenta de teste;
/// mede saidas acima da faixa do rastreador original).
#[cfg(test)]
pub(crate) fn measure_f0(sig: &[f32], sr: f32) -> f32 {
    let n = sig.len() / 2;
    let max_lag = ((sr / 50.0) as usize).min(n);
    let ac = |lag: usize| -> f64 {
        let (mut xy, mut xx, mut yy) = (0.0_f64, 0.0_f64, 0.0_f64);
        for i in 0..n {
            let (a, b) = (sig[i] as f64, sig[i + lag] as f64);
            xy += a * b;
            xx += a * a;
            yy += b * b;
        }
        xy / (xx * yy).sqrt().max(1e-12)
    };
    let r: Vec<f64> = (0..max_lag).map(ac).collect();
    let peak = r[2..].iter().cloned().fold(f64::MIN, f64::max);
    // primeiro maximo local acima de 90% do maior: evita sub-harmonicos
    let lag = (2..max_lag - 1)
        .find(|&l| r[l] >= 0.9 * peak && r[l] >= r[l - 1] && r[l] >= r[l + 1])
        .unwrap_or(0);
    if lag == 0 { return 0.0; }
    let (y0, y1, y2) = (r[lag - 1], r[lag], r[lag + 1]);
    let den = y0 - 2.0 * y1 + y2;
    let frac = if den.abs() > 1e-12 { 0.5 * (y0 - y2) / den } else { 0.0 };
    (sr as f64 / (lag as f64 + frac)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(f0: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let ph = 2.0 * std::f32::consts::PI * f0 * i as f32 / sr;
                0.3 * (ph.sin() + 0.5 * (2.0 * ph).sin() + 0.25 * (3.0 * ph).sin())
            })
            .collect()
    }

    #[test]
    fn tracks_harmonic_tones() {
        for sr in [44_100.0_f32, 48_000.0, 96_000.0] {
            for f0 in [82.4_f32, 110.0, 220.0, 440.0, 660.0] {
                let mut t = PitchTracker::new(sr);
                for &x in &tone(f0, sr, (sr * 0.5) as usize) { t.process(x); }
                assert!(t.voiced, "{f0} Hz @ {sr}");
                let expect = 69.0 + 12.0 * (f0 / 440.0).log2();
                assert!((t.midi - expect).abs() < 0.1, "{f0} Hz @ {sr}: midi {} esperado {expect}", t.midi);
                assert!((t.period - sr / f0).abs() < 0.5, "{f0} Hz @ {sr}: periodo {}", t.period);
            }
        }
    }

    #[test]
    fn silence_goes_unvoiced_after_hold() {
        let sr = 44_100.0;
        let mut t = PitchTracker::new(sr);
        for &x in &tone(220.0, sr, 22_050) { t.process(x); }
        assert!(t.voiced);
        let mut off_at = None;
        for i in 0..10_000 {
            if t.process(0.0) && !t.voiced { off_at = Some(i); break; }
        }
        let off_at = off_at.expect("deveria ficar unvoiced");
        assert!((2_000..3_500).contains(&off_at), "hold de ~50 ms, saiu em {off_at}");
    }
}

/// Regressao contra o DLL original: `tests/fixtures/tracker_voice_dll.txt` foi gerado rodando
/// 0x10008e60 de `DecaBuddy.dll` num emulador x86 (unicorn, FPU em 53 bits) sobre
/// `tracker_voice.wav` (glissando 98-490 Hz, tres vogais, ruido). Cada linha:
/// "amostra midi periodo unvoiced". O port deve dar as mesmas leituras nas mesmas amostras.
#[cfg(test)]
mod dll_reference {
    use super::*;

    fn read_wav_i16_mono(bytes: &[u8]) -> Vec<f32> {
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            if &bytes[pos..pos + 4] == b"data" {
                return bytes[pos + 8..pos + 8 + size]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&b| i16::from_le_bytes(b) as f32 / 32_768.0)
                    .collect();
            }
            pos += 8 + size + (size & 1);
        }
        panic!("WAV sem chunk data");
    }

    #[test]
    fn matches_the_original_dll_reading_for_reading() {
        let x = read_wav_i16_mono(include_bytes!("../tests/fixtures/tracker_voice.wav"));
        let expected: Vec<(usize, f32, f32, bool)> = include_str!("../tests/fixtures/tracker_voice_dll.txt")
            .lines()
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                (f[0].parse().unwrap(), f[1].parse().unwrap(), f[2].parse().unwrap(), f[3] == "1")
            })
            .filter(|e| e.0 > 0) // o DLL tambem sinaliza "unvoiced" na amostra 0
            .collect();
        let mut t = PitchTracker::new(44_100.0);
        let mut got = Vec::new();
        for (n, &v) in x.iter().enumerate() {
            if t.process(v) { got.push((n, t.midi, t.period, !t.voiced)); }
        }
        assert_eq!(got.len(), expected.len(), "numero de leituras");
        for (g, e) in got.iter().zip(&expected) {
            assert_eq!((g.0, g.3), (e.0, e.3), "leitura na amostra {}", e.0);
            assert!((g.1 - e.1).abs() < 1e-3 && (g.2 - e.2).abs() < 1e-2, "amostra {}: {g:?} vs {e:?}", e.0);
        }
    }
}
