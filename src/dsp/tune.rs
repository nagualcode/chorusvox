//! Correcao de afinacao da voz principal (a parte "auto-tune" do ChorusVox).
//!
//! O resultado e um DESLOCAMENTO em semitons, nunca uma altura absoluta: o PSOLA multiplica o
//! periodo de cada marca por esse deslocamento. Assim um erro de oitava do detector nao salta a
//! voz uma oitava: o deslocamento so depende da distancia ate a nota permitida mais proxima.
//!
//! A nota alvo tem histerese (o vibrato nao troca de nota) e `retune` e a constante de tempo do
//! deslocamento: em 0 a voz gruda na nota (efeito robotico, sem vibrato); acima de uns 20 ms o
//! vibrato passa intacto, porque e mais rapido que a correcao, e so o desvio lento e corrigido.

/// Vantagem (semitons) que a nota vizinha precisa ter sobre a atual para virar alvo na hora: entre
/// semitons vizinhos a troca acontece a 0,8 da nota atual, alem de um vibrato largo e descentrado.
/// Uma nota nova de verdade percorre o intervalo inteiro e passa disso imediatamente.
const HYSTERESIS: f32 = 0.6;
/// Tempo mais perto da vizinha para trocar sem chegar la (mais que meio ciclo de vibrato).
const DWELL_S: f32 = 0.15;
/// Maior correcao aplicada num quadro (semitons). So limita saltos de melodia em andamento: o
/// maior intervalo entre notas vizinhas das escalas e 3 semitons (metade: 1,5).
const MAX_SHIFT: f32 = 2.0;
/// Sem voz por mais que isso, a proxima nota e um novo ataque.
const PHRASE_GAP_S: f32 = 0.15;

/// Nota permitida mais proxima de `p` (MIDI fracionario). Mascara 0 = cromatica.
pub fn nearest_allowed(p: f32, mask: u16) -> i32 {
    let mask = if mask & 0xFFF == 0 { 0xFFF } else { mask };
    let center = p.round() as i32;
    let mut best = center;
    let mut best_d = f32::MAX;
    for n in center - 6..=center + 6 {
        let d = (n as f32 - p).abs();
        if mask & (1 << n.rem_euclid(12)) != 0 && d < best_d {
            best = n;
            best_d = d;
        }
    }
    best
}

/// Dobra um intervalo para a oitava mais proxima de zero (-6..6).
#[inline]
fn fold_octave(d: f32) -> f32 { d - 12.0 * (d / 12.0).round() }

/// Ajustes da correcao num quadro.
#[derive(Clone, Copy, Debug)]
pub struct TuneInput {
    /// Altura detectada no instante (MIDI fracionario).
    pub pitch: f32,
    /// Altura mediana em torno do instante: escolhe a nota sem seguir um quadro isolado.
    pub center: f32,
    /// Notas permitidas agora.
    pub mask_now: u16,
    /// Notas permitidas para uma nota nova (o proximo acorde, se o cantor antecipou).
    pub mask_next: u16,
    /// Constante de tempo da correcao (s).
    pub retune_s: f32,
    /// Fracao da correcao aplicada, 0..1.
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LeadTuner {
    note: Option<i32>,
    shift: f32,
    silence: f32,
    /// Tempo seguido mais perto de outra nota que da atual.
    away: f32,
}

impl LeadTuner {
    /// Deslocamento atual em semitons.
    pub fn shift(&self) -> f32 { self.shift }

    pub fn reset(&mut self) { *self = Self::default(); }

    /// Quadro sem voz: o deslocamento fica parado (caudas sopradas continuam na mesma correcao).
    pub fn step_unvoiced(&mut self, dt: f32) {
        self.silence += dt;
        if self.silence > PHRASE_GAP_S {
            self.note = None;
        }
    }

    /// Quadro com voz: devolve o deslocamento (semitons) a aplicar.
    pub fn step(&mut self, input: &TuneInput, dt: f32) -> f32 {
        self.silence = 0.0;
        let attack = self.note.is_none();
        let held = self.note;
        let current = held.filter(|&n| self.keep_note(n, input, dt));
        let note = current.unwrap_or_else(|| nearest_allowed(input.center, input.mask_next));
        if self.note != Some(note) {
            self.away = 0.0;
        }
        self.note = Some(note);
        // a nota e o detector podem discordar em oitavas (erro de oitava) ou por um instante
        // (salto de melodia): o deslocamento nunca passa da distancia ate a nota vizinha
        let target = fold_octave(note as f32 - input.pitch).clamp(-MAX_SHIFT, MAX_SHIFT) * input.amount.clamp(0.0, 1.0);
        if attack || input.retune_s <= 0.0 {
            self.shift = target;
        } else {
            self.shift += (target - self.shift) * (1.0 - (-dt / input.retune_s).exp());
        }
        self.shift
    }

