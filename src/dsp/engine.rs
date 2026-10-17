//! Motor do Heave: liga o rastreador de altura, as marcas de epoca, o plano harmonico e os
//! shifters PSOLA numa linha do tempo com latencia fixa.
//!
//! Linha do tempo: a amostra de entrada `n` sai em `n + L`. Com isso o plano harmonico do
//! instante `t` pode olhar a voz ate `t + LOOKAHEAD` (a nota em que o cantor vai chegar) e cada
//! voz pode adiantar-se ou atrasar-se em relacao ao lead como um cantor de verdade.
//!
//! Acorde, escala e notas MIDI chegam com o carimbo da amostra de ENTRADA em que valem e sao
//! consultados no instante do material analisado. Uma automacao de acorde no Ableton, que o host
//! entrega alinhada ao audio, muda a harmonia exatamente naquele ponto da musica.

use super::harmony::{choose_voicing, just_offset, HarmonyCtx, Mode, VoiceNotes, Voicing, MAX_VOICES};
use super::marks::MarkTracker;
use super::pitch::PitchTracker;
use super::psola::{max_half_len, GrainVoice, VoiceControl, VoiceState};
use super::util::{db_to_gain, hz_to_midi, midi_to_hz, smoothstep, Biquad, Drift, HannTable, Ring, Rng};

/// Vozes de harmonia + as dobras (uma por harmonia).
pub const NUM_VOICES: usize = MAX_VOICES * 2;
const LATENCY_MS: f32 = 80.0;
const LOOKAHEAD_MS: f32 = 30.0;
const CTRL_CAP: usize = 16384;

/// Parametros do motor (lidos do host a cada bloco).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineParams {
    pub mode: Mode,
    pub root: u8,
    pub quality: u8,
    pub scale: u8,
    pub voicing: Voicing,
    pub voices: u8,
    /// Nivel das harmonias (dB).
    pub harmony_db: f32,
    /// Nivel da voz original (dB; <= -60 = mudo).
    pub lead_db: f32,
    /// Humanizacao 0..1.
    pub humanize: f32,
    /// Abertura estereo 0..1.
    pub width: f32,
    /// Carater/garganta -1..1 (negativo = garganta longa, voz mais escura).
    pub formant: f32,
    /// Afinacao pura (justa) relativa ao lead.
    pub pure: bool,
    /// Dobra cada harmonia (dois cantores por voz).
    pub double: bool,
    pub bypass: bool,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            mode: Mode::Chord,
            root: 0,
            quality: 0,
            scale: 0,
            voicing: Voicing::Above,
            voices: 2,
            harmony_db: -4.0,
            lead_db: 0.0,
            humanize: 0.5,
            width: 0.7,
            formant: 0.0,
            pure: true,
            double: false,
            bypass: false,
        }
    }
}

/// Controle de uma voz num quadro de controle.
#[derive(Clone, Copy, Debug)]
struct VoiceCtrl {
    /// Nota alvo (MIDI fracionario); NaN = sem nota.
    midi: f32,
    formant: f32,
    gain: f32,
    delay: f32,
}

impl Default for VoiceCtrl {
    fn default() -> Self { Self { midi: f32::NAN, formant: 1.0, gain: 0.0, delay: 0.0 } }
}

#[derive(Clone, Copy, Debug, Default)]
struct CtrlFrame {
    v: [VoiceCtrl; NUM_VOICES],
    lead_midi: f32,
}

/// Estado de "humano" de cada cantor virtual.
#[derive(Clone, Copy)]
struct Singer {
    pitch: Drift,
    timing: Drift,
    level: Drift,
    /// Tendencia fixa de afinacao (cents) e de garganta (oitavas de leitura).
    tune_bias: f32,
    throat: f32,
    base_delay_ms: f32,
    active: f32,
}

