//! Akai DecaBuddy v1.02 & HV10 — Nucleo do Plugin e Processador de Audio/MIDI.

pub mod dsp;
pub mod scales;

use dsp::{
    gain_db_to_lin, lookahead_samples_for_sr, out_delay_for_sr, pan50_to_gains,
    soft_saturate, voice_db_to_lin, BiquadLowpass, HarmonyChannelDsp, PitchTracker,
    PsolaSubVoice, RING_MASK, RING_SIZE,
};
use scales::{ScaleBank, ScaleId};

#[derive(Clone, Copy, Debug)]
pub struct VstParamSpec {
    pub index: usize,
    pub id: &'static str,
    pub unit: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

pub const NUM_VST_PARAMS: usize = 95;

pub const VST_PARAM_SPECS: [VstParamSpec; NUM_VST_PARAMS] = [
    VstParamSpec { index: 0, id: "preset", unit: "", min: -1.0, max: 3.0, default: -1.0 },
    VstParamSpec { index: 1, id: "key", unit: "", min: 0.0, max: 11.0, default: 0.0 },
    VstParamSpec { index: 2, id: "midi ch", unit: "", min: 1.0, max: 16.0, default: 1.0 },
    VstParamSpec { index: 3, id: "tune", unit: "Hz", min: 427.0, max: 453.0, default: 440.0 },
    VstParamSpec { index: 4, id: "input gain", unit: "dB", min: -15.0, max: 15.0, default: 0.0 },
    VstParamSpec { index: 5, id: "Preset hotkey", unit: "", min: 0.0, max: 3.0, default: 0.0 },
    VstParamSpec { index: 6, id: "direct", unit: "", min: 0.0, max: 1.0, default: 1.0 },
    VstParamSpec { index: 7, id: "direct level", unit: "dB", min: -24.0, max: 0.0, default: 0.0 },
    VstParamSpec { index: 8, id: "direct pan", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 9, id: "dual", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 10, id: "dual random", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 11, id: "dual formant", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 12, id: "dual quantise", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 13, id: "dual level", unit: "dB", min: -24.0, max: 0.0, default: 0.0 },
    VstParamSpec { index: 14, id: "dual pan", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 15, id: "h1", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 16, id: "h1 dual", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 17, id: "h1 scale", unit: "", min: 0.0, max: 36.0, default: 19.0 },
    VstParamSpec { index: 18, id: "h1 random", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 19, id: "h1 formant", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 20, id: "h1 quantise", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 21, id: "h1 level", unit: "dB", min: -24.0, max: 0.0, default: 0.0 },
    VstParamSpec { index: 22, id: "h1 pan", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 23, id: "h2", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 24, id: "h2 dual", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 25, id: "h2 scale", unit: "", min: 0.0, max: 36.0, default: 19.0 },
    VstParamSpec { index: 26, id: "h2 random", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 27, id: "h2 formant", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 28, id: "h2 quantise", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 29, id: "h2 level", unit: "dB", min: -24.0, max: 0.0, default: 0.0 },
    VstParamSpec { index: 30, id: "h2 pan", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 31, id: "h3", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 32, id: "h3 dual", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 33, id: "h3 scale", unit: "", min: 0.0, max: 36.0, default: 19.0 },
    VstParamSpec { index: 34, id: "h3 random", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 35, id: "h3 formant", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 36, id: "h3 quantise", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 37, id: "h3 level", unit: "dB", min: -24.0, max: 0.0, default: 0.0 },
    VstParamSpec { index: 38, id: "h3 pan", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 39, id: "h4", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 40, id: "h4 dual", unit: "", min: 0.0, max: 1.0, default: 0.0 },
    VstParamSpec { index: 41, id: "h4 scale", unit: "", min: 0.0, max: 36.0, default: 19.0 },
    VstParamSpec { index: 42, id: "h4 random", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 43, id: "h4 formant", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 44, id: "h4 quantise", unit: "", min: 0.0, max: 100.0, default: 0.0 },
    VstParamSpec { index: 45, id: "h4 level", unit: "dB", min: -24.0, max: 0.0, default: 0.0 },
    VstParamSpec { index: 46, id: "h4 pan", unit: "", min: -50.0, max: 50.0, default: 0.0 },
    VstParamSpec { index: 47, id: "user1/0", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 48, id: "user1/1", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 49, id: "user1/2", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 50, id: "user1/3", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 51, id: "user1/4", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 52, id: "user1/5", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 53, id: "user1/6", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 54, id: "user1/7", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 55, id: "user1/8", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 56, id: "user1/9", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 57, id: "user1/10", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 58, id: "user1/11", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 59, id: "user2/0", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 60, id: "user2/1", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 61, id: "user2/2", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 62, id: "user2/3", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 63, id: "user2/4", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 64, id: "user2/5", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 65, id: "user2/6", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 66, id: "user2/7", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 67, id: "user2/8", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 68, id: "user2/9", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 69, id: "user2/10", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 70, id: "user2/11", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 71, id: "user3/0", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 72, id: "user3/1", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 73, id: "user3/2", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 74, id: "user3/3", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 75, id: "user3/4", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 76, id: "user3/5", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 77, id: "user3/6", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 78, id: "user3/7", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 79, id: "user3/8", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 80, id: "user3/9", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 81, id: "user3/10", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 82, id: "user3/11", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 83, id: "user4/0", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 84, id: "user4/1", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 85, id: "user4/2", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 86, id: "user4/3", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 87, id: "user4/4", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 88, id: "user4/5", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 89, id: "user4/6", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 90, id: "user4/7", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 91, id: "user4/8", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 92, id: "user4/9", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 93, id: "user4/10", unit: "", min: -128.0, max: 127.0, default: 0.0 },
    VstParamSpec { index: 94, id: "user4/11", unit: "", min: -128.0, max: 127.0, default: 0.0 },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiHarmonyMode {
    AutoChordOrMultiCh,
    MonoChannels,
    WaitAll,
    Priority,
    FlexiChord,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectParams {
    pub enabled: bool,
    pub level_db: f32,
    pub pan: f32,
}

impl Default for DirectParams {
    fn default() -> Self {
        Self {
            enabled: true,
            level_db: 0.0,
            pan: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnisonParams {
    pub enabled: bool,
    pub random: f32,
    pub formant: f32,
    pub quantise: f32,
    pub level_db: f32,
    pub pan: f32,
}

impl Default for UnisonParams {
    fn default() -> Self {
        Self {
            enabled: false,
            random: 0.0,
            formant: 0.0,
            quantise: 0.0,
            level_db: 0.0,
            pan: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HarmonyVoiceParams {
    pub enabled: bool,
    pub dual: bool,
    pub scale: ScaleId,
    pub random: f32,
    pub formant: f32,
    pub quantise: f32,
    pub level_db: f32,
    pub pan: f32,
}

impl Default for HarmonyVoiceParams {
    fn default() -> Self {
        Self {
            enabled: true,
            dual: false,
            scale: ScaleId::Unison,
            random: 25.0,
            formant: 0.0,
            quantise: 30.0,
            level_db: -6.0,
            pan: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DecaBuddyParams {
    pub bypass: bool,
    pub link: bool,
    pub preset: i8,
    pub key: u8,
    pub midi_base_channel: u8,
    pub midi_mode: MidiHarmonyMode,
    pub tune_hz: f32,
    pub input_gain_db: f32,
    pub preset_hotkey: u8,
    pub master_level_db: f32,
    pub direct: DirectParams,
    pub unison: UnisonParams,
    pub harmonies: [HarmonyVoiceParams; 4],
}

impl Default for DecaBuddyParams {
    fn default() -> Self {
        Self {
            bypass: false,
            link: false,
            preset: -1,
            key: 0,
            midi_base_channel: 1,
            midi_mode: MidiHarmonyMode::AutoChordOrMultiCh,
            tune_hz: 440.0,
            input_gain_db: 0.0,
            preset_hotkey: 0,
            master_level_db: 0.0,
            direct: DirectParams::default(),
            unison: UnisonParams::default(),
            harmonies: [
                HarmonyVoiceParams {
                    enabled: true,
                    dual: false,
                    scale: ScaleId::LowOctave,
                    random: 25.0,
                    formant: -15.0,
                    quantise: 35.0,
                    level_db: -6.0,
                    pan: -30.0,
                },
                HarmonyVoiceParams {
                    enabled: false,
                    dual: false,
                    scale: ScaleId::Unison,
                    random: 20.0,
                    formant: 0.0,
                    quantise: 25.0,
                    level_db: -8.0,
                    pan: 10.0,
                },
                HarmonyVoiceParams {
                    enabled: true,
                    dual: true,
                    scale: ScaleId::High3rdModal1,
                    random: 35.0,
                    formant: 10.0,
                    quantise: 35.0,
                    level_db: -7.0,
                    pan: 25.0,
                },
                HarmonyVoiceParams {
                    enabled: true,
                    dual: true,
                    scale: ScaleId::High5thModal1,
                    random: 35.0,
                    formant: 18.0,
                    quantise: 35.0,
                    level_db: -8.0,
                    pan: -20.0,
                },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InputMeterLeds {
    pub led_neg12: bool,
    pub led_neg6: bool,
    pub led_0_over: bool,
}

#[derive(Clone, Copy, Default)]
struct VoiceSlewState {
    gain_smooth: f32,
    gl_smooth: f32,
    gr_smooth: f32,
    was_active: bool,
}

pub struct DecaBuddyProcessor {
    pub sample_rate: f32,
    pub params: DecaBuddyParams,
    pub preset_slots: [DecaBuddyParams; 4],
    pub scale_bank: ScaleBank,
    pub leds: InputMeterLeds,

    hann_lut: Vec<f32>,
    dry_ring: Vec<f32>,
    lp_ring: Vec<f32>,
    mask_ring: Vec<f32>,
    write_pos: usize,
    hop_counter: usize,

    lp_filter: BiquadLowpass,
    pitch: PitchTracker,

    latest_mark: usize,
    voiced_env: f32,

    unison_dsp: PsolaSubVoice,
    harmony_dsp: [HarmonyChannelDsp; 4],

    midi_target_note: [Option<u8>; 4],
    channel_note: [Option<u8>; 4],
    chord_notes_fifo: [u8; 16],
    chord_notes_len: usize,

    harmony_slew: [VoiceSlewState; 4],
    cached_pitch_ratio: [f32; 4],
    cached_formant_ratio: [f32; 4],
    cached_uni_ratio: f32,

    dir_gain_smooth: f32,
    dir_gl_smooth: f32,
    dir_gr_smooth: f32,
    uni_gain_smooth: f32,
    uni_gl_smooth: f32,
    uni_gr_smooth: f32,
    uni_was_active: bool,
    master_gain_smooth: f32,
}

impl DecaBuddyProcessor {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8000.0);
        let mut lp = BiquadLowpass::default();
        lp.set_cutoff(400.0, sr);
        let pitch = PitchTracker::new(sr);
        let lookahead = lookahead_samples_for_sr(sr);
        let out_delay = out_delay_for_sr(sr);
        let init_write = lookahead * 2 + out_delay;
        let default_params = DecaBuddyParams::default();

        Self {
            sample_rate: sr,
            params: default_params.clone(),
            preset_slots: [
                default_params.clone(),
                default_params.clone(),
                default_params.clone(),
                default_params,
            ],
            scale_bank: ScaleBank::default(),
            leds: InputMeterLeds::default(),
            hann_lut: dsp::build_hann_lut(),
            dry_ring: vec![0.0; RING_SIZE],
            lp_ring: vec![0.0; RING_SIZE],
            mask_ring: vec![0.0; RING_SIZE],
            write_pos: init_write,
            hop_counter: 0,
            lp_filter: lp,
            pitch,
            latest_mark: init_write.wrapping_sub(lookahead),
            voiced_env: 0.0,
            unison_dsp: PsolaSubVoice::new(101),
            harmony_dsp: [
                HarmonyChannelDsp::new(201),
                HarmonyChannelDsp::new(301),
                HarmonyChannelDsp::new(401),
                HarmonyChannelDsp::new(501),
            ],
            midi_target_note: [None; 4],
            channel_note: [None; 4],
            chord_notes_fifo: [0; 16],
            chord_notes_len: 0,
            harmony_slew: [VoiceSlewState::default(); 4],
            cached_pitch_ratio: [1.0; 4],
            cached_formant_ratio: [1.0; 4],
            cached_uni_ratio: 1.0,
            dir_gain_smooth: 1.0,
            dir_gl_smooth: 0.707,
            dir_gr_smooth: 0.707,
            uni_gain_smooth: 0.0,
            uni_gl_smooth: 0.707,
            uni_gr_smooth: 0.707,
            uni_was_active: false,
            master_gain_smooth: 1.0,
        }
    }

    pub fn latency_samples(&self) -> u32 {
        (lookahead_samples_for_sr(self.sample_rate) + out_delay_for_sr(self.sample_rate)) as u32
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr.max(8000.0);
        self.lp_filter.set_cutoff(400.0, self.sample_rate);
        self.pitch.set_sample_rate(self.sample_rate);
        self.reset();
    }

    pub fn reset(&mut self) {
        let lookahead = lookahead_samples_for_sr(self.sample_rate);
        let out_delay = out_delay_for_sr(self.sample_rate);
        self.dry_ring.fill(0.0);
        self.lp_ring.fill(0.0);
        self.mask_ring.fill(0.0);
        self.pitch.reset();
        self.write_pos = lookahead * 2 + out_delay;
        self.latest_mark = self.write_pos.wrapping_sub(lookahead);
        self.hop_counter = 0;
        self.voiced_env = 0.0;
        self.unison_dsp.reset();
        for h in &mut self.harmony_dsp {
            h.reset();
        }
        self.midi_target_note = [None; 4];
        self.channel_note = [None; 4];
        self.chord_notes_len = 0;
        for s in &mut self.harmony_slew {
            *s = VoiceSlewState::default();
        }
        self.cached_pitch_ratio = [1.0; 4];
        self.cached_formant_ratio = [1.0; 4];
        self.cached_uni_ratio = 1.0;
        self.dir_gain_smooth = 1.0;
        self.dir_gl_smooth = 0.707;
        self.dir_gr_smooth = 0.707;
        self.uni_gain_smooth = 0.0;
        self.uni_gl_smooth = 0.707;
        self.uni_gr_smooth = 0.707;
        self.uni_was_active = false;
        self.master_gain_smooth = 1.0;
    }

    pub fn save_preset_slot(&mut self, slot: usize) {
        if slot < 4 {
            self.preset_slots[slot] = self.params.clone();
        }
    }

    pub fn recall_preset_slot(&mut self, slot: usize) {
        if slot < 4 {
            self.params = self.preset_slots[slot].clone();
            self.params.preset = slot as i8;
        }
    }

    pub fn detected_pitch(&self) -> (f32, i32) {
        (self.pitch.current_f0, self.pitch.current_note)
    }

    pub fn yin_hop_samples(&self) -> usize {
        self.pitch.yin_hop
    }

    pub fn set_vst_param_real(&mut self, index: usize, real_val: f32) -> bool {
        if index >= NUM_VST_PARAMS {
            return false;
        }
        let spec = &VST_PARAM_SPECS[index];
        let val = real_val.clamp(spec.min, spec.max);
        match index {
            0 => {
                let p = val.round() as i8;
                self.params.preset = p;
                if (0..=3).contains(&p) {
                    self.recall_preset_slot(p as usize);
                }
            }
            1 => self.params.key = val.round() as u8,
            2 => self.params.midi_base_channel = val.round() as u8,
            3 => self.params.tune_hz = val,
            4 => self.params.input_gain_db = val,
            5 => self.params.preset_hotkey = val.round() as u8,
            6 => self.params.direct.enabled = val >= 0.5,
            7 => self.params.direct.level_db = val,
            8 => self.params.direct.pan = val,
            9 => self.params.unison.enabled = val >= 0.5,
            10 => self.params.unison.random = val,
            11 => self.params.unison.formant = val,
            12 => self.params.unison.quantise = val,
            13 => self.params.unison.level_db = val,
            14 => self.params.unison.pan = val,
            15..=46 => {
                let v_idx = (index - 15) / 8;
                let field = (index - 15) % 8;
                let hm = &mut self.params.harmonies[v_idx];
                match field {
                    0 => hm.enabled = val >= 0.5,
                    1 => hm.dual = val >= 0.5,
                    2 => hm.scale = ScaleId::from_index(val.round() as usize),
                    3 => hm.random = val,
                    4 => hm.formant = val,
                    5 => hm.quantise = val,
                    6 => hm.level_db = val,
                    7 => hm.pan = val,
                    _ => unreachable!(),
                }
            }
            47..=94 => {
                let u_idx = (index - 47) / 12;
                let step = (index - 47) % 12;
                self.scale_bank.set_user_step_raw(u_idx, step, val.round() as i8);
            }
            _ => return false,
        }
        true
    }

    pub fn get_vst_param_real(&self, index: usize) -> Option<f32> {
        let val = match index {
            0 => self.params.preset as f32,
            1 => self.params.key as f32,
            2 => self.params.midi_base_channel as f32,
            3 => self.params.tune_hz,
            4 => self.params.input_gain_db,
            5 => self.params.preset_hotkey as f32,
            6 => if self.params.direct.enabled { 1.0 } else { 0.0 },
            7 => self.params.direct.level_db,
            8 => self.params.direct.pan,
            9 => if self.params.unison.enabled { 1.0 } else { 0.0 },
            10 => self.params.unison.random,
            11 => self.params.unison.formant,
            12 => self.params.unison.quantise,
            13 => self.params.unison.level_db,
            14 => self.params.unison.pan,
            15..=46 => {
                let v_idx = (index - 15) / 8;
                let field = (index - 15) % 8;
                let hm = &self.params.harmonies[v_idx];
                match field {
                    0 => if hm.enabled { 1.0 } else { 0.0 },
                    1 => if hm.dual { 1.0 } else { 0.0 },
                    2 => hm.scale as u8 as f32,
                    3 => hm.random,
                    4 => hm.formant,
                    5 => hm.quantise,
                    6 => hm.level_db,
                    7 => hm.pan,
                    _ => unreachable!(),
                }
            }
            47..=94 => {
                let u_idx = (index - 47) / 12;
                let step = (index - 47) % 12;
                self.scale_bank.get_user_step_raw(u_idx, step) as f32
            }
            _ => return None,
        };
        Some(val)
    }

    pub fn set_vst_param_normalized(&mut self, index: usize, norm: f32) -> bool {
        if index >= NUM_VST_PARAMS {
            return false;
        }
        let spec = &VST_PARAM_SPECS[index];
        let real_val = spec.min + norm.clamp(0.0, 1.0) * (spec.max - spec.min);
        self.set_vst_param_real(index, real_val)
    }

    pub fn set_vst_param_by_name(&mut self, name: &str, norm: f32) -> bool {
        if let Some(spec) = VST_PARAM_SPECS.iter().find(|s| s.id == name) {
            return self.set_vst_param_normalized(spec.index, norm);
        }
        false
    }

    pub fn handle_midi_event(&mut self, status: u8, data1: u8, data2: u8) {
        let msg_type = status & 0xF0;
        let ch = status & 0x0F;
        let base_ch = self.params.midi_base_channel.saturating_sub(1) & 0x0F;
        let offset_ch = (ch + 16 - base_ch) & 0x0F;

        match msg_type {
            0x90 if data2 > 0 => {
                if (offset_ch as usize) < 4 {
                    self.channel_note[offset_ch as usize] = Some(data1);
                }
                if ch == base_ch {
                    let mut found = false;
                    for i in 0..self.chord_notes_len {
                        if self.chord_notes_fifo[i] == data1 {
                            found = true;
                            break;
                        }
                    }
                    if !found && self.chord_notes_len < 16 {
                        self.chord_notes_fifo[self.chord_notes_len] = data1;
                        self.chord_notes_len += 1;
                    }
                }
                self.refresh_midi_targets();
            }
            0x80 | 0x90 => {
                if (offset_ch as usize) < 4
                    && self.channel_note[offset_ch as usize] == Some(data1)
                {
                    self.channel_note[offset_ch as usize] = None;
                }
                if ch == base_ch {
                    let mut new_len = 0;
                    for i in 0..self.chord_notes_len {
                        if self.chord_notes_fifo[i] != data1 {
                            self.chord_notes_fifo[new_len] = self.chord_notes_fifo[i];
                            new_len += 1;
                        }
                    }
                    self.chord_notes_len = new_len;
                }
                self.refresh_midi_targets();
            }
            0xB0 => {
                let norm = (data2 as f32) / 127.0;
                match data1 {
                    11 => self.params.master_level_db = -24.0 + norm * 24.0,
                    80 if data2 >= 64 => self.recall_preset_slot(0),
                    81 if data2 >= 64 => self.recall_preset_slot(1),
                    82 => self.params.bypass = data2 >= 64,
                    7 | 10 | 12 | 13 => {
                        if (offset_ch as usize) < 4 {
                            let v = &mut self.params.harmonies[offset_ch as usize];
                            match data1 {
                                7 => v.level_db = -24.0 + norm * 24.0,
                                10 => v.pan = -50.0 + norm * 100.0,
                                12 => v.formant = -50.0 + norm * 100.0,
                                13 => v.quantise = norm * 100.0,
                                _ => {}
                            }
                        }
                    }
                    120 | 123 => {
                        self.channel_note = [None; 4];
                        self.chord_notes_len = 0;
                        self.midi_target_note = [None; 4];
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn refresh_midi_targets(&mut self) {
        let mut active_voices = [0usize; 4];
        let mut num_active = 0;
        for i in 0..4 {
            if self.params.harmonies[i].enabled && self.params.harmonies[i].scale == ScaleId::Midi {
                active_voices[num_active] = i;
                num_active += 1;
            }
        }

        match self.params.midi_mode {
            MidiHarmonyMode::MonoChannels => {
                self.midi_target_note = self.channel_note;
            }
            MidiHarmonyMode::AutoChordOrMultiCh => {
                let has_secondary_ch = self.channel_note[1..4].iter().any(|n| n.is_some());
                if has_secondary_ch {
                    self.midi_target_note = self.channel_note;
                } else {
                    self.midi_target_note = [None; 4];
                    let mut sorted = [0u8; 16];
                    let len = self.chord_notes_len.min(16);
                    sorted[..len].copy_from_slice(&self.chord_notes_fifo[..len]);
                    sorted[..len].sort_unstable();
                    for k in 0..num_active {
                        if k < len {
                            self.midi_target_note[active_voices[k]] = Some(sorted[k]);
                        }
                    }
                }
            }
            MidiHarmonyMode::WaitAll => {
                if self.chord_notes_len >= num_active && num_active > 0 {
                    let mut sorted = [0u8; 16];
                    let len = self.chord_notes_len.min(16);
                    sorted[..len].copy_from_slice(&self.chord_notes_fifo[..len]);
                    sorted[..len].sort_unstable();
                    self.midi_target_note = [None; 4];
                    for k in 0..num_active {
                        self.midi_target_note[active_voices[k]] = Some(sorted[k]);
                    }
                } else {
                    self.midi_target_note = [None; 4];
                }
            }
            MidiHarmonyMode::Priority => {
                self.midi_target_note = [None; 4];
                for k in 0..num_active {
                    if k < self.chord_notes_len {
                        self.midi_target_note[active_voices[k]] = Some(self.chord_notes_fifo[k]);
                    }
                }
            }
            MidiHarmonyMode::FlexiChord => {}
        }
    }

    pub fn process_block(&mut self, input_mono: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = input_mono.len().min(out_l.len()).min(out_r.len());
        if n == 0 {
            return;
        }
        self.refresh_midi_targets();

        let in_gain = gain_db_to_lin(self.params.input_gain_db);
        let target_master_gain = if self.params.bypass {
            1.0
        } else {
            voice_db_to_lin(self.params.master_level_db)
        };
        let lookahead = lookahead_samples_for_sr(self.sample_rate);
        let out_delay = out_delay_for_sr(self.sample_rate);
        let yin_hop = self.pitch.yin_hop;

        let target_dir_gain = if !self.params.bypass && self.params.direct.enabled {
            voice_db_to_lin(self.params.direct.level_db)
        } else if self.params.bypass {
            1.0
        } else {
            0.0
        };
        let (dir_target_gl, dir_target_gr) = pan50_to_gains(self.params.direct.pan);

        let target_uni_gain = if !self.params.bypass && self.params.unison.enabled {
            voice_db_to_lin(self.params.unison.level_db)
        } else {
            0.0
        };
        let (uni_target_gl, uni_target_gr) = pan50_to_gains(self.params.unison.pan);
        let uni_formant_ratio = 2.0_f32.powf((self.params.unison.formant * 0.09) / 12.0);
        let uni_rand_norm = (self.params.unison.random * 0.01).clamp(0.0, 1.0);

        #[derive(Clone, Copy)]
        struct PreVoice {
            enabled: bool,
            dual: bool,
            target_gain: f32,
            target_gl: f32,
            target_gr: f32,
            d_gl: f32,
            d_gr: f32,
            dual_formant_ratio: f32,
            rand_norm: f32,
            base_delay_ms: f32,
        }

        let mut pre_voices = [PreVoice {
            enabled: false,
            dual: false,
            target_gain: 0.0,
            target_gl: 0.0,
            target_gr: 0.0,
            d_gl: 0.0,
            d_gr: 0.0,
            dual_formant_ratio: 1.0,
            rand_norm: 0.0,
            base_delay_ms: 0.0,
        }; 4];

        for v_idx in 0..4 {
            let vp = if self.params.link && v_idx > 0 {
                let mut linked = self.params.harmonies[v_idx];
                linked.random = self.params.harmonies[0].random;
                linked.formant = self.params.harmonies[0].formant;
                linked.quantise = self.params.harmonies[0].quantise;
                linked
            } else {
                self.params.harmonies[v_idx]
            };

            let raw_gain = if !self.params.bypass && vp.enabled {
                voice_db_to_lin(vp.level_db)
            } else {
                0.0
            };
            let (gl, gr) = pan50_to_gains(vp.pan);
            let (d_gl, d_gr) = pan50_to_gains(-vp.pan);
            let dual_formant_ratio = 2.0_f32.powf(((vp.formant * 0.09) + 0.2) / 12.0);
            let rand_norm = (vp.random * 0.01).clamp(0.0, 1.0);
            let base_delay_ms = (v_idx as f32 + 1.0) * 4.5 * rand_norm;
            let effective_gain = if vp.dual { raw_gain * 0.75 } else { raw_gain };

            pre_voices[v_idx] = PreVoice {
                enabled: vp.enabled,
                dual: vp.dual,
                target_gain: effective_gain,
                target_gl: gl,
                target_gr: gr,
                d_gl,
                d_gr,
                dual_formant_ratio,
                rand_norm,
                base_delay_ms,
            };
        }

        let mut peak = 0.0_f32;

        for i in 0..n {
            let x = input_mono[i] * in_gain;
            peak = peak.max(x.abs());

            let w_idx = self.write_pos & RING_MASK;
            self.dry_ring[w_idx] = x;
            let lp_x = self.lp_filter.process(x);
            self.lp_ring[w_idx] = lp_x;

            self.hop_counter += 1;
            if self.hop_counter >= yin_hop {
                self.hop_counter = 0;
                let was_voiced = self.pitch.voiced;
                self.pitch.analyze_frame(
                    &self.dry_ring,
                    self.write_pos,
                    self.sample_rate,
                    self.params.tune_hz,
                );
                if self.pitch.voiced {
                    let cutoff = (self.pitch.current_f0 * 2.2).clamp(250.0, 1500.0);
                    self.lp_filter.set_cutoff(cutoff, self.sample_rate);

                    // Atualiza razoes de pitch e formantes estritamente na taxa de controle
                    let uni_q_norm = (self.params.unison.quantise * 0.01).clamp(0.0, 1.0);
                    let dev = self.pitch.current_midi - (self.pitch.current_note as f32);
                    self.cached_uni_ratio = 2.0_f32.powf((-uni_q_norm * dev) / 12.0);

                    for v_idx in 0..4 {
                        let vp = if self.params.link && v_idx > 0 {
                            self.params.harmonies[0]
                        } else {
                            self.params.harmonies[v_idx]
                        };
                        self.cached_formant_ratio[v_idx] = 2.0_f32.powf((vp.formant * 0.09) / 12.0);
                        self.cached_pitch_ratio[v_idx] = if let Some(shift) =
                            self.compute_voice_shift_semitones(v_idx, &vp)
                        {
                            2.0_f32.powf(shift / 12.0)
                        } else {
                            1.0
                        };
                    }

                    if !was_voiced {
                        let max_t0 = (600.0 * (self.sample_rate / 44_100.0)).round() as usize;
                        let min_t0 = (20.0 * (self.sample_rate / 44_100.0)).round() as usize;
                        let t0 = (self.sample_rate / self.pitch.current_f0)
                            .clamp(min_t0 as f32, max_t0 as f32) as usize;
                        let r = (t0 / 2).max(2);
                        let center = self.write_pos.wrapping_sub(lookahead);
                        let mut best_p = center;
                        let mut best_v = -1e9_f32;
                        for k in 0..(2 * r + 1) {
                            let p = center.wrapping_sub(r).wrapping_add(k);
                            let v = self.lp_ring[p & RING_MASK];
                            if v > best_v {
                                best_v = v;
                                best_p = p;
                            }
                        }
                        self.latest_mark = best_p;
                    }
                }
            }

            let target_mask = if self.pitch.voiced { 1.0 } else { 0.0 };
            self.voiced_env = 0.992 * self.voiced_env + 0.008 * target_mask;

            let read_pos = self.write_pos.wrapping_sub(lookahead);
            self.mask_ring[read_pos & RING_MASK] = self.voiced_env;
            let emit_pos = read_pos.wrapping_sub(out_delay);
            let emit_mask = self.mask_ring[emit_pos & RING_MASK];
            let dry_delayed = self.dry_ring[emit_pos & RING_MASK];

            self.update_epoch_marks(read_pos);

            self.dir_gain_smooth = 0.995 * self.dir_gain_smooth + 0.005 * target_dir_gain;
            self.dir_gl_smooth = 0.995 * self.dir_gl_smooth + 0.005 * dir_target_gl;
            self.dir_gr_smooth = 0.995 * self.dir_gr_smooth + 0.005 * dir_target_gr;

            self.uni_gain_smooth = 0.995 * self.uni_gain_smooth + 0.005 * target_uni_gain;
            self.uni_gl_smooth = 0.995 * self.uni_gl_smooth + 0.005 * uni_target_gl;
            self.uni_gr_smooth = 0.995 * self.uni_gr_smooth + 0.005 * uni_target_gr;

            self.master_gain_smooth = 0.995 * self.master_gain_smooth + 0.005 * target_master_gain;

            let mut mix_l = dry_delayed * (self.dir_gain_smooth * self.dir_gl_smooth);
            let mut mix_r = dry_delayed * (self.dir_gain_smooth * self.dir_gr_smooth);

            // Unison com pulo de DSP quando ocioso
            let uni_active = self.params.unison.enabled || self.uni_gain_smooth > 1e-4;
            if uni_active {
                self.uni_was_active = true;
                let y_u = self.unison_dsp.step_sample(
                    &self.dry_ring,
                    &self.hann_lut,
                    read_pos,
                    emit_pos,
                    self.latest_mark,
                    self.voiced_env,
                    emit_mask,
                    self.pitch.current_f0,
                    self.cached_uni_ratio,
                    uni_formant_ratio,
                    uni_rand_norm,
                    0.0,
                    11.0 * uni_rand_norm,
                    0.35,
                    self.sample_rate,
                );
                mix_l += y_u * (self.uni_gain_smooth * self.uni_gl_smooth);
                mix_r += y_u * (self.uni_gain_smooth * self.uni_gr_smooth);
            } else if self.uni_was_active {
                self.unison_dsp.reset();
                self.uni_was_active = false;
            }

            // 4 Harmonias com execucao estritamente ativa (sem chamada a reset() a cada amostra)
            for v_idx in 0..4 {
                let pv = pre_voices[v_idx];
                let s = &mut self.harmony_slew[v_idx];

                let target_voice_gain = if pv.enabled && self.pitch.voiced && self.pitch.current_note >= 0 {
                    pv.target_gain
                } else {
                    0.0
                };

                s.gain_smooth = 0.995 * s.gain_smooth + 0.005 * target_voice_gain;
                s.gl_smooth = 0.995 * s.gl_smooth + 0.005 * pv.target_gl;
                s.gr_smooth = 0.995 * s.gr_smooth + 0.005 * pv.target_gr;

                let voice_active = pv.enabled || s.gain_smooth > 1e-4;
                if !voice_active {
                    if s.was_active {
                        self.harmony_dsp[v_idx].reset();
                        s.was_active = false;
                    }
                    continue;
                }
                s.was_active = true;

                let pitch_ratio = self.cached_pitch_ratio[v_idx];
                let formant_ratio = self.cached_formant_ratio[v_idx];

                let y_main = self.harmony_dsp[v_idx].main.step_sample(
                    &self.dry_ring,
                    &self.hann_lut,
                    read_pos,
                    emit_pos,
                    self.latest_mark,
                    self.voiced_env,
                    emit_mask,
                    self.pitch.current_f0,
                    pitch_ratio,
                    formant_ratio,
                    pv.rand_norm,
                    0.0,
                    pv.base_delay_ms,
                    0.0,
                    self.sample_rate,
                );

                if pv.dual {
                    let y_dual = self.harmony_dsp[v_idx].dual.step_sample(
                        &self.dry_ring,
                        &self.hann_lut,
                        read_pos,
                        emit_pos,
                        self.latest_mark,
                        self.voiced_env,
                        emit_mask,
                        self.pitch.current_f0,
                        pitch_ratio,
                        pv.dual_formant_ratio,
                        pv.rand_norm,
                        -9.0,
                        pv.base_delay_ms + 14.0,
                        0.0,
                        self.sample_rate,
                    );
                    mix_l += (y_main * s.gl_smooth + y_dual * pv.d_gl) * s.gain_smooth;
                    mix_r += (y_main * s.gr_smooth + y_dual * pv.d_gr) * s.gain_smooth;
                } else {
                    mix_l += y_main * (s.gain_smooth * s.gl_smooth);
                    mix_r += y_main * (s.gain_smooth * s.gr_smooth);
                }
            }

            out_l[i] = soft_saturate(mix_l * self.master_gain_smooth);
            out_r[i] = soft_saturate(mix_r * self.master_gain_smooth);

            self.write_pos = self.write_pos.wrapping_add(1);
        }

        self.leds.led_neg12 = peak >= 0.251;
        self.leds.led_neg6 = peak >= 0.501;
        self.leds.led_0_over = peak >= 0.980;
    }

    #[inline]
    fn update_epoch_marks(&mut self, read_pos: usize) {
        if !self.pitch.voiced || self.pitch.current_f0 <= 50.0 {
            self.latest_mark = read_pos;
            return;
        }

        let max_t0 = (600.0 * (self.sample_rate / 44_100.0)).round() as usize;
        let min_t0 = (20.0 * (self.sample_rate / 44_100.0)).round() as usize;
        let t0 = (self.sample_rate / self.pitch.current_f0).clamp(min_t0 as f32, max_t0 as f32) as usize;
        let elapsed = read_pos.wrapping_sub(self.latest_mark);
        if elapsed > RING_SIZE / 2 {
            return;
        }

        if elapsed >= t0 {
            let pred = self.latest_mark.wrapping_add(t0);
            let radius = ((t0 * 3) / 10).max(2);
            let start = pred.wrapping_sub(radius);
            let span = 2 * radius + 1;

            let mut best_pos = pred;
            let mut best_val = -1e9_f32;
            for k in 0..span {
                let p = start.wrapping_add(k);
                let v = self.lp_ring[p & RING_MASK];
                if v > best_val {
                    best_val = v;
                    best_pos = p;
                }
            }
            self.latest_mark = best_pos;
        }
    }

    fn compute_voice_shift_semitones(&self, v_idx: usize, vp: &HarmonyVoiceParams) -> Option<f32> {
        if self.pitch.current_note < 0 {
            return None;
        }
        let q_norm = (vp.quantise * 0.01).clamp(0.0, 1.0);
        let note = self.pitch.current_note;
        let dev = self.pitch.current_midi - (note as f32);

        if vp.scale == ScaleId::Midi {
            if self.params.midi_mode == MidiHarmonyMode::FlexiChord {
                let shift = self.flexichord_shift(v_idx, note)?;
                return Some((shift as f32) - q_norm * dev);
            }
            let target_midi = self.midi_target_note[v_idx]?;
            let raw_diff = (target_midi as i32 - note) as f32;
            return Some(raw_diff - q_norm * dev);
        }

        let key = (self.params.key % 12) as i32;
        let root_offset = vp.scale.root_pc() as i32;
        let pc = (note - key - root_offset).rem_euclid(12) as usize;
        let interval = self.scale_bank.lookup_offset(vp.scale, v_idx, pc)? as f32;
        Some(interval - q_norm * dev)
    }

    fn flexichord_shift(&self, v_idx: usize, input_note: i32) -> Option<i32> {
        if self.chord_notes_len < 3 {
            return None;
        }
        let mut pcs = [0i32; 16];
        let len = self.chord_notes_len.min(16);
        for i in 0..len {
            pcs[i] = (self.chord_notes_fifo[i] as i32).rem_euclid(12);
        }
        pcs[..len].sort_unstable();

        let in_pc = input_note.rem_euclid(12);
        match v_idx {
            0 => {
                let mut found = 0;
                for step in 1..=12 {
                    let cpc = (in_pc - step).rem_euclid(12);
                    if pcs[..len].contains(&cpc) {
                        found += 1;
                        if found == 2 {
                            return Some(-step);
                        }
                    }
                }
                None
            }
            1 => {
                for step in 1..=12 {
                    let cpc = (in_pc - step).rem_euclid(12);
                    if pcs[..len].contains(&cpc) {
                        return Some(-step);
                    }
                }
                None
            }
            2 => {
                for step in 1..=12 {
                    let cpc = (in_pc + step).rem_euclid(12);
                    if pcs[..len].contains(&cpc) {
                        return Some(step);
                    }
                }
                None
            }
            _ => {
                let mut found = 0;
                for step in 1..=12 {
                    let cpc = (in_pc + step).rem_euclid(12);
                    if pcs[..len].contains(&cpc) {
                        found += 1;
                        if found == 2 {
                            return Some(step);
                        }
                    }
                }
                None
            }
        }
    }
}

#[cfg(feature = "truce-plugin")]
pub mod truce_integration {
    use super::*;
    use std::sync::Arc;
    use truce::prelude::*;
    use truce_gui::IntoLayoutEditor;
    use truce_gui_types::layout::{dropdown, knob, meter, section, toggle, GridLayout};

    #[derive(ParamEnum, Debug)]
    pub enum KeyChoice {
        #[name = "C"] C, #[name = "C#"] Cs, #[name = "D"] D, #[name = "D#"] Ds,
        #[name = "E"] E, #[name = "F"] F, #[name = "F#"] Fs, #[name = "G"] G,
        #[name = "G#"] Gs, #[name = "A"] A, #[name = "A#"] As, #[name = "B"] B,
    }

    #[derive(ParamEnum, Debug)]
    pub enum MidiModeChoice {
        #[name = "Auto Ch"] AutoChordOrMultiCh,
        #[name = "Mono (1-4)"] MonoChannels,
        #[name = "Wait All"] WaitAll,
        #[name = "Priority"] Priority,
        #[name = "FlexiChd"] FlexiChord,
    }

    #[derive(ParamEnum, Debug)]
    pub enum ScaleChoice {
        #[name = "MIDI"] Midi,
        #[name = "User"] User,
        #[name = "Low Octave"] LowOctave,
        #[name = "Low 3rd | Mod 1"] Low3rdModal1,
        #[name = "Low 3rd | Mod 2"] Low3rdModal2,
        #[name = "Low 3rd | Mod 3"] Low3rdModal3,
        #[name = "Low 3rd | Mod 4"] Low3rdModal4,
        #[name = "Low 3rd | Mod 5"] Low3rdModal5,
        #[name = "Low 3rd | Har Min"] Low3rdHarmonicMinor,
        #[name = "Low 3rd | Mel Min"] Low3rdMelodicMinor,
        #[name = "Low 5th | Mod 1"] Low5thModal1,
        #[name = "Low 5th | Mod 2"] Low5thModal2,
        #[name = "Low 5th | Mod 3"] Low5thModal3,
        #[name = "Low 5th | Mod 4"] Low5thModal4,
        #[name = "Low 5th | Mod 5"] Low5thModal5,
        #[name = "Low 5th | Har Min"] Low5thHarmonicMinor,
        #[name = "Low 5th | Mel Min"] Low5thMelodicMinor,
        #[name = "Low 7th | Major"] Low7thMajor,
        #[name = "Low 7th | Minor"] Low7thMinor,
        #[name = "Unison"] Unison,
        #[name = "High 3rd | Mod 1"] High3rdModal1,
        #[name = "High 3rd | Mod 2"] High3rdModal2,
        #[name = "High 3rd | Mod 3"] High3rdModal3,
        #[name = "High 3rd | Mod 4"] High3rdModal4,
        #[name = "High 3rd | Mod 5"] High3rdModal5,
        #[name = "High 3rd | Har Min"] High3rdHarmonicMinor,
        #[name = "High 3rd | Mel Min"] High3rdMelodicMinor,
        #[name = "High 5th | Mod 1"] High5thModal1,
        #[name = "High 5th | Mod 2"] High5thModal2,
        #[name = "High 5th | Mod 3"] High5thModal3,
        #[name = "High 5th | Mod 4"] High5thModal4,
        #[name = "High 5th | Mod 5"] High5thModal5,
        #[name = "High 5th | Har Min"] High5thHarmonicMinor,
        #[name = "High 5th | Mel Min"] High5thMelodicMinor,
        #[name = "High 7th | Major"] High7thMajor,
        #[name = "High 7th | Minor"] High7thMinor,
        #[name = "High Octave"] HighOctave,
    }

    #[derive(Params)]
    pub struct ChorusVoxParams {
        #[param(name = "Key", default = 0)]
        pub key: EnumParam<KeyChoice>,
        #[param(name = "MIDI Mode", default = 0)]
        pub midi_mode: EnumParam<MidiModeChoice>,
        #[param(name = "MIDI Ch", range = "linear(1, 16)", default = 1)]
        pub midi_ch: IntParam,
        #[param(name = "Tune", range = "linear(427, 453)", default = 440.0, unit = "Hz")]
        pub tune: FloatParam,
        #[param(name = "In Gain", range = "linear(-15, 15)", default = 0.0, unit = "dB")]
        pub input_gain: FloatParam,
        #[param(name = "Link", default = false)]
        pub link: BoolParam,

        #[param(name = "Direct", default = true)]
        pub direct_on: BoolParam,
        #[param(name = "Dir Lev", range = "linear(-24, 0)", default = 0.0, unit = "dB")]
        pub direct_level: FloatParam,
        #[param(name = "Dir Pan", range = "linear(-50, 50)", default = 0.0)]
        pub direct_pan: FloatParam,

        #[param(name = "Unison", default = false)]
        pub unison_on: BoolParam,
        #[param(name = "U.Rand", range = "linear(0, 100)", default = 0.0)]
        pub unison_random: FloatParam,
        #[param(name = "U.Form", range = "linear(-50, 50)", default = 0.0)]
        pub unison_formant: FloatParam,
        #[param(name = "U.Quant", range = "linear(0, 100)", default = 0.0)]
        pub unison_quantise: FloatParam,
        #[param(name = "U.Level", range = "linear(-24, 0)", default = 0.0, unit = "dB")]
        pub unison_level: FloatParam,
        #[param(name = "U.Pan", range = "linear(-50, 50)", default = 0.0)]
        pub unison_pan: FloatParam,

        // Harmony 1
        #[param(name = "H1 Scale", default = 2)]
        pub h1_scale: EnumParam<ScaleChoice>,
        #[param(name = "H1 On", default = true)]
        pub h1_on: BoolParam,
        #[param(name = "H1 Dual", default = false)]
        pub h1_dual: BoolParam,
        #[param(name = "H1 Rand", range = "linear(0, 100)", default = 25.0)]
        pub h1_random: FloatParam,
        #[param(name = "H1 Form", range = "linear(-50, 50)", default = -15.0)]
        pub h1_formant: FloatParam,
        #[param(name = "H1 Quant", range = "linear(0, 100)", default = 35.0)]
        pub h1_quantise: FloatParam,
        #[param(name = "H1 Level", range = "linear(-24, 0)", default = -6.0, unit = "dB")]
        pub h1_level: FloatParam,
        #[param(name = "H1 Pan", range = "linear(-50, 50)", default = -30.0)]
        pub h1_pan: FloatParam,

        // Harmony 2
        #[param(name = "H2 Scale", default = 19)]
        pub h2_scale: EnumParam<ScaleChoice>,
        #[param(name = "H2 On", default = false)]
        pub h2_on: BoolParam,
        #[param(name = "H2 Dual", default = false)]
        pub h2_dual: BoolParam,
        #[param(name = "H2 Rand", range = "linear(0, 100)", default = 20.0)]
        pub h2_random: FloatParam,
        #[param(name = "H2 Form", range = "linear(-50, 50)", default = 0.0)]
        pub h2_formant: FloatParam,
        #[param(name = "H2 Quant", range = "linear(0, 100)", default = 25.0)]
        pub h2_quantise: FloatParam,
        #[param(name = "H2 Level", range = "linear(-24, 0)", default = -8.0, unit = "dB")]
        pub h2_level: FloatParam,
        #[param(name = "H2 Pan", range = "linear(-50, 50)", default = 10.0)]
        pub h2_pan: FloatParam,

        // Harmony 3
        #[param(name = "H3 Scale", default = 20)]
        pub h3_scale: EnumParam<ScaleChoice>,
        #[param(name = "H3 On", default = true)]
        pub h3_on: BoolParam,
        #[param(name = "H3 Dual", default = true)]
        pub h3_dual: BoolParam,
        #[param(name = "H3 Rand", range = "linear(0, 100)", default = 35.0)]
        pub h3_random: FloatParam,
        #[param(name = "H3 Form", range = "linear(-50, 50)", default = 10.0)]
        pub h3_formant: FloatParam,
        #[param(name = "H3 Quant", range = "linear(0, 100)", default = 35.0)]
        pub h3_quantise: FloatParam,
        #[param(name = "H3 Level", range = "linear(-24, 0)", default = -7.0, unit = "dB")]
        pub h3_level: FloatParam,
        #[param(name = "H3 Pan", range = "linear(-50, 50)", default = 25.0)]
        pub h3_pan: FloatParam,

        // Harmony 4
        #[param(name = "H4 Scale", default = 27)]
        pub h4_scale: EnumParam<ScaleChoice>,
        #[param(name = "H4 On", default = true)]
        pub h4_on: BoolParam,
        #[param(name = "H4 Dual", default = true)]
        pub h4_dual: BoolParam,
        #[param(name = "H4 Rand", range = "linear(0, 100)", default = 35.0)]
        pub h4_random: FloatParam,
        #[param(name = "H4 Form", range = "linear(-50, 50)", default = 18.0)]
        pub h4_formant: FloatParam,
        #[param(name = "H4 Quant", range = "linear(0, 100)", default = 35.0)]
        pub h4_quantise: FloatParam,
        #[param(name = "H4 Level", range = "linear(-24, 0)", default = -8.0, unit = "dB")]
        pub h4_level: FloatParam,
        #[param(name = "H4 Pan", range = "linear(-50, 50)", default = -20.0)]
        pub h4_pan: FloatParam,

        #[meter]
        pub meter_in: MeterSlot,
        #[meter]
        pub meter_out_l: MeterSlot,
        #[meter]
        pub meter_out_r: MeterSlot,
    }

    use ChorusVoxParamsParamId as P;

    pub struct ChorusVoxDsp {
        pub core: DecaBuddyProcessor,
        pub mono_scratch: Vec<f32>,
        pub out_l_scratch: Vec<f32>,
        pub out_r_scratch: Vec<f32>,
    }

    impl Default for ChorusVoxDsp {
        fn default() -> Self {
            Self {
                core: DecaBuddyProcessor::new(44_100.0),
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

        fn latency(state: &Self::DspState) -> u32 {
            state.core.latency_samples()
        }

        fn process(
            state: &mut Self::DspState,
            params: &Self::Params,
            buffer: &mut AudioBuffer,
            events: &EventList,
            ctx: &mut ProcessContext,
        ) -> ProcessStatus {
            let p = &mut state.core.params;
            p.key = params.key.value() as u8;
            p.midi_mode = match params.midi_mode.value() {
                MidiModeChoice::AutoChordOrMultiCh => MidiHarmonyMode::AutoChordOrMultiCh,
                MidiModeChoice::MonoChannels => MidiHarmonyMode::MonoChannels,
                MidiModeChoice::WaitAll => MidiHarmonyMode::WaitAll,
                MidiModeChoice::Priority => MidiHarmonyMode::Priority,
                MidiModeChoice::FlexiChord => MidiHarmonyMode::FlexiChord,
            };
            p.midi_base_channel = params.midi_ch.value() as u8;
            p.tune_hz = params.tune.read();
            p.input_gain_db = params.input_gain.read();
            p.link = params.link.value();

            p.direct.enabled = params.direct_on.value();
            p.direct.level_db = params.direct_level.read();
            p.direct.pan = params.direct_pan.read();

            p.unison.enabled = params.unison_on.value();
            p.unison.random = params.unison_random.read();
            p.unison.formant = params.unison_formant.read();
            p.unison.quantise = params.unison_quantise.read();
            p.unison.level_db = params.unison_level.read();
            p.unison.pan = params.unison_pan.read();

            p.harmonies[0].scale = ScaleId::from_index(params.h1_scale.value() as usize);
            p.harmonies[0].enabled = params.h1_on.value();
            p.harmonies[0].dual = params.h1_dual.value();
            p.harmonies[0].random = params.h1_random.read();
            p.harmonies[0].formant = params.h1_formant.read();
            p.harmonies[0].quantise = params.h1_quantise.read();
            p.harmonies[0].level_db = params.h1_level.read();
            p.harmonies[0].pan = params.h1_pan.read();

            p.harmonies[1].scale = ScaleId::from_index(params.h2_scale.value() as usize);
            p.harmonies[1].enabled = params.h2_on.value();
            p.harmonies[1].dual = params.h2_dual.value();
            p.harmonies[1].random = params.h2_random.read();
            p.harmonies[1].formant = params.h2_formant.read();
            p.harmonies[1].quantise = params.h2_quantise.read();
            p.harmonies[1].level_db = params.h2_level.read();
            p.harmonies[1].pan = params.h2_pan.read();

            p.harmonies[2].scale = ScaleId::from_index(params.h3_scale.value() as usize);
            p.harmonies[2].enabled = params.h3_on.value();
            p.harmonies[2].dual = params.h3_dual.value();
            p.harmonies[2].random = params.h3_random.read();
            p.harmonies[2].formant = params.h3_formant.read();
            p.harmonies[2].quantise = params.h3_quantise.read();
            p.harmonies[2].level_db = params.h3_level.read();
            p.harmonies[2].pan = params.h3_pan.read();

            p.harmonies[3].scale = ScaleId::from_index(params.h4_scale.value() as usize);
            p.harmonies[3].enabled = params.h4_on.value();
            p.harmonies[3].dual = params.h4_dual.value();
            p.harmonies[3].random = params.h4_random.read();
            p.harmonies[3].formant = params.h4_formant.read();
            p.harmonies[3].quantise = params.h4_quantise.read();
            p.harmonies[3].level_db = params.h4_level.read();
            p.harmonies[3].pan = params.h4_pan.read();

            for event in events.iter() {
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
                    _ => {}
                }
            }

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

            state.core.process_block(
                &state.mono_scratch[..n],
                &mut state.out_l_scratch[..n],
                &mut state.out_r_scratch[..n],
            );

            if buffer.num_output_channels() >= 2 {
                buffer.output(0)[..n].copy_from_slice(&state.out_l_scratch[..n]);
                buffer.output(1)[..n].copy_from_slice(&state.out_r_scratch[..n]);
            } else if buffer.num_output_channels() == 1 {
                for i in 0..n {
                    buffer.output(0)[i] = 0.5 * (state.out_l_scratch[i] + state.out_r_scratch[i]);
                }
            }

            let in_pk = state.mono_scratch[..n].iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            let out_l_pk = state.out_l_scratch[..n].iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            let out_r_pk = state.out_r_scratch[..n].iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));

            ctx.set_meter(P::MeterIn, in_pk);
            ctx.set_meter(P::MeterOutL, out_l_pk);
            if buffer.num_output_channels() >= 2 {
                ctx.set_meter(P::MeterOutR, out_r_pk);
            }

            ProcessStatus::Normal
        }

        /// Interface vertical (1 voz por linha), tamanho estritamente fixo e versao na barra superior.
        fn editor(params: Arc<Self::Params>) -> Box<dyn Editor> {
            GridLayout::build(vec![
                section(
                    "GLOBAL & INPUT",
                    vec![
                        dropdown(P::Key, "Key"),
                        dropdown(P::MidiMode, "MIDI Mode"),
                        knob(P::MidiCh, "MIDI Ch"),
                        knob(P::Tune, "Tune Hz"),
                        knob(P::InputGain, "In Gain"),
                        meter(&[P::MeterIn], "In"),
                    ],
                ),
                section(
                    "DIRECT & UNISON",
                    vec![
                        toggle(P::DirectOn, "Direct"),
                        knob(P::DirectLevel, "Dir Lev"),
                        knob(P::DirectPan, "Dir Pan"),
                        toggle(P::Link, "Link"),
                        toggle(P::UnisonOn, "Unison"),
                        knob(P::UnisonRandom, "U.Rand"),
                        knob(P::UnisonFormant, "U.Form"),
                        knob(P::UnisonQuantise, "U.Quant"),
                        knob(P::UnisonLevel, "U.Level"),
                        knob(P::UnisonPan, "U.Pan"),
                        meter(&[P::MeterOutL, P::MeterOutR], "Out"),
                    ],
                ),
                section(
                    "VOX 1  •  HARMONY 1",
                    vec![
                        toggle(P::H1On, "On"),
                        toggle(P::H1Dual, "Dual"),
                        knob(P::H1Scale, "Scale").cols(2),
                        knob(P::H1Random, "Random"),
                        knob(P::H1Formant, "Formant"),
                        knob(P::H1Quantise, "Quantise"),
                        knob(P::H1Level, "Level"),
                        knob(P::H1Pan, "Pan"),
                    ],
                ),
                section(
                    "VOX 2  •  HARMONY 2",
                    vec![
                        toggle(P::H2On, "On"),
                        toggle(P::H2Dual, "Dual"),
                        knob(P::H2Scale, "Scale").cols(2),
                        knob(P::H2Random, "Random"),
                        knob(P::H2Formant, "Formant"),
                        knob(P::H2Quantise, "Quantise"),
                        knob(P::H2Level, "Level"),
                        knob(P::H2Pan, "Pan"),
                    ],
                ),
                section(
                    "VOX 3  •  HARMONY 3",
                    vec![
                        toggle(P::H3On, "On"),
                        toggle(P::H3Dual, "Dual"),
                        knob(P::H3Scale, "Scale").cols(2),
                        knob(P::H3Random, "Random"),
                        knob(P::H3Formant, "Formant"),
                        knob(P::H3Quantise, "Quantise"),
                        knob(P::H3Level, "Level"),
                        knob(P::H3Pan, "Pan"),
                    ],
                ),
                section(
                    "VOX 4  •  HARMONY 4",
                    vec![
                        toggle(P::H4On, "On"),
                        toggle(P::H4Dual, "Dual"),
                        knob(P::H4Scale, "Scale").cols(2),
                        knob(P::H4Random, "Random"),
                        knob(P::H4Formant, "Formant"),
                        knob(P::H4Quantise, "Quantise"),
                        knob(P::H4Level, "Level"),
                        knob(P::H4Pan, "Pan"),
                    ],
                ),
            ])
            .with_title("CHORUSVOX DECABUDDY                                                            v0.2.0")
            .into_editor(&params)
        }
    }

    truce::plugin! {
        logic: ChorusVox,
        params: ChorusVoxParams,
    }
}
