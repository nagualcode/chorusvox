//! Teoria musical do harmonizador: acordes, escalas e a escolha das notas das vozes.
//!
//! A escolha e uma busca exaustiva com funcao de custo (poucos milhares de combinacoes, so quando
//! a nota do lead ou o acorde mudam). O custo segue regras de arranjo vocal:
//!  - notas do acorde; tensoes da escala so quando o lead canta nota de passagem;
//!  - nada de segunda menor / setima maior contra o lead, e evita trítono e segundas coladas;
//!  - conducao de vozes suave (movimento pequeno, nota comum mantida, sem quintas/oitavas paralelas);
//!  - acorde completo (a terca nao pode faltar), sem dobrar o lead;
//!  - deslocamentos de altura moderados (soam mais naturais) e forma de voicing do estilo escolhido.

pub const MAX_VOICES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Chord,
    Scale,
    MidiChord,
    MidiNotes,
}

impl Mode {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Mode::Scale,
            2 => Mode::MidiChord,
            3 => Mode::MidiNotes,
            _ => Mode::Chord,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voicing {
    Above,
    Below,
    Around,
    Wide,
}

impl Voicing {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Voicing::Below,
            2 => Voicing::Around,
            3 => Voicing::Wide,
            _ => Voicing::Above,
        }
    }
}

pub const CHORD_NAMES: [&str; 16] =
    ["Maj", "Min", "7", "Maj7", "m7", "6", "m6", "sus2", "sus4", "add9", "9", "m9", "dim", "dim7", "aug", "m7b5"];

/// Intervalos de cada qualidade de acorde (semitons a partir da fundamental).
const CHORDS: [&[u8]; 16] = [
    &[0, 4, 7],
    &[0, 3, 7],
    &[0, 4, 7, 10],
    &[0, 4, 7, 11],
    &[0, 3, 7, 10],
    &[0, 4, 7, 9],
    &[0, 3, 7, 9],
    &[0, 2, 7],
    &[0, 5, 7],
    &[0, 2, 4, 7],
    &[0, 2, 4, 7, 10],
    &[0, 2, 3, 7, 10],
    &[0, 3, 6],
    &[0, 3, 6, 9],
    &[0, 4, 8],
    &[0, 3, 6, 10],
];

/// Escala implicita de cada acorde (para notas de passagem).
const CHORD_SCALES: [&[u8]; 16] = [
    &[0, 2, 4, 5, 7, 9, 11],    // Maj: jonio
    &[0, 2, 3, 5, 7, 8, 10],    // Min: eolio
    &[0, 2, 4, 5, 7, 9, 10],    // 7: mixolidio
    &[0, 2, 4, 5, 7, 9, 11],    // Maj7
    &[0, 2, 3, 5, 7, 9, 10],    // m7: dorico
    &[0, 2, 4, 5, 7, 9, 11],    // 6
    &[0, 2, 3, 5, 7, 9, 10],    // m6: dorico
    &[0, 2, 4, 5, 7, 9, 11],    // sus2
    &[0, 2, 4, 5, 7, 9, 10],    // sus4: mixolidio
    &[0, 2, 4, 5, 7, 9, 11],    // add9
    &[0, 2, 4, 5, 7, 9, 10],    // 9
    &[0, 2, 3, 5, 7, 9, 10],    // m9
    &[0, 2, 3, 5, 6, 8, 9, 11], // dim: diminuta
    &[0, 2, 3, 5, 6, 8, 9, 11], // dim7
    &[0, 2, 4, 6, 8, 10],       // aug: tons inteiros
    &[0, 1, 3, 5, 6, 8, 10],    // m7b5: locrio
];

pub const SCALE_NAMES: [&str; 5] = ["Major", "Minor", "Dorian", "Mixolydian", "Harm. Minor"];
const SCALES: [&[u8]; 5] = [
    &[0, 2, 4, 5, 7, 9, 11],
    &[0, 2, 3, 5, 7, 8, 10],
    &[0, 2, 3, 5, 7, 9, 10],
    &[0, 2, 4, 5, 7, 9, 10],
    &[0, 2, 3, 5, 7, 8, 11],
];