/// Seguimento do lead e plano harmonico.
struct Plan {
    note: Option<i32>,
    harm: VoiceNotes,
    ctx: Option<HarmonyCtx>,
    /// Memoria de conducao de vozes (notas, lead) do ultimo acorde cantado.
    memory: Option<(VoiceNotes, i32)>,
    memory_age: f32,
    unvoiced_time: f32,
    time_in_note: f32,
    stability: f32,
    drift_far_time: f32,
    octave_time: f32,
    // transicao entre notas
    trans: bool,
    trans_time: f32,
    trans_pitch_based: bool,
    t0: [f32; NUM_VOICES],
    p0: f32,
    // desvio do lead em relacao a nota (vibrato, entonacao, scoops)
    d_lp1: f32,
    d_lp2: f32,
    last_p: f32,
    last_target: [f32; NUM_VOICES],
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            note: None,
            harm: [None; MAX_VOICES],
            ctx: None,
            memory: None,
            memory_age: 0.0,
            unvoiced_time: 1.0,
            time_in_note: 0.0,
            stability: 0.0,
            drift_far_time: 0.0,
            octave_time: 0.0,
            trans: false,
            trans_time: 0.0,
            trans_pitch_based: false,
            t0: [f32::NAN; NUM_VOICES],
            p0: 0.0,
            d_lp1: 0.0,
            d_lp2: 0.0,
            last_p: 60.0,
            last_target: [f32::NAN; NUM_VOICES],
        }
    }
}

pub struct Engine {
    sr: f32,
    latency: usize,
    lookahead: f64,
    hop: usize,
    input: Ring,
    dry_l: Ring,
    dry_r: Ring,
    pitch: PitchTracker,
    marks: MarkTracker,
    hann: HannTable,
    n_in: i64,
    mono: Vec<f32>,

    params: EngineParams,
    ctx_events: Vec<(i64, HarmonyCtx)>,
    midi_held: [u8; 128],

    frames: Vec<CtrlFrame>,
    frame_count: u64,
    plan: Plan,
    singers: [Singer; NUM_VOICES],
    rng: Rng,

    voices: Vec<GrainVoice>,
    hpf: [Biquad; NUM_VOICES],
    shelf: [Biquad; NUM_VOICES],
    lead_gain: f32,
    harm_gain: f32,
    pans: [(f32, f32); NUM_VOICES],
    pans_target: [(f32, f32); NUM_VOICES],

    /// Medidores para a interface.
    pub meter_lead: f32,
    pub meter_voices: [f32; MAX_VOICES],
}

impl Engine {
    pub fn new(sr: f32) -> Self {
        let sr = sr.max(8000.0);
        let latency = (sr * LATENCY_MS / 1000.0).round() as usize;
        let pitch = PitchTracker::new(sr);
        let hop = pitch.hop;
        let mut rng = Rng::new(0x48_45_41_56_45);
        let base = [13.0, 21.0, 8.0, 27.0, 33.0, 16.0, 24.0, 11.0];
        let throat = [0.05, -0.055, 0.07, -0.035, -0.045, 0.065, -0.07, 0.04];
        let singers = std::array::from_fn(|i| {
            let _ = rng.next_u64();
            Singer {
                pitch: Drift::new(1.0, 0.7),
                timing: Drift::new(1.0, 1.6),
                level: Drift::new(1.0, 1.1),
                tune_bias: (rng.uniform() - 0.5) * 2.0,
                throat: throat[i],
                base_delay_ms: base[i],
                active: 0.0,
            }
        });
        let sr64 = sr as f64;
        Self {
            sr,
            latency,
            lookahead: (sr * LOOKAHEAD_MS / 1000.0) as f64,
            hop,
            input: Ring::new(1 << 17),
            dry_l: Ring::new(1 << 15),
            dry_r: Ring::new(1 << 15),
            pitch,
            marks: MarkTracker::new(sr),
            hann: HannTable::new(),
            n_in: 0,
            mono: vec![0.0; 1024],
            params: EngineParams::default(),
            ctx_events: Vec::with_capacity(1024),
            midi_held: [0; 128],
            frames: vec![CtrlFrame::default(); CTRL_CAP],
            frame_count: 0,
            plan: Plan::default(),
            singers,
            rng,
            voices: (0..NUM_VOICES).map(|_| GrainVoice::new()).collect(),
            hpf: [Biquad::highpass(sr64, 75.0, 0.707); NUM_VOICES],
            shelf: [Biquad::high_shelf(sr64, 9000.0, -1.5); NUM_VOICES],
            lead_gain: 1.0,
            harm_gain: 0.0,
            pans: [(0.707, 0.707); NUM_VOICES],
            pans_target: [(0.707, 0.707); NUM_VOICES],
            meter_lead: 0.0,
            meter_voices: [0.0; MAX_VOICES],
        }
    }

