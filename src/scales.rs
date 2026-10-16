//! Tabelas de STYLE do Akai HV10 (UN, 8v, 3A–3H, 5A–5H, U1–U9).
//!
//! Fonte: pagina "Preset styles" do manual de operacao do HV10 (apendice, pag. 31), transcrita da
//! partitura nota a nota. 13 das 16 tabelas coincidem com as do plugin Akai DecaBuddy v1.02
//! (3A–3E = "Modal 1-5", 3F/3G = menor harmonica/melodica, 5A–5G); o DLL diverge em 3H e 5H
//! (la sao "7th Major/Minor") e na nota B do 5F (B->G# no DLL, B->F# no aparelho).
//! Indice = nota cantada relativa a KEY (C..B em C/Am); valor = intervalo da harmonia ACIMA da voz.
//! Nas partes L2/L1 (abaixo da voz) o HV10 usa a mesma nota uma oitava abaixo (valor - 12).

pub type ScaleTable = [Option<i8>; 12];

const N: Option<i8> = None;
const fn s(v: i8) -> Option<i8> { Some(v) }

/// Tabelas superiores, na ordem 3A..3H, 5A..5H.
const UPPER_TABLES: [ScaleTable; 16] = [
    //  C      C#  D      D#  E      F      F#  G      G#     A      A#  B
    [s(4), N, s(5), N, s(3), s(4), N, s(5), N, s(3), N, s(3)], // 3A
    [s(5), N, s(5), N, s(3), s(4), N, s(5), N, s(3), N, s(3)], // 3B
    [s(4), N, s(5), N, s(3), s(4), N, s(4), N, s(3), N, s(3)], // 3C
    [s(4), N, s(4), N, s(3), s(4), N, s(4), N, s(3), N, s(3)], // 3D
    [s(4), N, s(3), N, s(3), s(4), N, s(4), N, s(3), N, s(3)], // 3E
    [s(4), N, s(3), N, s(4), s(4), N, N, s(3), s(3), N, s(3)], // 3F
    [s(4), N, s(5), N, s(4), s(4), N, s(4), s(3), s(3), N, s(3)], // 3G
    [s(5), N, s(3), N, s(5), s(4), N, s(4), N, s(3), N, s(5)], // 3H
    [s(7), N, s(9), N, s(8), s(7), N, s(9), N, s(8), N, s(8)], // 5A
    [s(7), N, s(9), N, s(8), s(9), N, s(9), N, s(8), N, s(8)], // 5B
    [s(7), N, s(7), N, s(8), s(7), N, s(7), N, s(7), N, s(8)], // 5C
    [s(9), N, s(9), N, s(8), s(7), N, s(9), N, s(8), N, s(8)], // 5D
    [s(7), N, s(7), N, s(7), s(7), N, s(7), N, s(7), N, s(7)], // 5E
    [s(9), N, s(9), N, s(7), s(9), N, N, s(8), s(7), N, s(7)], // 5F
    [s(9), N, s(9), N, s(7), s(7), N, s(7), s(8), s(7), N, s(8)], // 5G
    [s(9), N, s(9), N, s(8), s(7), N, s(9), N, s(8), N, s(7)], // 5H
];

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
        (2..=17).contains(&i).then(|| i - 2)
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
                    let pc = (input_note - key).rem_euclid(12) as usize;
                    match UPPER_TABLES[t][pc] {
                        Some(v) => StyleLookup::Interval(v as i32 - if lower { 12 } else { 0 }),
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
    fn manual_tables_that_differ_from_the_dll() {
        let bank = StyleBank::default();
        let up = |st, n| match bank.lookup(st, false, 0, n) {
            StyleLookup::Interval(v) => Some(n + v),
            StyleLookup::Unassigned => None,
        };
        // 3H (pag. 31): C->F, D->F, E->A, F->A, G->B, A->C, B->E
        let h3: Vec<_> = [60, 62, 64, 65, 67, 69, 71].iter().map(|&n| up(StyleId::S3H, n)).collect();
        assert_eq!(h3, [Some(65), Some(65), Some(69), Some(69), Some(71), Some(72), Some(76)]);
        // 5H: C->A, D->B, E->C, F->C, G->E, A->F, B->F#
        let h5: Vec<_> = [60, 62, 64, 65, 67, 69, 71].iter().map(|&n| up(StyleId::S5H, n)).collect();
        assert_eq!(h5, [Some(69), Some(71), Some(72), Some(72), Some(76), Some(77), Some(78)]);
        // 5F: B->F# (o DLL dizia G#) e G sem atribuicao
        assert_eq!(up(StyleId::S5F, 71), Some(78));
        assert_eq!(up(StyleId::S5F, 67), None);
        // L2/L1: mesma nota uma oitava abaixo
        assert_eq!(bank.lookup(StyleId::S3A, true, 0, 60), StyleLookup::Interval(-8));
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