pub const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Desvio da afinacao justa em relacao ao temperamento igual, por classe de intervalo a partir
/// da fundamental (cents): 16/15, 9/8, 6/5, 5/4, 4/3, 45/32, 3/2, 8/5, 5/3, 16/9, 15/8.
const JUST_CENTS: [f32; 12] = [0.0, 11.7, 3.9, 15.6, -13.7, -2.0, -9.8, 2.0, 13.7, -15.6, -3.9, -11.7];

#[inline]
pub fn pc(n: i32) -> u8 { n.rem_euclid(12) as u8 }

#[inline]
fn mask_of(root: u8, iv: &[u8]) -> u16 { iv.iter().fold(0u16, |m, &i| m | 1 << ((root + i) % 12)) }

#[inline]
fn has(mask: u16, n: i32) -> bool { mask & (1 << pc(n)) != 0 }

/// Contexto harmonico num instante: tudo o que define as notas das vozes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HarmonyCtx {
    pub mode: Mode,
    pub root: u8,
    pub quality: u8,
    pub scale: u8,
    pub voicing: Voicing,
    pub voices: u8,
    /// Notas MIDI seguradas (ordenadas), para os modos MIDI.
    pub midi: [u8; 16],
    pub midi_n: u8,
}

impl Default for HarmonyCtx {
    fn default() -> Self {
        Self { mode: Mode::Chord, root: 0, quality: 0, scale: 0, voicing: Voicing::Above, voices: 2, midi: [0; 16], midi_n: 0 }
    }
}

impl HarmonyCtx {
    pub fn midi_notes(&self) -> &[u8] { &self.midi[..self.midi_n as usize] }

    /// Mascara do acorde, mascara de escala (tensoes permitidas) e fundamental, para a nota `lead`.
    pub fn chord_for(&self, lead: i32) -> (u16, u16, u8) {
        match self.mode {
            Mode::Chord => {
                let q = (self.quality as usize).min(15);
                (mask_of(self.root, CHORDS[q]), mask_of(self.root, CHORD_SCALES[q]), self.root)
            }
            Mode::Scale => {
                let sc = SCALES[(self.scale as usize).min(SCALES.len() - 1)];
                let smask = mask_of(self.root, sc);
                // grau do lead na escala (ou o mais proximo abaixo, se cromatico)
                let rel = (lead - self.root as i32).rem_euclid(12) as u8;
                let deg = sc.iter().rposition(|&d| d <= rel).unwrap_or(0) as i32;
                let n = sc.len() as i32;
                let tone = |k: i32| (self.root + sc[k.rem_euclid(n) as usize]) % 12;
                // triade diatonica que contem o lead, na posicao que o estilo pede
                let (lo, root_deg) = match self.voicing {
                    Voicing::Above | Voicing::Wide => (deg, deg),
                    Voicing::Below => (deg - 4, deg - 4),
                    Voicing::Around => (deg - 2, deg - 2),
                };
                let mut cm = 0u16;
                for k in 0..3 {
                    cm |= 1 << tone(lo + 2 * k);
                }
                cm |= 1 << pc(lead);
                (cm, smask, tone(root_deg))
            }
            Mode::MidiChord | Mode::MidiNotes => {
                let notes = self.midi_notes();
                let cm = notes.iter().fold(0u16, |m, &n| m | 1 << (n % 12));
                let root = notes.first().map_or(self.root, |&n| n % 12);
                (cm, cm, root)
            }
        }
    }
}

/// Afinacao justa: desvio (semitons) da voz `h` em relacao ao lead `lead`, para que o intervalo
/// entre eles seja puro em relacao a fundamental do acorde.
pub fn just_offset(h: i32, lead: i32, root: u8) -> f32 {
    let jh = JUST_CENTS[(h - root as i32).rem_euclid(12) as usize];
    let jl = JUST_CENTS[(lead - root as i32).rem_euclid(12) as usize];
    (jh - jl) / 100.0
}