    pub fn sample_rate(&self) -> f32 { self.sr }

    pub fn latency(&self) -> usize { self.latency }

    /// Acesso de diagnostico ao rastreador de altura.
    pub fn pitch_tracker(&self) -> &PitchTracker { &self.pitch }

    /// Diagnostico: (nota do lead, notas alvo das vozes) no instante `t` da linha do tempo.
    pub fn debug_targets(&self, t: f64) -> (f32, [f32; NUM_VOICES]) {
        let f = self.frame_at(t);
        (f.lead_midi, std::array::from_fn(|v| f.v[v].midi * if f.v[v].gain > 0.01 { 1.0 } else { f32::NAN }))
    }

    fn ctx_from(&self, p: &EngineParams) -> HarmonyCtx {
        let mut c = HarmonyCtx {
            mode: p.mode,
            root: p.root % 12,
            quality: p.quality.min(15),
            scale: p.scale,
            voicing: p.voicing,
            voices: p.voices.clamp(1, MAX_VOICES as u8),
            midi: [0; 16],
            midi_n: 0,
        };
        if matches!(p.mode, Mode::MidiChord | Mode::MidiNotes) {
            for (n, &v) in self.midi_held.iter().enumerate() {
                if v > 0 && (c.midi_n as usize) < c.midi.len() {
                    c.midi[c.midi_n as usize] = n as u8;
                    c.midi_n += 1;
                }
            }
        }
        c
    }

    fn push_ctx(&mut self, at: i64) {
        let c = self.ctx_from(&self.params);
        if self.ctx_events.last().is_some_and(|(_, last)| *last == c) {
            return;
        }
        if self.ctx_events.len() == self.ctx_events.capacity() {
            self.ctx_events.remove(0);
        }
        self.ctx_events.push((at, c));
    }

    /// Atualiza os parametros no inicio do bloco (o contexto harmonico entra na linha do tempo).
    pub fn set_params(&mut self, p: &EngineParams) {
        self.params = *p;
        self.push_ctx(self.n_in);
    }

    /// Evento MIDI no deslocamento `offset` do proximo bloco.
    pub fn note_on(&mut self, offset: usize, note: u8, vel: u8) {
        self.midi_held[(note & 0x7F) as usize] = vel.max(1);
        self.push_ctx(self.n_in + offset as i64);
    }

    pub fn note_off(&mut self, offset: usize, note: u8) {
        self.midi_held[(note & 0x7F) as usize] = 0;
        self.push_ctx(self.n_in + offset as i64);
    }

    pub fn all_notes_off(&mut self, offset: usize) {
        self.midi_held = [0; 128];
        self.push_ctx(self.n_in + offset as i64);
    }

    fn ctx_at(&self, t: f64) -> HarmonyCtx {
        for (at, c) in self.ctx_events.iter().rev() {
            if (*at as f64) <= t {
                return *c;
            }
        }
        self.ctx_events.first().map_or_else(|| self.ctx_from(&self.params), |(_, c)| *c)
    }

    fn prune_ctx(&mut self, before: f64) {
        // mantem o ultimo evento anterior a `before`
        let keep_from = self.ctx_events.iter().rposition(|(at, _)| (*at as f64) <= before).unwrap_or(0);
        if keep_from > 0 {
            self.ctx_events.drain(..keep_from);
        }
    }

