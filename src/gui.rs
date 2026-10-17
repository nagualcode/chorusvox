//! Interface do Heaven (egui): plana, escura e compacta, inspirada nos devices do Ableton Live.
//! Tres paineis: HARMONY (de onde vem a harmonia), VOICES (teclado ao vivo, quantas vozes e onde)
//! e MIX (cinco botoes). Tudo e parametro do host, entao tudo pode ser automatizado no Live.

use crate::dsp::harmony::{HarmonyCtx, Mode, Voicing, CHORD_NAMES, SCALE_NAMES};
use crate::truce_integration::{HeavenParams, P};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::HashMap;
use truce::params::ParamInfo;
use truce::prelude::*;
use truce_egui::EditorUi;

pub const WIDTH: u32 = 820;
pub const HEIGHT: u32 = 300;

const BG: Color32 = Color32::from_rgb(30, 30, 30);
const PANEL: Color32 = Color32::from_rgb(40, 40, 40);
const CELL: Color32 = Color32::from_rgb(52, 52, 52);
const CELL_HI: Color32 = Color32::from_rgb(66, 66, 66);
const LINE: Color32 = Color32::from_rgb(70, 70, 70);
const TEXT: Color32 = Color32::from_rgb(224, 224, 224);
const DIM: Color32 = Color32::from_rgb(130, 130, 130);
const DARK_TEXT: Color32 = Color32::from_rgb(20, 20, 20);
const ACCENT: Color32 = Color32::from_rgb(255, 118, 77);
const CLIP: Color32 = Color32::from_rgb(235, 72, 72);
const VOICE_COLORS: [Color32; 4] = [
    Color32::from_rgb(92, 176, 255),
    Color32::from_rgb(110, 214, 140),
    Color32::from_rgb(246, 200, 80),
    Color32::from_rgb(196, 140, 255),
];
const ROOTS: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

type Ctx = PluginContext<HeavenParams>;

#[derive(Default)]
pub struct HeavenGui {
    infos: HashMap<u32, ParamInfo>,
}

impl HeavenGui {
    fn ensure_infos(&mut self, state: &Ctx) {
        if self.infos.is_empty() {
            for info in state.params().param_infos() {
                self.infos.insert(info.id, info);
            }
        }
    }

    fn set_plain(&self, state: &Ctx, id: u32, plain: f64) {
        if let Some(info) = self.infos.get(&id) {
            state.automate(id, info.range.normalize(plain));
        }
    }

    // ------------------------------------------------------------------------------ widgets

