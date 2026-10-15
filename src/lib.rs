//! ChorusVox DecaBuddy — Nucleo do Plugin e Processador de Audio/MIDI.

pub mod dsp;
pub mod scales;

use dsp::{
    analog_bus_soft_limit, gain_db_to_lin, live_lookahead_for_sr, live_out_delay_for_sr,
    lookahead_samples_for_sr, max_grain_for_sr, out_delay_for_sr, pan50_to_gains, voice_db_to_lin,
    BiquadLowpass, ChoirAnalogBus, HarmonyChannelDsp, PitchTracker, PsolaSubVoice, RelayBypassModel,
    ScopedFtzDaz, SibilanceDetector, RING_MASK, RING_SIZE,
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

pub const NUM_VST_PARAMS: usize = 96;

const fn sp(index: usize, id: &'static str, unit: &'static str, min: f32, max: f32, default: f32) -> VstParamSpec {
    VstParamSpec { index, id, unit, min, max, default }
}

pub const VST_PARAM_SPECS: [VstParamSpec; NUM_VST_PARAMS] = [
    sp(0, "preset", "", -1.0, 3.0, -1.0), sp(1, "key", "", 0.0, 11.0, 0.0),
    sp(2, "midi ch", "", 1.0, 16.0, 1.0), sp(3, "tune", "Hz", 427.0, 453.0, 440.0),
    sp(4, "input gain", "dB", -15.0, 15.0, 0.0), sp(5, "Preset hotkey", "", 0.0, 3.0, 0.0),
    sp(6, "direct", "", 0.0, 1.0, 1.0), sp(7, "direct level", "dB", -24.0, 0.0, 0.0),
    sp(8, "direct pan", "", -50.0, 50.0, 0.0), sp(9, "dual", "", 0.0, 1.0, 0.0),
    sp(10, "dual random", "", 0.0, 100.0, 0.0), sp(11, "dual formant", "", -50.0, 50.0, 0.0),
    sp(12, "dual quantise", "", 0.0, 100.0, 0.0), sp(13, "dual level", "dB", -24.0, 0.0, 0.0),
    sp(14, "dual pan", "", -50.0, 50.0, 0.0),
    sp(15, "h1", "", 0.0, 1.0, 0.0), sp(16, "h1 dual", "", 0.0, 1.0, 0.0),
    sp(17, "h1 scale", "", 0.0, 36.0, 19.0), sp(18, "h1 random", "", 0.0, 100.0, 0.0),
    sp(19, "h1 formant", "", -50.0, 50.0, 0.0), sp(20, "h1 quantise", "", 0.0, 100.0, 0.0),
    sp(21, "h1 level", "dB", -24.0, 0.0, 0.0), sp(22, "h1 pan", "", -50.0, 50.0, 0.0),
    sp(23, "h2", "", 0.0, 1.0, 0.0), sp(24, "h2 dual", "", 0.0, 1.0, 0.0),
    sp(25, "h2 scale", "", 0.0, 36.0, 19.0), sp(26, "h2 random", "", 0.0, 100.0, 0.0),
    sp(27, "h2 formant", "", -50.0, 50.0, 0.0), sp(28, "h2 quantise", "", 0.0, 100.0, 0.0),
    sp(29, "h2 level", "dB", -24.0, 0.0, 0.0), sp(30, "h2 pan", "", -50.0, 50.0, 0.0),
    sp(31, "h3", "", 0.0, 1.0, 0.0), sp(32, "h3 dual", "", 0.0, 1.0, 0.0),
    sp(33, "h3 scale", "", 0.0, 36.0, 19.0), sp(34, "h3 random", "", 0.0, 100.0, 0.0),
    sp(35, "h3 formant", "", -50.0, 50.0, 0.0), sp(36, "h3 quantise", "", 0.0, 100.0, 0.0),
    sp(37, "h3 level", "dB", -24.0, 0.0, 0.0), sp(38, "h3 pan", "", -50.0, 50.0, 0.0),
    sp(39, "h4", "", 0.0, 1.0, 0.0), sp(40, "h4 dual", "", 0.0, 1.0, 0.0),
    sp(41, "h4 scale", "", 0.0, 36.0, 19.0), sp(42, "h4 random", "", 0.0, 100.0, 0.0),
    sp(43, "h4 formant", "", -50.0, 50.0, 0.0), sp(44, "h4 quantise", "", 0.0, 100.0, 0.0),
    sp(45, "h4 level", "dB", -24.0, 0.0, 0.0), sp(46, "h4 pan", "", -50.0, 50.0, 0.0),
    sp(47, "user1/0", "", -128.0, 127.0, 0.0), sp(48, "user1/1", "", -128.0, 127.0, 0.0),
    sp(49, "user1/2", "", -128.0, 127.0, 0.0), sp(50, "user1/3", "", -128.0, 127.0, 0.0),
    sp(51, "user1/4", "", -128.0, 127.0, 0.0), sp(52, "user1/5", "", -128.0, 127.0, 0.0),
    sp(53, "user1/6", "", -128.0, 127.0, 0.0), sp(54, "user1/7", "", -128.0, 127.0, 0.0),
    sp(55, "user1/8", "", -128.0, 127.0, 0.0), sp(56, "user1/9", "", -128.0, 127.0, 0.0),
    sp(57, "user1/10", "", -128.0, 127.0, 0.0), sp(58, "user1/11", "", -128.0, 127.0, 0.0),
    sp(59, "user2/0", "", -128.0, 127.0, 0.0), sp(60, "user2/1", "", -128.0, 127.0, 0.0),
    sp(61, "user2/2", "", -128.0, 127.0, 0.0), sp(62, "user2/3", "", -128.0, 127.0, 0.0),
    sp(63, "user2/4", "", -128.0, 127.0, 0.0), sp(64, "user2/5", "", -128.0, 127.0, 0.0),
    sp(65, "user2/6", "", -128.0, 127.0, 0.0), sp(66, "user2/7", "", -128.0, 127.0, 0.0),
    sp(67, "user2/8", "", -128.0, 127.0, 0.0), sp(68, "user2/9", "", -128.0, 127.0, 0.0),
    sp(69, "user2/10", "", -128.0, 127.0, 0.0), sp(70, "user2/11", "", -128.0, 127.0, 0.0),
    sp(71, "user3/0", "", -128.0, 127.0, 0.0), sp(72, "user3/1", "", -128.0, 127.0, 0.0),
    sp(73, "user3/2", "", -128.0, 127.0, 0.0), sp(74, "user3/3", "", -128.0, 127.0, 0.0),
    sp(75, "user3/4", "", -128.0, 127.0, 0.0), sp(76, "user3/5", "", -128.0, 127.0, 0.0),
    sp(77, "user3/6", "", -128.0, 127.0, 0.0), sp(78, "user3/7", "", -128.0, 127.0, 0.0),
    sp(79, "user3/8", "", -128.0, 127.0, 0.0), sp(80, "user3/9", "", -128.0, 127.0, 0.0),
    sp(81, "user3/10", "", -128.0, 127.0, 0.0), sp(82, "user3/11", "", -128.0, 127.0, 0.0),
    sp(83, "user4/0", "", -128.0, 127.0, 0.0), sp(84, "user4/1", "", -128.0, 127.0, 0.0),
    sp(85, "user4/2", "", -128.0, 127.0, 0.0), sp(86, "user4/3", "", -128.0, 127.0, 0.0),
    sp(87, "user4/4", "", -128.0, 127.0, 0.0), sp(88, "user4/5", "", -128.0, 127.0, 0.0),
    sp(89, "user4/6", "", -128.0, 127.0, 0.0), sp(90, "user4/7", "", -128.0, 127.0, 0.0),
    sp(91, "user4/8", "", -128.0, 127.0, 0.0), sp(92, "user4/9", "", -128.0, 127.0, 0.0),
    sp(93, "user4/10", "", -128.0, 127.0, 0.0), sp(94, "user4/11", "", -128.0, 127.0, 0.0),
    sp(95, "natural", "", 0.0, 1.0, 0.0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiHarmonyMode {
    AutoChordOrMultiCh, MonoChannels, WaitAll, Priority, FlexiChord,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectParams { pub enabled: bool, pub level_db: f32, pub pan: f32 }

impl Default for DirectParams {
    fn default() -> Self { Self { enabled: true, level_db: 0.0, pan: 0.0 } }
}

#[derive(Clone, Copy, Debug)]
pub struct UnisonParams {
    pub enabled: bool, pub random: f32, pub formant: f32, pub quantise: f32, pub level_db: f32, pub pan: f32,
}

impl Default for UnisonParams {
    fn default() -> Self {
        Self { enabled: false, random: 0.0, formant: 0.0, quantise: 0.0, level_db: 0.0, pan: 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HarmonyVoiceParams {
    pub enabled: bool, pub dual: bool, pub scale: ScaleId,
    pub random: f32, pub formant: f32, pub quantise: f32, pub level_db: f32, pub pan: f32,
}

impl Default for HarmonyVoiceParams {
    fn default() -> Self {
        Self { enabled: true, dual: false, scale: ScaleId::Unison, random: 0.0, formant: 0.0, quantise: 30.0, level_db: -6.0, pan: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct DecaBuddyParams {
    pub bypass: bool,
    pub natural_mode: bool,
    pub live_mode: bool,
    pub link: bool,
    pub preset: i8,
    pub key: u8,
    pub midi_base_channel: u8,
    pub midi_mode: MidiHarmonyMode,
    pub tune_hz: f32,
    pub input_gain_db: f32,
    pub analog_drive: f32,
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
            natural_mode: false,
            live_mode: true,
            link: false,
            preset: -1,
            key: 0,
            midi_base_channel: 1,
            midi_mode: MidiHarmonyMode::AutoChordOrMultiCh,
            tune_hz: 440.0,
            input_gain_db: 0.0,
            analog_drive: 35.0,
            preset_hotkey: 0,
            master_level_db: 0.0,
            direct: DirectParams::default(),
            unison: UnisonParams::default(),
            harmonies: [
                HarmonyVoiceParams { enabled: true, dual: false, scale: ScaleId::LowOctave, random: 0.0, formant: -15.0, quantise: 35.0, level_db: -6.0, pan: -30.0 },
                HarmonyVoiceParams { enabled: false, dual: false, scale: ScaleId::Unison, random: 0.0, formant: 0.0, quantise: 25.0, level_db: -8.0, pan: 10.0 },
                HarmonyVoiceParams { enabled: true, dual: true, scale: ScaleId::High3rdModal1, random: 0.0, formant: 10.0, quantise: 35.0, level_db: -7.0, pan: 25.0 },
                HarmonyVoiceParams { enabled: true, dual: true, scale: ScaleId::High5thModal1, random: 0.0, formant: 18.0, quantise: 35.0, level_db: -8.0, pan: -20.0 },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InputMeterLeds { pub led_neg12: bool, pub led_neg6: bool, pub led_0_over: bool }

#[derive(Clone, Copy)]
struct VoiceSlewState {
    gain_smooth: f32,
    gl_smooth: f32,
    gr_smooth: f32,
    pitch_ratio_smooth: f32,
    pitch_ratio_vel: f32,
    was_active: bool,
}

impl Default for VoiceSlewState {
    fn default() -> Self {
        Self {
            gain_smooth: 0.0,
            gl_smooth: 0.707,
            gr_smooth: 0.707,
            pitch_ratio_smooth: 1.0,
            pitch_ratio_vel: 0.0,
            was_active: false,
        }
    }
}

pub struct DecaBuddyProcessor {
    pub sample_rate: f32,
    pub host_offline: bool,
    pub params: DecaBuddyParams,
    pub preset_slots: [DecaBuddyParams; 4],
    pub scale_bank: ScaleBank,
    pub leds: InputMeterLeds,
    hann_lut: Vec<f32>,

    dry_ring: Vec<f32>,
    lp_ring: Vec<f32>,
    mask_ring: Vec<f32>,
    sib_ring: Vec<f32>,
    write_pos: usize,
    hop_counter: usize,

    choir_bus: ChoirAnalogBus,
    relay_bypass: RelayBypassModel,
    sibilance_det: SibilanceDetector,
    lp_filter: BiquadLowpass,
    pitch: PitchTracker,

    nju7032_env: f32,
    prev_live_low_lat: bool,

    latest_mark: usize,
    voiced_env: f32,

    unison_dsp: PsolaSubVoice,
    harmony_dsp: [HarmonyChannelDsp; 4],

    midi_target_note: [Option<u8>; 4],
    channel_note: [Option<u8>; 4],
    chord_notes_fifo: [u8; 16],
    chord_notes_len: usize,
    sustain_pedal: bool,
    physically_held: [bool; 128],
    sustained_notes: [bool; 128],
    last_flexi_shift: [Option<i32>; 4],

    harmony_slew: [VoiceSlewState; 4],
    cached_pitch_ratio: [f32; 4],
    cached_formant_ratio: [f32; 4],
    cached_uni_ratio: f32,

    pub max_grain: usize,
    pub lookahead: usize,
    pub out_delay: usize,
    pub live_lookahead: usize,
    pub live_out_delay: usize,
    pub min_t0: usize,
    pub max_t0: usize,

    epoch_polarity: f32,
    uni_ratio_smooth: f32,
    dir_gain_smooth: f32, dir_gl_smooth: f32, dir_gr_smooth: f32,
    uni_gain_smooth: f32, uni_gl_smooth: f32, uni_gr_smooth: f32,
    uni_was_active: bool,
    master_gain_smooth: f32,
}

impl DecaBuddyProcessor {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8000.0);
        let mut choir_bus = ChoirAnalogBus::default();
        choir_bus.set_sample_rate(sr);
        let mut lp = BiquadLowpass::default();
        lp.set_cutoff(310.96, sr);
        let pitch = PitchTracker::new(sr);

        let max_grain = max_grain_for_sr(sr);
        let lookahead = lookahead_samples_for_sr(sr);
        let out_delay = out_delay_for_sr(sr);
        let live_lookahead = live_lookahead_for_sr(sr);
        let live_out_delay = live_out_delay_for_sr(sr);
        let min_t0 = ((20.0 * (sr / 44_100.0)).round() as usize).max(2);
        let max_t0 = ((1050.0 * (sr / 44_100.0)).round() as usize).clamp(min_t0 + 1, 4096);

        let init_write = lookahead * 2 + out_delay;
        let default_params = DecaBuddyParams::default();

        Self {
            sample_rate: sr,
            host_offline: false,
            params: default_params.clone(),
            preset_slots: [default_params.clone(), default_params.clone(), default_params.clone(), default_params],
            scale_bank: ScaleBank::default(),
            leds: InputMeterLeds::default(),
            hann_lut: dsp::build_hann_lut(),
            dry_ring: vec![0.0; RING_SIZE],
            lp_ring: vec![0.0; RING_SIZE],
            mask_ring: vec![0.0; RING_SIZE],
            sib_ring: vec![0.0; RING_SIZE],
            write_pos: init_write,
            hop_counter: 0,
            choir_bus,
            relay_bypass: RelayBypassModel::new(),
            sibilance_det: SibilanceDetector::default(),
            lp_filter: lp,
            pitch,
            nju7032_env: 0.0,
            prev_live_low_lat: true,
            latest_mark: init_write.wrapping_sub(lookahead),
            voiced_env: 0.0,
            unison_dsp: PsolaSubVoice::new(101).with_dual_warmth(),
            harmony_dsp: [
                HarmonyChannelDsp::new(201), HarmonyChannelDsp::new(301),
                HarmonyChannelDsp::new(401), HarmonyChannelDsp::new(501),
            ],
            midi_target_note: [None; 4], channel_note: [None; 4],
            chord_notes_fifo: [0; 16], chord_notes_len: 0,
            sustain_pedal: false, physically_held: [false; 128], sustained_notes: [false; 128],
            last_flexi_shift: [None; 4],
            harmony_slew: [VoiceSlewState::default(); 4],
            cached_pitch_ratio: [1.0; 4], cached_formant_ratio: [1.0; 4], cached_uni_ratio: 1.0,
            max_grain, lookahead, out_delay, live_lookahead, live_out_delay, min_t0, max_t0,
            epoch_polarity: 1.0,
            uni_ratio_smooth: 1.0,
            dir_gain_smooth: 1.0, dir_gl_smooth: 0.707, dir_gr_smooth: 0.707,
            uni_gain_smooth: 0.0, uni_gl_smooth: 0.707, uni_gr_smooth: 0.707,
            uni_was_active: false, master_gain_smooth: 1.0,
        }
    }

    #[inline]
    pub fn is_live_low_latency(&self) -> bool { self.params.live_mode && !self.host_offline }

    pub fn latency_samples(&self) -> u32 {
        if self.is_live_low_latency() { 0 } else { (self.lookahead + self.out_delay) as u32 }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr.max(8000.0);
        self.choir_bus.set_sample_rate(self.sample_rate);
        self.lp_filter.set_cutoff(310.96, self.sample_rate);
        self.pitch.set_sample_rate(self.sample_rate);

        self.max_grain = max_grain_for_sr(self.sample_rate);
        self.lookahead = lookahead_samples_for_sr(self.sample_rate);
        self.out_delay = out_delay_for_sr(self.sample_rate);
        self.live_lookahead = live_lookahead_for_sr(self.sample_rate);
        self.live_out_delay = live_out_delay_for_sr(self.sample_rate);
        self.min_t0 = ((20.0 * (self.sample_rate / 44_100.0)).round() as usize).max(2);
        self.max_t0 = ((1050.0 * (self.sample_rate / 44_100.0)).round() as usize).clamp(self.min_t0 + 1, 4096);

        self.reset();
    }

    pub fn reset(&mut self) {
        self.dry_ring.fill(0.0);
        self.lp_ring.fill(0.0);
        self.mask_ring.fill(0.0);
        self.sib_ring.fill(0.0);
        self.choir_bus.reset();
        self.sibilance_det.reset();
        self.relay_bypass.reset();
        self.pitch.reset();
        self.nju7032_env = 0.0;
        self.write_pos = self.lookahead * 2 + self.out_delay;
        self.latest_mark = self.write_pos.wrapping_sub(self.lookahead);
        self.hop_counter = 0;
        self.voiced_env = 0.0;
        self.epoch_polarity = 1.0;
        self.uni_ratio_smooth = 1.0;
        self.unison_dsp.reset();
        for h in &mut self.harmony_dsp { h.reset(); }
        self.midi_target_note = [None; 4];
        self.channel_note = [None; 4];
        self.chord_notes_len = 0;
        self.sustain_pedal = false;
        self.physically_held = [false; 128];
        self.sustained_notes = [false; 128];
        self.last_flexi_shift = [None; 4];
        for s in &mut self.harmony_slew { *s = VoiceSlewState::default(); }
        self.cached_pitch_ratio = [1.0; 4];
        self.cached_formant_ratio = [1.0; 4];
        self.cached_uni_ratio = 1.0;
        self.dir_gain_smooth = 1.0; self.dir_gl_smooth = 0.707; self.dir_gr_smooth = 0.707;
        self.uni_gain_smooth = 0.0; self.uni_gl_smooth = 0.707; self.uni_gr_smooth = 0.707;
        self.uni_was_active = false;
        self.master_gain_smooth = 1.0;
    }

    pub fn save_preset_slot(&mut self, slot: usize) {
        if slot < 4 { self.preset_slots[slot] = self.params.clone(); }
    }

    pub fn recall_preset_slot(&mut self, slot: usize) {
        if slot < 4 {
            self.params = self.preset_slots[slot].clone();
            self.params.preset = slot as i8;
        }
    }

    pub fn detected_pitch(&self) -> (f32, i32) { (self.pitch.current_f0, self.pitch.current_note) }
    pub fn yin_hop_samples(&self) -> usize { self.pitch.yin_hop }

    pub fn set_vst_param_real(&mut self, index: usize, real_val: f32) -> bool {
        if index >= NUM_VST_PARAMS { return false; }
        let spec = &VST_PARAM_SPECS[index];
        let val = real_val.clamp(spec.min, spec.max);
        match index {
            0 => {
                let p = val.round() as i8;
                self.params.preset = p;
                if (0..=3).contains(&p) { self.recall_preset_slot(p as usize); }
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
            95 => self.params.natural_mode = val >= 0.5,
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
            95 => if self.params.natural_mode { 1.0 } else { 0.0 },
            _ => return None,
        };
        Some(val)
    }

    pub fn set_vst_param_normalized(&mut self, index: usize, norm: f32) -> bool {
        if index >= NUM_VST_PARAMS { return false; }
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

    fn remove_note_from_fifo(&mut self, note: u8) {
        let mut new_len = 0;
        for i in 0..self.chord_notes_len {
            if self.chord_notes_fifo[i] != note {
                self.chord_notes_fifo[new_len] = self.chord_notes_fifo[i];
                new_len += 1;
            }
        }
        self.chord_notes_len = new_len;
    }

    pub fn handle_midi_event(&mut self, status: u8, data1: u8, data2: u8) {
        let msg_type = status & 0xF0;
        let ch = status & 0x0F;
        let base_ch = self.params.midi_base_channel.saturating_sub(1) & 0x0F;
        let offset_ch = (ch + 16 - base_ch) & 0x0F;
        let note_idx = (data1 & 0x7F) as usize;

        match msg_type {
            0x90 if data2 > 0 => {
                if (offset_ch as usize) < 4 { self.channel_note[offset_ch as usize] = Some(data1); }
                if ch == base_ch {
                    self.physically_held[note_idx] = true;
                    self.sustained_notes[note_idx] = false;

                    let mut found = false;
                    for i in 0..self.chord_notes_len {
                        if self.chord_notes_fifo[i] == data1 { found = true; break; }
                    }
                    if !found {
                        if self.chord_notes_len >= 16 {
                            self.chord_notes_fifo.copy_within(1..16, 0);
                            self.chord_notes_len = 15;
                        }
                        self.chord_notes_fifo[self.chord_notes_len] = data1;
                        self.chord_notes_len += 1;
                    }
                }
                self.refresh_midi_targets();
            }
            0x80 | 0x90 => {
                if (offset_ch as usize) < 4
                    && self.channel_note[offset_ch as usize] == Some(data1)
                    && !self.sustain_pedal
                {
                    self.channel_note[offset_ch as usize] = None;
                }
                if ch == base_ch {
                    self.physically_held[note_idx] = false;
                    if self.sustain_pedal {
                        self.sustained_notes[note_idx] = true;
                    } else {
                        self.remove_note_from_fifo(data1);
                    }
                }
                self.refresh_midi_targets();
            }
            0xB0 => {
                let norm = (data2 as f32) / 127.0;
                match data1 {
                    64 => {
                        let was_sustained = self.sustain_pedal;
                        self.sustain_pedal = data2 >= 64;
                        if was_sustained && !self.sustain_pedal {
                            for n in 0..128 {
                                if self.sustained_notes[n] && !self.physically_held[n] {
                                    self.sustained_notes[n] = false;
                                    self.remove_note_from_fifo(n as u8);
                                    for ch_slot in &mut self.channel_note {
                                        if *ch_slot == Some(n as u8) { *ch_slot = None; }
                                    }
                                }
                            }
                            self.refresh_midi_targets();
                        }
                    }
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
                        self.sustain_pedal = false;
                        self.physically_held = [false; 128];
                        self.sustained_notes = [false; 128];
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
            MidiHarmonyMode::MonoChannels => { self.midi_target_note = self.channel_note; }
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
                        if k < len { self.midi_target_note[active_voices[k]] = Some(sorted[k]); }
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
                    for (k, &voice_idx) in active_voices.iter().enumerate().take(num_active) {
                        self.midi_target_note[voice_idx] = Some(sorted[k]);
                    }
                } else {
                    self.midi_target_note = [None; 4];
                }
            }
            MidiHarmonyMode::Priority => {
                self.midi_target_note = [None; 4];
                for (k, &voice_idx) in active_voices.iter().enumerate().take(num_active) {
                    if k < self.chord_notes_len {
                        self.midi_target_note[voice_idx] = Some(self.chord_notes_fifo[k]);
                    }
                }
            }
            MidiHarmonyMode::FlexiChord => {}
        }
    }

    pub fn process_block(&mut self, input_mono: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = input_mono.len().min(out_l.len()).min(out_r.len());
        if n == 0 { return; }
        let _ftz_daz_guard = ScopedFtzDaz::new();

        self.refresh_midi_targets();
        self.relay_bypass.update_bypass_trigger(self.params.bypass, self.sample_rate);

        let in_gain = gain_db_to_lin(self.params.input_gain_db);
        let drive_norm = (self.params.analog_drive * 0.01).clamp(0.0, 1.0);
        let target_master_gain = if self.params.bypass { 1.0 } else { voice_db_to_lin(self.params.master_level_db) };

        let live_low_lat = self.is_live_low_latency();
        if live_low_lat != self.prev_live_low_lat {
            self.prev_live_low_lat = live_low_lat;
            self.unison_dsp.reset();
            for h in &mut self.harmony_dsp { h.reset(); }
        }

        let active_lookahead = if live_low_lat { self.live_lookahead } else { self.lookahead };
        let active_out_delay = if live_low_lat { self.live_out_delay } else { self.out_delay };
        let yin_hop = self.pitch.yin_hop;

        let target_dir_gain = if !self.params.bypass && self.params.direct.enabled {
            voice_db_to_lin(self.params.direct.level_db)
        } else if self.params.bypass { 1.0 } else { 0.0 };
        let (dir_target_gl, dir_target_gr) = pan50_to_gains(self.params.direct.pan);

        let target_uni_gain = if !self.params.bypass && self.params.unison.enabled {
            voice_db_to_lin(self.params.unison.level_db)
        } else { 0.0 };
        let (uni_target_gl, uni_target_gr) = pan50_to_gains(self.params.unison.pan);
        let uni_formant_ratio = 2.0_f32.powf((self.params.unison.formant * 0.09) / 12.0);
        let uni_rand_norm = (self.params.unison.random * 0.01).clamp(0.0, 1.0);

        #[derive(Clone, Copy)]
        struct PreVoice {
            enabled: bool, dual: bool, target_gain: f32,
            target_gl: f32, target_gr: f32, d_gl: f32, d_gr: f32,
            dual_formant_ratio: f32, rand_norm: f32, base_delay_ms: f32,
        }

        let mut pre_voices = [PreVoice {
            enabled: false, dual: false, target_gain: 0.0,
            target_gl: 0.0, target_gr: 0.0, d_gl: 0.0, d_gr: 0.0,
            dual_formant_ratio: 1.0, rand_norm: 0.0, base_delay_ms: 0.0,
        }; 4];

        let mut num_active_voices = if self.params.unison.enabled { 1usize } else { 0usize };
        for (v_idx, pv_slot) in pre_voices.iter_mut().enumerate() {
            let mut vp = self.params.harmonies[v_idx];
            if self.params.link && v_idx > 0 {
                let h0 = self.params.harmonies[0];
                vp.random = h0.random; vp.formant = h0.formant; vp.quantise = h0.quantise;
            }
            if vp.enabled { num_active_voices += if vp.dual { 2 } else { 1 }; }
            let raw_gain = if !self.params.bypass && vp.enabled { voice_db_to_lin(vp.level_db) } else { 0.0 };
            let (gl, gr) = pan50_to_gains(vp.pan);
            let (d_gl, d_gr) = pan50_to_gains(-vp.pan);
            let dual_formant_ratio = 2.0_f32.powf(((vp.formant * 0.09) + 0.2) / 12.0);
            let rand_norm = (vp.random * 0.01).clamp(0.0, 1.0);
            let base_delay_ms = (v_idx as f32 + 1.0) * 4.5 * rand_norm;
            let effective_gain = if vp.dual { raw_gain * 0.75 } else { raw_gain };

            *pv_slot = PreVoice {
                enabled: vp.enabled, dual: vp.dual, target_gain: effective_gain,
                target_gl: gl, target_gr: gr, d_gl, d_gr,
                dual_formant_ratio, rand_norm, base_delay_ms,
            };
        }

        for i in 0..n {
            let x = input_mono[i] * in_gain;

            let abs_x = x.abs();
            if abs_x > self.nju7032_env {
                self.nju7032_env = 0.92 * self.nju7032_env + 0.08 * abs_x;
            } else {
                self.nju7032_env = 0.9995 * self.nju7032_env + 0.0005 * abs_x;
            }

            let w_idx = self.write_pos & RING_MASK;
            self.dry_ring[w_idx] = x;
            let lp_x = self.lp_filter.process(x);
            self.lp_ring[w_idx] = lp_x;
            let sib = self.sibilance_det.step(x, lp_x);
            self.sib_ring[w_idx] = sib;

            let was_voiced = self.pitch.voiced;
            if self.hop_counter == 0 {
                self.pitch.begin_sliced_frame(&self.dry_ring, self.write_pos, self.sample_rate);
            }
            self.hop_counter += 1;
            if self.hop_counter >= yin_hop {
                self.hop_counter = 0;
            }

            let frame_done = self.pitch.step_time_slice(self.sample_rate, self.params.tune_hz);
            if frame_done && self.pitch.voiced {
                let cutoff = (self.pitch.current_f0 * 2.1).clamp(220.0, 1200.0);
                self.lp_filter.set_cutoff(cutoff, self.sample_rate);

                let uni_q_norm = (self.params.unison.quantise * 0.01).clamp(0.0, 1.0);
                let dev = self.pitch.current_midi - (self.pitch.current_note as f32);
                self.cached_uni_ratio = 2.0_f32.powf((-uni_q_norm * dev) / 12.0);

                for v_idx in 0..4 {
                    let mut vp = self.params.harmonies[v_idx];
                    if self.params.link && v_idx > 0 {
                        let h0 = self.params.harmonies[0];
                        vp.random = h0.random; vp.formant = h0.formant; vp.quantise = h0.quantise;
                    }
                    self.cached_formant_ratio[v_idx] = 2.0_f32.powf((vp.formant * 0.09) / 12.0);
                    self.cached_pitch_ratio[v_idx] = if let Some(shift) =
                        self.compute_voice_shift_semitones(v_idx, &vp)
                    {
                        2.0_f32.powf(shift / 12.0)
                    } else { 1.0 };
                }

                if !was_voiced {
                    let t0 = (self.sample_rate / self.pitch.current_f0)
                        .clamp(self.min_t0 as f32, self.max_t0 as f32) as usize;
                    let r = (t0 / 2).max(2);
                    let center = self.write_pos.wrapping_sub(active_lookahead);
                    let mut best_p = center;
                    let mut best_score = -1e9_f32;
                    for k in 0..(2 * r + 1) {
                        let p = center.wrapping_sub(r).wrapping_add(k);
                        let v = self.lp_ring[p & RING_MASK];
                        let score = v.abs();
                        if score > best_score {
                            best_score = score;
                            best_p = p;
                        }
                    }
                    let mark_v = self.lp_ring[best_p & RING_MASK];
                    if mark_v.abs() > 0.005 {
                        self.epoch_polarity = mark_v.signum();
                    }
                    self.latest_mark = best_p;
                }
            }

            let target_mask = if self.pitch.voiced { 1.0 } else { 0.0 };
            self.voiced_env = 0.992 * self.voiced_env + 0.008 * target_mask;

            let read_pos = self.write_pos.wrapping_sub(active_lookahead);
            self.mask_ring[read_pos & RING_MASK] = self.voiced_env;
            let emit_pos = read_pos.wrapping_sub(active_out_delay);
            let emit_mask = self.mask_ring[emit_pos & RING_MASK];
            let emit_sib = self.sib_ring[emit_pos & RING_MASK];
            let current_sag = self.choir_bus.sag_env;

            let dry_signal = if live_low_lat { x } else { self.dry_ring[emit_pos & RING_MASK] };

            self.update_epoch_marks(read_pos);

            self.dir_gain_smooth = 0.995 * self.dir_gain_smooth + 0.005 * target_dir_gain;
            self.dir_gl_smooth = 0.995 * self.dir_gl_smooth + 0.005 * dir_target_gl;
            self.dir_gr_smooth = 0.995 * self.dir_gr_smooth + 0.005 * dir_target_gr;

            self.uni_gain_smooth = 0.995 * self.uni_gain_smooth + 0.005 * target_uni_gain;
            self.uni_gl_smooth = 0.995 * self.uni_gl_smooth + 0.005 * uni_target_gl;
            self.uni_gr_smooth = 0.995 * self.uni_gr_smooth + 0.005 * uni_target_gr;

            self.master_gain_smooth = 0.995 * self.master_gain_smooth + 0.005 * target_master_gain;

            let dir_l = dry_signal * (self.dir_gain_smooth * self.dir_gl_smooth);
            let dir_r = dry_signal * (self.dir_gain_smooth * self.dir_gr_smooth);

            let mut choir_l = 0.0_f32;
            let mut choir_r = 0.0_f32;

            let uni_active = target_uni_gain > 0.0 || self.uni_gain_smooth > 1e-4;
            if uni_active {
                if !self.uni_was_active || self.uni_ratio_smooth <= 0.01 {
                    self.uni_ratio_smooth = self.cached_uni_ratio;
                } else {
                    self.uni_ratio_smooth = 0.996 * self.uni_ratio_smooth + 0.004 * self.cached_uni_ratio;
                }
                self.uni_was_active = true;
                let y_u = self.unison_dsp.step_sample_with_cache(
                    &self.dry_ring, &self.hann_lut, None, emit_sib, current_sag,
                    self.write_pos, read_pos, emit_pos, self.latest_mark,
                    self.voiced_env, emit_mask, self.pitch.current_f0,
                    self.uni_ratio_smooth, uni_formant_ratio, uni_rand_norm,
                    0.0, 11.0 * uni_rand_norm, 0.35, self.sample_rate, self.max_grain,
                );
                choir_l += y_u * (self.uni_gain_smooth * self.uni_gl_smooth);
                choir_r += y_u * (self.uni_gain_smooth * self.uni_gr_smooth);
            } else if self.uni_was_active {
                self.unison_dsp.reset();
                self.uni_was_active = false;
                self.uni_ratio_smooth = 1.0;
            }

            for (v_idx, &pv) in pre_voices.iter().enumerate() {
                let s = &mut self.harmony_slew[v_idx];

                let target_voice_gain = if pv.enabled && self.pitch.voiced && self.pitch.current_note >= 0 {
                    pv.target_gain
                } else { 0.0 };

                s.gain_smooth = 0.995 * s.gain_smooth + 0.005 * target_voice_gain;
                s.gl_smooth = 0.995 * s.gl_smooth + 0.005 * pv.target_gl;
                s.gr_smooth = 0.995 * s.gr_smooth + 0.005 * pv.target_gr;

                let voice_active = target_voice_gain > 0.0 || s.gain_smooth > 1e-4;
                if !voice_active {
                    if s.was_active {
                        self.harmony_dsp[v_idx].reset();
                        s.was_active = false;
                        s.pitch_ratio_smooth = 1.0;
                    }
                    continue;
                }

                let target_pitch_ratio = self.cached_pitch_ratio[v_idx];
                if !s.was_active || s.pitch_ratio_smooth <= 0.01 {
                    s.pitch_ratio_smooth = target_pitch_ratio;
                    s.pitch_ratio_vel = 0.0;
                } else if self.params.natural_mode {
                    // Modo Natural: Filtro criticamente amortecido de 2a ordem (curva S organica de ~28 ms)
                    let diff = target_pitch_ratio - s.pitch_ratio_smooth;
                    let omega = 0.0035_f32;
                    let accel = omega * omega * diff - 2.0 * omega * s.pitch_ratio_vel;
                    s.pitch_ratio_vel += accel;
                    s.pitch_ratio_smooth += s.pitch_ratio_vel;
                } else {
                    let diff = (target_pitch_ratio - s.pitch_ratio_smooth).abs();
                    // Modo Classic HV10: Tracking continuo do microcontrolador Hitachi H8
                    let slew_k = if diff > 0.06 { 0.0025 } else { 0.006 };
                    s.pitch_ratio_smooth = (1.0 - slew_k) * s.pitch_ratio_smooth + slew_k * target_pitch_ratio;
                }
                s.was_active = true;

                let pitch_ratio = s.pitch_ratio_smooth;
                let formant_ratio = self.cached_formant_ratio[v_idx];

                let ch_dsp = &mut self.harmony_dsp[v_idx];
                let y_main = ch_dsp.main.step_sample_with_cache(
                    &self.dry_ring, &self.hann_lut,
                    if pv.dual { Some(&mut ch_dsp.shared_grain) } else { None },
                    emit_sib, current_sag,
                    self.write_pos, read_pos, emit_pos, self.latest_mark,
                    self.voiced_env, emit_mask, self.pitch.current_f0,
                    pitch_ratio, formant_ratio, pv.rand_norm,
                    0.0, pv.base_delay_ms, 0.0, self.sample_rate, self.max_grain,
                );

                if pv.dual {
                    let y_dual = ch_dsp.dual.step_sample_with_cache(
                        &self.dry_ring, &self.hann_lut,
                        Some(&mut ch_dsp.shared_grain),
                        emit_sib, current_sag,
                        self.write_pos, read_pos, emit_pos, self.latest_mark,
                        self.voiced_env, emit_mask, self.pitch.current_f0,
                        pitch_ratio, pv.dual_formant_ratio, pv.rand_norm,
                        -9.0, pv.base_delay_ms + 14.0, 0.0, self.sample_rate, self.max_grain,
                    );
                    choir_l += (y_main * s.gl_smooth + y_dual * pv.d_gl) * s.gain_smooth;
                    choir_r += (y_main * s.gr_smooth + y_dual * pv.d_gr) * s.gain_smooth;
                } else {
                    choir_l += y_main * (s.gain_smooth * s.gl_smooth);
                    choir_r += y_main * (s.gain_smooth * s.gr_smooth);
                }
            }

            // Modo Natural: Atenua a granulação de sibilâncias no barramento de harmonia
            // para manter as consoantes ("s", "t", "ch") limpas e naturais a partir do canal direto
            let sib_duck = if self.params.natural_mode {
                (1.0 - 0.70 * emit_sib).clamp(0.25, 1.0)
            } else {
                1.0
            };
            choir_l *= sib_duck;
            choir_r *= sib_duck;

            let (analog_choir_l, analog_choir_r) = if num_active_voices > 0 {
                self.choir_bus.process_stereo(choir_l, choir_r, drive_norm, num_active_voices)
            } else {
                (0.0, 0.0)
            };

            let wet_l = analog_bus_soft_limit((dir_l + analog_choir_l) * self.master_gain_smooth);
            let wet_r = analog_bus_soft_limit((dir_r + analog_choir_r) * self.master_gain_smooth);

            // Rele fisico Takamisawa RY-9WK com repique mecanico (contact bounce) ao alternar bypass
            let (final_l, final_r) = self.relay_bypass.process_stereo(input_mono[i], input_mono[i], wet_l, wet_r, self.sample_rate);
            out_l[i] = final_l;
            out_r[i] = final_r;

            self.write_pos = self.write_pos.wrapping_add(1);
        }

        self.leds.led_neg12 = self.nju7032_env >= 0.251;
        self.leds.led_neg6 = self.nju7032_env >= 0.501;
        self.leds.led_0_over = self.nju7032_env >= 0.950;
    }

    #[inline]
    fn update_epoch_marks(&mut self, read_pos: usize) {
        if !self.pitch.voiced || self.pitch.current_f0 <= 40.0 {
            self.latest_mark = read_pos;
            return;
        }

        let t0 = (self.sample_rate / self.pitch.current_f0)
            .clamp(self.min_t0 as f32, self.max_t0 as f32) as usize;
        let elapsed = read_pos.wrapping_sub(self.latest_mark);
        if elapsed > RING_SIZE / 2 {
            self.latest_mark = read_pos;
            return;
        }

        if elapsed >= t0 {
            let pred = self.latest_mark.wrapping_add(t0);
            let radius = ((t0 * 3) / 10).max(2);
            let start = pred.wrapping_sub(radius);
            let span = 2 * radius + 1;

            let pol_sign = if self.epoch_polarity >= 0.0 { 1.0 } else { -1.0 };
            let rad_f = radius as f32;

            let mut best_pos = pred;
            let mut best_score = -1e9_f32;

            for k in 0..span {
                let p = start.wrapping_add(k);
                let v = self.lp_ring[p & RING_MASK];
                // Ponderacao centrada no periodo predito T0 com imunidade a inversao de polaridade:
                let dist = ((k as isize) - (radius as isize)).abs() as f32;
                let dist_weight = 1.0 - 0.25 * (dist / rad_f);
                let score = (v * pol_sign) * dist_weight;
                if score > best_score {
                    best_score = score;
                    best_pos = p;
                }
            }

            let mark_v = self.lp_ring[best_pos & RING_MASK];
            if mark_v.abs() > 0.005 {
                self.epoch_polarity = 0.98 * self.epoch_polarity + 0.02 * mark_v.signum();
            }
            self.latest_mark = best_pos;
        }
    }

    fn compute_voice_shift_semitones(&mut self, v_idx: usize, vp: &HarmonyVoiceParams) -> Option<f32> {
        if self.pitch.current_note < 0 { return None; }
        let q_norm = (vp.quantise * 0.01).clamp(0.0, 1.0);
        let note = self.pitch.current_note;
        let dev = self.pitch.current_midi - (note as f32);

        if vp.scale == ScaleId::Midi {
            if self.params.midi_mode == MidiHarmonyMode::FlexiChord {
                let shift = self.flexichord_jazz_shift(v_idx, note)?;
                self.last_flexi_shift[v_idx] = Some(shift);
                return Some((shift as f32) - q_norm * dev);
            }
            let target_midi = self.midi_target_note[v_idx]? as f32;
            let in_midi = self.pitch.current_midi;
            let smooth_midi = self.pitch.smooth_midi;
            // Modo MIDI (patentes Akai JPH0331279B2 / IVL US 5,567,901):
            // Transição estritamente contínua sem saltos discretos de (1 - q_norm)
            // Em Quantise = 100%, crava no target_midi (target_midi - in_midi).
            // Em Quantise = 0%, reproduz 100% do vibrato orgânico do cantor (target_midi - smooth_midi).
            let shift = target_midi - q_norm * in_midi - (1.0 - q_norm) * smooth_midi;
            return Some(shift);
        }

        let key = (self.params.key % 12) as i32;
        let root_offset = vp.scale.root_pc() as i32;
        let pc = (note - key - root_offset).rem_euclid(12) as usize;
        let interval = self.scale_bank.lookup_offset(vp.scale, v_idx, pc)? as f32;
        Some(interval - q_norm * dev)
    }

    fn flexichord_jazz_shift(&self, v_idx: usize, input_note: i32) -> Option<i32> {
        if self.chord_notes_len == 0 { return None; }
        let len = self.chord_notes_len.min(16);
        if len == 1 { return Some(self.chord_notes_fifo[0] as i32 - input_note); }

        let start_idx = len.saturating_sub(6);
        let mut chord_mask = [false; 12];
        for i in start_idx..len { chord_mask[(self.chord_notes_fifo[i] as usize) % 12] = true; }

        let mut unique_count = chord_mask.iter().filter(|&&b| b).count();
        if unique_count < 3 {
            let key_pc = (self.params.key as usize) % 12;
            const DIATONIC: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];
            for &deg in &DIATONIC {
                let candidate = (key_pc + deg) % 12;
                let clashes = chord_mask[(candidate + 1) % 12] || chord_mask[(candidate + 11) % 12];
                if !chord_mask[candidate] && !clashes {
                    chord_mask[candidate] = true;
                    unique_count += 1;
                    if unique_count >= 4 { break; }
                }
            }
        }

        let in_pc = input_note.rem_euclid(12) as usize;
        let (dir, target_rank) = match v_idx {
            0 => (-1i32, 2usize),
            1 => (-1i32, 1usize),
            2 => (1i32, 1usize),
            _ => (1i32, 2usize),
        };

        let mut found = 0usize;
        let mut chosen_step = None;
        for step in 2..=14 {
            let offset = dir * step;
            let cpc = (in_pc as i32 + offset).rem_euclid(12) as usize;
            if chord_mask[cpc] {
                found += 1;
                if found == target_rank {
                    chosen_step = Some(offset);
                    break;
                }
            }
        }

        if let (Some(new_s), Some(prev_s)) = (chosen_step, self.last_flexi_shift[v_idx]) {
            let prev_pc = (in_pc as i32 + prev_s).rem_euclid(12) as usize;
            if chord_mask[prev_pc] && prev_s.abs() >= 2 && (new_s - prev_s).abs() <= 2 {
                return Some(prev_s);
            }
        }
        chosen_step
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
        #[name = "MIDI"] Midi, #[name = "User"] User, #[name = "Low Octave"] LowOctave,
        #[name = "Low 3rd | Mod 1"] Low3rdModal1, #[name = "Low 3rd | Mod 2"] Low3rdModal2,
        #[name = "Low 3rd | Mod 3"] Low3rdModal3, #[name = "Low 3rd | Mod 4"] Low3rdModal4,
        #[name = "Low 3rd | Mod 5"] Low3rdModal5, #[name = "Low 3rd | Har Min"] Low3rdHarmonicMinor,
        #[name = "Low 3rd | Mel Min"] Low3rdMelodicMinor, #[name = "Low 5th | Mod 1"] Low5thModal1,
        #[name = "Low 5th | Mod 2"] Low5thModal2, #[name = "Low 5th | Mod 3"] Low5thModal3,
        #[name = "Low 5th | Mod 4"] Low5thModal4, #[name = "Low 5th | Mod 5"] Low5thModal5,
        #[name = "Low 5th | Har Min"] Low5thHarmonicMinor, #[name = "Low 5th | Mel Min"] Low5thMelodicMinor,
        #[name = "Low 7th | Major"] Low7thMajor, #[name = "Low 7th | Minor"] Low7thMinor,
        #[name = "Unison"] Unison, #[name = "High 3rd | Mod 1"] High3rdModal1,
        #[name = "High 3rd | Mod 2"] High3rdModal2, #[name = "High 3rd | Mod 3"] High3rdModal3,
        #[name = "High 3rd | Mod 4"] High3rdModal4, #[name = "High 3rd | Mod 5"] High3rdModal5,
        #[name = "High 3rd | Har Min"] High3rdHarmonicMinor, #[name = "High 3rd | Mel Min"] High3rdMelodicMinor,
        #[name = "High 5th | Mod 1"] High5thModal1, #[name = "High 5th | Mod 2"] High5thModal2,
        #[name = "High 5th | Mod 3"] High5thModal3, #[name = "High 5th | Mod 4"] High5thModal4,
        #[name = "High 5th | Mod 5"] High5thModal5, #[name = "High 5th | Har Min"] High5thHarmonicMinor,
        #[name = "High 5th | Mel Min"] High5thMelodicMinor, #[name = "High 7th | Major"] High7thMajor,
        #[name = "High 7th | Minor"] High7thMinor, #[name = "High Octave"] HighOctave,
    }

    #[derive(Params)]
    pub struct ChorusVoxParams {
        #[param(name = "Key", default = 0)] pub key: EnumParam<KeyChoice>,
        #[param(name = "MIDI Mode", default = 0)] pub midi_mode: EnumParam<MidiModeChoice>,
        #[param(name = "MIDI Ch", range = "linear(1, 16)", default = 1)] pub midi_ch: IntParam,
        #[param(name = "Tune", range = "linear(427, 453)", default = 440.0, unit = "Hz")] pub tune: FloatParam,
        #[param(name = "In Gain", range = "linear(-15, 15)", default = 0.0, unit = "dB")] pub input_gain: FloatParam,
        #[param(name = "Drive", range = "linear(0, 100)", default = 35.0, unit = "%")] pub analog_drive: FloatParam,
        #[param(name = "Natural", default = false)] pub natural_mode: BoolParam,
        #[param(name = "Link", default = false)] pub link: BoolParam,

        #[param(name = "Direct", default = true)] pub direct_on: BoolParam,
        #[param(name = "Dir Lev", range = "linear(-24, 0)", default = 0.0, unit = "dB")] pub direct_level: FloatParam,
        #[param(name = "Dir Pan", range = "linear(-50, 50)", default = 0.0)] pub direct_pan: FloatParam,

        #[param(name = "Unison", default = false)] pub unison_on: BoolParam,
        #[param(name = "U.Rand", range = "linear(0, 100)", default = 0.0)] pub unison_random: FloatParam,
        #[param(name = "U.Form", range = "linear(-50, 50)", default = 0.0)] pub unison_formant: FloatParam,
        #[param(name = "U.Quant", range = "linear(0, 100)", default = 0.0)] pub unison_quantise: FloatParam,
        #[param(name = "U.Level", range = "linear(-24, 0)", default = 0.0, unit = "dB")] pub unison_level: FloatParam,
        #[param(name = "U.Pan", range = "linear(-50, 50)", default = 0.0)] pub unison_pan: FloatParam,

        #[param(name = "H1 Scale", default = 2)] pub h1_scale: EnumParam<ScaleChoice>,
        #[param(name = "H1 On", default = true)] pub h1_on: BoolParam,
        #[param(name = "H1 Dual", default = false)] pub h1_dual: BoolParam,
        #[param(name = "H1 Rand", range = "linear(0, 100)", default = 0.0)] pub h1_random: FloatParam,
        #[param(name = "H1 Form", range = "linear(-50, 50)", default = -15.0)] pub h1_formant: FloatParam,
        #[param(name = "H1 Quant", range = "linear(0, 100)", default = 35.0)] pub h1_quantise: FloatParam,
        #[param(name = "H1 Level", range = "linear(-24, 0)", default = -6.0, unit = "dB")] pub h1_level: FloatParam,
        #[param(name = "H1 Pan", range = "linear(-50, 50)", default = -30.0)] pub h1_pan: FloatParam,

        #[param(name = "H2 Scale", default = 19)] pub h2_scale: EnumParam<ScaleChoice>,
        #[param(name = "H2 On", default = false)] pub h2_on: BoolParam,
        #[param(name = "H2 Dual", default = false)] pub h2_dual: BoolParam,
        #[param(name = "H2 Rand", range = "linear(0, 100)", default = 0.0)] pub h2_random: FloatParam,
        #[param(name = "H2 Form", range = "linear(-50, 50)", default = 0.0)] pub h2_formant: FloatParam,
        #[param(name = "H2 Quant", range = "linear(0, 100)", default = 25.0)] pub h2_quantise: FloatParam,
        #[param(name = "H2 Level", range = "linear(-24, 0)", default = -8.0, unit = "dB")] pub h2_level: FloatParam,
        #[param(name = "H2 Pan", range = "linear(-50, 50)", default = 10.0)] pub h2_pan: FloatParam,

        #[param(name = "H3 Scale", default = 20)] pub h3_scale: EnumParam<ScaleChoice>,
        #[param(name = "H3 On", default = true)] pub h3_on: BoolParam,
        #[param(name = "H3 Dual", default = true)] pub h3_dual: BoolParam,
        #[param(name = "H3 Rand", range = "linear(0, 100)", default = 0.0)] pub h3_random: FloatParam,
        #[param(name = "H3 Form", range = "linear(-50, 50)", default = 10.0)] pub h3_formant: FloatParam,
        #[param(name = "H3 Quant", range = "linear(0, 100)", default = 35.0)] pub h3_quantise: FloatParam,
        #[param(name = "H3 Level", range = "linear(-24, 0)", default = -7.0, unit = "dB")] pub h3_level: FloatParam,
        #[param(name = "H3 Pan", range = "linear(-50, 50)", default = 25.0)] pub h3_pan: FloatParam,

        #[param(name = "H4 Scale", default = 27)] pub h4_scale: EnumParam<ScaleChoice>,
        #[param(name = "H4 On", default = true)] pub h4_on: BoolParam,
        #[param(name = "H4 Dual", default = true)] pub h4_dual: BoolParam,
        #[param(name = "H4 Rand", range = "linear(0, 100)", default = 0.0)] pub h4_random: FloatParam,
        #[param(name = "H4 Form", range = "linear(-50, 50)", default = 18.0)] pub h4_formant: FloatParam,
        #[param(name = "H4 Quant", range = "linear(0, 100)", default = 35.0)] pub h4_quantise: FloatParam,
        #[param(name = "H4 Level", range = "linear(-24, 0)", default = -8.0, unit = "dB")] pub h4_level: FloatParam,
        #[param(name = "H4 Pan", range = "linear(-50, 50)", default = -20.0)] pub h4_pan: FloatParam,

        #[meter] pub meter_in: MeterSlot,
        #[meter] pub meter_out_l: MeterSlot,
        #[meter] pub meter_out_r: MeterSlot,
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
            let is_off = config.process_mode.is_offline();
            state.core.host_offline = is_off;
            state.core.params.live_mode = !is_off;
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
            p.analog_drive = params.analog_drive.read();
            p.natural_mode = params.natural_mode.value();
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
            let mut cur_sample = 0usize;
            for event in events.iter() {
                let ev_offset = (event.sample_offset as usize).min(n);
                if ev_offset > cur_sample {
                    state.core.process_block(
                        &state.mono_scratch[cur_sample..ev_offset],
                        &mut state.out_l_scratch[cur_sample..ev_offset],
                        &mut state.out_r_scratch[cur_sample..ev_offset],
                    );
                    cur_sample = ev_offset;
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
                    _ => {}
                }
            }
            if cur_sample < n {
                state.core.process_block(
                    &state.mono_scratch[cur_sample..n],
                    &mut state.out_l_scratch[cur_sample..n],
                    &mut state.out_r_scratch[cur_sample..n],
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

            let in_pk = state.mono_scratch[..n].iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            let out_l_pk = state.out_l_scratch[..n].iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            let out_r_pk = state.out_r_scratch[..n].iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));

            ctx.set_meter(P::MeterIn, in_pk);
            ctx.set_meter(P::MeterOutL, out_l_pk);
            if buffer.num_output_channels() >= 2 { ctx.set_meter(P::MeterOutR, out_r_pk); }

            ProcessStatus::Normal
        }

        fn editor(params: Arc<Self::Params>) -> Box<dyn Editor> {
            GridLayout::build(vec![
                section("GLOBAL & INPUT -----------------------------------------------------------------------------------------------------------------------", vec![
                    dropdown(P::Key, "Key").cols(2), dropdown(P::MidiMode, "MIDI Mode").cols(2),
                    knob(P::MidiCh, "MIDI Ch"), knob(P::Tune, "Tune Hz").cols(2),
                    knob(P::InputGain, "In Gain").cols(2), knob(P::AnalogDrive, "Drive"),
                    toggle(P::NaturalMode, "Natural"),
                    meter(&[P::MeterIn], "In"),
                ]),
                section("DIRECT & UNISON ----------------------------------------------------------------------------------------------------------------------", vec![
                    toggle(P::DirectOn, "Direct"), knob(P::DirectLevel, "Dir Lev"),
                    knob(P::DirectPan, "Dir Pan"), toggle(P::Link, "Link"),
                    toggle(P::UnisonOn, "Unison"), knob(P::UnisonRandom, "U.Rand"),
                    knob(P::UnisonFormant, "U.Form"), knob(P::UnisonQuantise, "U.Quant"),
                    knob(P::UnisonLevel, "U.Level"), knob(P::UnisonPan, "U.Pan"),
                    meter(&[P::MeterOutL, P::MeterOutR], "Out"),
                ]),
                section("VOX 1 --------------------------------------------------------------------------------------------------------------------------------", vec![
                    toggle(P::H1On, "On"), toggle(P::H1Dual, "Dual"),
                    knob(P::H1Scale, "Scale").cols(4), knob(P::H1Random, "Random"),
                    knob(P::H1Formant, "Formant"), knob(P::H1Quantise, "Quantise"),
                    knob(P::H1Level, "Level"), knob(P::H1Pan, "Pan"),
                ]),
                section("VOX 2 --------------------------------------------------------------------------------------------------------------------------------", vec![
                    toggle(P::H2On, "On"), toggle(P::H2Dual, "Dual"),
                    knob(P::H2Scale, "Scale").cols(4), knob(P::H2Random, "Random"),
                    knob(P::H2Formant, "Formant"), knob(P::H2Quantise, "Quantise"),
                    knob(P::H2Level, "Level"), knob(P::H2Pan, "Pan"),
                ]),
                section("VOX 3 --------------------------------------------------------------------------------------------------------------------------------", vec![
                    toggle(P::H3On, "On"), toggle(P::H3Dual, "Dual"),
                    knob(P::H3Scale, "Scale").cols(4), knob(P::H3Random, "Random"),
                    knob(P::H3Formant, "Formant"), knob(P::H3Quantise, "Quantise"),
                    knob(P::H3Level, "Level"), knob(P::H3Pan, "Pan"),
                ]),
                section("VOX 4 --------------------------------------------------------------------------------------------------------------------------------", vec![
                    toggle(P::H4On, "On"), toggle(P::H4Dual, "Dual"),
                    knob(P::H4Scale, "Scale").cols(4), knob(P::H4Random, "Random"),
                    knob(P::H4Formant, "Formant"), knob(P::H4Quantise, "Quantise"),
                    knob(P::H4Level, "Level"), knob(P::H4Pan, "Pan"),
                ]),
            ])
            .with_title("CHORUSVOX DECABUDDY v0.2.13")
            .into_editor(&params)
        }
    }

    truce::plugin! { logic: ChorusVox, params: ChorusVoxParams }
}

#[cfg(test)]
mod jazz_and_live_tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_auto_live_vs_offline_bounce_latency_and_default_zero_random() {
        let mut proc = DecaBuddyProcessor::new(44_100.0);
        assert_eq!(proc.params.unison.random, 0.0, "U.Rand deve abrir em 0.0 por padrao");
        for h in &proc.params.harmonies {
            assert_eq!(h.random, 0.0, "Random das harmonias deve abrir em 0.0 por padrao");
        }
        assert_eq!(proc.latency_samples(), 0, "Em tempo real deve operar em modo Live (0 amostras reportadas)");
        proc.host_offline = true;
        assert_eq!(proc.latency_samples(), 2496, "Em Bounce offline deve reportar 2496 amostras para PDC");
    }

    #[test]
    fn test_jazz_dmaj7_to_fsharp_m6_with_sustain_pedal() {
        let mut proc = DecaBuddyProcessor::new(44_100.0);
        proc.params.midi_mode = MidiHarmonyMode::FlexiChord;
        for h in &mut proc.params.harmonies { h.scale = ScaleId::Midi; h.enabled = true; }
        proc.handle_midi_event(0xB0, 64, 127);
        for &n in &[62u8, 66, 69, 73] {
            proc.handle_midi_event(0x90, n, 100);
            proc.handle_midi_event(0x80, n, 0);
        }
        assert_eq!(proc.chord_notes_len, 4);
        for v in 0..4 {
            let shift = proc.flexichord_jazz_shift(v, 62).unwrap();
            assert!(shift.abs() >= 2);
        }
        for &n in &[66u8, 69, 73, 75] { proc.handle_midi_event(0x90, n, 100); }
        proc.handle_midi_event(0xB0, 64, 0);
        assert_eq!(proc.chord_notes_len, 4);
    }

    #[test]
    fn test_continuous_pitch_glide_and_negative_epoch_polarity() {
        let sr = 44_100.0;
        let mut proc = DecaBuddyProcessor::new(sr);
        proc.params.direct.enabled = false;
        proc.params.unison.enabled = false;
        proc.params.harmonies[0].enabled = true;
        proc.params.harmonies[0].scale = ScaleId::Midi;
        proc.params.midi_mode = MidiHarmonyMode::Priority;

        // 1. Sinal com impulsos glotais de polaridade negativa
        let n = 8192;
        let mut input = vec![0.0_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];
        for (i, item) in input.iter_mut().enumerate() {
            let t = (i as f32) / sr;
            // Sinal com pico agudo negativo para testar deteccao de polaridade
            let fund = (2.0 * PI * 130.81 * t).sin();
            let asym = if fund > 0.0 { fund * 0.4 } else { fund * 1.2 };
            *item = asym * 0.7;
        }

        proc.handle_midi_event(0x90, 60, 100); // C4
        proc.process_block(&input[..4096], &mut out_l[..4096], &mut out_r[..4096]);
        assert!(
            proc.epoch_polarity < 0.0,
            "Rastreador de epoca deve adaptar-se para polaridade glotal negativa (epoch_polarity={})",
            proc.epoch_polarity
        );

        // 2. Transicao de nota MIDI com glide musical continuo (sem degrau discreto de 256 amostras)
        let ratio_before = proc.harmony_slew[0].pitch_ratio_smooth;
        proc.handle_midi_event(0x80, 60, 0);
        proc.handle_midi_event(0x90, 64, 100); // E4 (salto de +4 semitons)
        proc.process_block(&input[4096..4096 + 400], &mut out_l[4096..4096 + 400], &mut out_r[4096..4096 + 400]);
        let ratio_mid = proc.harmony_slew[0].pitch_ratio_smooth;
        let target_ratio = proc.cached_pitch_ratio[0];

        assert!(
            ratio_mid > ratio_before && ratio_mid < target_ratio,
            "Pitch ratio deve deslizar continuamente (glide legato): before={ratio_before:.3}, mid={ratio_mid:.3}, target={target_ratio:.3}"
        );
    }

    #[test]
    fn test_sedreams_gci_stability_on_raspy_fry_voice() {
        let sr = 44_100.0;
        let mut proc = DecaBuddyProcessor::new(sr);
        proc.params.direct.enabled = true;
        proc.params.harmonies[0].enabled = true;
        proc.params.harmonies[0].scale = ScaleId::LowOctave;

        // Simulacao de vocal fry / voz rasgada: F0 em 75 Hz com segundo pico espurio de ressonancia F1 (formante)
        let n = 8192;
        let mut input = vec![0.0_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];
        let f0 = 75.0_f32;
        let f1 = 380.0_f32; // ressonancia que causaria falsos picos no rastreador simples

        for i in 0..n {
            let t = (i as f32) / sr;
            let pulse = (2.0 * PI * f0 * t).sin();
            let formant_ring = (2.0 * PI * f1 * t).sin() * (-15.0 * (t * f0).fract()).exp();
            input[i] = (pulse * 0.6 + formant_ring * 0.4) * 0.8;
        }

        proc.process_block(&input, &mut out_l, &mut out_r);

        assert!(
            proc.pitch.voiced,
            "O PitchTracker deve detectar vozeamento confiavel mesmo em vocal fry a 75 Hz com ressonancia F1"
        );
        assert!(
            (proc.pitch.current_f0 - f0).abs() < 5.0,
            "F0 detectado deve estar proximo de 75 Hz (detectado={:.2} Hz)",
            proc.pitch.current_f0
        );
    }

    #[test]
    fn test_natural_mode_s_curve_portamento_and_sibilance_duck() {
        let sr = 44_100.0;
        let mut proc = DecaBuddyProcessor::new(sr);
        proc.params.natural_mode = true;
        proc.params.direct.enabled = true;
        proc.params.harmonies[0].enabled = true;
        proc.params.harmonies[0].scale = ScaleId::Midi;
        proc.params.midi_mode = MidiHarmonyMode::Priority;

        let n = 4096;
        let mut input = vec![0.0_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];
        for (i, item) in input.iter_mut().enumerate() {
            let t = (i as f32) / sr;
            *item = (2.0 * PI * 220.0 * t).sin() * 0.7;
        }

        proc.handle_midi_event(0x90, 60, 100);
        proc.process_block(&input[..2048], &mut out_l[..2048], &mut out_r[..2048]);

        // Salto de nota para disparar portamento em curva S
        proc.handle_midi_event(0x90, 67, 100);
        proc.process_block(&input[2048..2048 + 300], &mut out_l[2048..2048 + 300], &mut out_r[2048..2048 + 300]);

        let vel = proc.harmony_slew[0].pitch_ratio_vel;
        assert!(
            vel.abs() > 1e-6,
            "No modo Natural, a velocidade de pitch_ratio_vel deve ser diferente de zero durante transicao em curva S"
        );
    }

    #[test]
    fn test_relay_bypass_hardware_contact_bounce() {
        let sr = 44_100.0;
        let mut proc = DecaBuddyProcessor::new(sr);
        proc.params.bypass = false;

        let n = 1024;
        let input = vec![0.5_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];
        proc.process_block(&input[..256], &mut out_l[..256], &mut out_r[..256]);

        // Aciona bypass de hardware
        proc.params.bypass = true;
        proc.process_block(&input[256..512], &mut out_l[256..512], &mut out_r[256..512]);

        // Durante o periodo de bounce (~97 amostras), o rele deve transicionar dinamicamente
        let bounce_segment = &out_l[256..256 + 60];
        let min_val = bounce_segment.iter().fold(f32::MAX, |a, &b| a.min(b));
        assert!(
            min_val < 0.45,
            "O repique mecânico dos reles RY-9WK deve provocar micro-interrupção na comutacao (min_val={min_val})"
        );

        // Apos assentar completamente (apos 200 amostras), deve entregar sinal limpo
        let settled_sample = out_l[256 + 200];
        assert!(
            (settled_sample - 0.5).abs() < 1e-3,
            "Apos o repique mecanico, o bypass fisico deve ser 100% transparente (sample={settled_sample})"
        );
    }

    #[test]
    fn test_continuous_high_pitch_note_clean() {
        let sr = 44_100.0;
        let mut proc = DecaBuddyProcessor::new(sr);
        proc.params.direct.enabled = true;
        proc.params.harmonies[0].enabled = true;
        proc.params.harmonies[0].scale = ScaleId::HighOctave;

        // Nota aguda contínua NA VOZ: F0 = 700 Hz (F5)
        let f0 = 700.0_f32;
        let n = 16384;
        let mut input = vec![0.0_f32; n];
        let mut out_l = vec![0.0_f32; n];
        let mut out_r = vec![0.0_f32; n];

        let mut glottal_phase = 0.0_f32;
        let mut glottal_lp = 0.0_f32;
        for i in 0..n {
            glottal_phase += f0 / sr;
            let pulse = if glottal_phase >= 1.0 {
                glottal_phase -= 1.0;
                1.0_f32
            } else {
                0.0_f32
            };
            glottal_lp = 0.92 * glottal_lp + pulse;
            input[i] = (glottal_lp - 0.25) * 0.6;
        }

        proc.process_block(&input, &mut out_l, &mut out_r);

        assert!(proc.pitch.voiced, "Voz aguda contínua deve ser detectada como voiced");
        assert!(
            (proc.pitch.current_f0 - f0).abs() < 10.0,
            "F0 detectado deve ser próximo de 700 Hz (detectado={:.2})",
            proc.pitch.current_f0
        );

        // Verifica que o sinal não apresenta descontinuidades espúrias
        let steady = &out_l[8192..16384];
        let max_val = steady.iter().fold(0.0_f32, |m, &x| m.max(x.abs()));
        assert!(max_val > 0.05 && max_val < 1.0, "Sinal deve ter amplitude válida sem ceifar (max={max_val})");
    }



    #[test]
    fn test_sq_diff_simd_vs_fallback() {
        let mut rng = crate::dsp::FastRng::new(12345);
        for len in [16, 31, 32, 63, 64, 127, 128, 255, 256, 512, 1024] {
            let mut a = vec![0.0_f32; len];
            let mut b = vec![0.0_f32; len];
            for i in 0..len {
                a[i] = rng.next_bipolar();
                b[i] = rng.next_bipolar();
            }
            let res_fallback = crate::dsp::sq_diff_fallback(&a, &b);
            let res_simd = crate::dsp::sq_diff_simd(&a, &b);
            let diff = (res_simd - res_fallback).abs();
            let rel = diff / res_fallback.max(1e-6);
            println!("len {}: fallback={}, simd={}, diff={}, rel={}", len, res_fallback, res_simd, diff, rel);
            assert!(rel < 1e-4, "SIMD mismatch at len {len}: simd={res_simd}, fallback={res_fallback}");
        }
    }

    #[test]
    fn test_legato_note_transition_has_no_transient_clicks() {
        let sr = 44_100.0;
        let mut proc = DecaBuddyProcessor::new(sr);
        proc.host_offline = true;
        proc.params.direct.enabled = true;
        proc.params.harmonies[0].enabled = true;
        proc.params.harmonies[0].scale = ScaleId::High3rdModal1;

        // C4 (261.63 Hz) transicionando legato para D4 (293.66 Hz)
        let n1 = (0.4 * sr) as usize;
        let n2 = (0.4 * sr) as usize;
        let total = n1 + n2;
        let mut input = vec![0.0_f32; total];

        for i in 0..n1 {
            let t = i as f32 / sr;
            input[i] = ((2.0 * PI * 261.63 * t).sin() * 0.6 + (2.0 * PI * 523.26 * t).sin() * 0.3) * 0.7;
        }
        for i in 0..n2 {
            let t = (n1 + i) as f32 / sr;
            input[n1 + i] = ((2.0 * PI * 293.66 * t).sin() * 0.6 + (2.0 * PI * 587.32 * t).sin() * 0.3) * 0.7;
        }

        let mut out_l = vec![0.0_f32; total];
        let mut out_r = vec![0.0_f32; total];

        for ((in_blk, l_blk), r_blk) in input.chunks(128).zip(out_l.chunks_mut(128)).zip(out_r.chunks_mut(128)) {
            proc.process_block(in_blk, l_blk, r_blk);
        }

        let latency = proc.latency_samples() as usize;
        let trans_out = n1 + latency;

        // Verifica a janela de transição (+/- 1500 amostras)
        let w_start = trans_out.saturating_sub(1500);
        let w_end = (trans_out + 1500).min(total - 1);
        let mut max_diff = 0.0_f32;
        for i in w_start..w_end {
            let d = (out_l[i + 1] - out_l[i]).abs();
            if d > max_diff {
                max_diff = d;
            }
        }

        assert!(
            max_diff < 0.12,
            "Transição de nota não deve conter picos de derivada ou cliques (max_diff={max_diff:.4})"
        );
    }
}



