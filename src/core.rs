//! Processador da emulacao do Akai DecaBuddy HV10: parametros do aparelho, cadeia do codec,
//! vozes de harmonia (L2, L1, H1, H2 + unisons), voz original (ORIGIN) e MIDI.

use crate::dsp::{
    formant_ratio, hw, level_gain, pan_gains, primary_delay, quantize_16, rand_detune_semitones,
    unison_delay, Biquad, Butter4, FastRng, PitchShifter, RandDelay, RandomLfo,
    ScopedFtzDaz, ShiftParams, RING_MASK, RING_SIZE,
};
use crate::harmony::{origin_corrected_shift, HarmonyEngine, HarmonySettings, NUM_PARTS};
use crate::pitch::PitchTracker;
use crate::programs::{factory_programs, program_change_index, FactoryProgram};
use crate::scales::{StyleBank, StyleId};

/// BUDDIES de uma parte de harmonia: desligada, ligada, ligada + unison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Buddies {
    Off,
    On,
    OnUnison,
}

impl Buddies {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Buddies::On,
            2 => Buddies::OnUnison,
            _ => Buddies::Off,
        }
    }
}

/// BUDDIES da voz original: desligada, ligada, ligada + unison, FX (so o unison).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginBuddies {
    Off,
    On,
    OnUnison,
    Fx,
}

impl OriginBuddies {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => OriginBuddies::On,
            2 => OriginBuddies::OnUnison,
            3 => OriginBuddies::Fx,
            _ => OriginBuddies::Off,
        }
    }
}