    /// Knob: arco de 270 graus, rotulo acima e valor abaixo. Arraste vertical (Shift = fino),
    /// duplo clique volta ao padrao.
    fn knob(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, label: &str, bipolar: bool) {
        let size = Vec2::new(58.0, 64.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let info = self.infos.get(&id).copied();
        let mut value = state.get_param(id);
        let acc_id = ui.make_persistent_id(("heaven-knob", id));
        if resp.drag_started() {
            state.begin_edit(id);
            ui.memory_mut(|m| m.data.insert_temp(acc_id, value));
        }
        if resp.dragged() {
            let fine = ui.input(|i| i.modifiers.shift);
            let delta = -resp.drag_delta().y / if fine { 700.0 } else { 180.0 };
            let prev: f32 = ui.memory(|m| m.data.get_temp(acc_id)).unwrap_or(value);
            let un = (prev + delta).clamp(0.0, 1.0);
            ui.memory_mut(|m| m.data.insert_temp(acc_id, un));
            if (un - value).abs() > 1e-6 {
                value = un;
                state.set_param(id, f64::from(value));
            }
        }
        if resp.drag_stopped() {
            state.end_edit(id);
            ui.memory_mut(|m| m.data.remove_temp::<f32>(acc_id));
        }
        if resp.double_clicked() {
            if let Some(i) = info {
                state.automate(id, i.range.normalize(i.default_plain));
            }
        }
        let p = ui.painter_at(rect.expand(2.0));
        p.text(Pos2::new(rect.center().x, rect.top()), Align2::CENTER_TOP, label, FontId::proportional(10.0), DIM);
        let c = Pos2::new(rect.center().x, rect.top() + 31.0);
        let r = 15.0;
        let start = std::f32::consts::FRAC_PI_4 * 3.0;
        let sweep = std::f32::consts::FRAC_PI_2 * 3.0;
        let hovered = resp.hovered() || resp.dragged();
        p.add(egui::Shape::line(arc(c, r, start, sweep), Stroke::new(3.0_f32, if hovered { CELL_HI } else { LINE })));
        let (a0, a1) = if bipolar {
            let mid = start + sweep * 0.5;
            let v = start + sweep * value;
            (mid.min(v), mid.max(v))
        } else {
            (start, start + sweep * value)
        };
        if a1 - a0 > 0.01 {
            p.add(egui::Shape::line(arc(c, r, a0, a1 - a0), Stroke::new(3.0_f32, ACCENT)));
        }
        let ang = start + sweep * value;
        p.line_segment([c, c + Vec2::angled(ang) * (r - 4.0)], Stroke::new(1.8_f32, TEXT));
        p.text(
            Pos2::new(rect.center().x, rect.bottom()),
            Align2::CENTER_BOTTOM,
            state.format_param(id),
            FontId::proportional(10.5),
            if hovered { ACCENT } else { TEXT },
        );
    }

    /// Grade de botoes para um parametro discreto (um botao por valor).
    #[allow(clippy::too_many_arguments)]
    fn grid(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, labels: &[&str], cols: usize, width: f32, cell_h: f32, color: Color32) {
        let Some(info) = self.infos.get(&id).copied() else { return };
        let rows = labels.len().div_ceil(cols);
        let gap = 2.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, rows as f32 * (cell_h + gap) - gap), Sense::hover());
        let w = (width - gap * (cols as f32 - 1.0)) / cols as f32;
        let cur = state.get_param_plain(id).round() as i64;
        let min = info.range.min().round() as i64;
        for (k, lab) in labels.iter().enumerate() {
            let (r, c) = (k / cols, k % cols);
            let cell = Rect::from_min_size(
                Pos2::new(rect.left() + c as f32 * (w + gap), rect.top() + r as f32 * (cell_h + gap)),
                Vec2::new(w, cell_h),
            );
            let resp = ui.interact(cell, ui.id().with(("grid", id, k)), Sense::click());
            let on = cur == min + k as i64;
            let fill = if on { color } else if resp.hovered() { CELL_HI } else { CELL };
            ui.painter().rect_filled(cell, CornerRadius::same(2), fill);
            ui.painter().text(cell.center(), Align2::CENTER_CENTER, *lab, FontId::proportional(11.0), if on { DARK_TEXT } else { TEXT });
            if resp.clicked() {
                self.set_plain(state, id, (min + k as i64) as f64);
            }
        }
    }

    fn toggle(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, label: &str, width: f32) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::click());
        let on = state.get_param(id) > 0.5;
        let fill = if on { ACCENT } else if resp.hovered() { CELL_HI } else { CELL };
        ui.painter().rect_filled(rect, CornerRadius::same(2), fill);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(10.5), if on { DARK_TEXT } else { TEXT });
        if resp.clicked() {
            state.automate(id, if on { 0.0 } else { 1.0 });
        }
    }

    fn meter(ui: &mut egui::Ui, state: &Ctx, ids: &[u32], height: f32) {
        let w = 5.0 * ids.len() as f32 + 2.0 * (ids.len() as f32 - 1.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
        for (k, &id) in ids.iter().enumerate() {
            let x = rect.left() + k as f32 * 7.0;
            let bar = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(5.0, height));
            ui.painter().rect_filled(bar, CornerRadius::same(1), CELL);
            let pk = state.get_meter(id);
            let db = 20.0 * pk.max(1e-6).log10();
            let v = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
            if v > 0.001 {
                let fill = Rect::from_min_max(Pos2::new(x, bar.bottom() - height * v), bar.max);
                ui.painter().rect_filled(fill, CornerRadius::same(1), if pk > 0.98 { CLIP } else { Color32::from_rgb(110, 220, 120) });
            }
        }
    }

    // ------------------------------------------------------------------------------ secoes

    fn header(&self, ui: &mut egui::Ui, state: &Ctx) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("HEAVEN").size(16.0).strong().color(TEXT));
            ui.label(egui::RichText::new("vocal harmony").size(10.0).color(DIM));
            ui.add_space(14.0);
            self.grid(ui, state, P::Mode.into(), &["CHORD", "SCALE", "MIDI CHORD", "MIDI NOTES"], 4, 330.0, 20.0, ACCENT);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                Self::meter(ui, state, &[P::MeterOutL.into(), P::MeterOutR.into()], 18.0);
                ui.label(egui::RichText::new("OUT").size(9.0).color(DIM));
                Self::meter(ui, state, &[P::MeterIn.into()], 18.0);
                ui.label(egui::RichText::new("IN").size(9.0).color(DIM));
                ui.add_space(8.0);
                self.toggle(ui, state, P::Bypass.into(), "BYPASS", 60.0);
            });
        });
    }

    fn harmony_panel(&self, ui: &mut egui::Ui, state: &Ctx, mode: Mode) {
        let w = ui.available_width();
        match mode {
            Mode::Chord => {
                caption(ui, "ROOT");
                self.grid(ui, state, P::Root.into(), &ROOTS, 6, w, 20.0, ACCENT);
                ui.add_space(4.0);
                caption(ui, "CHORD");
                self.grid(ui, state, P::Chord.into(), &CHORD_NAMES, 4, w, 20.0, ACCENT);
            }
            Mode::Scale => {
                caption(ui, "KEY");
                self.grid(ui, state, P::Root.into(), &ROOTS, 6, w, 20.0, ACCENT);
                ui.add_space(4.0);
                caption(ui, "SCALE");
                self.grid(ui, state, P::Scale.into(), &SCALE_NAMES, 3, w, 20.0, ACCENT);
                ui.add_space(6.0);
                hint(ui, "Harmonies follow the key in diatonic thirds.\nNo chords needed.");
            }
            Mode::MidiChord | Mode::MidiNotes => {
                caption(ui, "MIDI INPUT");
                let mask = state.get_meter(P::MeterMidi) as u32;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 42.0), Sense::hover());
                let cw = (w - 11.0 * 2.0) / 12.0;
                for k in 0..12 {
                    let r = Rect::from_min_size(Pos2::new(rect.left() + k as f32 * (cw + 2.0), rect.top()), Vec2::new(cw, 42.0));
                    let on = mask & (1 << k) != 0;
                    ui.painter().rect_filled(r, CornerRadius::same(2), if on { ACCENT } else { CELL });
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        ROOTS[k],
                        FontId::proportional(10.0),
                        if on { DARK_TEXT } else { DIM },
                    );
                }
                ui.add_space(8.0);
                if mode == Mode::MidiChord {
                    hint(ui, "Play or record chords on a MIDI track and set\nits output to this track (MIDI To > Heaven).\nThe voices pick the best notes of the chord.");
                } else {
                    hint(ui, "Each held MIDI note is sung by one voice\n(up to 4). Route a MIDI track to Heaven.");
                }
            }
        }
    }

    fn voices_panel(&self, ui: &mut egui::Ui, state: &Ctx, mode: Mode) {
        let w = ui.available_width();
        // teclado ao vivo
        let lead = state.get_meter(P::MeterLead);
        let voices = [P::MeterV1, P::MeterV2, P::MeterV3, P::MeterV4].map(|m| state.get_meter(m));
        let params = state.params();
        let ctx = HarmonyCtx {
            mode,
            root: params.root.index() as u8,
            quality: params.chord.index() as u8,
            scale: params.scale.index() as u8,
            voicing: Voicing::from_index(params.voicing.index() as usize),
            voices: 4,
            ..Default::default()
        };
        let chord_mask = match mode {
            Mode::Chord => ctx.chord_for(60).0,
            Mode::Scale => ctx.chord_for(60).1,
            _ => state.get_meter(P::MeterMidi) as u16,
        };
        keyboard(ui, w, 62.0, lead, &voices, chord_mask);
        // leitura ao vivo: lead -> vozes
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(egui::RichText::new("LEAD").size(9.0).color(DIM));
            ui.label(egui::RichText::new(note_name(lead)).size(12.0).strong().color(TEXT));
            ui.label(egui::RichText::new(">").size(11.0).color(DIM));
            for (v, &m) in voices.iter().enumerate() {
                if m > 0.0 {
                    ui.label(egui::RichText::new(note_name(m)).size(12.0).strong().color(VOICE_COLORS[v]));
                }
            }
        });
        ui.add_space(2.0);
        if mode == Mode::MidiNotes {
            caption(ui, "VOICES");
            hint(ui, "One voice per held MIDI note.");
            ui.add_space(26.0);
        } else {
            caption(ui, "VOICES");
            self.grid(ui, state, P::Voices.into(), &["1", "2", "3", "4"], 4, w, 22.0, ACCENT);
            ui.add_space(4.0);
            caption(ui, "VOICING");
            self.grid(ui, state, P::Voicing.into(), &["Above", "Below", "Around", "Wide"], 4, w, 22.0, ACCENT);
        }
    }

    fn mix_panel(&self, ui: &mut egui::Ui, state: &Ctx) {
        ui.horizontal(|ui| {
            self.knob(ui, state, P::Harmony.into(), "HARMONY", false);
            self.knob(ui, state, P::Lead.into(), "LEAD", false);
            self.knob(ui, state, P::Humanize.into(), "HUMANIZE", false);
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            self.knob(ui, state, P::Width.into(), "WIDTH", false);
            self.knob(ui, state, P::Character.into(), "CHARACTER", true);
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            self.toggle(ui, state, P::Pure.into(), "PURE TUNING", 92.0);
            self.toggle(ui, state, P::Double.into(), "DOUBLE", 80.0);
        });
    }
}

