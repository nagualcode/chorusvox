//! Tabelas de STYLE do Akai HV10 (UN, 8v, 3A–3H, 5A–5H, U1–U9).
//!
//! As 16 tabelas de harmonia vem das strings de STYLE EDIT do plugin Akai DecaBuddy v1.02
//! (`reference/DecaBuddy.dll`, .data1 0x10026200..0x10026a80) e conferem com os exemplos do
//! manual do HV10: 3A em C gera C->E, D->G, E->G, F->A, G->C, A->C, B->D e 3E gera D->F, G->B.
//! O DLL rotula as 16 tabelas como "3rd Modal 1-5 / Har Min / Mel Min", "5th Modal 1-5 / Har Min /
//! Mel Min" e "7th Major / Minor"; o HV10 as chama de 3A–3H e 5A–5H. A correspondencia 3A–3G e
//! 5A–5G e direta pela ordem; 3H = 7th Major e 5H = 7th Minor e uma inferencia (nao documentada).

pub type ScaleTable = [Option<i8>; 12];

const N: Option<i8> = None;
const fn s(v: i8) -> Option<i8> { Some(v) }

/// Tabelas inferiores (partes L2/L1). Indice de entrada = classe de altura relativa a raiz da
/// tabela (C para as tabelas modais, A para as menores). Valor = intervalo em semitons.
const LOWER_TABLES: [ScaleTable; 16] = [
    // 3rd Modal 1..5 (3A..3E)
    [s(-8), N, s(-7), N, s(-9), s(-8), N, s(-7), N, s(-9), N, s(-9)],
    [s(-7), N, s(-7), N, s(-9), s(-8), N, s(-7), N, s(-9), N, s(-9)],
    [s(-8), N, s(-7), N, s(-9), s(-8), N, s(-8), N, s(-9), N, s(-9)],
    [s(-8), N, s(-8), N, s(-9), s(-8), N, s(-8), N, s(-9), N, s(-9)],
    [s(-8), N, s(-9), N, s(-9), s(-8), N, s(-8), N, s(-9), N, s(-9)],
    // 3rd Harmonic Minor, Melodic Minor (3F, 3G) - raiz A
    [s(-9), N, s(-9), s(-8), N, s(-9), N, s(-8), s(-8), N, N, s(-9)],
    [s(-9), N, s(-9), s(-8), N, s(-7), N, s(-8), s(-8), N, s(-8), s(-9)],
    // 5th Modal 1..5 (5A..5E)
    [s(-5), N, s(-3), N, s(-4), s(-5), N, s(-3), N, s(-4), N, s(-4)],
    [s(-5), N, s(-3), N, s(-4), s(-3), N, s(-3), N, s(-4), N, s(-4)],
    [s(-5), N, s(-5), N, s(-4), s(-5), N, s(-5), N, s(-5), N, s(-4)],
    [s(-3), N, s(-3), N, s(-4), s(-5), N, s(-3), N, s(-4), N, s(-4)],
    [s(-5), N, s(-5), N, s(-5), s(-5), N, s(-5), N, s(-5), N, s(-5)],
    // 5th Harmonic Minor, Melodic Minor (5F, 5G) - raiz A
    [s(-5), N, s(-3), s(-3), N, s(-3), N, s(-5), s(-3), N, N, s(-4)],
    [s(-5), N, s(-4), s(-3), N, s(-3), N, s(-5), s(-5), N, s(-5), s(-4)],
    // 7th Major (3H) - raiz C, 7th Minor (5H) - raiz A
    [s(-1), N, s(-2), N, s(-2), s(-1), N, s(-2), N, s(-2), N, s(-2)],
    [s(-2), N, s(-2), s(-1), N, s(-2), N, s(-2), s(-1), N, s(-2), N],
];

/// Raiz (classe de altura relativa a KEY) de cada tabela: 0 = C (maior), 9 = A (relativa menor).
const TABLE_ROOT: [u8; 16] = [0, 0, 0, 0, 0, 9, 9, 0, 0, 0, 0, 0, 9, 9, 0, 9];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StyleId {
    Un = 0, Oct,
    S3A, S3B, S3C, S3D, S3E, S3F, S3G, S3H,
    S5A, S5B, S5C, S5D, S5E, S5F, S5G, S5H,
    U1, U2, U3, U4, U5, U6, U7, U8, U9,
}

