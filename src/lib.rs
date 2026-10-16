//! ChorusVox DecaBuddy: emulacao do Akai DecaBuddy HV10 (Harmony Processing Unit).

pub mod core;
pub mod dsp;
#[cfg(feature = "truce-plugin")]
pub mod gui;
pub mod harmony;
pub mod programs;
pub mod scales;

pub use crate::core::{Buddies, DecaBuddyProcessor, Hv10Params, InputMeterLeds, OriginBuddies, OriginParams, PartParams};

#[cfg(feature = "truce-plugin")]
pub mod truce_integration {
    use super::*;
    use crate::dsp::RandDelay;
    use crate::harmony::{ChordVoicing, FlexiVoicing, HarmonyMode, ModalVoicing, TemplateDegree, NUM_PARTS};
    use crate::scales::{StyleBank, StyleId, NUM_USER_STYLES};
    use std::sync::Arc;
    use truce::prelude::*;

    #[derive(ParamEnum, Debug)]
    pub enum ModeChoice {
        #[name = "STYLE"] V0,
        #[name = "MODAL"] V1,
        #[name = "FLEXICHORD"] V2,
        #[name = "MONO CHN."] V3,
        #[name = "WAIT ALL"] V4,
        #[name = "PRIORITY"] V5,
        #[name = "SMART"] V6,
        #[name = "CHORD"] V7,
    }
    #[derive(ParamEnum, Debug)]
    pub enum ChordChoice {
        #[name = "01 Normal"] V0,
        #[name = "02 Diatonic"] V1,
        #[name = "03 7th"] V2,
        #[name = "04 Com-Dim"] V3,
        #[name = "05 Diminish"] V4,
        #[name = "06 Augment"] V5,
        #[name = "07 IIm7/V"] V6,
        #[name = "08 6/9"] V7,
    }
    #[derive(ParamEnum, Debug)]
    pub enum ChordVcChoice {
        #[name = "UN"] V0,
        #[name = "L8"] V1,
        #[name = "H8"] V2,
        #[name = "HI"] V3,
        #[name = "LO"] V4,
    }
    #[derive(ParamEnum, Debug)]
    pub enum KeyChoice {
        #[name = "C/Am"] V0,
        #[name = "Db/Bbm"] V1,
        #[name = "D/Bm"] V2,
        #[name = "Eb/Cm"] V3,
        #[name = "E/C#m"] V4,
        #[name = "F/Dm"] V5,
        #[name = "F#/D#m"] V6,
        #[name = "G/Em"] V7,
        #[name = "Ab/Fm"] V8,
        #[name = "A/F#m"] V9,
        #[name = "Bb/Gm"] V10,
        #[name = "B/G#m"] V11,
    }
    #[derive(ParamEnum, Debug)]
    pub enum StyleChoice {
        #[name = "UN"] V0,
        #[name = "8v"] V1,
        #[name = "3A"] V2,
        #[name = "3B"] V3,
        #[name = "3C"] V4,
        #[name = "3D"] V5,
        #[name = "3E"] V6,
        #[name = "3F"] V7,
        #[name = "3G"] V8,
        #[name = "3H"] V9,
        #[name = "5A"] V10,
        #[name = "5B"] V11,
        #[name = "5C"] V12,
        #[name = "5D"] V13,
        #[name = "5E"] V14,
        #[name = "5F"] V15,
        #[name = "5G"] V16,
        #[name = "5H"] V17,
        #[name = "U1"] V18,
        #[name = "U2"] V19,
        #[name = "U3"] V20,
        #[name = "U4"] V21,
        #[name = "U5"] V22,
        #[name = "U6"] V23,
        #[name = "U7"] V24,
        #[name = "U8"] V25,
        #[name = "U9"] V26,
    }
    #[derive(ParamEnum, Debug)]
    pub enum ModalChoice {
        #[name = "UN"] V0,
        #[name = "8L"] V1,
        #[name = "8H"] V2,
        #[name = "L8"] V3,
        #[name = "L7"] V4,
        #[name = "L6"] V5,
        #[name = "L5"] V6,
        #[name = "L4"] V7,
        #[name = "L3"] V8,
        #[name = "L2"] V9,
        #[name = "L1"] V10,
        #[name = "H1"] V11,
        #[name = "H2"] V12,
        #[name = "H3"] V13,
        #[name = "H4"] V14,
        #[name = "H5"] V15,
        #[name = "H6"] V16,
        #[name = "H7"] V17,
        #[name = "H8"] V18,
    }
    #[derive(ParamEnum, Debug)]
    pub enum BuddiesChoice {
        #[name = "--"] V0,
        #[name = "ON"] V1,
        #[name = "ON+UN"] V2,
    }
    #[derive(ParamEnum, Debug)]
    pub enum OriginBuddiesChoice {
        #[name = "--"] V0,
        #[name = "ON"] V1,
        #[name = "ON+UN"] V2,
        #[name = "FX"] V3,
    }
    #[derive(ParamEnum, Debug)]
    pub enum FlexiChoice {
        #[name = "LO LO LO HI"] V0,
        #[name = "LO LO HI HI"] V1,
        #[name = "LO HI HI HI"] V2,
    }
    #[derive(ParamEnum, Debug)]
    pub enum Tpl3Choice {
        #[name = "--"] V0,
        #[name = "3"] V1,
        #[name = "3m"] V2,
        #[name = "3M"] V3,
    }
    #[derive(ParamEnum, Debug)]
    pub enum Tpl7Choice {
        #[name = "--"] V0,
        #[name = "7"] V1,
        #[name = "7m"] V2,
        #[name = "7M"] V3,
    }