    /// Processa um bloco estereo. As fatias devem ter o mesmo tamanho.
    pub fn process(&mut self, in_l: &[f32], in_r: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = in_l.len().min(in_r.len()).min(out_l.len()).min(out_r.len());
        let mut pos = 0;
        while pos < n {
            let m = (n - pos).min(self.mono.len());
            self.process_chunk(&in_l[pos..pos + m], &in_r[pos..pos + m], &mut out_l[pos..pos + m], &mut out_r[pos..pos + m]);
            pos += m;
        }
    }

    fn process_chunk(&mut self, in_l: &[f32], in_r: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = in_l.len();
        let n0 = self.n_in;
        for i in 0..n {
            let (l, r) = (sane(in_l[i]), sane(in_r[i]));
            let m = 0.5 * (l + r);
            self.mono[i] = m;
            let t = n0 + i as i64;
            self.input.set(t, m);
            self.dry_l.set(t, l);
            self.dry_r.set(t, r);
        }
        let mono = std::mem::take(&mut self.mono);
        self.pitch.push(&mono[..n]);
        self.mono = mono;
        self.n_in += n as i64;
        self.marks.update(&self.pitch, self.n_in);

        // quadros de controle
        let hop = self.hop as f64;
        while (self.frame_count as f64) * hop + self.lookahead <= self.pitch.latest_time() {
            let k = self.frame_count;
            let fr = self.control_frame(k as f64 * hop);
            self.frames[(k % CTRL_CAP as u64) as usize] = fr;
            self.frame_count += 1;
        }
        let lat = self.latency as i64;
        self.prune_ctx((n0 - lat) as f64 - self.sr as f64);

        // graos
        let tau_end = (n0 + n as i64 - lat) as f64;
        let until = tau_end + max_half_len(self.sr);
        let mut voices = std::mem::take(&mut self.voices);
        for (v, voice) in voices.iter_mut().enumerate() {
            voice.plan(until, &self.input, &self.marks, &Ctl { e: self, v }, &self.hann);
        }
        self.voices = voices;

        // mistura
        let p = self.params;
        let lead_t = if p.bypass { 1.0 } else { db_to_gain(p.lead_db) };
        let k = self.active_voice_count();
        // "Harmony" e o nivel do grupo: soma de potencia normalizada (2 vozes = referencia)
        let copies = k * if p.double { 2 } else { 1 };
        let group = (2.0 / copies as f32).sqrt().min(1.2);
        let harm_t = if p.bypass { 0.0 } else { db_to_gain(p.harmony_db) * group };
        for v in 0..NUM_VOICES {
            self.pans_target[v] = pan_law(pan_position(v, k, p.width));
        }
        let a = 1.0 - (-1.0 / (0.02 * self.sr)).exp();
        for i in 0..n {
            let tau = n0 + i as i64 - lat;
            self.lead_gain += (lead_t - self.lead_gain) * a;
            self.harm_gain += (harm_t - self.harm_gain) * a;
            let (mut l, mut r) = (self.dry_l.at(tau) * self.lead_gain, self.dry_r.at(tau) * self.lead_gain);
            for v in 0..NUM_VOICES {
                let x = self.voices[v].take(tau);
                let pt = self.pans_target[v];
                let pn = &mut self.pans[v];
                pn.0 += (pt.0 - pn.0) * a;
                pn.1 += (pt.1 - pn.1) * a;
                if x != 0.0 {
                    let y = self.shelf[v].process(self.hpf[v].process(x)) * self.harm_gain;
                    l += y * pn.0;
                    r += y * pn.1;
                } else {
                    self.shelf[v].process(self.hpf[v].process(0.0));
                }
            }
            out_l[i] = l;
            out_r[i] = r;
        }
        // medidores (instante de saida mais recente)
        let f = self.frame_at(tau_end);
        self.meter_lead = if f.lead_midi > 0.0 { f.lead_midi } else { 0.0 };
        for v in 0..MAX_VOICES {
            let c = f.v[v];
            self.meter_voices[v] = if c.midi.is_finite() && c.gain > 0.05 { c.midi } else { 0.0 };
        }
    }

