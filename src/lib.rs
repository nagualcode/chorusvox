//! Heave: harmonizador vocal natural.
//!
//! O DSP (`dsp`) nao depende do framework e e o mesmo usado pelo CLI e pelos testes; aqui fica
//! a ponte com o truce (parametros automatizaveis, MIDI, medidores e a interface).

pub mod dsp;
#[cfg(feature = "truce-plugin")]
pub mod gui;

#[cfg(feature = "truce-plugin")]
pub mod truce_integration {
    use crate::dsp::harmony::{Mode, Voicing};
    use crate::dsp::{Engine, EngineParams};
    use std::sync::Arc;
    use truce::prelude::*;

    #[derive(ParamEnum, Debug)]
    pub enum ModeChoice {
        #[name = "Chord"] V0,
        #[name = "Scale"] V1,
        #[name = "MIDI Chord"] V2,
        #[name = "MIDI Notes"] V3,
    }

    #[derive(ParamEnum, Debug)]
    pub enum RootChoice {
        #[name = "C"] V0,
        #[name = "C#"] V1,
        #[name = "D"] V2,
        #[name = "Eb"] V3,
        #[name = "E"] V4,
        #[name = "F"] V5,
        #[name = "F#"] V6,
        #[name = "G"] V7,
        #[name = "Ab"] V8,
        #[name = "A"] V9,
        #[name = "Bb"] V10,
        #[name = "B"] V11,
    }

    #[derive(ParamEnum, Debug)]
    pub enum ChordChoice {
        #[name = "Maj"] V0,
        #[name = "Min"] V1,
        #[name = "7"] V2,
        #[name = "Maj7"] V3,
        #[name = "m7"] V4,
        #[name = "6"] V5,
        #[name = "m6"] V6,
        #[name = "sus2"] V7,
        #[name = "sus4"] V8,
        #[name = "add9"] V9,
        #[name = "9"] V10,
        #[name = "m9"] V11,
        #[name = "dim"] V12,
        #[name = "dim7"] V13,
        #[name = "aug"] V14,
        #[name = "m7b5"] V15,
    }

    #[derive(ParamEnum, Debug)]
    pub enum ScaleChoice {
        #[name = "Major"] V0,
        #[name = "Minor"] V1,
        #[name = "Dorian"] V2,
        #[name = "Mixolydian"] V3,
        #[name = "Harm. Minor"] V4,
    }

    #[derive(ParamEnum, Debug)]
    pub enum VoicingChoice {
        #[name = "Above"] V0,
        #[name = "Below"] V1,
        #[name = "Around"] V2,
        #[name = "Wide"] V3,
    }

    #[derive(Params)]
    pub struct HeaveParams {
        #[param(name = "Mode", default = 0)] pub mode: EnumParam<ModeChoice>,
        #[param(name = "Root", default = 0)] pub root: EnumParam<RootChoice>,
        #[param(name = "Chord", default = 0)] pub chord: EnumParam<ChordChoice>,
        #[param(name = "Scale", default = 0)] pub scale: EnumParam<ScaleChoice>,
        #[param(name = "Voicing", default = 0)] pub voicing: EnumParam<VoicingChoice>,
        #[param(name = "Voices", range = "linear(1, 4)", default = 2)] pub voices: IntParam,
        #[param(name = "Harmony", range = "linear(-60, 6)", default = -4.0, unit = "dB")] pub harmony: FloatParam,
        #[param(name = "Lead", range = "linear(-60, 6)", default = 0.0, unit = "dB")] pub lead: FloatParam,
        #[param(name = "Humanize", range = "linear(0, 1)", default = 0.5, unit = "%")] pub humanize: FloatParam,
        #[param(name = "Width", range = "linear(0, 1)", default = 0.7, unit = "%")] pub width: FloatParam,
        #[param(name = "Character", range = "linear(-1, 1)", default = 0.0, unit = "%")] pub character: FloatParam,
        #[param(name = "Pure Tuning", default = true)] pub pure: BoolParam,
        #[param(name = "Double", default = false)] pub double: BoolParam,
        #[param(name = "Bypass", default = false)] pub bypass: BoolParam,

        #[meter] pub meter_in: MeterSlot,
        #[meter] pub meter_out_l: MeterSlot,
        #[meter] pub meter_out_r: MeterSlot,
        /// Nota MIDI (fracionaria) do lead; 0 = sem voz.
        #[meter] pub meter_lead: MeterSlot,
        #[meter] pub meter_v1: MeterSlot,
        #[meter] pub meter_v2: MeterSlot,
        #[meter] pub meter_v3: MeterSlot,
        #[meter] pub meter_v4: MeterSlot,
        /// Notas MIDI seguradas (mascara de classes de altura, para a interface).
        #[meter] pub meter_midi: MeterSlot,
    }

    impl HeaveParams {
        pub fn engine_params(&self) -> EngineParams {
            EngineParams {
                mode: Mode::from_index(self.mode.index() as usize),
                root: self.root.index() as u8,
                quality: self.chord.index() as u8,
                scale: self.scale.index() as u8,
                voicing: Voicing::from_index(self.voicing.index() as usize),
                voices: self.voices.value_i32().clamp(1, 4) as u8,
                harmony_db: self.harmony.read(),
                lead_db: self.lead.read(),
                humanize: self.humanize.read(),
                width: self.width.read(),
                formant: self.character.read(),
                pure: self.pure.value(),
                double: self.double.value(),
                bypass: self.bypass.value(),
            }
        }
    }

    pub struct HeaveDsp {
        engine: Engine,
        in_l: Vec<f32>,
        in_r: Vec<f32>,
        out_l: Vec<f32>,
        out_r: Vec<f32>,
        midi_mask: u16,
        held: [bool; 128],
    }

