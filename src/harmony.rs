//! Geracao de harmonias do Akai HV10: modos STYLE, MODAL, FLEXICHORD, MONO CHN., WAIT ALL,
//! PRIORITY (manual de operacao HV10, rev. 08/2002) e o modo moderno SMART (motor de jazz com
//! inferencia de acorde, memoria de conducao de vozes e anti-choque de b9).
//!
//! As partes de harmonia seguem a nomenclatura do HV10: L2, L1 (abaixo), H1, H2 (acima).

use crate::scales::{StyleBank, StyleId, StyleLookup};

pub const NUM_PARTS: usize = 4;
pub const PART_NAMES: [&str; NUM_PARTS] = ["L2", "L1", "H1", "H2"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarmonyMode {
    Style,
    Modal,
    FlexiChord,
    MonoChn,
    WaitAll,
    Priority,
    /// Modo moderno (nao existe no HV10): acorde inteligente com conducao de vozes.
    Smart,
}

/// VOICING do modo MODAL: UN, 8L, 8H, L1..L8, H1..H8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalVoicing {
    Un,
    OctLow,
    OctHigh,
    Low(u8),
    High(u8),
}

impl ModalVoicing {
    /// Indice 0..18: UN, 8L, 8H, L8..L1, H1..H8 (ordem do display do HV10).
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => ModalVoicing::Un,
            1 => ModalVoicing::OctLow,
            2 => ModalVoicing::OctHigh,
            3..=10 => ModalVoicing::Low((11 - i) as u8),
            11..=18 => ModalVoicing::High((i - 10) as u8),
            _ => ModalVoicing::Un,
        }
    }
}

/// Grau do TEMPLATE do modo MODAL (3 e 7 aceitam m/M).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateDegree {
    Off,
    On,
    Minor,
    Major,
}

/// VOICING do modo FLEXICHORD: LO,LO,LO,HI / LO,LO,HI,HI / LO,HI,HI,HI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexiVoicing {
    LllH,
    LlHh,
    LHhh,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HarmonySettings {
    pub mode: HarmonyMode,
    pub key: u8,
    pub styles: [StyleId; NUM_PARTS],
    pub modal_voicing: [ModalVoicing; NUM_PARTS],
    /// Graus 1..7 do TEMPLATE.
    pub template: [TemplateDegree; 7],
    pub flexi_voicing: FlexiVoicing,
    /// PRIORITY: ordem (1..4) da nota recebida atribuida a cada parte L2, L1, H1, H2.
    pub priority_rank: [u8; NUM_PARTS],
    /// Partes ligadas em BUDDIES (usado por WAIT ALL).
    pub part_on: [bool; NUM_PARTS],
}

impl Default for HarmonySettings {
    fn default() -> Self {
        Self {
            mode: HarmonyMode::Style,
            key: 0,
            styles: [StyleId::Un, StyleId::Un, StyleId::S3A, StyleId::S3A],
            modal_voicing: [ModalVoicing::Low(2), ModalVoicing::Low(1), ModalVoicing::High(1), ModalVoicing::High(2)],
            template: [
                TemplateDegree::On, TemplateDegree::Off, TemplateDegree::On, TemplateDegree::Off,
                TemplateDegree::On, TemplateDegree::Off, TemplateDegree::Off,
            ],
            flexi_voicing: FlexiVoicing::LlHh,
            priority_rank: [1, 2, 3, 4],
            part_on: [false, false, true, true],
        }
    }
}