    #[derive(Params)]
    pub struct ChorusVoxParams {
        #[param(name = "Mode", default = 0)] pub mode: EnumParam<ModeChoice>,
        #[param(name = "Key", default = 0)] pub key: EnumParam<KeyChoice>,
        #[param(name = "Sub", default = false)] pub sub: BoolParam,
        #[param(name = "Sub Key", default = 0)] pub sub_key: EnumParam<KeyChoice>,
        #[param(name = "Tune", range = "linear(427, 453)", default = 440, unit = "Hz")] pub tune: IntParam,
        #[param(name = "MIDI Ch", range = "linear(1, 16)", default = 1)] pub midi_ch: IntParam,
        #[param(name = "Input", range = "linear(-15, 15)", default = 0.0, unit = "dB")] pub input: FloatParam,
        #[param(name = "Master", range = "linear(0, 9)", default = 7)] pub master: IntParam,
        #[param(name = "Bypass", default = false)] pub bypass: BoolParam,

        #[param(name = "Origin", default = 2)] pub org_buddies: EnumParam<OriginBuddiesChoice>,
        #[param(name = "Origin Formant", range = "linear(-9, 9)", default = 0, format = "fmt_signed")] pub org_formant: IntParam,
        #[param(name = "Origin Level", range = "linear(0, 9)", default = 9)] pub org_level: IntParam,
        #[param(name = "Origin Pan", range = "linear(-9, 9)", default = 0, format = "fmt_pan")] pub org_pan: IntParam,
        #[param(name = "Origin R.Delay", range = "linear(0, 18)", default = 0, format = "fmt_rdelay")] pub org_rdelay: IntParam,

        #[param(name = "L2", default = 0)] pub l2_buddies: EnumParam<BuddiesChoice>,
        #[param(name = "L2 Style", default = 0)] pub l2_style: EnumParam<StyleChoice>,
        #[param(name = "L2 Sub Style", default = 0)] pub l2_sub_style: EnumParam<StyleChoice>,
        #[param(name = "L2 Modal", default = 9)] pub l2_modal: EnumParam<ModalChoice>,
        #[param(name = "L2 Formant", range = "linear(-9, 9)", default = 0, format = "fmt_signed")] pub l2_formant: IntParam,
        #[param(name = "L2 Level", range = "linear(0, 9)", default = 0)] pub l2_level: IntParam,
        #[param(name = "L2 Pan", range = "linear(-9, 9)", default = 0, format = "fmt_pan")] pub l2_pan: IntParam,
        #[param(name = "L2 P.Correct", range = "linear(0, 9)", default = 0, format = "fmt_pcorr")] pub l2_pcorr: IntParam,
        #[param(name = "L2 R.Delay", range = "linear(0, 18)", default = 0, format = "fmt_rdelay")] pub l2_rdelay: IntParam,
        #[param(name = "L2 Priority", range = "linear(1, 4)", default = 1)] pub l2_prio: IntParam,

        #[param(name = "L1", default = 0)] pub l1_buddies: EnumParam<BuddiesChoice>,
        #[param(name = "L1 Style", default = 0)] pub l1_style: EnumParam<StyleChoice>,
        #[param(name = "L1 Sub Style", default = 0)] pub l1_sub_style: EnumParam<StyleChoice>,
        #[param(name = "L1 Modal", default = 10)] pub l1_modal: EnumParam<ModalChoice>,
        #[param(name = "L1 Formant", range = "linear(-9, 9)", default = 0, format = "fmt_signed")] pub l1_formant: IntParam,
        #[param(name = "L1 Level", range = "linear(0, 9)", default = 0)] pub l1_level: IntParam,
        #[param(name = "L1 Pan", range = "linear(-9, 9)", default = 0, format = "fmt_pan")] pub l1_pan: IntParam,
        #[param(name = "L1 P.Correct", range = "linear(0, 9)", default = 0, format = "fmt_pcorr")] pub l1_pcorr: IntParam,
        #[param(name = "L1 R.Delay", range = "linear(0, 18)", default = 0, format = "fmt_rdelay")] pub l1_rdelay: IntParam,
        #[param(name = "L1 Priority", range = "linear(1, 4)", default = 2)] pub l1_prio: IntParam,

        #[param(name = "H1", default = 2)] pub h1_buddies: EnumParam<BuddiesChoice>,
        #[param(name = "H1 Style", default = 2)] pub h1_style: EnumParam<StyleChoice>,
        #[param(name = "H1 Sub Style", default = 6)] pub h1_sub_style: EnumParam<StyleChoice>,
        #[param(name = "H1 Modal", default = 11)] pub h1_modal: EnumParam<ModalChoice>,
        #[param(name = "H1 Formant", range = "linear(-9, 9)", default = 3, format = "fmt_signed")] pub h1_formant: IntParam,
        #[param(name = "H1 Level", range = "linear(0, 9)", default = 6)] pub h1_level: IntParam,
        #[param(name = "H1 Pan", range = "linear(-9, 9)", default = -8, format = "fmt_pan")] pub h1_pan: IntParam,
        #[param(name = "H1 P.Correct", range = "linear(0, 9)", default = 1, format = "fmt_pcorr")] pub h1_pcorr: IntParam,
        #[param(name = "H1 R.Delay", range = "linear(0, 18)", default = 13, format = "fmt_rdelay")] pub h1_rdelay: IntParam,
        #[param(name = "H1 Priority", range = "linear(1, 4)", default = 3)] pub h1_prio: IntParam,

