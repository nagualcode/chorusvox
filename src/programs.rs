//! Os 100 programas de fabrica do HV10 (bancos 01-50, A/B), transcritos da "Preset Program List"
//! do manual de operacao (apendice, pags. 28-29). Dados em `factory_programs.txt`.
//!
//! Campos do manual que o plugin ainda nao modela sao ignorados: Sub VOICING fora do modo STYLE,
//! Sub TEMPLATE e a ordem Sub do PRIORITY.

use crate::core::{Buddies, Hv10Params, OriginBuddies, OriginParams, PartParams};
use crate::dsp::RandDelay;
use crate::harmony::{ChordVoicing, FlexiVoicing, HarmonyMode, ModalVoicing, TemplateDegree, NUM_PARTS};
use crate::scales::StyleId;

const DATA: &str = include_str!("factory_programs.txt");

#[derive(Clone, Debug)]
pub struct FactoryProgram {
    /// "01A" .. "50B"
    pub id: String,
    pub name: String,
    pub params: Hv10Params,
}

impl FactoryProgram {
    /// Nome exibido: "01A 3A Two Part".
    pub fn display_name(&self) -> String { format!("{} {}", self.id, self.name) }
}

/// Indice 0..100 do programa para a mensagem MIDI Program Change (0-based):
/// PC 1-50 -> 01A-50A, PC 51-100 -> 01B-50B (manual, "Program Change").
pub fn program_change_index(pc: u8) -> Option<usize> {
    match pc {
        0..=49 => Some(pc as usize * 2),
        50..=99 => Some((pc as usize - 50) * 2 + 1),
        _ => None,
    }
}

fn key_index(k: &str) -> u8 {
    match k {
        "C" => 0, "Db" | "C#" => 1, "D" => 2, "Eb" | "D#" => 3, "E" => 4, "F" => 5,
        "F#" | "Gb" => 6, "G" => 7, "Ab" | "G#" => 8, "A" => 9, "Bb" | "A#" => 10, "B" => 11,
        _ => 0,
    }
}

fn style(tok: &str) -> StyleId {
    const NAMES: [&str; 18] = ["UN", "8v", "3A", "3B", "3C", "3D", "3E", "3F", "3G", "3H", "5A", "5B", "5C", "5D", "5E", "5F", "5G", "5H"];
    NAMES.iter().position(|&n| n == tok).map(StyleId::from_index).unwrap_or(StyleId::Un)
}

fn modal(tok: &str) -> ModalVoicing {
    match tok {
        "8L" => ModalVoicing::OctLow,
        "8H" => ModalVoicing::OctHigh,
        t if t.starts_with('H') => ModalVoicing::High(t[1..].parse().unwrap_or(1)),
        t if t.starts_with('L') => ModalVoicing::Low(t[1..].parse().unwrap_or(1)),
        _ => ModalVoicing::Un,
    }
}

fn chord_voicing(tok: &str) -> ChordVoicing {
    match tok {
        "L8" | "8L" => ChordVoicing::OctLow,
        "H8" | "8H" => ChordVoicing::OctHigh,
        "HI" => ChordVoicing::Hi,
        "LO" => ChordVoicing::Lo,
        _ => ChordVoicing::Un,
    }
}

fn signed(tok: &str) -> i32 { tok.trim_start_matches('+').parse().unwrap_or(0) }

