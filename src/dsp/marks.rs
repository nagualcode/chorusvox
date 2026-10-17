//! Marcas de analise sincronas com o periodo (epocas) para o PSOLA.
//!
//! Em trechos vozeados ha uma marca por ciclo glotal. A primeira marca de cada trecho vai no pico
//! de maior amplitude do primeiro ciclo (perto do fechamento glotal). As seguintes sao achadas
//! por correlacao cruzada normalizada com o ciclo anterior, numa janela de +-15% do periodo. Assim
//! todos os graos ficam alinhados na mesma fase da onda e somam de forma coerente: sem
//! cancelamentos, "phasing" ou aspereza.
//!
//! Em trechos sem vozeamento as marcas ficam numa grade fixa de 5 ms.

use super::pitch::PitchTracker;

#[derive(Clone, Copy, Debug, Default)]
pub struct Mark {
    pub pos: f64,
    /// Periodo local em amostras (ou o passo da grade fixa, se nao vozeado).
    pub period: f32,
    pub voiced: bool,
}

const CAP: usize = 16384;

pub struct MarkTracker {
    marks: Vec<Mark>,
    count: u64,
    pub uv_hop: f32,
    last: Mark,
    started: bool,
}

impl MarkTracker {
    pub fn new(sr: f32) -> Self {
        let uv_hop = (sr * 0.005).round();
        Self {
            marks: vec![Mark::default(); CAP],
            count: 0,
            uv_hop,
            last: Mark { pos: 0.0, period: uv_hop, voiced: false },
            started: false,
        }
    }

    fn push(&mut self, m: Mark) {
        self.marks[(self.count % CAP as u64) as usize] = m;
        self.count += 1;
        self.last = m;
    }

    fn get(&self, k: u64) -> &Mark { &self.marks[(k % CAP as u64) as usize] }

    pub fn latest_pos(&self) -> f64 { if self.count == 0 { f64::NEG_INFINITY } else { self.last.pos } }

    /// Gera marcas enquanto houver sinal e quadros de altura suficientes.
    /// `avail` = numero de amostras ja escritas em `pt.filtered`.
    pub fn update(&mut self, pt: &PitchTracker, avail: i64) {
        let x = &pt.filtered;
        if !self.started {
            self.push(Mark { pos: 0.0, period: self.uv_hop, voiced: false });
            self.started = true;
        }
        loop {
            let last = self.last;
            let step = if last.voiced { last.period as f64 } else { self.uv_hop as f64 };
            let guess = last.pos + step;
            // precisamos de quadros de altura que cubram o proximo ciclo e de sinal para a busca
            if pt.latest_time() < guess + 2.0 * step {
                break;
            }
            let fr = pt.at(guess);
            if fr.voiced() {
                let p = pt_period(pt, &fr);
                let need = guess + 1.3 * p as f64 + 2.0;
                if need as i64 >= avail {
                    break;
                }
                let pos = if last.voiced && (p - last.period).abs() < 0.35 * p {
                    refine_ncc(x, last.pos, guess, p).unwrap_or(guess)
                } else {
                    // inicio de trecho vozeado: pico do primeiro ciclo
                    let a = guess.floor() as i64;
                    let b = a + p.ceil() as i64;
                    let mut best = a;
                    let mut bv = -1.0f32;
                    for i in a..b {
                        let v = x.at(i).abs();
                        if v > bv {
                            bv = v;
                            best = i;
                        }
                    }
                    best as f64
                };
                let pos = pos.max(last.pos + 0.5 * p as f64);
                self.push(Mark { pos, period: p, voiced: true });
            } else {
                if guess as i64 + 2 >= avail {
                    break;
                }
                self.push(Mark { pos: guess, period: self.uv_hop, voiced: false });
            }
        }
    }

    /// Marca mais proxima de `s`.
    pub fn nearest(&self, s: f64) -> Mark {
        if self.count == 0 {
            return Mark { pos: s, period: self.uv_hop, voiced: false };
        }
        let last = self.count - 1;
        let oldest = self.count.saturating_sub(CAP as u64 - 8);
        if s >= self.get(last).pos {
            return *self.get(last);
        }
        if s <= self.get(oldest).pos {
            return *self.get(oldest);
        }
        let (mut lo, mut hi) = (oldest, last);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.get(mid).pos <= s { lo = mid } else { hi = mid }
        }
        let (a, b) = (*self.get(lo), *self.get(hi));
        if s - a.pos <= b.pos - s { a } else { b }
    }
}

fn pt_period(pt: &PitchTracker, fr: &super::pitch::PitchFrame) -> f32 { pt.sr() / fr.f0 }

/// Procura, em +-15% do periodo em torno de `guess`, a posicao cujo ciclo mais se parece com o
/// ciclo em torno de `prev` (correlacao normalizada), com refinamento sub-amostra parabolico.
fn refine_ncc(x: &super::util::Ring, prev: f64, guess: f64, p: f32) -> Option<f64> {
    let half = (p * 0.5).round() as i64;
    let radius = (p * 0.15).ceil().max(1.0) as i64;
    let pc = prev.round() as i64;
    let gc = guess.round() as i64;
    let mut e_t = 0.0f32;
    for j in -half..=half {
        let v = x.at(pc + j);
        e_t += v * v;
    }
    if e_t < 1e-10 {
        return None;
    }
    let mut scores = [0.0f32; 512];
    let n = (2 * radius + 1).min(512) as usize;
    let mut best_k = 0usize;
    let mut best = f32::MIN;
    for k in 0..n {
        let c = gc - radius + k as i64;
        let (mut num, mut e_c) = (0.0f32, 0.0f32);
        for j in -half..=half {
            let v = x.at(c + j);
            num += x.at(pc + j) * v;
            e_c += v * v;
        }
        let s = num / (e_t * e_c).sqrt().max(1e-12);
        scores[k] = s;
        if s > best {
            best = s;
            best_k = k;
        }
    }
    if best < 0.3 {
        return None;
    }
    let mut pos = (gc - radius + best_k as i64) as f64;
    if best_k > 0 && best_k + 1 < n {
        let (a, b, c) = (scores[best_k - 1], scores[best_k], scores[best_k + 1]);
        let den = a - 2.0 * b + c;
        if den.abs() > 1e-9 {
            pos += (0.5 * (a - c) / den).clamp(-0.5, 0.5) as f64;
        }
    }
    // preserva a parte fracionaria da marca anterior relativa a sua posicao arredondada
    Some(pos + (prev - pc as f64))
}