pub const NUM_USER_STYLES: usize = 9;

impl StyleId {
    pub const ALL: [StyleId; 27] = [
        StyleId::Un, StyleId::Oct,
        StyleId::S3A, StyleId::S3B, StyleId::S3C, StyleId::S3D,
        StyleId::S3E, StyleId::S3F, StyleId::S3G, StyleId::S3H,
        StyleId::S5A, StyleId::S5B, StyleId::S5C, StyleId::S5D,
        StyleId::S5E, StyleId::S5F, StyleId::S5G, StyleId::S5H,
        StyleId::U1, StyleId::U2, StyleId::U3, StyleId::U4, StyleId::U5,
        StyleId::U6, StyleId::U7, StyleId::U8, StyleId::U9,
    ];

    pub fn from_index(idx: usize) -> Self {
        Self::ALL.get(idx).copied().unwrap_or(StyleId::Un)
    }

    /// Indice nas tabelas de preset (0..16) para 3A..5H.
    fn table_index(self) -> Option<usize> {
        let i = self as usize;
        match i {
            2..=8 => Some(i - 2),       // 3A..3G -> 3rd Modal 1..5, Har Min, Mel Min
            9 => Some(14),              // 3H -> 7th Major
            10..=16 => Some(i - 10 + 7), // 5A..5G -> 5th Modal 1..5, Har Min, Mel Min
            17 => Some(15),             // 5H -> 7th Minor
            _ => None,
        }
    }

    pub fn user_index(self) -> Option<usize> {
        let i = self as usize;
        if i >= StyleId::U1 as usize { Some(i - StyleId::U1 as usize) } else { None }
    }
}

/// Resultado da consulta de STYLE para uma nota de entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleLookup {
    /// Intervalo definido (semitons).
    Interval(i32),
    /// Nota nao atribuida na tabela ("-" no HV10, "*" no DLL).
    Unassigned,
}

/// Banco de User Styles (U1..U9), comum a todos os programas como no HV10.
/// Cada passo guarda o intervalo em semitons a partir da nota de entrada (C..B relativo a KEY),
/// ou `None` para nota nao atribuida.
#[derive(Clone, Debug)]
pub struct StyleBank {
    pub user: [ScaleTable; NUM_USER_STYLES],
}

impl Default for StyleBank {
    /// Valores iniciais das tabelas User do DSP: -5, +4, +7, +12 (U1..U4); U5..U9 em uníssono.
    fn default() -> Self {
        let mut user = [[s(0); 12]; NUM_USER_STYLES];
        user[0] = [s(-5); 12];
        user[1] = [s(4); 12];
        user[2] = [s(7); 12];
        user[3] = [s(12); 12];
        Self { user }
    }
}

impl StyleBank {
    /// Consulta o intervalo para `input_note` (MIDI) no `key` (0 = C/Am).
    /// `lower` = parte L2/L1 (harmonia abaixo da voz original), senao H1/H2 (acima).
    pub fn lookup(&self, style: StyleId, lower: bool, key: u8, input_note: i32) -> StyleLookup {
        let key = (key % 12) as i32;
        match style {
            StyleId::Un => StyleLookup::Interval(0),
            StyleId::Oct => StyleLookup::Interval(if lower { -12 } else { 12 }),
            _ => {
                if let Some(t) = style.table_index() {
                    let root = TABLE_ROOT[t] as i32;
                    let pc = (input_note - key - root).rem_euclid(12) as usize;
                    match LOWER_TABLES[t][pc] {
                        Some(v) => StyleLookup::Interval(v as i32 + if lower { 0 } else { 12 }),
                        None => StyleLookup::Unassigned,
                    }
                } else if let Some(u) = style.user_index() {
                    let pc = (input_note - key).rem_euclid(12) as usize;
                    match self.user[u][pc] {
                        Some(v) => StyleLookup::Interval(v as i32),
                        None => StyleLookup::Unassigned,
                    }
                } else {
                    StyleLookup::Unassigned
                }
            }
        }
    }

