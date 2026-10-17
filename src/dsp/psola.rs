//! Shifter PSOLA (Pitch-Synchronous Overlap-Add) de uma voz de harmonia.
//!
//! Cada grao e um ciclo glotal da voz principal (janela de Hann de 2 periodos centrada numa
//! marca de epoca). Os graos sao recolocados no periodo da nota alvo: a altura muda e o envelope
//! espectral (formantes, "a cara da voz") fica, porque cada grao carrega a resposta do trato vocal.
//! Ler o grao mais rapido ou mais devagar (`formant`) encurta ou alonga a "garganta" da voz.
//!
//! O tempo da voz e um atraso variavel: para cada grao de saida escolhe-se a marca de analise
//! mais proxima do instante de origem `s = c - atraso`. Atraso variavel nao gera efeito Doppler:
//! so repete ou pula ciclos inteiros, como um cantor que chega um pouco antes ou depois.
//!
//! Trechos sem vozeamento (consoantes, respiracao) passam sem mudanca de altura, em graos de
//! 5 ms sobre a grade original: a reconstrucao e exata quando o atraso e constante.

use super::marks::MarkTracker;
use super::util::{HannTable, Ring};

/// O que a voz deve fazer num instante de saida.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceState {
    /// Periodo alvo de sintese (amostras); 0 = sem alvo (passa sem deslocar).
    pub period: f32,
    /// Leitura do grao: 1 = formantes preservadas, >1 = garganta curta, <1 = garganta longa.
    pub formant: f32,
    /// Ganho dos graos vozeados.
    pub gain: f32,
    /// Ganho dos graos de trechos fracamente periodicos (caudas sopradas).
    pub gain_weak: f32,
    /// Ganho dos graos sem vozeamento (consoantes).
    pub gain_uv: f32,
}

pub trait VoiceControl {
    /// Atraso (amostras) da voz no instante de saida `c`.
    fn delay(&self, c: f64) -> f64;
    /// Estado da voz no instante de ORIGEM `s` (o material que o grao vai ler).
    fn state(&self, s: f64) -> VoiceState;
}

pub struct GrainVoice {
    acc: Ring,
    next_center: f64,
    started: bool,
    /// Ultima posicao ja entregue (nada pode ser somado antes disso).
    emitted: i64,
    uv_gain_state: f32,
    v_gain_state: f32,
}

/// Maior meia-janela de saida (amostras), usada para planejar os graos com antecedencia.
pub fn max_half_len(sr: f32) -> f64 { (sr / super::pitch::F0_MIN) as f64 / 0.75 + 4.0 }

impl GrainVoice {
    pub fn new() -> Self {
        Self { acc: Ring::new(1 << 15), next_center: 0.0, started: false, emitted: 0, uv_gain_state: 0.0, v_gain_state: 0.0 }
    }

    pub fn reset(&mut self) {
        self.acc.clear();
        self.started = false;
        self.emitted = 0;
        self.uv_gain_state = 0.0;
        self.v_gain_state = 0.0;
    }

    /// Planeja e soma no acumulador todos os graos com centro antes de `until`.
    pub fn plan(&mut self, until: f64, input: &Ring, marks: &MarkTracker, ctl: &impl VoiceControl, hann: &HannTable) {
        if !self.started {
            self.next_center = self.emitted as f64;
            self.started = true;
        }
        while self.next_center < until {
            let c = self.next_center;
            let s = c - ctl.delay(c);
            let m = marks.nearest(s);
            let st = ctl.state(s);
            if m.voiced && st.period > 0.0 && (m.pos - s).abs() < 1.5 * m.period as f64 {
                let ps = st.period.max(8.0);
                let phi = st.formant.clamp(0.6, 1.6);
                let h_src = m.period as f64;
                let g_target = if m.weak { st.gain_weak } else { st.gain };
                self.v_gain_state += (g_target - self.v_gain_state) * 0.6;
                self.uv_gain_state += (st.gain_uv - self.uv_gain_state) * 0.3;
                // normalizacao de potencia: densidade de graos e duracao do grao
                let norm = ((ps as f64 * phi as f64) / h_src).sqrt().clamp(0.4, 2.0) as f32;
                self.add_grain(c, m.pos, h_src, phi as f64, self.v_gain_state * norm, input, hann);
                self.next_center = c + ps as f64;
            } else {
                let hop = marks.uv_hop as f64;
                self.uv_gain_state += (st.gain_uv - self.uv_gain_state) * 0.3;
                self.v_gain_state += (st.gain - self.v_gain_state) * 0.3;
                self.add_grain(c, s, hop, 1.0, self.uv_gain_state, input, hann);
                self.next_center = c + hop;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn add_grain(&mut self, c: f64, src: f64, h_src: f64, phi: f64, g: f32, input: &Ring, hann: &HannTable) {
        if g <= 1e-6 {
            return;
        }
        let h_out = h_src / phi;
        let n0 = ((c - h_out).ceil() as i64).max(self.emitted);
        let n1 = (c + h_out).floor() as i64;
        let inv = (1.0 / h_out) as f32;
        for n in n0..=n1 {
            let dn = n as f64 - c;
            let w = hann.w(dn as f32 * inv);
            let x = input.interp(src + dn * phi);
            self.acc.add(n, g * w * x);
        }
    }

    /// Entrega a amostra de saida `n` (instante de saida, em amostras).
    #[inline]
    pub fn take(&mut self, n: i64) -> f32 {
        self.emitted = n + 1;
        self.acc.take(n)
    }
}

impl Default for GrainVoice {
    fn default() -> Self { Self::new() }
}