fn pan(tok: &str) -> f32 {
    match tok.as_bytes().first() {
        Some(b'L') => -(tok[1..].parse::<f32>().unwrap_or(0.0)),
        Some(b'R') => tok[1..].parse::<f32>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn pcorr(tok: &str) -> f32 { tok.parse().unwrap_or(0.0) }

fn rdelay(tok: &str) -> RandDelay {
    let n: u8 = tok.get(1..).and_then(|v| v.parse().ok()).unwrap_or(0);
    match tok.as_bytes().first() {
        Some(b'D') => RandDelay::Fixed(n),
        Some(b'R') => RandDelay::Random(n),
        _ => RandDelay::Off,
    }
}

fn buddies(tok: &str) -> Buddies {
    match tok {
        "*" => Buddies::On,
        "**" => Buddies::OnUnison,
        _ => Buddies::Off,
    }
}

fn template(spec: &str) -> [TemplateDegree; 7] {
    let mut t = [TemplateDegree::Off; 7];
    for (i, c) in spec.chars().take(7).enumerate() {
        t[i] = match c {
            '-' => TemplateDegree::Off,
            'm' => TemplateDegree::Minor,
            'M' => TemplateDegree::Major,
            _ => TemplateDegree::On,
        };
    }
    t
}

fn parse_line(line: &str) -> Option<FactoryProgram> {
    let f: Vec<&str> = line.split('|').collect();
    if f.len() != 12 { return None; }
    let mut p = Hv10Params::default();
    let mode = match f[2] {
        "STYLE" => HarmonyMode::Style,
        "CHORD" => HarmonyMode::Chord,
        "MODAL" => HarmonyMode::Modal,
        "FLEXICHORD" => HarmonyMode::FlexiChord,
        "MONOCHN" => HarmonyMode::MonoChn,
        "WAITALL" => HarmonyMode::WaitAll,
        "PRIORITY" => HarmonyMode::Priority,
        _ => return None,
    };
    p.harmony.mode = mode;
    p.harmony.key = key_index(f[3]);
    p.sub_key = if f[4] == "-" { p.harmony.key } else { key_index(f[4]) };
    p.sub_active = false;
    p.master_level = f[5].parse().ok()?;
    for kv in f[6].split(';') {
        if let Some(v) = kv.strip_prefix("chord=") {
            p.harmony.chord = v.parse::<u8>().ok()?.saturating_sub(1);
        } else if let Some(v) = kv.strip_prefix("template=") {
            p.harmony.template = template(v);
        }
    }

    // Colunas na ordem L2, L1, ORIGIN, H1, H2 -> partes 0..3 = L2, L1, H1, H2
    let part_cols = [f[7], f[8], f[10], f[11]];
    let mut flexi = ['L'; NUM_PARTS];
    for (i, col) in part_cols.iter().enumerate() {
        let c: Vec<&str> = col.split(',').collect();
        if c.len() != 8 { return None; }
        p.parts[i] = PartParams {
            buddies: buddies(c[0]),
            formant: signed(c[3]),
            level: c[4].parse().ok()?,
            pan: pan(c[5]),
            p_correct: pcorr(c[6]),
            rand_delay: rdelay(c[7]),
        };
        p.harmony.styles[i] = StyleId::Un;
        p.sub_styles[i] = StyleId::Un;
        match mode {
            HarmonyMode::Style => {
                p.harmony.styles[i] = style(c[1]);
                p.sub_styles[i] = style(c[2]);
            }
            HarmonyMode::Chord => p.harmony.chord_voicing[i] = chord_voicing(c[1]),
            HarmonyMode::Modal => p.harmony.modal_voicing[i] = modal(c[1]),
            HarmonyMode::FlexiChord => flexi[i] = if c[1] == "HI" { 'H' } else { 'L' },
            HarmonyMode::Priority => p.harmony.priority_rank[i] = c[1].trim_start_matches('P').parse().unwrap_or(i as u8 + 1),
            _ => {}
        }
    }
    if mode == HarmonyMode::FlexiChord {
        p.harmony.flexi_voicing = match flexi.iter().filter(|&&c| c == 'H').count() {
            1 => FlexiVoicing::LllH,
            3 => FlexiVoicing::LHhh,
            _ => FlexiVoicing::LlHh,
        };
    }

    let o: Vec<&str> = f[9].split(',').collect();
    if o.len() != 6 { return None; }
    p.origin = OriginParams {
        buddies: match o[0] {
            "*" => OriginBuddies::On,
            "**" => OriginBuddies::OnUnison,
            "FX" => OriginBuddies::Fx,
            _ => OriginBuddies::Off,
        },
        formant: signed(o[1]),
        level: o[2].parse().ok()?,
        pan: pan(o[3]),
        p_correct: pcorr(o[4]),
        rand_delay: rdelay(o[5]),
    };
    Some(FactoryProgram { id: f[0].to_string(), name: f[1].to_string(), params: p })
}

/// Todos os programas de fabrica na ordem 01A, 01B, 02A, ... 50B.
pub fn factory_programs() -> Vec<FactoryProgram> {
    DATA.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(parse_line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_hundred_programs_parse_in_order() {
        let progs = factory_programs();
        assert_eq!(progs.len(), 100);
        for (i, p) in progs.iter().enumerate() {
            let expect = format!("{:02}{}", i / 2 + 1, if i % 2 == 0 { 'A' } else { 'B' });
            assert_eq!(p.id, expect);
        }
    }

    #[test]
    fn spot_check_against_the_manual() {
        let progs = factory_programs();
        // 01A 3A Two Part: H1 3A formant +3 level 6 pan L8 P.CORRECT 1 R4, MASTER 7
        let p = &progs[0].params;
        assert_eq!(progs[0].name, "3A Two Part");
        assert_eq!(p.harmony.styles[2], StyleId::S3A);
        assert_eq!(p.sub_styles[2], StyleId::S3E);
        assert_eq!((p.parts[2].formant, p.parts[2].level, p.parts[2].pan), (3, 6.0, -8.0));
        assert_eq!(p.parts[2].rand_delay, RandDelay::Random(4));
        assert_eq!(p.master_level, 7.0);
        assert_eq!(p.origin.buddies, OriginBuddies::OnUnison);
        // 39A Minor: CHORD 01 em Eb/Cm
        let m = &progs[76].params;
        assert_eq!((m.harmony.mode, m.harmony.key, m.harmony.chord), (HarmonyMode::Chord, 3, 0));
        // 41A Modal 6th: template 1-3-56-
        let t = &progs[80].params.harmony;
        assert_eq!(t.template[5], TemplateDegree::On);
        assert_eq!(t.template[1], TemplateDegree::Off);
        assert_eq!(t.modal_voicing[0], ModalVoicing::High(1));
        // 44A Easy Chord 2: FLEXICHORD LO HI HI HI
        assert_eq!(progs[86].params.harmony.flexi_voicing, FlexiVoicing::LHhh);
        // 49A Synth Vox: PRIORITY, voz original desligada
        let s = &progs[96].params;
        assert_eq!(s.harmony.priority_rank, [1, 2, 3, 4]);
        assert_eq!(s.origin.buddies, OriginBuddies::Off);
    }

    #[test]
    fn program_change_numbering() {
        assert_eq!(program_change_index(0), Some(0)); // PC 1 -> 01A
        assert_eq!(program_change_index(49), Some(98)); // PC 50 -> 50A
        assert_eq!(program_change_index(50), Some(1)); // PC 51 -> 01B
        assert_eq!(program_change_index(99), Some(99)); // PC 100 -> 50B
        assert_eq!(program_change_index(100), None);
    }
}
