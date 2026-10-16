//! Interface do ChorusVox (egui): visual plano, compacto e colorido inspirado no Ableton Live.
//! Cada voz do HV10 e uma faixa vertical com a sua cor (as cores das harmonias do plugin
//! DecaBuddy original: azul, amarelo, verde e laranja); so os controles do modo ativo aparecem.

use crate::truce_integration::{ChorusVoxParams, ChorusVoxParamsParamId as P};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::HashMap;
use truce::params::ParamInfo;
use truce::prelude::*;
use truce_egui::EditorUi;

pub const WIDTH: u32 = 868;
pub const HEIGHT: u32 = 346;
const STRIP_H: f32 = 226.0;
const STRIP_GAP: f32 = 6.0;

// --- Paleta (tema escuro do Live) ---
const BG: Color32 = Color32::from_rgb(30, 30, 30);
const PANEL: Color32 = Color32::from_rgb(42, 42, 42);
const PANEL_HI: Color32 = Color32::from_rgb(54, 54, 54);
const LINE: Color32 = Color32::from_rgb(64, 64, 64);
const TEXT: Color32 = Color32::from_rgb(222, 222, 222);
const DIM: Color32 = Color32::from_rgb(136, 136, 136);
const DARK_TEXT: Color32 = Color32::from_rgb(24, 24, 24);
const ACCENT: Color32 = Color32::from_rgb(255, 118, 77);
const GLOBAL: Color32 = Color32::from_rgb(170, 146, 255);
const CLIP: Color32 = Color32::from_rgb(235, 72, 72);

const VOICE_COLORS: [Color32; 5] = [
    Color32::from_rgb(208, 208, 208), // ORIGIN
    Color32::from_rgb(74, 144, 255),  // L2
    Color32::from_rgb(245, 197, 66),  // L1
    Color32::from_rgb(63, 191, 106),  // H1
    Color32::from_rgb(255, 138, 61),  // H2
];
const VOICE_NAMES: [&str; 5] = ["ORIGIN", "L2", "L1", "H1", "H2"];
const VOICE_SUB: [&str; 5] = ["original voice", "low 2", "low 1", "high 1", "high 2"];

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

struct PartIds {
    buddies: u32,
    style: u32,
    sub_style: u32,
    modal: u32,
    formant: u32,
    level: u32,
    pan: u32,
    pcorr: u32,
    rdelay: u32,
    prio: u32,
}

fn part_ids(i: usize) -> PartIds {
    let ids = match i {
        0 => [P::L2Buddies, P::L2Style, P::L2SubStyle, P::L2Modal, P::L2Formant, P::L2Level, P::L2Pan, P::L2Pcorr, P::L2Rdelay, P::L2Prio],
        1 => [P::L1Buddies, P::L1Style, P::L1SubStyle, P::L1Modal, P::L1Formant, P::L1Level, P::L1Pan, P::L1Pcorr, P::L1Rdelay, P::L1Prio],
        2 => [P::H1Buddies, P::H1Style, P::H1SubStyle, P::H1Modal, P::H1Formant, P::H1Level, P::H1Pan, P::H1Pcorr, P::H1Rdelay, P::H1Prio],
        _ => [P::H2Buddies, P::H2Style, P::H2SubStyle, P::H2Modal, P::H2Formant, P::H2Level, P::H2Pan, P::H2Pcorr, P::H2Rdelay, P::H2Prio],
    };
    let [buddies, style, sub_style, modal, formant, level, pan, pcorr, rdelay, prio] = ids.map(u32::from);
    PartIds { buddies, style, sub_style, modal, formant, level, pan, pcorr, rdelay, prio }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Voices,
    UserStyles,
}

pub struct HvGui {
    infos: HashMap<u32, ParamInfo>,
    by_name: HashMap<&'static str, u32>,
    tab: Tab,
    user_sel: usize,
}

impl Default for HvGui {
    fn default() -> Self {
        Self { infos: HashMap::new(), by_name: HashMap::new(), tab: Tab::Voices, user_sel: 0 }
    }
}

type Ctx = PluginContext<ChorusVoxParams>;