/// Notas das vozes (da mais aguda para a mais grave). `None` = voz calada.
pub type VoiceNotes = [Option<i32>; MAX_VOICES];

/// Escolhe as notas das `k` vozes para o lead `lead`.
/// `prev` = (notas anteriores, lead anterior) para conducao de vozes.
pub fn choose_voicing(ctx: &HarmonyCtx, lead: i32, prev: Option<(&VoiceNotes, i32)>) -> VoiceNotes {
    let mut out: VoiceNotes = [None; MAX_VOICES];
    if ctx.mode == Mode::MidiNotes {
        // As vozes cantam as notas seguradas (as 4 mais proximas do lead), da mais aguda a mais grave.
        let mut notes: Vec<i32> = Vec::new();
        notes.extend(ctx.midi_notes().iter().map(|&n| n as i32));
        notes.sort_by_key(|&n| (n - lead).abs());
        notes.truncate(MAX_VOICES);
        notes.sort_by(|a, b| b.cmp(a));
        for (o, n) in out.iter_mut().zip(notes) {
            *o = Some(n);
        }
        return out;
    }
    let k = (ctx.voices as usize).clamp(1, MAX_VOICES);
    let (chord, scale, root) = ctx.chord_for(lead);
    if chord == 0 {
        return out;
    }
    let lead_in_chord = has(chord, lead);
    let (lo, hi) = match ctx.voicing {
        Voicing::Above => (lead + 1, lead + 17),
        Voicing::Below => (lead - 17, lead - 1),
        Voicing::Around => (lead - 13, lead + 13),
        Voicing::Wide => (lead - 20, lead + 20),
    };
    let mut cands: [i32; 48] = [0; 48];
    let mut nc = 0;
    for n in lo.max(28)..=hi.min(91) {
        if n == lead {
            continue;
        }
        let ok = has(chord, n) || (!lead_in_chord && has(scale, n));
        if ok && nc < cands.len() {
            cands[nc] = n;
            nc += 1;
        }
    }
    // Se faltarem candidatos (acorde de duas notas, por exemplo), aceita dobrar o lead em oitava.
    if nc < k {
        for n in lo.max(28)..=hi.min(91) {
            if n != lead && pc(n) == pc(lead) && nc < cands.len() {
                cands[nc] = n;
                nc += 1;
            }
        }
    }
    if nc < k {
        return out;
    }
    // cands em ordem crescente; vamos montar combinacoes decrescentes
    let cands = &cands[..nc];
    let mut best_cost = f32::MAX;
    let mut best = [0i32; MAX_VOICES];
    let mut idx = [0usize; MAX_VOICES];
    // enumeracao de combinacoes de k indices crescentes
    for (i, v) in idx.iter_mut().enumerate().take(k) {
        *v = i;
    }
    loop {
        let mut set = [0i32; MAX_VOICES];
        for i in 0..k {
            set[i] = cands[idx[k - 1 - i]]; // decrescente
        }
        let c = cost(ctx, lead, &set[..k], chord, root, lead_in_chord, prev);
        if c < best_cost {
            best_cost = c;
            best = set;
        }
        // proxima combinacao
        let mut i = k;
        loop {
            if i == 0 {
                for (o, &n) in out.iter_mut().zip(best.iter()).take(k) {
                    *o = Some(n);
                }
                return out;
            }
            i -= 1;
            if idx[i] < nc - k + i {
                idx[i] += 1;
                for j in i + 1..k {
                    idx[j] = idx[j - 1] + 1;
                }
                break;
            }
        }
    }
}