    fn active_voice_count(&self) -> usize {
        if self.params.mode == Mode::MidiNotes {
            self.plan.harm.iter().filter(|h| h.is_some()).count().max(1)
        } else {
            self.params.voices.clamp(1, MAX_VOICES as u8) as usize
        }
    }

    fn frame_at(&self, t: f64) -> CtrlFrame {
        if self.frame_count == 0 {
            return CtrlFrame::default();
        }
        let k = (t / self.hop as f64).floor().max(0.0) as u64;
        let last = self.frame_count - 1;
        let oldest = self.frame_count.saturating_sub(CTRL_CAP as u64 - 4);
        let k = k.clamp(oldest, last);
        self.frames[(k % CTRL_CAP as u64) as usize]
    }

    /// Controle interpolado de uma voz no instante `t`.
    fn voice_at(&self, v: usize, t: f64) -> VoiceCtrl {
        if self.frame_count == 0 {
            return VoiceCtrl::default();
        }
        let x = (t / self.hop as f64).max(0.0);
        let last = self.frame_count - 1;
        let oldest = self.frame_count.saturating_sub(CTRL_CAP as u64 - 4);
        let k0 = (x.floor() as u64).clamp(oldest, last);
        let k1 = (k0 + 1).min(last);
        let f = (x - k0 as f64).clamp(0.0, 1.0) as f32;
        let a = self.frames[(k0 % CTRL_CAP as u64) as usize].v[v];
        let b = self.frames[(k1 % CTRL_CAP as u64) as usize].v[v];
        let lerp = |p: f32, q: f32| p + (q - p) * f;
        VoiceCtrl {
            midi: if a.midi.is_finite() && b.midi.is_finite() {
                lerp(a.midi, b.midi)
            } else if f < 0.5 {
                a.midi
            } else {
                b.midi
            },
            formant: lerp(a.formant, b.formant),
            gain: lerp(a.gain, b.gain),
            delay: lerp(a.delay, b.delay),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Plano harmonico
    // -----------------------------------------------------------------------------------------

    /// Nota em que o lead vai estabilizar nos ultimos ~25 ms da janela de antecipacao.
    fn future_stable_note(&self, t: f64) -> Option<i32> {
        let hop = self.hop as f64;
        let n = (self.lookahead / hop).floor() as i32;
        let mut vals = [0.0f32; 16];
        let mut c = 0;
        let k0 = (n - 6).max(0);
        for j in k0..=n {
            let fr = self.pitch.at(t + j as f64 * hop);
            if fr.voiced() && c < vals.len() {
                vals[c] = hz_to_midi(fr.f0);
                c += 1;
            }
        }
        let total = (n - k0 + 1) as usize;
        if c * 5 < total * 4 {
            return None;
        }
        let v = &mut vals[..c];
        v.sort_by(f32::total_cmp);
        if v[c - 1] - v[0] > 1.0 {
            return None;
        }
        Some(v[c / 2].round() as i32)
    }

    /// Nota de inicio de frase: mediana dos quadros vozeados de toda a janela de antecipacao
    /// (robusta a um quadro errado no ataque).
    fn onset_note(&self, t: f64) -> Option<i32> {
        let hop = self.hop as f64;
        let n = (self.lookahead / hop).floor() as usize;
        let mut vals = [0.0f32; 32];
        let mut c = 0;
        for j in 0..=n.min(31) {
            let fr = self.pitch.at(t + j as f64 * hop);
            if fr.voiced() {
                vals[c] = hz_to_midi(fr.f0);
                c += 1;
            }
        }
        if c < 3 {
            return None;
        }
        let v = &mut vals[..c];
        v.sort_by(f32::total_cmp);
        Some(v[c / 2].round() as i32)
    }

    fn control_frame(&mut self, t: f64) -> CtrlFrame {
        let dt = self.hop as f32 / self.sr;
        let fr = self.pitch.at(t);
        let voiced = fr.voiced() && fr.conf > 0.02;
        let mut p = if voiced { hz_to_midi(fr.f0) } else { self.plan.last_p };
        // Guarda de oitava: excursao de ~12 semitons em relacao a nota atual e quase sempre erro
        // de deteccao (voz creaky, 2o harmonico forte). So vira salto real se persistir 120 ms.
        let mut octave_suspect = false;
        if let (true, Some(nn), true) = (voiced, self.plan.note, self.plan.time_in_note > 0.08) {
            let d = p - nn as f32;
            let folded = p - 12.0 * (d / 12.0).round();
            if d.abs() > 9.0 && (folded - nn as f32).abs() < 2.5 {
                self.plan.octave_time += dt;
                if self.plan.octave_time < 0.12 {
                    p = folded;
                    octave_suspect = true;
                }
            } else {
                self.plan.octave_time = 0.0;
            }
        }
        let params = self.params;

        // contexto harmonico, com antecipacao: no inicio de uma nota, um acorde que comeca
        // nos proximos 80 ms ja vale para ela (o cantor costuma antecipar o tempo forte)
        let ctx_now = self.ctx_at(t);
        let ctx_soon = self.ctx_at(t + 0.08 * self.sr as f64);
        let mut ctx = ctx_now;

        let mut switched = false;
        let mut new_note: Option<i32> = self.plan.note;
        if voiced {
            self.plan.unvoiced_time = 0.0;
            let future = self.future_stable_note(t).filter(|&f| {
                // durante uma suspeita de oitava, nao aceita a nota uma oitava acima/abaixo
                !(octave_suspect || self.plan.note.is_some_and(|nn| (f - nn).abs() % 12 == 0 && f != nn && self.plan.octave_time < 0.12))
            });
            match self.plan.note {
                None => {
                    new_note = Some(future.or_else(|| self.onset_note(t)).unwrap_or(p.round() as i32));
                    switched = true;
                }
                Some(nn) => {
                    let d = p - nn as f32;
                    if let Some(f) = future {
                        if f != nn {
                            let toward = d * (f - nn) as f32 > 0.0;
                            if (toward && d.abs() > 0.3) || (p - f as f32).abs() < 0.5 {
                                new_note = Some(f);
                                switched = true;
                            }
                        }
                    }
                    if !switched {
                        // deriva longa sem nota estavel a frente (passagens rapidas)
                        if d.abs() > 0.8 {
                            self.plan.drift_far_time += dt;
                            if self.plan.drift_far_time > 0.07 {
                                new_note = Some(p.round() as i32);
                                switched = new_note != self.plan.note;
                            }
                        } else {
                            self.plan.drift_far_time = 0.0;
                        }
                    }
                }
            }
        } else {
            self.plan.unvoiced_time += dt;
            if self.plan.unvoiced_time > 0.25 && self.plan.note.is_some() {
                // fim de frase
                self.plan.memory = Some((self.plan.harm, self.plan.note.unwrap_or(60)));
                self.plan.memory_age = 0.0;
                self.plan.note = None;
                new_note = None;
            }
        }
        if self.plan.note.is_none() {
            self.plan.memory_age += dt;
            if self.plan.memory_age > 2.0 {
                self.plan.memory = None;
            }
        }
        // uma vez adotado o proximo acorde, ele vale ate chegar (sem voltar ao anterior)
        if ctx_soon != ctx_now && (switched || self.plan.ctx == Some(ctx_soon)) {
            ctx = ctx_soon;
        }
        let ctx_changed = self.plan.ctx != Some(ctx);

        // recalcula as notas das vozes
        if let Some(lead) = new_note {
            if switched || ctx_changed {
                let prev = if self.plan.note.is_some() {
                    Some((self.plan.harm, self.plan.note.unwrap_or(lead)))
                } else {
                    self.plan.memory
                };
                let harm = choose_voicing(&ctx, lead, prev.as_ref().map(|(h, l)| (h, *l)));
                let continuing = self.plan.note.is_some();
                // inicio de transicao: guarda os alvos atuais
                if continuing {
                    self.plan.trans = true;
                    self.plan.trans_time = 0.0;
                    self.plan.t0 = self.plan.last_target;
                    self.plan.p0 = p;
                    self.plan.trans_pitch_based = switched && (lead as f32 - p).abs() > 0.3;
                } else {
                    self.plan.trans = false;
                }
                if switched {
                    self.plan.time_in_note = 0.0;
                    let d = p - lead as f32;
                    self.plan.d_lp1 = d;
                    self.plan.d_lp2 = d;
                    self.plan.stability = 0.0;
                    self.plan.drift_far_time = 0.0;
                }
                self.plan.harm = harm;
                self.plan.ctx = Some(ctx);
            }
        }
        self.plan.note = new_note;
        if voiced {
            self.plan.last_p = p;
        }

        // ------------------------------------------------------------------ alvos por voz
        let hum = params.humanize.clamp(0.0, 1.0);
        let mut out = CtrlFrame { lead_midi: if voiced { p } else { 0.0 }, ..Default::default() };
        let lead = self.plan.note;
        let mut dev = 0.0;
        if let (Some(nn), true) = (lead, voiced) {
            self.plan.time_in_note += dt;
            let d = p - nn as f32;
            // filtro passa-baixa de 2 polos (~2 Hz) sobre o desvio: separa entonacao de vibrato
            let a = 1.0 - (-std::f32::consts::TAU * 2.0 * dt).exp();
            self.plan.d_lp1 += (d - self.plan.d_lp1) * a;
            self.plan.d_lp2 += (self.plan.d_lp1 - self.plan.d_lp2) * a;
            let st_target = if d.abs() < 0.4 && self.plan.time_in_note > 0.06 { 1.0 } else { 0.0 };
            let sa = 1.0 - (-dt / 0.04).exp();
            self.plan.stability += (st_target - self.plan.stability) * sa;
            let s = self.plan.stability;
            // nota estavel: segue a entonacao do lead (85%) e parte do vibrato (55%);
            // nota em movimento (scoop, glissando): segue o lead inteiro
            let (alpha, beta) = (0.85, 0.55);
            let dl = self.plan.d_lp2;
            dev = (1.0 - s) * d + s * (alpha * dl + beta * (d - dl));
        }
        if self.plan.trans {
            self.plan.trans_time += dt;
        }
        let trans_prog = if self.plan.trans {
            let time_prog = self.plan.trans_time / if self.plan.trans_pitch_based { 0.18 } else { 0.07 };
            let pitch_prog = match (lead, self.plan.trans_pitch_based) {
                (Some(nn), true) => {
                    let span = nn as f32 - self.plan.p0;
                    if span.abs() > 1e-3 { ((p - self.plan.p0) / span).clamp(0.0, 1.0) } else { 1.0 }
                }
                _ => 0.0,
            };
            let pr = pitch_prog.max(time_prog).min(1.0);
            if pr >= 1.0 {
                self.plan.trans = false;
            }
            smoothstep(pr)
        } else {
            1.0
        };

        let (_, _, root) = lead.map_or((0, 0, ctx.root), |nn| ctx.chord_for(nn));
        let k = if ctx.mode == Mode::MidiNotes { MAX_VOICES } else { ctx.voices as usize };
        let sr_ms = self.sr / 1000.0;
        let lead_hz_ok = voiced;
        for v in 0..NUM_VOICES {
            let base_v = v % MAX_VOICES;
            let is_double = v >= MAX_VOICES;
            let note = self.plan.harm[base_v];
            let wanted = base_v < k && note.is_some() && (!is_double || params.double) && !params.bypass;
            let sg = &mut self.singers[v];
            // humanizacao: deriva de afinacao, tempo e nivel
            let pitch_sigma = (2.0 + 9.0 * hum) * if is_double { 1.3 } else { 1.0 };
            let cents = sg.pitch.step(dt, &mut self.rng) * pitch_sigma + sg.tune_bias * 3.0 * hum;
            let timing = sg.timing.step(dt, &mut self.rng) * 7.0 * hum;
            let level_db = sg.level.step(dt, &mut self.rng) * 1.2 * hum;
            let act_t = if wanted { 1.0 } else { 0.0 };
            let act_a = 1.0 - (-dt / if wanted { 0.03 } else { 0.06 }).exp();
            sg.active += (act_t - sg.active) * act_a;
            let delay_ms = (sg.base_delay_ms * (0.25 + 1.5 * hum) + timing + if is_double { 6.0 } else { 0.0 }).clamp(-3.0, 48.0);

            let mut vc = VoiceCtrl { delay: delay_ms * sr_ms, gain: 0.0, formant: 1.0, midi: f32::NAN };
            if let (Some(h), Some(nn)) = (note, lead) {
                let tune = if params.pure { 0.7 * just_offset(h, nn, root) } else { 0.0 };
                let target = h as f32 + dev + tune + cents / 100.0;
                let mut midi = target;
                if self.plan.trans {
                    let t0 = self.plan.t0[v];
                    if t0.is_finite() {
                        midi = t0 + (target - t0) * trans_prog;
                    }
                }
                vc.midi = midi;
                self.plan.last_target[v] = midi;
            } else if let Some(h) = note {
                // lead em consoante/silencio: mantem o ultimo alvo
                let last = self.plan.last_target[v];
                vc.midi = if last.is_finite() { last } else { h as f32 };
            } else {
                vc.midi = self.plan.last_target[v];
            }
            // garganta: identidade do cantor + acompanha um pouco o deslocamento
            let shift = if vc.midi.is_finite() && lead_hz_ok { (vc.midi - p) / 12.0 } else { 0.0 };
            let throat_oct = sg.throat * (0.3 + 1.4 * hum) + params.formant * 0.3 + 0.1 * shift.clamp(-2.0, 2.0);
            vc.formant = throat_oct.exp2().clamp(0.75, 1.4);
            let dbl = if is_double { db_to_gain(-2.5) } else { 1.0 };
            vc.gain = sg.active * db_to_gain(level_db) * dbl;
            out.v[v] = vc;
        }
        out
    }
}

/// Visao de uma voz para o shifter.
struct Ctl<'a> {
    e: &'a Engine,
    v: usize,
}

impl VoiceControl for Ctl<'_> {
    fn delay(&self, c: f64) -> f64 { self.e.voice_at(self.v, c).delay as f64 }