    fn keep_note(&mut self, note: i32, input: &TuneInput, dt: f32) -> bool {
        let mask = if input.mask_now & 0xFFF == 0 { 0xFFF } else { input.mask_now };
        if mask & (1 << note.rem_euclid(12)) == 0 {
            return false;
        }
        let other = nearest_allowed(input.center, input.mask_next);
        if other == note {
            self.away = 0.0;
            return true;
        }
        self.away += dt;
        self.away < DWELL_S && (input.center - note as f32).abs() < (input.center - other as f32).abs() + HYSTERESIS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C_MAJOR: u16 = 0b1010_1011_0101;
    const DT: f32 = 0.004;

    fn input(pitch: f32, retune_s: f32) -> TuneInput {
        TuneInput { pitch, center: pitch, mask_now: C_MAJOR, mask_next: C_MAJOR, retune_s, amount: 1.0 }
    }

    #[test]
    fn nearest_note_respects_the_scale() {
        assert_eq!(nearest_allowed(61.2, C_MAJOR), 62);
        assert_eq!(nearest_allowed(60.8, C_MAJOR), 60);
        assert_eq!(nearest_allowed(61.2, 0), 61);
        assert_eq!(nearest_allowed(66.4, 1), 60 + 12 * ((66.4f32 - 60.0) / 12.0).round() as i32);
    }

    #[test]
    fn instant_retune_lands_on_the_note() {
        let mut t = LeadTuner::default();
        let s = t.step(&input(60.3, 0.0), DT);
        assert!((60.3 + s - 60.0).abs() < 1e-4);
    }

    #[test]
    fn vibrato_does_not_flip_the_note() {
        let mut t = LeadTuner::default();
        t.step(&input(60.0, 0.03), DT);
        for k in 0..2000 {
            let p = 60.0 + 0.6 * (k as f32 * DT * std::f32::consts::TAU * 5.5).sin();
            t.step(&TuneInput { mask_now: 0, mask_next: 0, ..input(p, 0.03) }, DT);
            assert_eq!(t.note, Some(60));
        }
    }

    #[test]
    fn slow_retune_removes_offset_and_keeps_vibrato() {
        let mut t = LeadTuner::default();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for k in 0..2000 {
            let p = 60.25 + 0.4 * (k as f32 * DT * std::f32::consts::TAU * 5.5).sin();
            let out = p + t.step(&input(p, 0.05), DT);
            if k > 1000 {
                lo = lo.min(out);
                hi = hi.max(out);
            }
        }
        assert!(((lo + hi) / 2.0 - 60.0).abs() < 0.05, "centro {}", (lo + hi) / 2.0);
        assert!(hi - lo > 0.5, "vibrato restante {}", hi - lo);
    }

    #[test]
    fn octave_error_keeps_the_correction() {
        let mut t = LeadTuner::default();
        let a = t.step(&input(60.3, 0.0), DT);
        // detector erra a oitava num quadro: a mediana fica, o deslocamento nao salta
        let b = t.step(&TuneInput { center: 60.3, ..input(48.3, 0.0) }, DT);
        assert!((a - b).abs() < 1e-3, "{a} {b}");
    }

    #[test]
    fn note_left_out_of_the_new_chord_moves() {
        let mut t = LeadTuner::default();
        t.step(&input(61.0, 0.0), DT);
        assert_eq!(t.note, Some(60));
        let only_d = 1 << 2;
        t.step(&TuneInput { mask_now: only_d, mask_next: only_d, ..input(61.0, 0.0) }, DT);
        assert_eq!(t.note, Some(62));
    }

    #[test]
    fn off_center_wide_vibrato_keeps_the_note() {
        let mut t = LeadTuner::default();
        t.step(&input(60.0, 0.0), DT);
        for k in 0..2000 {
            let p = 60.3 + 0.45 * (k as f32 * DT * std::f32::consts::TAU * 5.5).sin();
            t.step(&TuneInput { mask_now: 0, mask_next: 0, ..input(p, 0.0) }, DT);
            assert_eq!(t.note, Some(60));
        }
    }

    #[test]
    fn real_note_change_is_immediate_and_held_drift_moves_later() {
        let mut t = LeadTuner::default();
        t.step(&input(60.0, 0.0), DT);
        t.step(&input(62.0, 0.0), DT);
        assert_eq!(t.note, Some(62));
        // parado em 62,6 (cromatico): mais perto de 63, mas dentro da histerese
        let hold = TuneInput { mask_now: 0, mask_next: 0, ..input(62.6, 0.0) };
        t.step(&hold, DT);
        assert_eq!(t.note, Some(62));
        for _ in 0..60 {
            t.step(&hold, DT);
        }
        assert_eq!(t.note, Some(63));
    }

    #[test]
    fn amount_scales_the_correction() {
        let mut t = LeadTuner::default();
        let s = t.step(&TuneInput { amount: 0.5, ..input(60.4, 0.0) }, DT);
        assert!((s + 0.2).abs() < 1e-4);
    }

    #[test]
    fn unvoiced_gap_holds_shift_then_forgets_the_note() {
        let mut t = LeadTuner::default();
        let s = t.step(&input(60.3, 0.0), DT);
        t.step_unvoiced(0.1);
        assert_eq!(t.shift(), s);
        assert!(t.note.is_some());
        t.step_unvoiced(0.1);
        assert!(t.note.is_none());
    }
}