        #[param(name = "H2", default = 2)] pub h2_buddies: EnumParam<BuddiesChoice>,
        #[param(name = "H2 Style", default = 2)] pub h2_style: EnumParam<StyleChoice>,
        #[param(name = "H2 Sub Style", default = 6)] pub h2_sub_style: EnumParam<StyleChoice>,
        #[param(name = "H2 Modal", default = 12)] pub h2_modal: EnumParam<ModalChoice>,
        #[param(name = "H2 Formant", range = "linear(-9, 9)", default = 6, format = "fmt_signed")] pub h2_formant: IntParam,
        #[param(name = "H2 Level", range = "linear(0, 9)", default = 6)] pub h2_level: IntParam,
        #[param(name = "H2 Pan", range = "linear(-9, 9)", default = 8, format = "fmt_pan")] pub h2_pan: IntParam,
        #[param(name = "H2 P.Correct", range = "linear(0, 9)", default = 1, format = "fmt_pcorr")] pub h2_pcorr: IntParam,
        #[param(name = "H2 R.Delay", range = "linear(0, 18)", default = 16, format = "fmt_rdelay")] pub h2_rdelay: IntParam,
        #[param(name = "H2 Priority", range = "linear(1, 4)", default = 4)] pub h2_prio: IntParam,

        #[param(name = "Template 1", default = true)] pub tpl1: BoolParam,
        #[param(name = "Template 2", default = false)] pub tpl2: BoolParam,
        #[param(name = "Template 3", default = 1)] pub tpl3: EnumParam<Tpl3Choice>,
        #[param(name = "Template 4", default = false)] pub tpl4: BoolParam,
        #[param(name = "Template 5", default = true)] pub tpl5: BoolParam,
        #[param(name = "Template 6", default = false)] pub tpl6: BoolParam,
        #[param(name = "Template 7", default = 0)] pub tpl7: EnumParam<Tpl7Choice>,
        #[param(name = "FlexiChord Voicing", default = 1)] pub flexi: EnumParam<FlexiChoice>,

        #[param(name = "U1 C", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_c: IntParam,
        #[param(name = "U1 C#", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_cs: IntParam,
        #[param(name = "U1 D", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_d: IntParam,
        #[param(name = "U1 D#", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_ds: IntParam,
        #[param(name = "U1 E", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_e: IntParam,
        #[param(name = "U1 F", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_f: IntParam,
        #[param(name = "U1 F#", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_fs: IntParam,
        #[param(name = "U1 G", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_g: IntParam,
        #[param(name = "U1 G#", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_gs: IntParam,
        #[param(name = "U1 A", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_a: IntParam,
        #[param(name = "U1 A#", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_as: IntParam,
        #[param(name = "U1 B", range = "linear(-25, 24)", default = -5, format = "fmt_user")] pub u1_b: IntParam,
        #[param(name = "U2 C", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_c: IntParam,
        #[param(name = "U2 C#", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_cs: IntParam,
        #[param(name = "U2 D", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_d: IntParam,
        #[param(name = "U2 D#", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_ds: IntParam,
        #[param(name = "U2 E", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_e: IntParam,
        #[param(name = "U2 F", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_f: IntParam,
        #[param(name = "U2 F#", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_fs: IntParam,
        #[param(name = "U2 G", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_g: IntParam,
        #[param(name = "U2 G#", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_gs: IntParam,
        #[param(name = "U2 A", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_a: IntParam,
        #[param(name = "U2 A#", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_as: IntParam,
        #[param(name = "U2 B", range = "linear(-25, 24)", default = 4, format = "fmt_user")] pub u2_b: IntParam,
        #[param(name = "U3 C", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_c: IntParam,
        #[param(name = "U3 C#", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_cs: IntParam,
        #[param(name = "U3 D", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_d: IntParam,
        #[param(name = "U3 D#", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_ds: IntParam,
        #[param(name = "U3 E", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_e: IntParam,
        #[param(name = "U3 F", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_f: IntParam,
        #[param(name = "U3 F#", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_fs: IntParam,
        #[param(name = "U3 G", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_g: IntParam,
        #[param(name = "U3 G#", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_gs: IntParam,
        #[param(name = "U3 A", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_a: IntParam,
        #[param(name = "U3 A#", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_as: IntParam,
        #[param(name = "U3 B", range = "linear(-25, 24)", default = 7, format = "fmt_user")] pub u3_b: IntParam,
        #[param(name = "U4 C", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_c: IntParam,
        #[param(name = "U4 C#", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_cs: IntParam,
        #[param(name = "U4 D", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_d: IntParam,
        #[param(name = "U4 D#", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_ds: IntParam,
        #[param(name = "U4 E", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_e: IntParam,
        #[param(name = "U4 F", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_f: IntParam,
        #[param(name = "U4 F#", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_fs: IntParam,
        #[param(name = "U4 G", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_g: IntParam,
        #[param(name = "U4 G#", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_gs: IntParam,
        #[param(name = "U4 A", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_a: IntParam,
        #[param(name = "U4 A#", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_as: IntParam,
        #[param(name = "U4 B", range = "linear(-25, 24)", default = 12, format = "fmt_user")] pub u4_b: IntParam,
        #[param(name = "U5 C", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_c: IntParam,
        #[param(name = "U5 C#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_cs: IntParam,
        #[param(name = "U5 D", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_d: IntParam,
        #[param(name = "U5 D#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_ds: IntParam,
        #[param(name = "U5 E", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_e: IntParam,
        #[param(name = "U5 F", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_f: IntParam,
        #[param(name = "U5 F#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_fs: IntParam,
        #[param(name = "U5 G", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_g: IntParam,
        #[param(name = "U5 G#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_gs: IntParam,
        #[param(name = "U5 A", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_a: IntParam,
        #[param(name = "U5 A#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_as: IntParam,
        #[param(name = "U5 B", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u5_b: IntParam,
        #[param(name = "U6 C", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_c: IntParam,
        #[param(name = "U6 C#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_cs: IntParam,
        #[param(name = "U6 D", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_d: IntParam,
        #[param(name = "U6 D#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_ds: IntParam,
        #[param(name = "U6 E", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_e: IntParam,
        #[param(name = "U6 F", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_f: IntParam,
        #[param(name = "U6 F#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_fs: IntParam,
        #[param(name = "U6 G", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_g: IntParam,
        #[param(name = "U6 G#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_gs: IntParam,
        #[param(name = "U6 A", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_a: IntParam,
        #[param(name = "U6 A#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_as: IntParam,
        #[param(name = "U6 B", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u6_b: IntParam,
        #[param(name = "U7 C", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_c: IntParam,
        #[param(name = "U7 C#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_cs: IntParam,
        #[param(name = "U7 D", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_d: IntParam,
        #[param(name = "U7 D#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_ds: IntParam,
        #[param(name = "U7 E", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_e: IntParam,
        #[param(name = "U7 F", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_f: IntParam,
        #[param(name = "U7 F#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_fs: IntParam,
        #[param(name = "U7 G", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_g: IntParam,
        #[param(name = "U7 G#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_gs: IntParam,
        #[param(name = "U7 A", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_a: IntParam,
        #[param(name = "U7 A#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_as: IntParam,
        #[param(name = "U7 B", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u7_b: IntParam,
        #[param(name = "U8 C", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_c: IntParam,
        #[param(name = "U8 C#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_cs: IntParam,
        #[param(name = "U8 D", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_d: IntParam,
        #[param(name = "U8 D#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_ds: IntParam,
        #[param(name = "U8 E", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_e: IntParam,
        #[param(name = "U8 F", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_f: IntParam,
        #[param(name = "U8 F#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_fs: IntParam,
        #[param(name = "U8 G", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_g: IntParam,
        #[param(name = "U8 G#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_gs: IntParam,
        #[param(name = "U8 A", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_a: IntParam,
        #[param(name = "U8 A#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_as: IntParam,
        #[param(name = "U8 B", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u8_b: IntParam,
        #[param(name = "U9 C", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_c: IntParam,
        #[param(name = "U9 C#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_cs: IntParam,
        #[param(name = "U9 D", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_d: IntParam,
        #[param(name = "U9 D#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_ds: IntParam,
        #[param(name = "U9 E", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_e: IntParam,
        #[param(name = "U9 F", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_f: IntParam,
        #[param(name = "U9 F#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_fs: IntParam,
        #[param(name = "U9 G", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_g: IntParam,
        #[param(name = "U9 G#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_gs: IntParam,
        #[param(name = "U9 A", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_a: IntParam,
        #[param(name = "U9 A#", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_as: IntParam,
        #[param(name = "U9 B", range = "linear(-25, 24)", default = 0, format = "fmt_user")] pub u9_b: IntParam,