    impl Default for HeaveDsp {
        fn default() -> Self {
            Self {
                engine: Engine::new(48_000.0),
                in_l: vec![0.0; 4096],
                in_r: vec![0.0; 4096],
                out_l: vec![0.0; 4096],
                out_r: vec![0.0; 4096],
                midi_mask: 0,
                held: [false; 128],
            }
        }
    }

    impl HeaveDsp {
        fn update_mask(&mut self) {
            self.midi_mask = self.held.iter().enumerate().filter(|(_, &h)| h).fold(0u16, |m, (n, _)| m | 1 << (n % 12));
        }
    }

    pub struct Heave;

    impl PluginLogic for Heave {
        type Params = HeaveParams;
        type DspState = HeaveDsp;

        fn init(_params: &Self::Params, _cx: &InitContext) -> Self::DspState { HeaveDsp::default() }

        fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
            // reconstruir o motor so quando a taxa muda (reset tambem chega em play/stop)
            if (state.engine.sample_rate() - config.sample_rate as f32).abs() > 0.5 {
                state.engine = Engine::new(config.sample_rate as f32);
            }
            let max_blk = config.max_block_size.max(4096);
            for v in [&mut state.in_l, &mut state.in_r, &mut state.out_l, &mut state.out_r] {
                v.resize(max_blk, 0.0);
            }
        }

        fn latency(state: &Self::DspState) -> u32 { state.engine.latency() as u32 }

        fn process(
            state: &mut Self::DspState,
            params: &Self::Params,
            buffer: &mut AudioBuffer,
            events: &EventList,
            ctx: &mut ProcessContext,
        ) -> ProcessStatus {
            let n = buffer.num_samples();
            state.engine.set_params(&params.engine_params());
            // MIDI: os eventos entram na linha do tempo com o deslocamento exato no bloco
            for event in events.iter() {
                let at = (event.sample_offset as usize).min(n);
                match &event.body {
                    EventBody::NoteOn { note, velocity, .. } => {
                        let vel = *velocity;
                        if vel > 0 {
                            state.held[(*note & 0x7F) as usize] = true;
                            state.engine.note_on(at, *note, vel);
                        } else {
                            state.held[(*note & 0x7F) as usize] = false;
                            state.engine.note_off(at, *note);
                        }
                    }
                    EventBody::NoteOff { note, .. } => {
                        state.held[(*note & 0x7F) as usize] = false;
                        state.engine.note_off(at, *note);
                    }
                    EventBody::ControlChange { cc, .. } if *cc == 123 || *cc == 120 => {
                        state.held = [false; 128];
                        state.engine.all_notes_off(at);
                    }
                    _ => {}
                }
            }
            state.update_mask();

            let mut peaks = [0.0f32; 3];
            let mut pos = 0;
            while pos < n {
                let m = (n - pos).min(state.in_l.len());
                let range = pos..pos + m;
                let ch_in = buffer.num_input_channels();
                if ch_in >= 2 {
                    state.in_l[..m].copy_from_slice(&buffer.input(0)[range.clone()]);
                    state.in_r[..m].copy_from_slice(&buffer.input(1)[range.clone()]);
                } else if ch_in == 1 {
                    state.in_l[..m].copy_from_slice(&buffer.input(0)[range.clone()]);
                    state.in_r[..m].copy_from_slice(&buffer.input(0)[range.clone()]);
                } else {
                    state.in_l[..m].fill(0.0);
                    state.in_r[..m].fill(0.0);
                }
                let (il, ir, ol, or) = (&state.in_l[..m], &state.in_r[..m], &mut state.out_l[..m], &mut state.out_r[..m]);
                state.engine.process(il, ir, ol, or);
                let pk = |s: &[f32]| s.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
                peaks[0] = peaks[0].max(pk(il).max(pk(ir)));
                peaks[1] = peaks[1].max(pk(ol));
                peaks[2] = peaks[2].max(pk(or));
                let ch_out = buffer.num_output_channels();
                if ch_out >= 2 {
                    buffer.output(0)[range.clone()].copy_from_slice(&state.out_l[..m]);
                    buffer.output(1)[range].copy_from_slice(&state.out_r[..m]);
                } else if ch_out == 1 {
                    for (o, (&l, &r)) in buffer.output(0)[range].iter_mut().zip(state.out_l[..m].iter().zip(&state.out_r[..m])) {
                        *o = 0.5 * (l + r);
                    }
                }
                pos += m;
            }

            ctx.set_meter(P::MeterIn, peaks[0]);
            ctx.set_meter(P::MeterOutL, peaks[1]);
            ctx.set_meter(P::MeterOutR, peaks[2]);
            ctx.set_meter(P::MeterLead, state.engine.meter_lead);
            let mv = state.engine.meter_voices;
            ctx.set_meter(P::MeterV1, mv[0]);
            ctx.set_meter(P::MeterV2, mv[1]);
            ctx.set_meter(P::MeterV3, mv[2]);
            ctx.set_meter(P::MeterV4, mv[3]);
            ctx.set_meter(P::MeterMidi, state.midi_mask as f32);
            ProcessStatus::Normal
        }

        fn editor(params: Arc<Self::Params>) -> Box<dyn Editor> {
            Box::new(
                truce_egui::EguiEditor::with_ui(params, (crate::gui::WIDTH, crate::gui::HEIGHT), crate::gui::HeaveGui::default())
                    .with_visuals(crate::gui::visuals())
                    .resizable(false),
            )
        }
    }

    pub use HeaveParamsParamId as P;

    truce::plugin! { logic: Heave, params: HeaveParams }
}