impl EditorUi<HeavenParams> for HeavenGui {
    fn ui(&mut self, ui: &mut egui::Ui, state: &Ctx) {
        self.ensure_infos(state);
        ui.ctx().request_repaint();
        let mode = Mode::from_index(state.params().mode.index() as usize);
        egui::Frame::NONE.fill(BG).inner_margin(10).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            self.header(ui, state);
            ui.add_space(4.0);
            let (area, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let widths = [290.0, 278.0, area.width() - 290.0 - 278.0 - 16.0];
            let titles = ["HARMONY", "VOICES", "MIX"];
            let mut x = area.left();
            for (k, &pw) in widths.iter().enumerate() {
                let rect = Rect::from_min_size(Pos2::new(x, area.top()), Vec2::new(pw, area.height()));
                ui.painter().rect_filled(rect, CornerRadius::same(3), PANEL);
                let inner = rect.shrink2(Vec2::new(10.0, 8.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
                child.spacing_mut().item_spacing = Vec2::new(6.0, 3.0);
                child.label(egui::RichText::new(titles[k]).size(11.0).strong().color(TEXT));
                child.add_space(2.0);
                match k {
                    0 => self.harmony_panel(&mut child, state, mode),
                    1 => self.voices_panel(&mut child, state, mode),
                    _ => self.mix_panel(&mut child, state),
                }
                x += pw + 8.0;
            }
        });
    }
}

fn caption(ui: &mut egui::Ui, text: &str) { ui.label(egui::RichText::new(text).size(9.0).color(DIM)); }

fn hint(ui: &mut egui::Ui, text: &str) { ui.label(egui::RichText::new(text).size(10.0).color(DIM)); }

fn arc(c: Pos2, r: f32, start: f32, sweep: f32) -> Vec<Pos2> {
    let n = ((sweep.abs() / std::f32::consts::TAU) * 64.0).ceil().max(2.0) as usize;
    (0..=n).map(|i| c + Vec2::angled(start + sweep * i as f32 / n as f32) * r).collect()
}

/// Teclado de C2 a B5: notas do acorde/escala marcadas, o lead em branco e cada voz na sua cor.
fn keyboard(ui: &mut egui::Ui, w: f32, h: f32, lead: f32, voices: &[f32; 4], chord_mask: u16) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter_at(rect);
    let (lo, hi) = (36, 84);
    let whites: Vec<i32> = (lo..hi).filter(|n| !is_black(*n)).collect();
    let kw = w / whites.len() as f32;
    let key_x = |n: i32| -> (f32, bool) {
        let idx = whites.iter().filter(|&&x| x < n).count() as f32;
        if is_black(n) { (rect.left() + idx * kw - kw * 0.3, true) } else { (rect.left() + idx * kw, false) }
    };
    let mark = |n: i32| -> Option<Color32> {
        if lead > 0.0 && (lead.round() as i32) == n {
            return Some(TEXT);
        }
        for (v, &m) in voices.iter().enumerate() {
            if m > 0.0 && (m.round() as i32) == n {
                return Some(VOICE_COLORS[v]);
            }
        }
        None
    };
    for n in lo..hi {
        let (x, black) = key_x(n);
        if black {
            continue;
        }
        let r = Rect::from_min_size(Pos2::new(x + 0.5, rect.top()), Vec2::new(kw - 1.0, h));
        let in_chord = chord_mask & (1 << (n % 12)) != 0;
        p.rect_filled(r, CornerRadius::same(1), if in_chord { Color32::from_rgb(214, 170, 150) } else { Color32::from_rgb(168, 168, 168) });
        if let Some(c) = mark(n) {
            p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 1.0, r.bottom() - 16.0), Vec2::new(r.width() - 2.0, 14.0)), CornerRadius::same(2), c);
        }
        if n % 12 == 0 {
            p.text(Pos2::new(r.center().x, r.bottom() - 2.0), Align2::CENTER_BOTTOM, format!("C{}", n / 12 - 1), FontId::proportional(8.0), Color32::from_rgb(60, 60, 60));
        }
    }
    for n in lo..hi {
        let (x, black) = key_x(n);
        if !black {
            continue;
        }
        let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(kw * 0.6, h * 0.58));
        let in_chord = chord_mask & (1 << (n % 12)) != 0;
        p.rect_filled(r, CornerRadius::same(1), if in_chord { Color32::from_rgb(120, 72, 54) } else { Color32::from_rgb(24, 24, 24) });
        if let Some(c) = mark(n) {
            p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 1.0, r.bottom() - 12.0), Vec2::new(r.width() - 2.0, 10.0)), CornerRadius::same(2), c);
        }
    }
}

fn note_name(m: f32) -> String {
    if m <= 0.0 {
        return "—".to_string();
    }
    let n = m.round() as i32;
    format!("{}{}", ROOTS[n.rem_euclid(12) as usize], n / 12 - 1)
}

fn is_black(n: i32) -> bool { matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10) }

pub fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = PANEL;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(2);
        w.bg_stroke = Stroke::NONE;
    }
    v.selection.bg_fill = ACCENT.gamma_multiply(0.5);
    v
}