        // Parametros adicionados na v0.3.2 (no fim para manter os IDs anteriores estaveis)
        #[param(name = "Chord", default = 0)] pub chord: EnumParam<ChordChoice>,
        #[param(name = "L2 Chord", default = 3)] pub l2_chord_vc: EnumParam<ChordVcChoice>,
        #[param(name = "L1 Chord", default = 3)] pub l1_chord_vc: EnumParam<ChordVcChoice>,
        #[param(name = "H1 Chord", default = 3)] pub h1_chord_vc: EnumParam<ChordVcChoice>,
        #[param(name = "H2 Chord", default = 3)] pub h2_chord_vc: EnumParam<ChordVcChoice>,

        #[meter] pub meter_in: MeterSlot,
        #[meter] pub meter_out_l: MeterSlot,
        #[meter] pub meter_out_r: MeterSlot,
        /// Ultimo Program Change recebido (ponte DSP -> interface, ver core::program_token).
        #[meter] pub meter_program: MeterSlot,
    }

    impl ChorusVoxParams {
        fn fmt_signed(&self, v: f64) -> String {
            let v = v.round() as i64;
            if v > 0 { format!("+{v}") } else { format!("{v}") }
        }

        fn fmt_pan(&self, v: f64) -> String {
            let v = v.round() as i64;
            match v {
                0 => "C".to_string(),
                v if v < 0 => format!("L{}", -v),
                v => format!("R{v}"),
            }
        }

        fn fmt_pcorr(&self, v: f64) -> String {
            let v = v.round() as i64;
            if v <= 0 { "--".to_string() } else { format!("{v}") }
        }

        fn fmt_rdelay(&self, v: f64) -> String {
            match v.round() as i64 {
                n @ 1..=9 => format!("D{n}"),
                n @ 10..=18 => format!("R{}", n - 9),
                _ => "--".to_string(),
            }
        }

        fn fmt_user(&self, v: f64) -> String {
            let v = v.round() as i64;
            if v <= -25 { "--".to_string() } else if v > 0 { format!("+{v}") } else { format!("{v}") }
        }