impl HvGui {
    fn ensure_infos(&mut self, state: &Ctx) {
        if self.infos.is_empty() {
            for info in state.params().param_infos() {
                self.by_name.insert(info.name, info.id);
                self.infos.insert(info.id, info);
            }
        }
    }

    fn plain(state: &Ctx, id: u32) -> f32 { state.get_param_plain(id) }

    fn set_plain(&self, state: &Ctx, id: u32, plain: f64) {
        if let Some(info) = self.infos.get(&id) {
            state.automate(id, info.range.normalize(plain));
        }
    }

    fn label_of(state: &Ctx, id: u32, plain: f64) -> String {
        state.params().format_value(id, plain).unwrap_or_else(|| format!("{plain:.0}"))
    }

    // -----------------------------------------------------------------------------------------
    // Widgets
    // -----------------------------------------------------------------------------------------

    /// Knob compacto: arco de 270 graus na cor da voz, valor abaixo, rotulo acima.
    /// Arraste vertical; duplo clique volta ao valor padrao. `bipolar` desenha a partir do centro.
    fn knob(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, label: &str, color: Color32, bipolar: bool) {
        let size = Vec2::new(46.0, 56.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let info = self.infos.get(&id).copied();
        let mut value = state.get_param(id);
        let steps = info.and_then(|i| i.range.step_count()).map(|s| s.get() as f32);
        let acc_id = ui.make_persistent_id(("hv-knob", id));

        if resp.drag_started() {
            state.begin_edit(id);
            ui.memory_mut(|m| m.data.insert_temp(acc_id, value));
        }
        if resp.dragged() {
            let fine = ui.input(|i| i.modifiers.shift);
            let delta = -resp.drag_delta().y / if fine { 600.0 } else { 160.0 };
            let prev: f32 = ui.memory(|m| m.data.get_temp(acc_id)).unwrap_or(value);
            let un = (prev + delta).clamp(0.0, 1.0);
            ui.memory_mut(|m| m.data.insert_temp(acc_id, un));
            let snapped = match steps { Some(n) => (un * n).round() / n, None => un };
            if (snapped - value).abs() > 1e-5 {
                value = snapped;
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
        p.text(Pos2::new(rect.center().x, rect.top()), Align2::CENTER_TOP, label, FontId::proportional(9.0), DIM);
        let c = Pos2::new(rect.center().x, rect.top() + 26.0);
        let r = 12.0;
        let start = std::f32::consts::FRAC_PI_4 * 3.0;
        let sweep = std::f32::consts::FRAC_PI_2 * 3.0;
        let hovered = resp.hovered() || resp.dragged();
        p.add(egui::Shape::line(arc(c, r, start, sweep), Stroke::new(3.0_f32, if hovered { LINE.gamma_multiply(1.4) } else { LINE })));
        let (a0, a1) = if bipolar {
            let mid = start + sweep * 0.5;
            let v = start + sweep * value;
            (mid.min(v), mid.max(v))
        } else {
            (start, start + sweep * value)
        };
        if a1 - a0 > 0.01 {
            p.add(egui::Shape::line(arc(c, r, a0, a1 - a0), Stroke::new(3.0_f32, color)));
        }
        let ang = start + sweep * value;
        p.line_segment([c, c + Vec2::angled(ang) * (r - 4.0)], Stroke::new(1.6_f32, TEXT));
        p.text(
            Pos2::new(rect.center().x, rect.bottom()),
            Align2::CENTER_BOTTOM,
            state.format_param(id),
            FontId::proportional(10.0),
            if hovered { color } else { TEXT },
        );
    }

    /// Botoes segmentados para parametros discretos (BUDDIES, Priority, template...).
    fn segmented(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, labels: &[&str], width: f32, color: Color32) {
        let Some(info) = self.infos.get(&id).copied() else { return };
        let n = labels.len();
        let h = 18.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
        let cur = Self::plain(state, id).round() as i64;
        let min = info.range.min().round() as i64;
        let w = width / n as f32;
        for (k, lab) in labels.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(rect.left() + w * k as f32, rect.top()), Vec2::new(w - 1.0, h));
            let resp = ui.interact(r, ui.id().with(("seg", id, k)), Sense::click());
            let on = cur == min + k as i64;
            let fill = if on { color } else if resp.hovered() { PANEL_HI } else { PANEL };
            ui.painter().rect_filled(r, CornerRadius::same(2), fill);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, *lab, FontId::proportional(10.0), if on { DARK_TEXT } else { DIM });
            if resp.clicked() {
                self.set_plain(state, id, (min + k as i64) as f64);
            }
        }
    }