    /// Como `lookup`, mas preenche lacunas de 2 ou mais notas sem atribuicao como o DSP faz ao
    /// carregar a tabela (0x1000c670): metade com o intervalo do vizinho esquerdo, metade com o
    /// do direito. Lacunas isoladas continuam `Unassigned`.
    pub fn lookup_filled(&self, style: StyleId, lower: bool, key: u8, input_note: i32) -> StyleLookup {
        let direct = self.lookup(style, lower, key, input_note);
        if direct != StyleLookup::Unassigned {
            return direct;
        }
        let at = |d: i32| self.lookup(style, lower, key, input_note + d);
        let mut left = None;
        for d in 1..12 {
            if let StyleLookup::Interval(v) = at(-d) { left = Some((d, v)); break; }
        }
        let mut right = None;
        for d in 1..12 {
            if let StyleLookup::Interval(v) = at(d) { right = Some((d, v)); break; }
        }
        match (left, right) {
            (Some((dl, vl)), Some((dr, vr))) => {
                let gap = dl + dr - 1;
                if gap < 2 {
                    StyleLookup::Unassigned
                } else if dl <= (gap + 1) / 2 {
                    StyleLookup::Interval(vl)
                } else {
                    StyleLookup::Interval(vr)
                }
            }
            _ => StyleLookup::Unassigned,
        }
    }

    pub fn set_user_step(&mut self, user_idx: usize, step: usize, value: Option<i8>) {
        if user_idx < NUM_USER_STYLES && step < 12 {
            self.user[user_idx][step] = value.map(|v| v.clamp(-24, 24));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_examples_3a_and_3e_in_c() {
        let bank = StyleBank::default();
        // 3A em C (manual HV10, Sub Style): C->E, D->G, E->G, F->A, G->C, A->C, B->D
        let expect_3a = [(60, 64), (62, 67), (64, 67), (65, 69), (67, 72), (69, 72), (71, 74)];
        for (inp, out) in expect_3a {
            assert_eq!(bank.lookup(StyleId::S3A, false, 0, inp), StyleLookup::Interval(out - inp));
        }
        // 3E em C: C->E, D->F, E->G, F->A, G->B, A->C, B->D
        let expect_3e = [(60, 64), (62, 65), (64, 67), (65, 69), (67, 71), (69, 72), (71, 74)];
        for (inp, out) in expect_3e {
            assert_eq!(bank.lookup(StyleId::S3E, false, 0, inp), StyleLookup::Interval(out - inp));
        }
    }

    #[test]
    fn lower_parts_are_always_below_and_upper_above() {
        let bank = StyleBank::default();
        for st in StyleId::ALL.iter().copied().filter(|s| s.table_index().is_some()) {
            for key in 0..12u8 {
                for note in 48..72 {
                    if let StyleLookup::Interval(v) = bank.lookup(st, true, key, note) {
                        assert!((-12..0).contains(&v), "{st:?} lower {v}");
                    }
                    if let StyleLookup::Interval(v) = bank.lookup(st, false, key, note) {
                        assert!((1..=12).contains(&v), "{st:?} upper {v}");
                    }
                }
            }
        }
    }

    #[test]
    fn long_gaps_are_filled_half_left_half_right() {
        let mut bank = StyleBank::default();
        for step in 0..12 { bank.set_user_step(0, step, None); }
        // U1: C=+4, G=+3, resto sem atribuicao -> lacuna C#..F# (6 notas)
        bank.set_user_step(0, 0, Some(4));
        bank.set_user_step(0, 7, Some(3));
        assert_eq!(bank.lookup_filled(StyleId::U1, false, 0, 61), StyleLookup::Interval(4));
        assert_eq!(bank.lookup_filled(StyleId::U1, false, 0, 63), StyleLookup::Interval(4));
        assert_eq!(bank.lookup_filled(StyleId::U1, false, 0, 64), StyleLookup::Interval(3));
        assert_eq!(bank.lookup_filled(StyleId::U1, false, 0, 66), StyleLookup::Interval(3));
        // Lacuna isolada continua sem atribuicao
        assert_eq!(bank.lookup_filled(StyleId::S3A, false, 0, 61), StyleLookup::Unassigned);
    }

    #[test]
    fn chromatic_notes_are_unassigned_in_c_major_3a() {
        let bank = StyleBank::default();
        assert_eq!(bank.lookup(StyleId::S3A, false, 0, 61), StyleLookup::Unassigned);
    }
}