        fn user_steps(&self) -> [[i32; 12]; NUM_USER_STYLES] {
            [

                [self.u1_c.value_i32(), self.u1_cs.value_i32(), self.u1_d.value_i32(), self.u1_ds.value_i32(), self.u1_e.value_i32(), self.u1_f.value_i32(), self.u1_fs.value_i32(), self.u1_g.value_i32(), self.u1_gs.value_i32(), self.u1_a.value_i32(), self.u1_as.value_i32(), self.u1_b.value_i32()],
                [self.u2_c.value_i32(), self.u2_cs.value_i32(), self.u2_d.value_i32(), self.u2_ds.value_i32(), self.u2_e.value_i32(), self.u2_f.value_i32(), self.u2_fs.value_i32(), self.u2_g.value_i32(), self.u2_gs.value_i32(), self.u2_a.value_i32(), self.u2_as.value_i32(), self.u2_b.value_i32()],
                [self.u3_c.value_i32(), self.u3_cs.value_i32(), self.u3_d.value_i32(), self.u3_ds.value_i32(), self.u3_e.value_i32(), self.u3_f.value_i32(), self.u3_fs.value_i32(), self.u3_g.value_i32(), self.u3_gs.value_i32(), self.u3_a.value_i32(), self.u3_as.value_i32(), self.u3_b.value_i32()],
                [self.u4_c.value_i32(), self.u4_cs.value_i32(), self.u4_d.value_i32(), self.u4_ds.value_i32(), self.u4_e.value_i32(), self.u4_f.value_i32(), self.u4_fs.value_i32(), self.u4_g.value_i32(), self.u4_gs.value_i32(), self.u4_a.value_i32(), self.u4_as.value_i32(), self.u4_b.value_i32()],
                [self.u5_c.value_i32(), self.u5_cs.value_i32(), self.u5_d.value_i32(), self.u5_ds.value_i32(), self.u5_e.value_i32(), self.u5_f.value_i32(), self.u5_fs.value_i32(), self.u5_g.value_i32(), self.u5_gs.value_i32(), self.u5_a.value_i32(), self.u5_as.value_i32(), self.u5_b.value_i32()],
                [self.u6_c.value_i32(), self.u6_cs.value_i32(), self.u6_d.value_i32(), self.u6_ds.value_i32(), self.u6_e.value_i32(), self.u6_f.value_i32(), self.u6_fs.value_i32(), self.u6_g.value_i32(), self.u6_gs.value_i32(), self.u6_a.value_i32(), self.u6_as.value_i32(), self.u6_b.value_i32()],
                [self.u7_c.value_i32(), self.u7_cs.value_i32(), self.u7_d.value_i32(), self.u7_ds.value_i32(), self.u7_e.value_i32(), self.u7_f.value_i32(), self.u7_fs.value_i32(), self.u7_g.value_i32(), self.u7_gs.value_i32(), self.u7_a.value_i32(), self.u7_as.value_i32(), self.u7_b.value_i32()],
                [self.u8_c.value_i32(), self.u8_cs.value_i32(), self.u8_d.value_i32(), self.u8_ds.value_i32(), self.u8_e.value_i32(), self.u8_f.value_i32(), self.u8_fs.value_i32(), self.u8_g.value_i32(), self.u8_gs.value_i32(), self.u8_a.value_i32(), self.u8_as.value_i32(), self.u8_b.value_i32()],
                [self.u9_c.value_i32(), self.u9_cs.value_i32(), self.u9_d.value_i32(), self.u9_ds.value_i32(), self.u9_e.value_i32(), self.u9_f.value_i32(), self.u9_fs.value_i32(), self.u9_g.value_i32(), self.u9_gs.value_i32(), self.u9_a.value_i32(), self.u9_as.value_i32(), self.u9_b.value_i32()],
            ]
        }

        #[cfg(test)]
        pub(crate) fn to_hv10_for_tests(&self) -> Hv10Params { self.to_hv10() }