    /// Botao liga/desliga.
    fn toggle(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, label: &str, width: f32, color: Color32) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::click());
        let on = state.get_param(id) > 0.5;
        let fill = if on { color } else if resp.hovered() { PANEL_HI } else { PANEL };
        ui.painter().rect_filled(rect, CornerRadius::same(2), fill);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(10.0), if on { DARK_TEXT } else { DIM });
        if resp.clicked() {
            state.automate(id, if on { 0.0 } else { 1.0 });
        }
    }

    /// Lista suspensa compacta com rotulo pequeno acima.
    fn dropdown(&self, ui: &mut egui::Ui, state: &Ctx, id: u32, label: &str, width: f32, color: Color32) {
        let Some(info) = self.infos.get(&id).copied() else { return };
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(egui::RichText::new(label).size(9.0).color(DIM));
            let count = info.range.step_count_usize() + 1;
            let cur = Self::plain(state, id).round() as i64;
            let min = info.range.min().round() as i64;
            egui::ComboBox::from_id_salt(("hv-dd", id))
                .selected_text(egui::RichText::new(state.format_param(id)).size(11.0).color(color))
                .width(width)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for k in 0..count {
                        let plain = (min + k as i64) as f64;
                        let text = Self::label_of(state, id, plain);
                        if ui.selectable_label(cur == min + k as i64, text).clicked() {
                            self.set_plain(state, id, plain);
                        }
                    }
                });
        });
    }

    fn meter(ui: &mut egui::Ui, state: &Ctx, ids: &[u32], height: f32) {
        ui.ctx().request_repaint();
        let w = 5.0 * ids.len() as f32 + 2.0 * (ids.len() as f32 - 1.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
        for (k, &id) in ids.iter().enumerate() {
            let x = rect.left() + k as f32 * 7.0;
            let bar = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(5.0, height));
            ui.painter().rect_filled(bar, CornerRadius::same(1), PANEL);
            let v = meter_display(state.get_meter(id)).clamp(0.0, 1.0);
            if v > 0.001 {
                let fill = Rect::from_min_max(Pos2::new(x, bar.bottom() - height * v), bar.max);
                ui.painter().rect_filled(fill, CornerRadius::same(1), if v > 0.95 { CLIP } else { Color32::from_rgb(110, 220, 120) });
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Secoes
    // -----------------------------------------------------------------------------------------

    fn header(&mut self, ui: &mut egui::Ui, state: &Ctx) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DECABUDDY").size(15.0).strong().color(TEXT));
            ui.label(egui::RichText::new("HV10 harmony processor").size(10.0).color(DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                Self::meter(ui, state, &[P::MeterOutL.into(), P::MeterOutR.into()], 16.0);
                ui.label(egui::RichText::new("OUT").size(9.0).color(DIM));
                Self::meter(ui, state, &[P::MeterIn.into()], 16.0);
                ui.label(egui::RichText::new("IN").size(9.0).color(DIM));
                ui.add_space(8.0);
                self.toggle(ui, state, P::Bypass.into(), "BYPASS", 58.0, ACCENT);
                ui.add_space(8.0);
                for (tab, name) in [(Tab::UserStyles, "USER STYLES"), (Tab::Voices, "VOICES")] {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(78.0, 18.0), Sense::click());
                    let on = self.tab == tab;
                    ui.painter().rect_filled(rect, CornerRadius::same(2), if on { PANEL_HI } else { BG });
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, name, FontId::proportional(10.0), if on { TEXT } else { DIM });
                    if resp.clicked() {
                        self.tab = tab;
                    }
                }
            });
        });
    }

    fn global_bar(&mut self, ui: &mut egui::Ui, state: &Ctx) {
        let mode = state.params().mode.index();
        panel(ui, PANEL.gamma_multiply(0.75), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                self.dropdown(ui, state, P::Mode.into(), "MODE", 96.0, GLOBAL);
                self.dropdown(ui, state, P::Key.into(), "KEY", 62.0, GLOBAL);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(egui::RichText::new("SUB").size(9.0).color(DIM));
                    ui.add_space(2.0);
                    self.toggle(ui, state, P::Sub.into(), "ON", 36.0, GLOBAL);
                });
                self.dropdown(ui, state, P::SubKey.into(), "SUB KEY", 62.0, GLOBAL);
                ui.add_space(4.0);
                self.knob(ui, state, P::Input.into(), "INPUT", GLOBAL, true);
                self.knob(ui, state, P::Master.into(), "MASTER", GLOBAL, false);
                self.knob(ui, state, P::Tune.into(), "TUNE", GLOBAL, true);
                self.knob(ui, state, P::MidiCh.into(), "MIDI CH", GLOBAL, false);
                ui.add_space(6.0);
                // Painel contextual do modo
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    match mode {
                        1 => {
                            ui.label(egui::RichText::new("MODAL TEMPLATE").size(9.0).color(DIM));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                self.toggle(ui, state, P::Tpl1.into(), "1", 22.0, GLOBAL);
                                self.toggle(ui, state, P::Tpl2.into(), "2", 22.0, GLOBAL);
                                self.segmented(ui, state, P::Tpl3.into(), &["-", "3", "m", "M"], 72.0, GLOBAL);
                                self.toggle(ui, state, P::Tpl4.into(), "4", 22.0, GLOBAL);
                                self.toggle(ui, state, P::Tpl5.into(), "5", 22.0, GLOBAL);
                                self.toggle(ui, state, P::Tpl6.into(), "6", 22.0, GLOBAL);
                                self.segmented(ui, state, P::Tpl7.into(), &["-", "7", "m", "M"], 72.0, GLOBAL);
                            });
                            hint(ui, "chord root from a MIDI note (base channel)");
                        }
                        2 => {
                            ui.label(egui::RichText::new("FLEXICHORD VOICING").size(9.0).color(DIM));
                            self.segmented(ui, state, P::Flexi.into(), &["L L L H", "L L H H", "L H H H"], 246.0, GLOBAL);
                            hint(ui, "play a 3+ note chord on the base channel");
                        }
                        3 => hint(ui, "each part plays its own MIDI channel (base+1 .. base+4)"),
                        4 => hint(ui, "harmony waits for one held note per active part"),
                        5 => hint(ui, "notes assigned in playing order (Priority per part)"),
                        6 => hint(ui, "SMART: modern chord engine with voice leading (not on the HV10)"),
                        _ => hint(ui, "STYLE: harmony from Key + Style; Sub swaps Key / Style"),
                    }
                });
            });
        });
    }

    fn strip(&self, ui: &mut egui::Ui, state: &Ctx, v: usize, mode: u32, frame_rect: Rect) {
        let width = frame_rect.width();
        let color = VOICE_COLORS[v];
        let (on, buddies_id) = if v == 0 {
            (Self::plain(state, P::OrgBuddies.into()) > 0.5, u32::from(P::OrgBuddies))
        } else {
            let ids = part_ids(v - 1);
            (Self::plain(state, ids.buddies) > 0.5, ids.buddies)
        };
        let accent = if on { color } else { color.gamma_multiply(0.45) };

        {
            ui.painter().rect_filled(frame_rect, CornerRadius::same(3), PANEL);
            // Cabecalho colorido
            let head = Rect::from_min_size(frame_rect.min, Vec2::new(width, 24.0));
            ui.painter().rect_filled(head, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, accent);
            ui.painter().text(head.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, VOICE_NAMES[v], FontId::proportional(13.0), DARK_TEXT);
            ui.painter().text(head.right_center() - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER, VOICE_SUB[v], FontId::proportional(9.0), DARK_TEXT.gamma_multiply(0.8));

            ui.add_space(30.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let inner = width - 12.0;
                    if v == 0 {
                        self.segmented(ui, state, buddies_id, &["OFF", "ON", "+UN", "FX"], inner, color);
                    } else {
                        self.segmented(ui, state, buddies_id, &["OFF", "ON", "+UNISON"], inner, color);
                    }

                    // Seletor do modo ativo
                    ui.allocate_ui(Vec2::new(inner, 40.0), |ui| {
                        if v == 0 {
                            hint(ui, "dry voice through the codec");
                            hint(ui, "+UN adds a double, FX = double only");
                        } else {
                            let ids = part_ids(v - 1);
                            match mode {
                                0 => {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        self.dropdown(ui, state, ids.style, "STYLE", inner / 2.0 - 14.0, color);
                                        self.dropdown(ui, state, ids.sub_style, "SUB STYLE", inner / 2.0 - 14.0, color);
                                    });
                                }
                                1 => self.dropdown(ui, state, ids.modal, "MODAL VOICING", inner - 10.0, color),
                                5 => {
                                    ui.label(egui::RichText::new("PRIORITY  (note order)").size(9.0).color(DIM));
                                    self.segmented(ui, state, ids.prio, &["1st", "2nd", "3rd", "4th"], inner, color);
                                }
                                3 => hint(ui, &format!("MIDI channel = base + {}", v)),
                                2 => hint(ui, "nearest chord tones (FlexiChord voicing)"),
                                4 => hint(ui, "held notes, by pitch order"),
                                _ => hint(ui, "SMART chord tone"),
                            }
                        }
                    });

                    // Knobs
                    if v == 0 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            self.knob(ui, state, P::OrgLevel.into(), "LEVEL", color, false);
                            self.knob(ui, state, P::OrgPan.into(), "PAN", color, true);
                            self.knob(ui, state, P::OrgFormant.into(), "FORMANT", color, true);
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            self.knob(ui, state, P::OrgRdelay.into(), "R.DELAY", color, false);
                        });
                    } else {
                        let ids = part_ids(v - 1);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            self.knob(ui, state, ids.level, "LEVEL", color, false);
                            self.knob(ui, state, ids.pan, "PAN", color, true);
                            self.knob(ui, state, ids.formant, "FORMANT", color, true);
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            self.knob(ui, state, ids.pcorr, "P.CORRECT", color, false);
                            self.knob(ui, state, ids.rdelay, "R.DELAY", color, false);
                        });
                    }
                });
            });

            if !on {
                ui.painter().rect_filled(frame_rect, CornerRadius::same(3), Color32::from_black_alpha(90));
            }
        }
    }

    fn user_styles(&mut self, ui: &mut egui::Ui, state: &Ctx) {
        panel(ui, PANEL, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("USER STYLE").size(10.0).color(DIM));
                for u in 0..9 {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(30.0, 18.0), Sense::click());
                    let on = self.user_sel == u;
                    ui.painter().rect_filled(rect, CornerRadius::same(2), if on { GLOBAL } else { PANEL_HI });
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("U{}", u + 1), FontId::proportional(10.0), if on { DARK_TEXT } else { DIM });
                    if resp.clicked() {
                        self.user_sel = u;
                    }
                }
                hint(ui, "drag a bar to set the interval (semitones); double-click = unassigned (--)");
            });
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            let key = state.params().key.index() as usize;
            let row_h = 19.0;
            let width = ui.available_width();
            for (n, note) in NOTE_NAMES.iter().enumerate() {
                let name = format!("U{} {}", self.user_sel + 1, note);
                let Some(&id) = self.by_name.get(name.as_str()) else { continue };
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click_and_drag());
                let p = ui.painter();
                let input_pc = (key + n) % 12;
                p.text(Pos2::new(rect.left() + 4.0, rect.center().y), Align2::LEFT_CENTER, format!("{:<2}", NOTE_NAMES[input_pc]), FontId::monospace(11.0), TEXT);
                let bar = Rect::from_min_max(Pos2::new(rect.left() + 40.0, rect.top() + 2.0), Pos2::new(rect.right() - 150.0, rect.bottom() - 2.0));
                p.rect_filled(bar, CornerRadius::same(2), BG);
                let zero_x = bar.left() + bar.width() * (25.0 / 49.0);
                p.line_segment([Pos2::new(zero_x, bar.top()), Pos2::new(zero_x, bar.bottom())], Stroke::new(1.0_f32, LINE));
                for oct in [-24.0_f32, -12.0, 12.0, 24.0] {
                    let x = bar.left() + bar.width() * ((oct + 25.0) / 49.0);
                    p.line_segment([Pos2::new(x, bar.top() + 4.0), Pos2::new(x, bar.bottom() - 4.0)], Stroke::new(1.0_f32, LINE));
                }
                let v = Self::plain(state, id).round() as i32;
                let col = GLOBAL;
                if v > -25 {
                    let x = bar.left() + bar.width() * ((v as f32 + 25.0) / 49.0);
                    let r = Rect::from_min_max(Pos2::new(zero_x.min(x), bar.top() + 3.0), Pos2::new(zero_x.max(x).max(zero_x.min(x) + 3.0), bar.bottom() - 3.0));
                    p.rect_filled(r, CornerRadius::same(2), col);
                    let target = (input_pc as i32 + v).rem_euclid(12) as usize;
                    p.text(Pos2::new(bar.right() + 8.0, rect.center().y), Align2::LEFT_CENTER,
                        format!("{:+3}  ->  {}", v, NOTE_NAMES[target]), FontId::monospace(11.0), col);
                } else {
                    p.text(Pos2::new(bar.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, " --  (unassigned)", FontId::monospace(11.0), DIM);
                }
                if resp.double_clicked() {
                    self.set_plain(state, id, -25.0);
                } else if resp.dragged() || resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let t = ((pos.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                        let val = (t * 49.0 - 25.0).round().clamp(-24.0, 24.0);
                        if val as i32 != v {
                            self.set_plain(state, id, val as f64);
                        }
                    }
                }
            }
        });
    }
}