/// Estado MIDI recebido (notas, pedal, ultimo acorde/raiz travados).
#[derive(Clone, Debug)]
pub struct MidiState {
    pub base_channel: u8,
    held: [bool; 128],
    sustained: [bool; 128],
    sustain_pedal: bool,
    /// Notas do canal base em ordem cronologica de chegada.
    fifo: [u8; 16],
    fifo_len: usize,
    /// MONO CHN.: nota por parte (canal base + 1..4).
    part_note: [Option<u8>; NUM_PARTS],
    /// MODAL: ultima raiz recebida (travada).
    modal_root: Option<u8>,
    /// FLEXICHORD: classes de altura do ultimo acorde reconhecido (travado).
    flexi_chord: Option<[bool; 12]>,
    /// SMART: memoria de conducao de vozes.
    last_smart_shift: [Option<i32>; NUM_PARTS],
}

impl Default for MidiState {
    fn default() -> Self {
        Self {
            base_channel: 0,
            held: [false; 128],
            sustained: [false; 128],
            sustain_pedal: false,
            fifo: [0; 16],
            fifo_len: 0,
            part_note: [None; NUM_PARTS],
            modal_root: None,
            flexi_chord: None,
            last_smart_shift: [None; NUM_PARTS],
        }
    }
}

impl MidiState {
    pub fn reset(&mut self) {
        let base = self.base_channel;
        *self = Self::default();
        self.base_channel = base;
    }

    fn remove_from_fifo(&mut self, note: u8) {
        let mut j = 0;
        for i in 0..self.fifo_len {
            if self.fifo[i] != note {
                self.fifo[j] = self.fifo[i];
                j += 1;
            }
        }
        self.fifo_len = j;
    }

    fn push_fifo(&mut self, note: u8) {
        if self.fifo[..self.fifo_len].contains(&note) { return; }
        if self.fifo_len == self.fifo.len() {
            self.fifo.copy_within(1.., 0);
            self.fifo_len -= 1;
        }
        self.fifo[self.fifo_len] = note;
        self.fifo_len += 1;
    }

    /// Canal relativo ao canal base (0 = base/ORIGINAL, 1 = L2, 2 = L1, 3 = H1, 4 = H2).
    pub fn channel_offset(&self, ch: u8) -> u8 {
        (ch.wrapping_add(16).wrapping_sub(self.base_channel)) & 0x0F
    }

    pub fn note_on(&mut self, ch: u8, note: u8) {
        let note = note & 0x7F;
        let off = self.channel_offset(ch);
        if (1..=4).contains(&off) {
            self.part_note[(off - 1) as usize] = Some(note);
        }
        if off == 0 {
            self.held[note as usize] = true;
            self.sustained[note as usize] = false;
            self.push_fifo(note);
            self.modal_root = Some(note);
            self.update_flexi_chord();
        }
    }

    pub fn note_off(&mut self, ch: u8, note: u8) {
        let note = note & 0x7F;
        let off = self.channel_offset(ch);
        if (1..=4).contains(&off) && self.part_note[(off - 1) as usize] == Some(note) && !self.sustain_pedal {
            self.part_note[(off - 1) as usize] = None;
        }
        if off == 0 {
            self.held[note as usize] = false;
            if self.sustain_pedal {
                self.sustained[note as usize] = true;
            } else {
                self.remove_from_fifo(note);
            }
        }
    }