    fn state(&self, s: f64) -> VoiceState {
        let c = self.e.voice_at(self.v, s);
        let fr = self.e.pitch.at(s);
        // consoantes mais baixas nas harmonias: sibilantes multiplicadas soam artificiais
        let uv = if fr.hf > 0.3 { 0.18 } else { 0.32 };
        let uv = if self.v >= MAX_VOICES { uv * 0.6 } else { uv };
        VoiceState {
            period: if c.midi.is_finite() { self.e.sr / midi_to_hz(c.midi) } else { 0.0 },
            formant: c.formant,
            gain: c.gain,
            gain_weak: c.gain * 0.55,
            gain_uv: c.gain * uv,
        }
    }
}

#[inline]
fn sane(x: f32) -> f32 { if x.is_finite() { x.clamp(-8.0, 8.0) } else { 0.0 } }

/// Posicao estereo (-1..1) da voz `v` com `k` harmonias ativas.
fn pan_position(v: usize, k: usize, width: f32) -> f32 {
    let base = v % MAX_VOICES;
    let p = match (k, base) {
        (1, _) => 0.35,
        (2, 0) => -0.65,
        (2, _) => 0.65,
        (3, 0) => -0.75,
        (3, 1) => 0.75,
        (3, _) => 0.2,
        (_, 0) => -0.85,
        (_, 1) => 0.85,
        (_, 2) => -0.4,
        _ => 0.4,
    };
    let p = if v >= MAX_VOICES { -p * 0.9 } else { p };
    (p * width).clamp(-1.0, 1.0)
}

/// Lei de pan de potencia constante.
fn pan_law(p: f32) -> (f32, f32) {
    let a = (p + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}
