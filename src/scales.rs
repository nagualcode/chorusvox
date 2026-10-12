//! Tabelas de escalas (HARMONYDATA.cpp) extraidas da DecaBuddy.dll (0x021100..0x0211A0).

pub const USER_NONE: i8 = i8::MIN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ScaleId {
    Midi = 0,
    User = 1,
    LowOctave = 2,
    Low3rdModal1 = 3,
    Low3rdModal2 = 4,
    Low3rdModal3 = 5,
    Low3rdModal4 = 6,
    Low3rdModal5 = 7,
    Low3rdHarmonicMinor = 8,
    Low3rdMelodicMinor = 9,
    Low5thModal1 = 10,
    Low5thModal2 = 11,
    Low5thModal3 = 12,
    Low5thModal4 = 13,
    Low5thModal5 = 14,
    Low5thHarmonicMinor = 15,
    Low5thMelodicMinor = 16,
    Low7thMajor = 17,
    Low7thMinor = 18,
    Unison = 19,
    High3rdModal1 = 20,
    High3rdModal2 = 21,
    High3rdModal3 = 22,
    High3rdModal4 = 23,
    High3rdModal5 = 24,
    High3rdHarmonicMinor = 25,
    High3rdMelodicMinor = 26,
    High5thModal1 = 27,
    High5thModal2 = 28,
    High5thModal3 = 29,
    High5thModal4 = 30,
    High5thModal5 = 31,
    High5thHarmonicMinor = 32,
    High5thMelodicMinor = 33,
    High7thMajor = 34,
    High7thMinor = 35,
    HighOctave = 36,
}

impl ScaleId {
    pub const ALL: [ScaleId; 37] = [
        ScaleId::Midi,
        ScaleId::User,
        ScaleId::LowOctave,
        ScaleId::Low3rdModal1,
        ScaleId::Low3rdModal2,
        ScaleId::Low3rdModal3,
        ScaleId::Low3rdModal4,
        ScaleId::Low3rdModal5,
        ScaleId::Low3rdHarmonicMinor,
        ScaleId::Low3rdMelodicMinor,
        ScaleId::Low5thModal1,
        ScaleId::Low5thModal2,
        ScaleId::Low5thModal3,
        ScaleId::Low5thModal4,
        ScaleId::Low5thModal5,
        ScaleId::Low5thHarmonicMinor,
        ScaleId::Low5thMelodicMinor,
        ScaleId::Low7thMajor,
        ScaleId::Low7thMinor,
        ScaleId::Unison,
        ScaleId::High3rdModal1,
        ScaleId::High3rdModal2,
        ScaleId::High3rdModal3,
        ScaleId::High3rdModal4,
        ScaleId::High3rdModal5,
        ScaleId::High3rdHarmonicMinor,
        ScaleId::High3rdMelodicMinor,
        ScaleId::High5thModal1,
        ScaleId::High5thModal2,
        ScaleId::High5thModal3,
        ScaleId::High5thModal4,
        ScaleId::High5thModal5,
        ScaleId::High5thHarmonicMinor,
        ScaleId::High5thMelodicMinor,
        ScaleId::High7thMajor,
        ScaleId::High7thMinor,
        ScaleId::HighOctave,
    ];

    pub fn from_index(idx: usize) -> Self {
        Self::ALL.get(idx).copied().unwrap_or(ScaleId::Unison)
    }

    pub fn menu_name(self) -> &'static str {
        AKAI_SCALE_NAMES[self as usize]
    }

    pub fn display_name(self, voice_idx: usize) -> &'static str {
        if self == ScaleId::User {
            match voice_idx {
                0 => "User 1",
                1 => "User 2",
                2 => "User 3",
                _ => "User 4",
            }
        } else {
            self.menu_name()
        }
    }

    pub fn root_pc(self) -> usize {
        use ScaleId::*;
        match self {
            Low3rdHarmonicMinor
            | Low3rdMelodicMinor
            | Low5thHarmonicMinor
            | Low5thMelodicMinor
            | Low7thMinor
            | High3rdHarmonicMinor
            | High3rdMelodicMinor
            | High5thHarmonicMinor
            | High5thMelodicMinor
            | High7thMinor => 9,
            _ => 0,
        }
    }
}