        /// Estado do HV10 lido dos parametros do host.
        fn to_hv10(&self) -> Hv10Params {
            let mut p = Hv10Params::default();
            p.harmony.mode = match self.mode.index() {
                0 => HarmonyMode::Style,
                1 => HarmonyMode::Modal,
                2 => HarmonyMode::FlexiChord,
                3 => HarmonyMode::MonoChn,
                4 => HarmonyMode::WaitAll,
                5 => HarmonyMode::Priority,
                6 => HarmonyMode::Smart,
                _ => HarmonyMode::Chord,
            };
            p.harmony.chord = self.chord.index() as u8;
            p.harmony.chord_voicing = [
                ChordVoicing::from_index(self.l2_chord_vc.index() as usize),
                ChordVoicing::from_index(self.l1_chord_vc.index() as usize),
                ChordVoicing::from_index(self.h1_chord_vc.index() as usize),
                ChordVoicing::from_index(self.h2_chord_vc.index() as usize),
            ];
            p.harmony.key = self.key.index() as u8;
            p.sub_active = self.sub.value();
            p.sub_key = self.sub_key.index() as u8;
            p.tune_hz = self.tune.value_f32();
            p.midi_base_channel = self.midi_ch.value_i32().clamp(1, 16) as u8;
            p.input_gain_db = self.input.read();
            p.master_level = self.master.value_f32();
            p.bypass = self.bypass.value();
            p.origin = OriginParams {
                buddies: OriginBuddies::from_index(self.org_buddies.index() as usize),
                formant: self.org_formant.value_i32(),
                level: self.org_level.value_f32(),
                pan: self.org_pan.value_f32(),
                rand_delay: RandDelay::from_index(self.org_rdelay.value_usize()),
            };
            let deg = |on: bool| if on { TemplateDegree::On } else { TemplateDegree::Off };
            let deg_mm = |i: u32| match i {
                1 => TemplateDegree::On,
                2 => TemplateDegree::Minor,
                3 => TemplateDegree::Major,
                _ => TemplateDegree::Off,
            };
            p.harmony.template = [
                deg(self.tpl1.value()), deg(self.tpl2.value()), deg_mm(self.tpl3.index()), deg(self.tpl4.value()),
                deg(self.tpl5.value()), deg(self.tpl6.value()), deg_mm(self.tpl7.index()),
            ];
            p.harmony.flexi_voicing = match self.flexi.index() {
                0 => FlexiVoicing::LllH,
                2 => FlexiVoicing::LHhh,
                _ => FlexiVoicing::LlHh,
            };

            p.parts[0] = PartParams {
                buddies: Buddies::from_index(self.l2_buddies.index() as usize),
                formant: self.l2_formant.value_i32(),
                level: self.l2_level.value_f32(),
                pan: self.l2_pan.value_f32(),
                p_correct: self.l2_pcorr.value_f32(),
                rand_delay: RandDelay::from_index(self.l2_rdelay.value_usize()),
            };
            p.harmony.styles[0] = StyleId::from_index(self.l2_style.index() as usize);
            p.sub_styles[0] = StyleId::from_index(self.l2_sub_style.index() as usize);
            p.harmony.modal_voicing[0] = ModalVoicing::from_index(self.l2_modal.index() as usize);
            p.harmony.priority_rank[0] = self.l2_prio.value_i32().clamp(1, 4) as u8;
            p.parts[1] = PartParams {
                buddies: Buddies::from_index(self.l1_buddies.index() as usize),
                formant: self.l1_formant.value_i32(),
                level: self.l1_level.value_f32(),
                pan: self.l1_pan.value_f32(),
                p_correct: self.l1_pcorr.value_f32(),
                rand_delay: RandDelay::from_index(self.l1_rdelay.value_usize()),
            };
            p.harmony.styles[1] = StyleId::from_index(self.l1_style.index() as usize);
            p.sub_styles[1] = StyleId::from_index(self.l1_sub_style.index() as usize);
            p.harmony.modal_voicing[1] = ModalVoicing::from_index(self.l1_modal.index() as usize);
            p.harmony.priority_rank[1] = self.l1_prio.value_i32().clamp(1, 4) as u8;
            p.parts[2] = PartParams {
                buddies: Buddies::from_index(self.h1_buddies.index() as usize),
                formant: self.h1_formant.value_i32(),
                level: self.h1_level.value_f32(),
                pan: self.h1_pan.value_f32(),
                p_correct: self.h1_pcorr.value_f32(),
                rand_delay: RandDelay::from_index(self.h1_rdelay.value_usize()),
            };
            p.harmony.styles[2] = StyleId::from_index(self.h1_style.index() as usize);
            p.sub_styles[2] = StyleId::from_index(self.h1_sub_style.index() as usize);
            p.harmony.modal_voicing[2] = ModalVoicing::from_index(self.h1_modal.index() as usize);
            p.harmony.priority_rank[2] = self.h1_prio.value_i32().clamp(1, 4) as u8;
            p.parts[3] = PartParams {
                buddies: Buddies::from_index(self.h2_buddies.index() as usize),
                formant: self.h2_formant.value_i32(),
                level: self.h2_level.value_f32(),
                pan: self.h2_pan.value_f32(),
                p_correct: self.h2_pcorr.value_f32(),
                rand_delay: RandDelay::from_index(self.h2_rdelay.value_usize()),
            };
            p.harmony.styles[3] = StyleId::from_index(self.h2_style.index() as usize);
            p.sub_styles[3] = StyleId::from_index(self.h2_sub_style.index() as usize);
            p.harmony.modal_voicing[3] = ModalVoicing::from_index(self.h2_modal.index() as usize);
            p.harmony.priority_rank[3] = self.h2_prio.value_i32().clamp(1, 4) as u8;
            p
        }
    }