fn cost(
    ctx: &HarmonyCtx,
    lead: i32,
    set: &[i32],
    chord: u16,
    root: u8,
    lead_in_chord: bool,
    prev: Option<(&VoiceNotes, i32)>,
) -> f32 {
    let mut c = 0.0f32;
    let k = set.len();
    let mut above = 0i32;
    let mut below = 0i32;
    for &h in set {
        // notas fora do acorde
        if !has(chord, h) {
            c += if lead_in_chord { 6.0 } else { 0.7 };
        }
        let d = h - lead;
        let a = d.abs();
        if d > 0 { above += 1 } else { below += 1 }
        match a % 12 {
            1 | 11 => c += if a == 1 { 16.0 } else { 6.0 },
            2 | 10 => c += if a == 2 { 2.2 } else if a < 12 { 1.0 } else { 0.5 },
            6 => c += 1.6,
            0 => c += 3.0,
            _ => {}
        }
        // deslocamentos grandes soam menos naturais
        if a > 9 {
            c += 0.12 * (a - 9) as f32;
        }
        if a > 12 {
            c += 0.6 * (a - 12) as f32;
        }
        // registro absoluto razoavel
        if h > 84 {
            c += 0.5 * (h - 84) as f32;
        }
        if h < 38 {
            c += 0.5 * (38 - h) as f32;
        }
    }
    // intervalos entre as vozes
    for i in 0..k {
        for j in i + 1..k {
            let d = (set[i] - set[j]).abs();
            match d {
                1 => c += 6.0,
                2 => c += 1.8,
                _ => {}
            }
            if d % 12 == 0 {
                c += 2.5;
            }
        }
    }
    // forma do voicing
    let top = set.iter().copied().max().unwrap_or(lead).max(lead);
    let bot = set.iter().copied().min().unwrap_or(lead).min(lead);
    let span = top - bot;
    match ctx.voicing {
        Voicing::Above | Voicing::Below | Voicing::Around => {
            if span > 12 {
                c += 0.35 * (span - 12) as f32;
            }
            if ctx.voicing == Voicing::Around && k >= 2 && (above == 0 || below == 0) {
                c += 3.0;
            }
            if ctx.voicing == Voicing::Around && k >= 2 && above.abs_diff(below) > 1 {
                c += 1.0;
            }
        }
        Voicing::Wide => {
            let mut all: [i32; MAX_VOICES + 1] = [0; MAX_VOICES + 1];
            all[..k].copy_from_slice(set);
            all[k] = lead;
            let all = &mut all[..=k];
            all.sort_unstable();
            for w in all.windows(2) {
                let g = w[1] - w[0];
                if g < 5 {
                    c += 0.6 * (5 - g) as f32;
                }
            }
            let want = 7 * k as i32;
            if span < want {
                c += 0.2 * (want - span) as f32;
            }
            if ctx.voices >= 2 && (above == 0 || below == 0) {
                c += 2.0;
            }
        }
    }
    // acorde completo
    if lead_in_chord {
        let mut covered = 1u16 << pc(lead);
        for &h in set {
            covered |= 1 << pc(h);
        }
        let missing = (chord & !covered).count_ones() as f32;
        c += 0.45 * missing;
        // a terca (3 ou 4 semitons acima da fundamental) define o acorde
        let third_m = 1u16 << ((root + 3) % 12);
        let third_maj = 1u16 << ((root + 4) % 12);
        let thirds = chord & (third_m | third_maj);
        if thirds != 0 && covered & thirds == 0 {
            c += 1.4;
        }
    }
    // conducao de vozes
    if let Some((pn, pl)) = prev {
        let prevs: Vec<i32> = pn.iter().flatten().copied().collect();
        if prevs.len() == k {
            let lead_moved = lead != pl;
            // num salto grande do lead o voicing se reorganiza: movimento pesa menos
            let leap = (lead - pl).abs() > 7;
            let w_mv = if leap { 0.1 } else { 0.3 };
            for i in 0..k {
                let mv = (set[i] - prevs[i]).abs();
                c += w_mv * mv.min(7) as f32;
                if mv > 4 && !leap {
                    c += 0.6;
                }
                // quintas e oitavas paralelas com o lead
                let iv_prev = (prevs[i] - pl).rem_euclid(12);
                let iv_now = (set[i] - lead).rem_euclid(12);
                if lead_moved && mv > 0 && iv_prev == iv_now && (iv_now == 7 || iv_now == 0) {
                    c += 1.2;
                }
                // cruzar o lead
                if (prevs[i] > pl) != (set[i] > lead) {
                    c += 0.8;
                }
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(root: u8, q: u8, v: Voicing, k: u8) -> HarmonyCtx {
        HarmonyCtx { root, quality: q, voicing: v, voices: k, ..Default::default() }
    }

    fn notes(v: VoiceNotes) -> Vec<i32> { v.iter().flatten().copied().collect() }

    #[test]
    fn c_major_above_from_root() {
        let c = ctx(0, 0, Voicing::Above, 2);
        assert_eq!(notes(choose_voicing(&c, 60, None)), vec![67, 64]);
    }

    #[test]
    fn c_major_above_from_third_and_fifth() {
        let c = ctx(0, 0, Voicing::Above, 2);
        assert_eq!(notes(choose_voicing(&c, 64, None)), vec![72, 67]);
        assert_eq!(notes(choose_voicing(&c, 67, None)), vec![76, 72]);
    }

    #[test]
    fn below_voicing_stays_below() {
        let c = ctx(9, 1, Voicing::Below, 2); // A minor
        let v = notes(choose_voicing(&c, 69, None));
        assert!(v.iter().all(|&n| n < 69), "{v:?}");
        assert!(v.iter().all(|&n| has(mask_of(9, CHORDS[1]), n)));
    }

    #[test]
    fn around_has_both_sides() {
        let c = ctx(0, 0, Voicing::Around, 2);
        let v = notes(choose_voicing(&c, 64, None));
        assert!(v[0] > 64 && v[1] < 64, "{v:?}");
    }

    #[test]
    fn passing_tone_moves_in_parallel() {
        // C maior, lead C -> D (passagem): a voz de cima deveria ir de E para F (terca diatonica)
        let c = ctx(0, 0, Voicing::Above, 1);
        let first = choose_voicing(&c, 60, None);
        assert_eq!(notes(first), vec![64]);
        let next = notes(choose_voicing(&c, 62, Some((&first, 60))));
        assert!(next == vec![65] || next == vec![64] || next == vec![67], "{next:?}");
        assert_ne!(next, vec![63]);
    }

    #[test]
    fn scale_mode_diatonic_thirds() {
        let c = HarmonyCtx { mode: Mode::Scale, root: 0, scale: 0, voicing: Voicing::Above, voices: 1, ..Default::default() };
        // D em do maior -> F (terca menor diatonica)
        assert_eq!(notes(choose_voicing(&c, 62, None)), vec![65]);
        // E -> G
        assert_eq!(notes(choose_voicing(&c, 64, None)), vec![67]);
        // C -> E
        assert_eq!(notes(choose_voicing(&c, 60, None)), vec![64]);
    }

    #[test]
    fn midi_notes_mode_uses_held_notes() {
        let mut c = HarmonyCtx { mode: Mode::MidiNotes, ..Default::default() };
        c.midi[..3].copy_from_slice(&[55, 64, 67]);
        c.midi_n = 3;
        assert_eq!(notes(choose_voicing(&c, 60, None)), vec![67, 64, 55]);
    }

    #[test]
    fn never_minor_second_against_lead() {
        for q in 0..16u8 {
            for v in [Voicing::Above, Voicing::Below, Voicing::Around, Voicing::Wide] {
                for k in 1..=4u8 {
                    let c = ctx(2, q, v, k);
                    let (cm, _, _) = c.chord_for(62);
                    for lead in 50..76 {
                        if !has(cm, lead) {
                            continue;
                        }
                        let ns = notes(choose_voicing(&c, lead, None));
                        for n in ns {
                            assert_ne!((n - lead).abs(), 1, "q{q} {v:?} k{k} lead {lead}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn just_offsets_make_pure_third() {
        // lead C (fundamental) e voz E: terca maior pura = -13.7 cents
        assert!((just_offset(64, 60, 0) + 0.137).abs() < 1e-3);
        // lead E, voz G: terca menor pura (G e +2, E e -13.7) => +15.7 cents
        assert!((just_offset(67, 64, 0) - 0.157).abs() < 1e-3);
    }
}