pub type ScaleTable = [Option<i8>; 12];

pub const AKAI_SCALE_NAMES: [&str; 37] = [
    "MIDI",
    "User",
    "Low Octave",
    "Low 3rd|Modal 1",
    "Low 3rd|Modal 2",
    "Low 3rd|Modal 3",
    "Low 3rd|Modal 4",
    "Low 3rd|Modal 5",
    "Low 3rd|Harmonic Minor",
    "Low 3rd|Melodic Minor",
    "Low 5th|Modal 1",
    "Low 5th|Modal 2",
    "Low 5th|Modal 3",
    "Low 5th|Modal 4",
    "Low 5th|Modal 5",
    "Low 5th|Harmonic Minor",
    "Low 5th|Melodic Minor",
    "Low 7th|Major",
    "Low 7th|Minor",
    "Unison",
    "High 3rd|Modal 1",
    "High 3rd|Modal 2",
    "High 3rd|Modal 3",
    "High 3rd|Modal 4",
    "High 3rd|Modal 5",
    "High 3rd|Harmonic Minor",
    "High 3rd|Melodic Minor",
    "High 5th|Modal 1",
    "High 5th|Modal 2",
    "High 5th|Modal 3",
    "High 5th|Modal 4",
    "High 5th|Modal 5",
    "High 5th|Harmonic Minor",
    "High 5th|Melodic Minor",
    "High 7th|Major",
    "High 7th|Minor",
    "High Octave",
];

const N: Option<i8> = None;
const fn s(v: i8) -> Option<i8> {
    Some(v)
}

pub const PRESET_TABLES: [ScaleTable; 37] = [
    [N; 12],
    [N; 12],
    [s(-12); 12],
    [s(-8), N, s(-7), N, s(-9), s(-8), N, s(-7), N, s(-9), N, s(-9)],
    [s(-7), N, s(-7), N, s(-9), s(-8), N, s(-7), N, s(-9), N, s(-9)],
    [s(-8), N, s(-7), N, s(-9), s(-8), N, s(-8), N, s(-9), N, s(-9)],
    [s(-8), N, s(-8), N, s(-9), s(-8), N, s(-8), N, s(-9), N, s(-9)],
    [s(-8), N, s(-9), N, s(-9), s(-8), N, s(-8), N, s(-9), N, s(-9)],
    [s(-9), N, s(-9), s(-8), N, s(-9), N, s(-8), s(-8), N, N, s(-9)],
    [s(-9), N, s(-9), s(-8), N, s(-7), N, s(-8), s(-8), N, s(-8), s(-9)],
    [s(-5), N, s(-3), N, s(-4), s(-5), N, s(-3), N, s(-4), N, s(-4)],
    [s(-5), N, s(-3), N, s(-4), s(-3), N, s(-3), N, s(-4), N, s(-4)],
    [s(-5), N, s(-5), N, s(-4), s(-5), N, s(-5), N, s(-5), N, s(-4)],
    [s(-3), N, s(-3), N, s(-4), s(-5), N, s(-3), N, s(-4), N, s(-4)],
    [s(-5), N, s(-5), N, s(-5), s(-5), N, s(-5), N, s(-5), N, s(-5)],
    [s(-5), N, s(-3), s(-3), N, s(-3), N, s(-5), s(-3), N, N, s(-4)],
    [s(-5), N, s(-4), s(-3), N, s(-3), N, s(-5), s(-5), N, s(-5), s(-4)],
    [s(-1), N, s(-2), N, s(-2), s(-1), N, s(-2), N, s(-2), N, s(-2)],
    [s(-2), N, s(-2), s(-1), N, s(-2), N, s(-2), s(-1), N, s(-2), N],
    [s(0); 12],
    [s(4), N, s(5), N, s(3), s(4), N, s(5), N, s(3), N, s(3)],
    [s(5), N, s(5), N, s(3), s(4), N, s(5), N, s(3), N, s(3)],
    [s(4), N, s(5), N, s(3), s(4), N, s(4), N, s(3), N, s(3)],
    [s(4), N, s(4), N, s(3), s(4), N, s(4), N, s(3), N, s(3)],
    [s(4), N, s(3), N, s(3), s(4), N, s(4), N, s(3), N, s(3)],
    [s(3), N, s(3), s(4), N, s(3), N, s(4), s(4), N, N, s(3)],
    [s(3), N, s(3), s(4), N, s(5), N, s(4), s(4), N, s(4), s(3)],
    [s(7), N, s(9), N, s(8), s(7), N, s(9), N, s(8), N, s(8)],
    [s(7), N, s(9), N, s(8), s(9), N, s(9), N, s(8), N, s(8)],
    [s(7), N, s(7), N, s(8), s(7), N, s(7), N, s(7), N, s(8)],
    [s(9), N, s(9), N, s(8), s(7), N, s(9), N, s(8), N, s(8)],
    [s(7), N, s(7), N, s(7), s(7), N, s(7), N, s(7), N, s(7)],
    [s(7), N, s(9), s(9), N, s(9), N, s(7), s(9), N, N, s(8)],
    [s(7), N, s(8), s(9), N, s(9), N, s(7), s(7), N, s(7), s(8)],
    [s(11), N, s(10), N, s(10), s(11), N, s(10), N, s(10), N, s(10)],
    [s(10), N, s(10), s(11), N, s(10), N, s(10), s(11), N, s(10), N],
    [s(12); 12],
];