    /// Valores de parametro (nome do campo, id, valor "plain") que reproduzem um estado do HV10.
    /// Usado pelos presets de fabrica e pela troca de programa na interface. Parametros GLOBAIS
    /// (Tune, MIDI Ch, Input, Bypass) e os User Styles nao fazem parte de um programa.
    pub fn program_values(params: &ChorusVoxParams, p: &Hv10Params) -> Vec<(&'static str, u32, f64)> {
        use crate::core::{Buddies, OriginBuddies};
        let mode = match p.harmony.mode {
            HarmonyMode::Style => 0,
            HarmonyMode::Modal => 1,
            HarmonyMode::FlexiChord => 2,
            HarmonyMode::MonoChn => 3,
            HarmonyMode::WaitAll => 4,
            HarmonyMode::Priority => 5,
            HarmonyMode::Smart => 6,
            HarmonyMode::Chord => 7,
        };
        let rd = |r: RandDelay| match r {
            RandDelay::Off => 0.0,
            RandDelay::Fixed(n) => n as f64,
            RandDelay::Random(n) => 9.0 + n as f64,
        };
        let bud = |b: Buddies| match b {
            Buddies::Off => 0.0,
            Buddies::On => 1.0,
            Buddies::OnUnison => 2.0,
        };
        let modal = |m: ModalVoicing| match m {
            ModalVoicing::Un => 0.0,
            ModalVoicing::OctLow => 1.0,
            ModalVoicing::OctHigh => 2.0,
            ModalVoicing::Low(n) => 11.0 - n as f64,
            ModalVoicing::High(n) => 10.0 + n as f64,
        };
        let cvc = |c: ChordVoicing| match c {
            ChordVoicing::Un => 0.0,
            ChordVoicing::OctLow => 1.0,
            ChordVoicing::OctHigh => 2.0,
            ChordVoicing::Hi => 3.0,
            ChordVoicing::Lo => 4.0,
        };
        let deg = |d: TemplateDegree| -> f64 { match d {
            TemplateDegree::Off => 0.0,
            TemplateDegree::On => 1.0,
            TemplateDegree::Minor => 2.0,
            TemplateDegree::Major => 3.0,
        } };
        let h = &p.harmony;
        let mut v: Vec<(&'static str, u32, f64)> = Vec::with_capacity(64);
        macro_rules! put {
            ($field:ident, $val:expr) => {
                v.push((stringify!($field), params.$field.id(), $val as f64));
            };
        }
        put!(mode, mode);
        put!(key, h.key);
        put!(sub, if p.sub_active { 1.0 } else { 0.0 });
        put!(sub_key, p.sub_key);
        put!(master, p.master_level);
        put!(org_buddies, match p.origin.buddies {
            OriginBuddies::Off => 0.0,
            OriginBuddies::On => 1.0,
            OriginBuddies::OnUnison => 2.0,
            OriginBuddies::Fx => 3.0,
        });
        put!(org_formant, p.origin.formant);
        put!(org_level, p.origin.level);
        put!(org_pan, p.origin.pan);
        put!(org_rdelay, rd(p.origin.rand_delay));
        macro_rules! part {
            ($i:expr, $b:ident, $st:ident, $sub:ident, $md:ident, $fo:ident, $lv:ident, $pn:ident, $pc:ident, $rdl:ident, $pr:ident, $cv:ident) => {
                put!($b, bud(p.parts[$i].buddies));
                put!($st, h.styles[$i] as u8);
                put!($sub, p.sub_styles[$i] as u8);
                put!($md, modal(h.modal_voicing[$i]));
                put!($fo, p.parts[$i].formant);
                put!($lv, p.parts[$i].level);
                put!($pn, p.parts[$i].pan);
                put!($pc, p.parts[$i].p_correct);
                put!($rdl, rd(p.parts[$i].rand_delay));
                put!($pr, h.priority_rank[$i]);
                put!($cv, cvc(h.chord_voicing[$i]));
            };
        }
        part!(0, l2_buddies, l2_style, l2_sub_style, l2_modal, l2_formant, l2_level, l2_pan, l2_pcorr, l2_rdelay, l2_prio, l2_chord_vc);
        part!(1, l1_buddies, l1_style, l1_sub_style, l1_modal, l1_formant, l1_level, l1_pan, l1_pcorr, l1_rdelay, l1_prio, l1_chord_vc);
        part!(2, h1_buddies, h1_style, h1_sub_style, h1_modal, h1_formant, h1_level, h1_pan, h1_pcorr, h1_rdelay, h1_prio, h1_chord_vc);
        part!(3, h2_buddies, h2_style, h2_sub_style, h2_modal, h2_formant, h2_level, h2_pan, h2_pcorr, h2_rdelay, h2_prio, h2_chord_vc);
        put!(tpl1, deg(h.template[0]).min(1.0));
        put!(tpl2, deg(h.template[1]).min(1.0));
        put!(tpl3, deg(h.template[2]));
        put!(tpl4, deg(h.template[3]).min(1.0));
        put!(tpl5, deg(h.template[4]).min(1.0));
        put!(tpl6, deg(h.template[5]).min(1.0));
        put!(tpl7, deg(h.template[6]));
        put!(flexi, match h.flexi_voicing {
            FlexiVoicing::LllH => 0.0,
            FlexiVoicing::LlHh => 1.0,
            FlexiVoicing::LHhh => 2.0,
        });
        put!(chord, h.chord);
        v
    }

    /// Copia para o nucleo apenas o que mudou no host desde o ultimo bloco, preservando
    /// alteracoes recebidas por MIDI CC (CC7/10/11/12/13/80/81/82) ate o controle ser mexido.
    fn sync_changed(core: &mut Hv10Params, new: &Hv10Params, old: &Hv10Params) {
        macro_rules! sync {
            ($($path:tt)*) => {
                if old.$($path)* != new.$($path)* { core.$($path)* = new.$($path)*.clone(); }
            };
        }
        sync!(harmony.mode);
        sync!(harmony.key);
        sync!(harmony.template);
        sync!(harmony.flexi_voicing);
        sync!(harmony.chord);
        sync!(sub_active);
        sync!(sub_key);
        sync!(tune_hz);
        sync!(input_gain_db);
        sync!(master_level);
        sync!(bypass);
        sync!(midi_base_channel);
        sync!(origin.buddies);
        sync!(origin.formant);
        sync!(origin.level);
        sync!(origin.pan);
        sync!(origin.rand_delay);
        for p in 0..NUM_PARTS {
            sync!(harmony.styles[p]);
            sync!(harmony.modal_voicing[p]);
            sync!(harmony.priority_rank[p]);
            sync!(harmony.chord_voicing[p]);
            sync!(sub_styles[p]);
            sync!(parts[p].buddies);
            sync!(parts[p].formant);
            sync!(parts[p].level);
            sync!(parts[p].pan);
            sync!(parts[p].p_correct);
            sync!(parts[p].rand_delay);
        }
    }

    fn apply_user_styles(bank: &mut StyleBank, steps: &[[i32; 12]; NUM_USER_STYLES]) {
        for (u, row) in steps.iter().enumerate() {
            for (k, &v) in row.iter().enumerate() {
                bank.set_user_step(u, k, if v <= -25 { None } else { Some(v as i8) });
            }
        }
    }

    pub struct ChorusVoxDsp {
        pub core: DecaBuddyProcessor,
        last_host: Option<Hv10Params>,
        mono_scratch: Vec<f32>,
        out_l_scratch: Vec<f32>,
        out_r_scratch: Vec<f32>,
    }

    impl Default for ChorusVoxDsp {
        fn default() -> Self {
            Self {
                core: DecaBuddyProcessor::new(44_100.0),
                last_host: None,
                mono_scratch: vec![0.0; 4096],
                out_l_scratch: vec![0.0; 4096],
                out_r_scratch: vec![0.0; 4096],
            }
        }
    }

    pub struct ChorusVox;

    impl PluginLogic for ChorusVox {
        type Params = ChorusVoxParams;
        type DspState = ChorusVoxDsp;

        fn init(_params: &Self::Params, _cx: &InitContext) -> Self::DspState {
            ChorusVoxDsp::default()
        }

        fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
            state.core.set_sample_rate(config.sample_rate as f32);
            let max_blk = config.max_block_size.max(4096);
            state.mono_scratch.resize(max_blk, 0.0);
            state.out_l_scratch.resize(max_blk, 0.0);
            state.out_r_scratch.resize(max_blk, 0.0);
        }

        fn latency(state: &Self::DspState) -> u32 { state.core.latency_samples() }

        fn process(
            state: &mut Self::DspState,
            params: &Self::Params,
            buffer: &mut AudioBuffer,
            events: &EventList,
            ctx: &mut ProcessContext,
        ) -> ProcessStatus {
            let host = params.to_hv10();
            match &state.last_host {
                Some(old) => sync_changed(&mut state.core.params, &host, old),
                None => state.core.params = host.clone(),
            }
            state.last_host = Some(host);
            apply_user_styles(&mut state.core.styles, &params.user_steps());

            let n = buffer.num_samples();
            if state.mono_scratch.len() < n {
                state.mono_scratch.resize(n, 0.0);
                state.out_l_scratch.resize(n, 0.0);
                state.out_r_scratch.resize(n, 0.0);
            }

            let in_l = buffer.input(0);
            if buffer.num_input_channels() >= 2 {
                let in_r = buffer.input(1);
                for i in 0..n {
                    state.mono_scratch[i] = 0.5 * (in_l[i] + in_r[i]);
                }
            } else {
                state.mono_scratch[..n].copy_from_slice(&in_l[..n]);
            }

            let mut cur = 0usize;
            for event in events.iter() {
                let at = (event.sample_offset as usize).min(n);
                if at > cur {
                    state.core.process_block(
                        &state.mono_scratch[cur..at],
                        &mut state.out_l_scratch[cur..at],
                        &mut state.out_r_scratch[cur..at],
                    );
                    cur = at;
                }
                match &event.body {
                    EventBody::NoteOn { channel, note, velocity, .. } => {
                        state.core.handle_midi_event(0x90 | (*channel & 0x0F), *note, *velocity);
                    }
                    EventBody::NoteOff { channel, note, .. } => {
                        state.core.handle_midi_event(0x80 | (*channel & 0x0F), *note, 0);
                    }
                    EventBody::ControlChange { channel, cc, value, .. } => {
                        state.core.handle_midi_event(0xB0 | (*channel & 0x0F), *cc, *value);
                    }
                    EventBody::ProgramChange { channel, program, .. } => {
                        state.core.handle_midi_event(0xC0 | (*channel & 0x0F), *program, 0);
                    }
                    _ => {}
                }
            }
            if cur < n {
                state.core.process_block(
                    &state.mono_scratch[cur..n],
                    &mut state.out_l_scratch[cur..n],
                    &mut state.out_r_scratch[cur..n],
                );
            }

            if buffer.num_output_channels() >= 2 {
                buffer.output(0)[..n].copy_from_slice(&state.out_l_scratch[..n]);
                buffer.output(1)[..n].copy_from_slice(&state.out_r_scratch[..n]);
            } else if buffer.num_output_channels() == 1 {
                for i in 0..n {
                    buffer.output(0)[i] = 0.5 * (state.out_l_scratch[i] + state.out_r_scratch[i]);
                }
            }

            let peak = |s: &[f32]| s.iter().fold(0.0_f32, |a, &x| a.max(x.abs()));
            ctx.set_meter(P::MeterProgram, state.core.program_token);
            ctx.set_meter(P::MeterIn, peak(&state.mono_scratch[..n]));
            ctx.set_meter(P::MeterOutL, peak(&state.out_l_scratch[..n]));
            if buffer.num_output_channels() >= 2 {
                ctx.set_meter(P::MeterOutR, peak(&state.out_r_scratch[..n]));
            }
            ProcessStatus::Normal
        }

        fn editor(params: Arc<Self::Params>) -> Box<dyn Editor> {
            Box::new(
                truce_egui::EguiEditor::with_ui(params, (crate::gui::WIDTH, crate::gui::HEIGHT), crate::gui::HvGui::default())
                    .with_visuals(crate::gui::visuals())
                    .resizable(false),
            )
        }
    }

    use ChorusVoxParamsParamId as P;

    truce::plugin! { logic: ChorusVox, params: ChorusVoxParams }
}

#[cfg(all(test, feature = "truce-plugin"))]
mod program_roundtrip_tests {
    use super::truce_integration::{program_values, ChorusVoxParams};
    use crate::programs::factory_programs;
    use truce::params::Params;

    /// Programa -> valores de parametro -> parametros do host -> estado do HV10 sem perdas.
    #[test]
    fn every_factory_program_survives_the_parameter_roundtrip() {
        for prog in factory_programs() {
            let params = ChorusVoxParams::default();
            for (_, id, value) in program_values(&params, &prog.params) {
                params.set_plain(id, value);
            }
            let back = params.to_hv10_for_tests();
            let mut expect = prog.params.clone();
            // Globais nao fazem parte do programa
            (expect.tune_hz, expect.midi_base_channel, expect.input_gain_db, expect.bypass) =
                (back.tune_hz, back.midi_base_channel, back.input_gain_db, back.bypass);
            expect.harmony.part_on = back.harmony.part_on;
            assert_eq!(back, expect, "programa {}", prog.display_name());
        }
    }
}