    pub fn sustain(&mut self, on: bool) {
        let was = self.sustain_pedal;
        self.sustain_pedal = on;
        if was && !on {
            for n in 0..128 {
                if self.sustained[n] && !self.held[n] {
                    self.sustained[n] = false;
                    self.remove_from_fifo(n as u8);
                    for slot in &mut self.part_note {
                        if *slot == Some(n as u8) { *slot = None; }
                    }
                }
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.reset();
    }

    fn update_flexi_chord(&mut self) {
        if self.fifo_len < 3 { return; }
        let mut mask = [false; 12];
        for &n in &self.fifo[..self.fifo_len] { mask[(n % 12) as usize] = true; }
        if recognize_chord(&mask) { self.flexi_chord = Some(mask); }
    }

    fn sorted_notes(&self) -> ([u8; 16], usize) {
        let mut s = [0u8; 16];
        s[..self.fifo_len].copy_from_slice(&self.fifo[..self.fifo_len]);
        s[..self.fifo_len].sort_unstable();
        (s, self.fifo_len)
    }
}

/// Formas de acorde reconhecidas pelo FLEXICHORD (intervalos a partir da raiz).
const CHORD_SHAPES: [&[u8]; 16] = [
    &[0, 4, 7], &[0, 3, 7], &[0, 3, 6], &[0, 4, 8], &[0, 5, 7], &[0, 2, 7],
    &[0, 4, 7, 10], &[0, 4, 7, 11], &[0, 3, 7, 10], &[0, 3, 6, 10], &[0, 3, 6, 9],
    &[0, 4, 7, 9], &[0, 3, 7, 9], &[0, 3, 7, 11], &[0, 5, 7, 10], &[0, 2, 4, 7],
];

/// Reconhece se as classes de altura formam um acorde conhecido (com qualquer inversao).
pub fn recognize_chord(mask: &[bool; 12]) -> bool {
    let count = mask.iter().filter(|&&b| b).count();
    for root in 0..12 {
        if !mask[root] { continue; }
        for shape in CHORD_SHAPES {
            if shape.len() != count { continue; }
            if shape.iter().all(|&iv| mask[(root + iv as usize) % 12]) { return true; }
        }
    }
    false
}

/// n-esima nota de acorde estritamente acima (`up`) ou abaixo da nota de entrada.
fn nth_chord_tone(mask: &[bool; 12], input_note: i32, n: u8, up: bool) -> Option<i32> {
    if n == 0 { return None; }
    let mut found = 0u8;
    for step in 1..=48 {
        let cand = if up { input_note + step } else { input_note - step };
        if mask[cand.rem_euclid(12) as usize] {
            found += 1;
            if found == n { return Some(cand - input_note); }
        }
    }
    None
}

const MAJOR_SCALE: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Classes de altura do TEMPLATE para a raiz recebida via MIDI, na tonalidade `key`.
fn modal_chord_mask(template: &[TemplateDegree; 7], key: u8, root: u8) -> [bool; 12] {
    let key = (key % 12) as i32;
    let root_pc = (root as i32).rem_euclid(12);
    let rel = (root_pc - key).rem_euclid(12);
    let degree_idx = MAJOR_SCALE.iter().position(|&d| d == rel);
    let mut mask = [false; 12];
    for (d, &deg) in template.iter().enumerate() {
        if deg == TemplateDegree::Off { continue; }
        let interval = match (d, deg) {
            (2, TemplateDegree::Minor) => 3,
            (2, TemplateDegree::Major) => 4,
            (6, TemplateDegree::Minor) => 10,
            (6, TemplateDegree::Major) => 11,
            _ => match degree_idx {
                // Raiz diatonica: grau diatonico da escala de KEY (harmonia modal).
                Some(i) => {
                    let from = MAJOR_SCALE[i];
                    let to = MAJOR_SCALE[(i + d) % 7] + if i + d >= 7 { 12 } else { 0 };
                    to - from
                }
                // Raiz cromatica: graus da escala maior sobre a raiz.
                None => MAJOR_SCALE[d],
            },
        };
        mask[((root_pc + interval).rem_euclid(12)) as usize] = true;
    }
    mask
}

/// Motor de harmonia: calcula o deslocamento (semitons) de cada parte para o pitch cantado.
#[derive(Clone, Debug, Default)]
pub struct HarmonyEngine {
    pub midi: MidiState,
}

/// Desloca `s` por oitavas ate caber em +-MIDI_SHIFT_RANGE (o DSP so alcanca +-12 st).
fn fold_into_range(mut s: f32) -> f32 {
    let r = crate::dsp::hw::MIDI_SHIFT_RANGE;
    while s > r { s -= 12.0; }
    while s < -r { s += 12.0; }
    s
}

/// P.CORRECT do DSP (0x10005c50). `interval(n)` = intervalo da harmonia para a nota n.
/// `c` = 1 - correcao. Com c = 1 a saida interpola linearmente entre os alvos (nota + intervalo)
/// das notas atribuidas vizinhas ao pitch cantado: o vibrato passa e nao ha degrau na fronteira
/// de semitom. Com c = 0 a saida trava no alvo da nota mais proxima (nota sem atribuicao usa o
/// vizinho na direcao do desvio, como o DSP). Para vizinhos adjacentes, e exatamente a formula
/// do DSP: nota + e + c*B*(e2 - e + 1).
fn corrected_shift<F: Fn(i32) -> Option<i32>>(interval: F, input_midi: f32, c: f32) -> Option<f32> {
    let target = |n: i32| interval(n).map(|e| (n + e) as f32);
    let n0 = (input_midi + 0.5).floor() as i32;
    let base = if interval(n0).is_some() {
        n0
    } else {
        let toward = if input_midi > n0 as f32 { 1 } else { -1 };
        (1..=6)
            .flat_map(|d| [n0 + toward * d, n0 - toward * d])
            .find(|&n| interval(n).is_some())?
    };
    let t_base = target(base)?;

    let fl = input_midi.floor() as i32;
    let left = (0..=12).map(|d| fl - d).find(|&n| interval(n).is_some());
    let right = (1..=12).map(|d| fl + d).find(|&n| interval(n).is_some());
    let free = match (left, right) {
        (Some(l), Some(r)) => {
            let tl = target(l)?;
            let tr = target(r)?;
            tl + (input_midi - l as f32) / (r - l) as f32 * (tr - tl)
        }
        _ => t_base + (input_midi - base as f32),
    };
    Some(t_base + c * (free - t_base) - input_midi)
}

impl HarmonyEngine {
    pub fn reset(&mut self) {
        self.midi.reset();
    }

    /// Intervalo da parte `p` para a nota `note` nos modos relativos (STYLE, MODAL,
    /// FLEXICHORD, SMART).
    fn interval(&self, settings: &HarmonySettings, styles: &StyleBank, p: usize, note: i32) -> Option<i32> {
        match settings.mode {
            HarmonyMode::Style => match styles.lookup_filled(settings.styles[p], p < 2, settings.key, note) {
                StyleLookup::Interval(v) => Some(v),
                StyleLookup::Unassigned => None,
            },
            HarmonyMode::Modal => {
                let root = self.midi.modal_root?;
                let mask = modal_chord_mask(&settings.template, settings.key, root);
                match settings.modal_voicing[p] {
                    ModalVoicing::Un => Some(0),
                    ModalVoicing::OctLow => Some(-12),
                    ModalVoicing::OctHigh => Some(12),
                    ModalVoicing::Low(k) => nth_chord_tone(&mask, note, k, false),
                    ModalVoicing::High(k) => nth_chord_tone(&mask, note, k, true),
                }
            }
            HarmonyMode::FlexiChord => {
                let mask = self.midi.flexi_chord?;
                let (up, k) = match (settings.flexi_voicing, p) {
                    (FlexiVoicing::LllH, 0) => (false, 3),
                    (FlexiVoicing::LllH, 1) => (false, 2),
                    (FlexiVoicing::LllH, 2) => (false, 1),
                    (FlexiVoicing::LllH, _) => (true, 1),
                    (FlexiVoicing::LlHh, 0) => (false, 2),
                    (FlexiVoicing::LlHh, 1) => (false, 1),
                    (FlexiVoicing::LlHh, 2) => (true, 1),
                    (FlexiVoicing::LlHh, _) => (true, 2),
                    (FlexiVoicing::LHhh, 0) => (false, 1),
                    (FlexiVoicing::LHhh, 1) => (true, 1),
                    (FlexiVoicing::LHhh, 2) => (true, 2),
                    (FlexiVoicing::LHhh, _) => (true, 3),
                };
                nth_chord_tone(&mask, note, k, up)
            }
            HarmonyMode::Smart => self.smart_shift(p, settings.key, note),
            _ => None,
        }
    }

    /// Indica se a parte tem harmonia definida (para silencia-la quando nao ha alvo MIDI).
    pub fn part_available(&self, settings: &HarmonySettings, styles: &StyleBank, p: usize) -> bool {
        match settings.mode {
            HarmonyMode::Style => true,
            HarmonyMode::MonoChn | HarmonyMode::WaitAll | HarmonyMode::Priority => self.absolute_note(settings, p).is_some(),
            _ => (0..12).any(|n| self.interval(settings, styles, p, 60 + n).is_some()),
        }
    }

    fn absolute_note(&self, settings: &HarmonySettings, p: usize) -> Option<i32> {
        match settings.mode {
            HarmonyMode::MonoChn => self.midi.part_note[p].map(|n| n as i32),
            HarmonyMode::WaitAll => {
                let active = settings.part_on.iter().filter(|&&on| on).count();
                let (sorted, len) = self.midi.sorted_notes();
                if !settings.part_on[p] || len < active { return None; }
                let k = settings.part_on[..p].iter().filter(|&&on| on).count();
                Some(sorted[k] as i32)
            }
            HarmonyMode::Priority => {
                let rank = settings.priority_rank[p] as usize;
                if rank >= 1 && rank <= self.midi.fifo_len { Some(self.midi.fifo[rank - 1] as i32) } else { None }
            }
            _ => None,
        }
    }

    /// Deslocamento (semitons) de cada parte para o pitch cantado `input_midi`.
    /// `correction` = P.CORRECT normalizado (0 = --, 1 = 9).
    pub fn shifts(
        &mut self,
        settings: &HarmonySettings,
        styles: &StyleBank,
        input_midi: f32,
        correction: [f32; NUM_PARTS],
    ) -> [Option<f32>; NUM_PARTS] {
        let mut out = [None; NUM_PARTS];
        match settings.mode {
            // Modos de nota MIDI: afinacao rigida na nota recebida (o DSP ignora P.CORRECT).
            HarmonyMode::MonoChn | HarmonyMode::WaitAll | HarmonyMode::Priority => {
                for (p, o) in out.iter_mut().enumerate() {
                    *o = self.absolute_note(settings, p).map(|t| fold_into_range(t as f32 - input_midi));
                }
            }
            _ => {
                for (p, o) in out.iter_mut().enumerate() {
                    let c = 1.0 - correction[p].clamp(0.0, 1.0);
                    *o = corrected_shift(|n| self.interval(settings, styles, p, n), input_midi, c);
                }
                if settings.mode == HarmonyMode::Smart {
                    let note = (input_midi + 0.5).floor() as i32;
                    for p in 0..NUM_PARTS {
                        if let Some(s) = self.smart_shift(p, settings.key, note) {
                            self.midi.last_smart_shift[p] = Some(s);
                        }
                    }
                }
            }
        }
        out
    }

    /// Modo SMART: infere o acorde (notas seguradas + KEY), escolhe notas de acorde por
    /// posto (L2/L1 abaixo, H1/H2 acima), mantem notas comuns e evita b9 contra a melodia.
    fn smart_shift(&self, part: usize, key: u8, input_note: i32) -> Option<i32> {
        let m = &self.midi;
        if m.fifo_len == 0 { return None; }
        let len = m.fifo_len;
        if len == 1 { return Some(m.fifo[0] as i32 - input_note); }

        let start = len.saturating_sub(6);
        let mut mask = [false; 12];
        for &n in &m.fifo[start..len] { mask[(n % 12) as usize] = true; }

        let mut unique = mask.iter().filter(|&&b| b).count();
        if unique < 3 {
            let key_pc = (key % 12) as usize;
            for &deg in &MAJOR_SCALE {
                let cand = (key_pc + deg as usize) % 12;
                let clashes = mask[(cand + 1) % 12] || mask[(cand + 11) % 12];
                if !mask[cand] && !clashes {
                    mask[cand] = true;
                    unique += 1;
                    if unique >= 4 { break; }
                }
            }
        }

        let in_pc = input_note.rem_euclid(12);
        let (dir, rank) = match part {
            0 => (-1i32, 2usize),
            1 => (-1, 1),
            2 => (1, 1),
            _ => (1, 2),
        };

        let mut found = 0;
        let mut chosen = None;
        for step in 2..=14 {
            let off = dir * step;
            if mask[(in_pc + off).rem_euclid(12) as usize] {
                found += 1;
                if found == rank {
                    chosen = Some(off);
                    break;
                }
            }
        }

        if let (Some(new_s), Some(prev_s)) = (chosen, m.last_smart_shift[part]) {
            let prev_pc = (in_pc + prev_s).rem_euclid(12) as usize;
            if mask[prev_pc] && prev_s.abs() >= 2 && (new_s - prev_s).abs() <= 2 {
                return Some(prev_s);
            }
        }
        chosen
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    const NO_CORR: [f32; NUM_PARTS] = [0.0; NUM_PARTS];
    const FULL_CORR: [f32; NUM_PARTS] = [1.0; NUM_PARTS];

    fn rounded(v: [Option<f32>; NUM_PARTS]) -> [Option<i32>; NUM_PARTS] {
        v.map(|o| o.map(|x| x.round() as i32))
    }

    #[test]
    fn modal_manual_example_c_root_template_1_3_5() {
        // Manual HV10: KEY C/Am, TEMPLATE 1-3-5, raiz C via MIDI, cantando C:
        // H1..H4 = E, G, C, E e L1..L4 = G, E, C, G.
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        let mut s = HarmonySettings { mode: HarmonyMode::Modal, ..Default::default() };
        eng.midi.note_on(0, 48);
        s.modal_voicing = [ModalVoicing::High(1), ModalVoicing::High(2), ModalVoicing::High(3), ModalVoicing::High(4)];
        assert_eq!(rounded(eng.shifts(&s, &styles, 60.0, FULL_CORR)), [Some(4), Some(7), Some(12), Some(16)]);
        s.modal_voicing = [ModalVoicing::Low(1), ModalVoicing::Low(2), ModalVoicing::Low(3), ModalVoicing::Low(4)];
        assert_eq!(rounded(eng.shifts(&s, &styles, 60.0, FULL_CORR)), [Some(-5), Some(-8), Some(-12), Some(-17)]);
    }

    #[test]
    fn modal_auto_third_is_diatonic_and_m_forces_minor() {
        let mut t = [TemplateDegree::Off; 7];
        t[0] = TemplateDegree::On;
        t[2] = TemplateDegree::On;
        // D em C maior -> terca diatonica F (menor)
        assert!(modal_chord_mask(&t, 0, 62)[5]);
        t[2] = TemplateDegree::Major;
        assert!(modal_chord_mask(&t, 0, 62)[6]);
    }

    #[test]
    fn wait_all_assigns_by_note_number() {
        // Manual: BUDDIES L2, H1, H2 ligados + acorde C5 E5 G5 -> L2=C5, H1=E5, H2=G5.
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        let mut s = HarmonySettings { mode: HarmonyMode::WaitAll, ..Default::default() };
        s.part_on = [true, false, true, true];
        eng.midi.note_on(0, 67);
        eng.midi.note_on(0, 60);
        assert_eq!(eng.shifts(&s, &styles, 60.0, NO_CORR), [None; 4], "Espera todas as notas");
        eng.midi.note_on(0, 64);
        assert_eq!(rounded(eng.shifts(&s, &styles, 60.0, NO_CORR)), [Some(0), None, Some(4), Some(7)]);
    }

    #[test]
    fn priority_follows_rank_permutation() {
        // Manual exemplo 3: VOICING [3-1 -- 2-4], tocando C, E, G, B -> L1 H1 L2 H2.
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        let s = HarmonySettings { mode: HarmonyMode::Priority, priority_rank: [3, 1, 2, 4], ..Default::default() };
        for n in [60, 64, 67, 71] { eng.midi.note_on(0, n); }
        assert_eq!(rounded(eng.shifts(&s, &styles, 60.0, NO_CORR)), [Some(7), Some(0), Some(4), Some(11)]);
    }

    #[test]
    fn midi_notes_are_rigid_and_folded_into_one_octave() {
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        eng.midi.base_channel = 14; // canal 15
        let s = HarmonySettings { mode: HarmonyMode::MonoChn, ..Default::default() };
        eng.midi.note_on(15, 50); // L2 = 16ch
        eng.midi.note_on(0, 76); // L1 = 1ch
        let v = eng.shifts(&s, &styles, 60.3, NO_CORR);
        // Vibrato da entrada e eliminado: saida = nota MIDI exata.
        assert!((v[0].unwrap() - (50.0 - 60.3)).abs() < 1e-4);
        // +16 st dobra para +4 (alcance de +-12 do DSP)
        assert!((v[1].unwrap() - (64.0 - 60.3)).abs() < 1e-4);
    }

    #[test]
    fn flexichord_requires_real_chord_and_latches() {
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        let s = HarmonySettings { mode: HarmonyMode::FlexiChord, flexi_voicing: FlexiVoicing::LlHh, ..Default::default() };
        for n in [60, 64, 66] { eng.midi.note_on(0, n); }
        assert_eq!(eng.shifts(&s, &styles, 62.0, FULL_CORR), [None; 4]);
        for n in [60, 64, 66] { eng.midi.note_off(0, n); }
        for n in [52, 57, 60] { eng.midi.note_on(0, n); }
        for n in [52, 57, 60] { eng.midi.note_off(0, n); }
        // Cantando D (62) sobre Am: abaixo C(-2), A(-5); acima E(+2), A(+7)
        assert_eq!(rounded(eng.shifts(&s, &styles, 62.0, FULL_CORR)), [Some(-5), Some(-2), Some(2), Some(7)]);
    }

    #[test]
    fn p_correct_blends_between_free_inflection_and_rigid_grid() {
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        let s = HarmonySettings::default(); // H1/H2 = 3A, KEY C
        // Cantando C +20 cents. Sem correcao a saida interpola entre os alvos de C (E=64) e do
        // proximo vizinho atribuido D (G=67): 64 + 0.2/2 * 3 = 64.3.
        let free = eng.shifts(&s, &styles, 60.2, NO_CORR)[2].unwrap();
        assert!((free - 4.1).abs() < 1e-4, "{free}");
        // Correcao total: saida trava em E (64) -> deslocamento 64 - 60.2
        let rigid = eng.shifts(&s, &styles, 60.2, FULL_CORR)[2].unwrap();
        assert!((rigid - 3.8).abs() < 1e-4, "{rigid}");
    }

    #[test]
    fn continuous_glide_between_assigned_neighbours_has_no_step() {
        // 3A em C: D -> G (+5), E -> G (+3). Deslizando de D para E sem correcao, a saida deve
        // ser continua (sem degrau na fronteira de semitom).
        let mut eng = HarmonyEngine::default();
        let styles = StyleBank::default();
        let s = HarmonySettings::default();
        let mut prev: Option<f32> = None;
        let mut m = 61.6_f32;
        while m < 64.4 {
            let out = m + eng.shifts(&s, &styles, m, NO_CORR)[2].unwrap();
            if let Some(p) = prev {
                assert!((out - p).abs() < 0.1, "degrau em {m}: {p} -> {out}");
            }
            prev = Some(out);
            m += 0.01;
        }
    }
}