#[derive(Clone, Debug)]
pub struct ScaleBank {
    pub preset_tables: [ScaleTable; 37],
    pub user_tables: [ScaleTable; 4],
}

impl Default for ScaleBank {
    fn default() -> Self {
        Self {
            preset_tables: PRESET_TABLES,
            user_tables: [[Some(0); 12]; 4],
        }
    }
}

impl ScaleBank {
    pub fn get_table(&self, id: ScaleId, voice_idx: usize) -> &ScaleTable {
        match id {
            ScaleId::User => &self.user_tables[voice_idx.min(3)],
            other => &self.preset_tables[other as usize],
        }
    }

    pub fn lookup_offset(&self, id: ScaleId, voice_idx: usize, pitch_class: usize) -> Option<i8> {
        let table = self.get_table(id, voice_idx);
        let pc = pitch_class % 12;
        if let Some(off) = table[pc] {
            return Some(off);
        }
        for d in 1..=6 {
            let down = (pc + 12 - d) % 12;
            if let Some(off) = table[down] {
                return Some(off);
            }
            let up = (pc + d) % 12;
            if let Some(off) = table[up] {
                return Some(off);
            }
        }
        None
    }

    pub fn set_user_step_raw(&mut self, user_idx: usize, step: usize, raw_val: i8) {
        if user_idx < 4 && step < 12 {
            self.user_tables[user_idx][step] = if raw_val == USER_NONE {
                None
            } else {
                Some(raw_val.clamp(-48, 48))
            };
        }
    }

    pub fn get_user_step_raw(&self, user_idx: usize, step: usize) -> i8 {
        if user_idx < 4 && step < 12 {
            self.user_tables[user_idx][step].unwrap_or(USER_NONE)
        } else {
            USER_NONE
        }
    }

    pub fn copy_scale(&mut self, src: ScaleId, src_voice: usize, dst_user_idx: Option<usize>) {
        let copied = *self.get_table(src, src_voice);
        match dst_user_idx {
            Some(i) if i < 4 => self.user_tables[i] = copied,
            None => {
                for slot in &mut self.user_tables {
                    *slot = copied;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_tables_integrity() {
        let bank = ScaleBank::default();
        assert_eq!(bank.preset_tables[ScaleId::LowOctave as usize], [Some(-12); 12]);
        assert_eq!(bank.preset_tables[ScaleId::Unison as usize], [Some(0); 12]);
        assert_eq!(bank.preset_tables[ScaleId::HighOctave as usize], [Some(12); 12]);
        for i in 3..=18 {
            let low = bank.preset_tables[i];
            let high = bank.preset_tables[i + 17];
            for pc in 0..12 {
                assert_eq!(low[pc].map(|v| v + 12), high[pc]);
            }
        }
    }

    #[test]
    fn ten_tables_are_rooted_on_the_relative_minor() {
        assert_eq!(
            ScaleId::ALL.iter().filter(|s| s.root_pc() == 9).count(),
            10
        );
    }
}