impl EditorUi<ChorusVoxParams> for HvGui {
    fn ui(&mut self, ui: &mut egui::Ui, state: &Ctx) {
        self.ensure_infos(state);
        egui::Frame::NONE.fill(BG).inner_margin(10).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            self.header(ui, state);
            match self.tab {
                Tab::Voices => {
                    self.global_bar(ui, state);
                    let mode = state.params().mode.index();
                    let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), STRIP_H), Sense::hover());
                    let w = (area.width() - 4.0 * STRIP_GAP) / 5.0;
                    for v in 0..5 {
                        let rect = Rect::from_min_size(
                            area.min + Vec2::new(v as f32 * (w + STRIP_GAP), 0.0),
                            Vec2::new(w, STRIP_H),
                        );
                        let mut child = ui.new_child(
                            egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        self.strip(&mut child, state, v, mode, rect);
                    }
                }
                Tab::UserStyles => self.user_styles(ui, state),
            }
        });
    }
}

fn panel(ui: &mut egui::Ui, fill: Color32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE.fill(fill).corner_radius(3).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(9.5).color(DIM).italics());
}

fn arc(c: Pos2, r: f32, start: f32, sweep: f32) -> Vec<Pos2> {
    let n = ((sweep.abs() / std::f32::consts::TAU) * 64.0).ceil().max(2.0) as usize;
    (0..=n).map(|i| c + Vec2::angled(start + sweep * i as f32 / n as f32) * r).collect()
}

/// Visuais egui da interface (tema escuro plano).
pub fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = PANEL;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    v.menu_corner_radius = CornerRadius::same(3);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(2);
        w.bg_stroke = Stroke::NONE;
    }
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, DIM);
    v.widgets.inactive.bg_fill = PANEL_HI;
    v.widgets.inactive.weak_bg_fill = PANEL_HI;
    v.widgets.hovered.bg_fill = Color32::from_rgb(70, 70, 70);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(70, 70, 70);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.open.weak_bg_fill = Color32::from_rgb(70, 70, 70);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.5);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    let _ = StrokeKind::Inside;
    v
}