/// VOICE parameters de uma parte (L2, L1, H1, H2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartParams {
    pub buddies: Buddies,
    /// FORMANT -9..9
    pub formant: i32,
    /// LEVEL 0..9 (float para aceitar CC7)
    pub level: f32,
    /// PAN -9 (L9) .. 9 (R9)
    pub pan: f32,
    /// P.CORRECT 0 (--) .. 9
    pub p_correct: f32,
    pub rand_delay: RandDelay,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OriginParams {
    pub buddies: OriginBuddies,
    pub formant: i32,
    pub level: f32,
    pub pan: f32,
    /// P.CORRECT 0 (--) .. 9. So age no unison/FX: o sinal seco nao passa pelo transpositor.
    pub p_correct: f32,
    pub rand_delay: RandDelay,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hv10Params {
    pub harmony: HarmonySettings,
    /// Sub Parameter (footswitch / CC80-81): troca KEY e STYLE pelos valores Sub.
    pub sub_active: bool,
    pub sub_key: u8,
    pub sub_styles: [StyleId; NUM_PARTS],
    /// TUNE ADJUST 427..453 Hz
    pub tune_hz: f32,
    /// INPUT LEVEL (dB)
    pub input_gain_db: f32,
    /// MASTER LVL 0..9
    pub master_level: f32,
    /// BYPASS (Normal Bypass: o sinal seco passa pelo codec)
    pub bypass: bool,
    /// Canal MIDI base 1..16
    pub midi_base_channel: u8,
    pub origin: OriginParams,
    pub parts: [PartParams; NUM_PARTS],
}

impl Default for Hv10Params {
    /// Programa 01A de fabrica (Preset Program List): voz original com unison + 3A em H1 e H2.
    fn default() -> Self {
        let off = PartParams { buddies: Buddies::Off, formant: 0, level: 0.0, pan: 0.0, p_correct: 0.0, rand_delay: RandDelay::Off };
        Self {
            harmony: HarmonySettings::default(),
            sub_active: false,
            sub_key: 0,
            sub_styles: [StyleId::Un, StyleId::Un, StyleId::S3E, StyleId::S3E],
            tune_hz: 440.0,
            input_gain_db: 0.0,
            master_level: 7.0,
            bypass: false,
            midi_base_channel: 1,
            origin: OriginParams { buddies: OriginBuddies::OnUnison, formant: 0, level: 9.0, pan: 0.0, p_correct: 0.0, rand_delay: RandDelay::Off },
            parts: [
                off,
                off,
                PartParams { buddies: Buddies::OnUnison, formant: 3, level: 6.0, pan: -8.0, p_correct: 1.0, rand_delay: RandDelay::Random(4) },
                PartParams { buddies: Buddies::OnUnison, formant: 6, level: 6.0, pan: 8.0, p_correct: 1.0, rand_delay: RandDelay::Random(7) },
            ],
        }
    }
}

impl Hv10Params {
    /// Configuracao de harmonia efetiva (Sub Parameter aplicado, partes ligadas).
    pub fn effective_harmony(&self) -> HarmonySettings {
        let mut h = self.harmony.clone();
        if self.sub_active {
            h.key = self.sub_key;
            h.styles = self.sub_styles;
        }
        for p in 0..NUM_PARTS {
            h.part_on[p] = self.parts[p].buddies != Buddies::Off;
        }
        h
    }
}

/// LEDs do medidor de entrada (NJU7032D, IC401): -12 dB, -6 dB, 0 dB/OVER.
#[derive(Clone, Copy, Debug, Default)]
pub struct InputMeterLeds {
    pub led_neg12: bool,
    pub led_neg6: bool,
    pub led_0_over: bool,
}

#[derive(Clone, Debug)]
struct PartDsp {
    pri: PitchShifter,
    uni: PitchShifter,
    pri_params: ShiftParams,
    uni_params: ShiftParams,
    has_target: bool,
    gl: f32,
    gr: f32,
    ul: f32,
    ur: f32,
    active: bool,
}

impl Default for PartDsp {
    fn default() -> Self {
        let p = ShiftParams::new(128.0, 0.0, 1.0);
        Self {
            pri: PitchShifter::default(),
            uni: PitchShifter::default(),
            pri_params: p,
            uni_params: p,
            has_target: false,
            gl: 0.0,
            gr: 0.0,
            ul: 0.0,
            ur: 0.0,
            active: false,
        }
    }
}

impl PartDsp {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct Hv10Processor {
    pub sample_rate: f32,
    /// Os 100 programas de fabrica (MIDI Program Change).
    pub programs: Vec<FactoryProgram>,
    /// Ultimo programa recebido por Program Change, publicado para a interface:
    /// sequencia * 1000 + indice + 1 (0 = nenhum).
    pub program_token: f32,
    program_seq: u32,
    pub params: Hv10Params,
    pub styles: StyleBank,
    pub harmony: HarmonyEngine,
    pub leds: InputMeterLeds,

    ring: Vec<f32>,
    abs_pos: u64,
    pitch: PitchTracker,
    lfo: RandomLfo,
    dither: FastRng,
    in_hp: Biquad,
    out_hp: [Biquad; 2],
    codec_lp: Option<[Butter4; 2]>,

    parts: [PartDsp; NUM_PARTS],
    origin_uni: PitchShifter,
    origin_uni_params: ShiftParams,
    origin_l: f32,
    origin_r: f32,
    origin_ul: f32,
    origin_ur: f32,
    origin_uni_active: bool,
    master: f32,
    bypass_mix: f32,
    smooth: f32,
    meter_env: f32,
}

impl Hv10Processor {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8000.0);
        let mut p = Self {
            sample_rate: sr,
            programs: factory_programs(),
            program_token: 0.0,
            program_seq: 0,
            params: Hv10Params::default(),
            styles: StyleBank::default(),
            harmony: HarmonyEngine::default(),
            leds: InputMeterLeds::default(),
            ring: vec![0.0; RING_SIZE],
            abs_pos: 0,
            pitch: PitchTracker::new(sr),
            lfo: RandomLfo::new(sr),
            dither: FastRng::new(0x5EED_1999),
            in_hp: Biquad::default(),
            out_hp: [Biquad::default(); 2],
            codec_lp: None,
            parts: Default::default(),
            origin_uni: PitchShifter::default(),
            origin_uni_params: ShiftParams::new(128.0, 0.0, 1.0),
            origin_l: 0.0,
            origin_r: 0.0,
            origin_ul: 0.0,
            origin_ur: 0.0,
            origin_uni_active: false,
            master: 1.0,
            bypass_mix: 0.0,
            smooth: 0.0,
            meter_env: 0.0,
        };
        p.set_sample_rate(sr);
        p
    }

    /// O HV10 nao compensa atraso: a voz original sai direto e as harmonias atrasam
    /// 0.5-1.5 periodo (mais RAND.DELAY), como no aparelho.
    pub fn latency_samples(&self) -> u32 { 0 }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr.max(8000.0);
        let sr = self.sample_rate;
        self.pitch.set_sample_rate(sr);
        self.lfo.set_sample_rate(sr);
        // Capacitores de acoplamento de entrada/saida (corte subsonico de poucos Hz).
        self.in_hp = Biquad::highpass(3.0, std::f32::consts::FRAC_1_SQRT_2, sr);
        self.out_hp = [Biquad::highpass(2.0, std::f32::consts::FRAC_1_SQRT_2, sr); 2];
        // Banda do PCM3003E a 44.1 kHz (passa-faixa ate 0.454 fs = 20 kHz). A taxas maiores
        // limita o caminho processado a mesma banda.
        self.codec_lp = if sr > hw::NATIVE_SR * 1.05 {
            Some([Butter4::lowpass(20_000.0, sr); 2])
        } else {
            None
        };
        self.smooth = (-1.0 / (0.010 * sr)).exp();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.abs_pos = 0;
        self.pitch.reset();
        self.lfo.reset();
        self.in_hp.reset();
        for f in &mut self.out_hp { f.reset(); }
        if let Some(lp) = &mut self.codec_lp {
            for f in lp { f.reset(); }
        }
        for p in &mut self.parts { p.reset(); }
        self.origin_uni.reset();
        self.origin_uni_active = false;
        self.harmony.reset();
        self.meter_env = 0.0;
        self.update_voices();
    }

    /// (f0 em Hz, MIDI relativo a TUNE, vozeado) da ultima leitura do rastreador.
    pub fn detected_pitch(&self) -> (f32, f32, bool) {
        let f0 = if self.pitch.period > 0.0 { self.sample_rate / self.pitch.period } else { 0.0 };
        (f0, self.tracked_midi(), self.pitch.voiced)
    }

    /// MIDI do rastreador (69 = A 440) referido a TUNE ADJUST.
    fn tracked_midi(&self) -> f32 {
        self.pitch.midi - 12.0 * (self.params.tune_hz.clamp(427.0, 453.0) / 440.0).log2()
    }

    /// Recalcula os parametros das sub-vozes a partir do pitch atual (em degrau, por leitura).
    fn update_voices(&mut self) {
        let sr = self.sample_rate;
        let settings = self.params.effective_harmony();
        self.harmony.midi.base_channel = self.params.midi_base_channel.saturating_sub(1) & 0x0F;
        let voiced = self.pitch.voiced && self.pitch.period > 0.0;
        let period = if voiced { self.pitch.period } else { hw::UNVOICED_PERIOD_44K * sr / hw::NATIVE_SR };

        let corr = self.params.parts.map(|pp| pp.p_correct / 9.0);
        let shifts = if voiced {
            self.harmony.shifts(&settings, &self.styles, self.tracked_midi(), corr)
        } else {
            [None; NUM_PARTS]
        };

        for (p, &voiced_shift) in shifts.iter().enumerate() {
            let pp = self.params.parts[p];
            // Sem pitch, as harmonias passam a entrada em uníssono (como o DSP).
            let shift = if voiced {
                voiced_shift
            } else if self.harmony.part_available(&settings, &self.styles, p) {
                Some(0.0)
            } else {
                None
            };
            let d = rand_detune_semitones(pp.rand_delay);
            let fr = formant_ratio(pp.formant);
            let s = shift.unwrap_or(0.0);
            let dsp = &mut self.parts[p];
            dsp.has_target = shift.is_some();
            dsp.pri_params = ShiftParams::new(period, s + d, fr);
            dsp.uni_params = ShiftParams::new(period, s - d, fr);
        }

        let o = self.params.origin;
        let o_shift = if voiced { origin_corrected_shift(self.tracked_midi(), o.p_correct / 9.0) } else { 0.0 };
        self.origin_uni_params =
            ShiftParams::new(period, o_shift - rand_detune_semitones(o.rand_delay), formant_ratio(o.formant));
    }

    pub fn process_block(&mut self, input_mono: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = input_mono.len().min(out_l.len()).min(out_r.len());
        if n == 0 { return; }
        let _ftz = ScopedFtzDaz::new();
        let sr = self.sample_rate;
        let in_gain = 10.0_f32.powf(self.params.input_gain_db / 20.0);
        let k = self.smooth;
        self.harmony.midi.base_channel = self.params.midi_base_channel.saturating_sub(1) & 0x0F;

        // Ganhos-alvo por bloco (LEVEL, PAN, BUDDIES).
        let mut targets = [[0.0_f32; 4]; NUM_PARTS];
        for (p, t) in targets.iter_mut().enumerate() {
            let pp = self.params.parts[p];
            let g = if pp.buddies != Buddies::Off && self.parts[p].has_target { level_gain(pp.level) } else { 0.0 };
            let pan_c = (pp.pan / 18.0).clamp(-0.5, 0.5);
            if pp.buddies == Buddies::OnUnison {
                let s = if p % 2 == 0 { 1.0 } else { -1.0 };
                let d = hw::UNISON_PAN_SPREAD * s * (0.5 - pan_c.abs());
                let (pl, pr) = pan_gains(0.5 + pan_c + d);
                let (ul, ur) = pan_gains(0.5 + pan_c - d);
                let gu = g * hw::UNISON_GAIN;
                *t = [g * pl, g * pr, gu * ul, gu * ur];
            } else {
                let (pl, pr) = pan_gains(0.5 + pan_c);
                *t = [g * pl, g * pr, 0.0, 0.0];
            }
        }
        let o = self.params.origin;
        let og = level_gain(o.level);
        let o_pan = (o.pan / 18.0).clamp(-0.5, 0.5);
        let (odl, odr) = pan_gains(0.5 + o_pan);
        let (dry_t, uni_t) = match o.buddies {
            OriginBuddies::Off => ([0.0, 0.0], [0.0, 0.0]),
            OriginBuddies::On => ([og * odl, og * odr], [0.0, 0.0]),
            OriginBuddies::OnUnison => {
                let (ul, ur) = pan_gains(0.5 + o_pan + hw::UNISON_PAN_SPREAD * (0.5 - o_pan.abs()));
                let gu = og * hw::UNISON_GAIN;
                ([og * odl, og * odr], [gu * ul, gu * ur])
            }
            OriginBuddies::Fx => ([0.0, 0.0], [og * odl, og * odr]),
        };
        let master_t = level_gain(self.params.master_level);
        let bypass_t = if self.params.bypass { 1.0 } else { 0.0 };

        for i in 0..n {
            // --- Entrada analogica -> ADC PCM3003E (16 bits, ceifa em 0 dBFS) ---
            // NaN/infinito vindo do host contaminaria filtros e anel para sempre.
            let x_in = if input_mono[i].is_finite() { input_mono[i] } else { 0.0 };
            let x_an = self.in_hp.process(x_in * in_gain);
            let dither = (self.dither.next_unit() - self.dither.next_unit()) * (1.0 / 32_768.0);
            let x = quantize_16(x_an + dither);

            let ax = x.abs();
            self.meter_env = if ax > self.meter_env { ax } else { self.meter_env * 0.9995 };

            // --- Rastreador de pitch: cada leitura (ou mudanca de voicing) atualiza as vozes,
            // antes de a amostra entrar no anel, como no DSP ---
            if self.pitch.process(x) {
                self.update_voices();
            }
            let w = self.abs_pos;
            self.ring[w as usize & RING_MASK] = x;
            self.lfo.tick();

            // --- Vozes de harmonia ---
            let mut l = 0.0_f32;
            let mut r = 0.0_f32;
            for (p, (t, dsp)) in targets.iter().zip(self.parts.iter_mut()).enumerate() {
                dsp.gl = t[0] + (dsp.gl - t[0]) * k;
                dsp.gr = t[1] + (dsp.gr - t[1]) * k;
                dsp.ul = t[2] + (dsp.ul - t[2]) * k;
                dsp.ur = t[3] + (dsp.ur - t[3]) * k;
                let audible = dsp.gl + dsp.gr > 1e-6;
                let uni_audible = dsp.ul + dsp.ur > 1e-6;
                if !audible && !uni_audible {
                    if dsp.active {
                        dsp.pri.reset();
                        dsp.uni.reset();
                        dsp.active = false;
                    }
                    continue;
                }
                dsp.active = true;
                let rd = self.params.parts[p].rand_delay;
                let y = dsp.pri.step(&self.ring, w, &dsp.pri_params, primary_delay(p + 1, rd, &self.lfo, sr));
                l += y * dsp.gl;
                r += y * dsp.gr;
                if uni_audible {
                    let yu = dsp.uni.step(&self.ring, w, &dsp.uni_params, unison_delay(p, &self.lfo, sr));
                    l += yu * dsp.ul;
                    r += yu * dsp.ur;
                } else {
                    dsp.uni.reset();
                }
            }

            // --- Voz original e seu unison ---
            self.origin_l = dry_t[0] + (self.origin_l - dry_t[0]) * k;
            self.origin_r = dry_t[1] + (self.origin_r - dry_t[1]) * k;
            l += x * self.origin_l;
            r += x * self.origin_r;
            self.origin_ul = uni_t[0] + (self.origin_ul - uni_t[0]) * k;
            self.origin_ur = uni_t[1] + (self.origin_ur - uni_t[1]) * k;
            if self.origin_ul + self.origin_ur > 1e-6 {
                self.origin_uni_active = true;
                let yu = self.origin_uni.step(
                    &self.ring, w, &self.origin_uni_params, primary_delay(0, o.rand_delay, &self.lfo, sr),
                );
                l += yu * self.origin_ul;
                r += yu * self.origin_ur;
            } else if self.origin_uni_active {
                self.origin_uni.reset();
                self.origin_uni_active = false;
            }

            // --- MASTER LVL, Normal Bypass, saturacao 16 bits do DSP -> DAC ---
            self.master = master_t + (self.master - master_t) * k;
            self.bypass_mix = bypass_t + (self.bypass_mix - bypass_t) * k;
            let ml = l * self.master * (1.0 - self.bypass_mix) + x * self.bypass_mix;
            let mr = r * self.master * (1.0 - self.bypass_mix) + x * self.bypass_mix;
            let mut yl = self.out_hp[0].process(quantize_16(ml));
            let mut yr = self.out_hp[1].process(quantize_16(mr));
            if let Some(lp) = &mut self.codec_lp {
                yl = lp[0].process(yl);
                yr = lp[1].process(yr);
            }
            out_l[i] = yl;
            out_r[i] = yr;

            self.abs_pos = self.abs_pos.wrapping_add(1);
        }

        self.leds.led_neg12 = self.meter_env >= 0.251;
        self.leds.led_neg6 = self.meter_env >= 0.501;
        self.leds.led_0_over = self.meter_env >= 0.999;
    }

    /// Carrega um programa de fabrica (0..100 = 01A, 01B, ... 50B). Parametros GLOBAIS do HV10
    /// (TUNE, canal MIDI, nivel de entrada, BYPASS) nao fazem parte do programa.
    pub fn load_program(&mut self, index: usize) {
        let Some(prog) = self.programs.get(index) else { return };
        let keep = (self.params.tune_hz, self.params.midi_base_channel, self.params.input_gain_db, self.params.bypass);
        self.params = prog.params.clone();
        (self.params.tune_hz, self.params.midi_base_channel, self.params.input_gain_db, self.params.bypass) = keep;
        self.program_seq = self.program_seq.wrapping_add(1) % 1000;
        self.program_token = (self.program_seq * 1000 + index as u32 + 1) as f32;
    }

    /// Mensagens MIDI (implementacao do HV10: notas, Program Change, CC7/10/11/12/13,
    /// 80/81/82, 64, 120/123).
    pub fn handle_midi_event(&mut self, status: u8, data1: u8, data2: u8) {
        let kind = status & 0xF0;
        let ch = status & 0x0F;
        self.harmony.midi.base_channel = self.params.midi_base_channel.saturating_sub(1) & 0x0F;
        match kind {
            0x90 if data2 > 0 => self.harmony.midi.note_on(ch, data1),
            0x80 | 0x90 => self.harmony.midi.note_off(ch, data1),
            // Program Change no canal base: PC 1-50 -> 01A-50A, 51-100 -> 01B-50B
            0xC0 if self.harmony.midi.channel_offset(ch) == 0 => {
                if let Some(i) = program_change_index(data1) {
                    self.load_program(i);
                }
            }
            0xB0 => {
                let off = self.harmony.midi.channel_offset(ch) as usize;
                let norm = data2 as f32 / 127.0;
                match data1 {
                    7 | 10 | 12 | 13 => {
                        if off == 0 {
                            let o = &mut self.params.origin;
                            match data1 {
                                7 => o.level = norm * 9.0,
                                10 => o.pan = -9.0 + norm * 18.0,
                                12 => o.formant = (-9.0 + norm * 18.0).round() as i32,
                                _ => o.p_correct = norm * 9.0,
                            }
                        } else if off <= NUM_PARTS {
                            let pp = &mut self.params.parts[off - 1];
                            match data1 {
                                7 => pp.level = norm * 9.0,
                                10 => pp.pan = -9.0 + norm * 18.0,
                                12 => pp.formant = (-9.0 + norm * 18.0).round() as i32,
                                _ => pp.p_correct = norm * 9.0,
                            }
                        }
                    }
                    11 if off == 0 => self.params.master_level = norm * 9.0,
                    64 if off == 0 => self.harmony.midi.sustain(data2 >= 64),
                    80 | 81 if off == 0 => self.params.sub_active = data2 >= 64,
                    82 if off == 0 => self.params.bypass = data2 >= 64,
                    120 | 123 => self.harmony.midi.all_notes_off(),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::HarmonyMode;
    use std::f32::consts::PI;

    fn voice(f0: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                (0.45 * (2.0 * PI * f0 * t).sin() + 0.2 * (2.0 * PI * 2.0 * f0 * t).sin() + 0.1 * (2.0 * PI * 3.0 * f0 * t).sin()) * 0.8
            })
            .collect()
    }

    fn render(proc: &mut Hv10Processor, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut l = vec![0.0; input.len()];
        let mut r = vec![0.0; input.len()];
        for ((i, lb), rb) in input.chunks(256).zip(l.chunks_mut(256)).zip(r.chunks_mut(256)) {
            proc.process_block(i, lb, rb);
        }
        (l, r)
    }

    fn estimate_f0(sig: &[f32], sr: f32) -> f32 { crate::pitch::measure_f0(sig, sr) }

    fn solo_part(proc: &mut Hv10Processor, part: usize, style: StyleId) {
        proc.params.origin.buddies = OriginBuddies::Off;
        for p in 0..NUM_PARTS {
            proc.params.parts[p].buddies = Buddies::Off;
        }
        let pp = &mut proc.params.parts[part];
        pp.buddies = Buddies::On;
        pp.level = 9.0;
        pp.pan = 0.0;
        pp.formant = 0;
        pp.p_correct = 9.0;
        pp.rand_delay = RandDelay::Off;
        proc.params.harmony.styles[part] = style;
    }

    #[test]
    fn style_harmonies_land_on_the_right_pitch() {
        let sr = 44_100.0;
        // (f_in, parte, estilo, razao esperada) em C maior
        let cases = [
            (130.81_f32, 0usize, StyleId::Oct, 0.5_f32),   // C3 -> C2 (L2 8v)
            (130.81, 2, StyleId::S3A, 1.259_921),          // C3 -> E3 (H1 3A)
            (220.0, 3, StyleId::S5A, 1.587_401),           // A3 -> F4 (H2 5A: A -> +8)
            (82.41, 2, StyleId::S5A, 1.587_401),           // E2 -> C3 (H1 5A, voz grave)
            (523.25, 2, StyleId::Oct, 2.0),                // C5 -> C6 (H1 8v, voz aguda)
        ];
        for (f_in, part, style, ratio) in cases {
            let mut proc = Hv10Processor::new(sr);
            solo_part(&mut proc, part, style);
            let input = voice(f_in, sr, 32_768);
            let (l, _) = render(&mut proc, &input);
            let f = estimate_f0(&l[12_000..32_000], sr);
            let cents = 1200.0 * (f / (f_in * ratio)).log2();
            assert!(cents.abs() < 12.0, "{style:?} parte {part}: f_in={f_in} esperado {} medido {f}", f_in * ratio);
        }
    }

    #[test]
    fn origin_path_is_bit_transparent_through_the_codec() {
        let sr = 44_100.0;
        let mut proc = Hv10Processor::new(sr);
        for p in 0..NUM_PARTS { proc.params.parts[p].buddies = Buddies::Off; }
        proc.params.origin.buddies = OriginBuddies::On;
        proc.params.master_level = 9.0;
        let input = voice(220.0, sr, 16_384);
        let (l, r) = render(&mut proc, &input);
        // ORIGIN nivel 9 centro: -3 dB por canal (lei de pan do DSP). Alem da quantizacao de
        // 16 bits, so os capacitores de acoplamento (HPF de poucos Hz) atuam: ganho unitario.
        let g = std::f32::consts::FRAC_1_SQRT_2;
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let ratio_l = rms(&l[8_000..]) / (rms(&input[8_000..]) * g);
        let ratio_r = rms(&r[8_000..]) / (rms(&input[8_000..]) * g);
        assert!((ratio_l - 1.0).abs() < 2e-3 && (ratio_r - 1.0).abs() < 2e-3, "{ratio_l} {ratio_r}");
        assert!(l.iter().zip(&r).all(|(a, b)| (a - b).abs() < 1e-6), "centro deve ser identico em L/R");
    }

    #[test]
    fn origin_p_correct_pulls_the_double_to_the_semitone() {
        let sr = 44_100.0;
        // A3 desafinado +35 cents. Em FX so o dobro (transpositor) sai.
        let f_in = 220.0 * 2.0_f32.powf(35.0 / 1200.0);
        for (pc, expect_cents) in [(0.0_f32, 35.0_f32), (9.0, 0.0)] {
            let mut proc = Hv10Processor::new(sr);
            for p in 0..NUM_PARTS { proc.params.parts[p].buddies = Buddies::Off; }
            proc.params.origin = OriginParams {
                buddies: OriginBuddies::Fx, formant: 0, level: 9.0, pan: 0.0, p_correct: pc, rand_delay: RandDelay::Off,
            };
            let input = voice(f_in, sr, 32_768);
            let (l, _) = render(&mut proc, &input);
            let cents = 1200.0 * (estimate_f0(&l[12_000..32_000], sr) / 220.0).log2();
            assert!((cents - expect_cents).abs() < 6.0, "P.CORRECT {pc}: {cents} cents, esperado {expect_cents}");
        }
    }

    #[test]
    fn unvoiced_input_passes_through_harmonies_in_unison() {
        let sr = 44_100.0;
        let mut proc = Hv10Processor::new(sr);
        solo_part(&mut proc, 2, StyleId::S3A);
        let mut rng = FastRng::new(7);
        let noise: Vec<f32> = (0..16_384).map(|_| rng.next_bipolar() * 0.3).collect();
        let (l, _) = render(&mut proc, &noise);
        let rms = (l[4000..].iter().map(|v| v * v).sum::<f32>() / (l.len() - 4000) as f32).sqrt();
        // O rastreador original as vezes "acha" um pitch no ruido; numa nota cromatica sem
        // atribuicao no 3A a parte cala por aquela leitura (como no DSP).
        assert!(rms > 0.03, "sibilantes devem passar nas harmonias (rms {rms})");
    }

    #[test]
    fn note_change_has_no_clicks() {
        let sr = 44_100.0;
        let mut proc = Hv10Processor::new(sr);
        solo_part(&mut proc, 2, StyleId::S3A);
        proc.params.parts[2].rand_delay = RandDelay::Random(5);
        let n1 = 20_000;
        let mut input = voice(261.63, sr, n1);
        let tail = voice(293.66, sr, n1);
        // continuidade de fase na troca de nota
        input.extend(tail.iter().enumerate().map(|(i, _)| {
            let t = (n1 + i) as f32 / sr;
            let ph = 2.0 * PI * 261.63 * n1 as f32 / sr + 2.0 * PI * 293.66 * (t - n1 as f32 / sr);
            (0.45 * ph.sin() + 0.2 * (2.0 * ph).sin() + 0.1 * (3.0 * ph).sin()) * 0.8
        }));
        let (l, _) = render(&mut proc, &input);
        let max_step = l[n1 - 3000..n1 + 3000].windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        let steady = l[n1 - 9000..n1 - 3000].windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(max_step < steady * 2.5 + 0.02, "derivada na transicao {max_step} vs estavel {steady}");
    }

    #[test]
    fn midi_cc_controls_levels_and_bypass() {
        let mut proc = Hv10Processor::new(44_100.0);
        proc.handle_midi_event(0xB3, 7, 0); // H1 (base + 3)
        assert_eq!(proc.params.parts[2].level, 0.0);
        proc.handle_midi_event(0xB0, 82, 127);
        assert!(proc.params.bypass);
        proc.handle_midi_event(0xB0, 80, 127);
        assert!(proc.params.sub_active);
    }

    #[test]
    fn program_change_loads_factory_program_but_keeps_globals() {
        let mut proc = Hv10Processor::new(44_100.0);
        proc.params.tune_hz = 445.0;
        proc.handle_midi_event(0xC0, 48, 0); // PC 49 -> 49A Synth Vox
        assert_eq!(proc.params.harmony.mode, HarmonyMode::Priority);
        assert_eq!(proc.params.origin.buddies, OriginBuddies::Off);
        assert_eq!(proc.params.tune_hz, 445.0);
        assert_eq!(proc.program_token as u32 % 1000, 97);
        proc.handle_midi_event(0xC3, 0, 0); // canal errado: ignorado
        assert_eq!(proc.params.harmony.mode, HarmonyMode::Priority);
    }

    #[test]
    fn smart_mode_keeps_jazz_voicings_with_sustain() {
        let mut proc = Hv10Processor::new(44_100.0);
        proc.params.harmony.mode = HarmonyMode::Smart;
        proc.handle_midi_event(0xB0, 64, 127);
        for &n in &[62u8, 66, 69, 73] {
            proc.handle_midi_event(0x90, n, 100);
            proc.handle_midi_event(0x80, n, 0);
        }
        let s = proc.params.effective_harmony();
        let shifts = proc.harmony.shifts(&s, &proc.styles, 62.0, [1.0; 4]);
        for v in shifts {
            assert!(v.unwrap().abs() >= 2.0);
        }
    }

    #[test]
    fn digital_overload_clips_like_the_dsp() {
        let sr = 44_100.0;
        let mut proc = Hv10Processor::new(sr);
        proc.params.input_gain_db = 15.0;
        let input = voice(220.0, sr, 8192);
        let (l, _) = render(&mut proc, &input);
        let peak = l.iter().fold(0.0_f32, |a, &v| a.max(v.abs()));
        // Saturacao de 16 bits do DSP; o capacitor de acoplamento apos o DAC permite um
        // pequeno overshoot da onda ceifada, como no aparelho.
        assert!(peak <= 1.08, "saida deve saturar em 0 dBFS (pico {peak})");
    }
}
