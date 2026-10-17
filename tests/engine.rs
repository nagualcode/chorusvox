//! Testes de integracao do motor com uma voz sintetica (trem de pulsos glotais filtrado).

use heave::dsp::harmony::{Mode, Voicing};
use heave::dsp::{Engine, EngineParams};

const SR: f32 = 48_000.0;

/// Voz sintetica: pulsos de Rosenberg na altura `midi(t)` passados por dois formantes.
fn voice(len_s: f32, midi: impl Fn(f32) -> f32) -> Vec<f32> {
    let n = (len_s * SR) as usize;
    let mut out = Vec::with_capacity(n);
    let mut ph = 0.0f32;
    let mut prev = 0.0f32;
    let (mut a1, mut a2, mut b1, mut b2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let res = |fc: f32, bw: f32| {
        let r = (-std::f32::consts::PI * bw / SR).exp();
        (2.0 * r * (std::f32::consts::TAU * fc / SR).cos(), -r * r, 1.0 - r)
    };
    let (c1a, c1b, g1) = res(700.0, 80.0);
    let (c2a, c2b, g2) = res(1200.0, 100.0);
    for i in 0..n {
        let t = i as f32 / SR;
        let f0 = 440.0 * ((midi(t) - 69.0) / 12.0).exp2();
        ph = (ph + f0 / SR).fract();
        let g = if ph < 0.4 { 0.5 * (1.0 - (std::f32::consts::PI * ph / 0.4).cos()) } else if ph < 0.56 { (std::f32::consts::PI * (ph - 0.4) / 0.32).cos() } else { 0.0 };
        let x = g - prev;
        prev = g;
        let y1 = g1 * x + c1a * a1 + c1b * a2;
        a2 = a1;
        a1 = y1;
        let y2 = g2 * y1 + c2a * b1 + c2b * b2;
        b2 = b1;
        b1 = y2;
        out.push(y2 * 20.0);
    }
    out
}

fn run(eng: &mut Engine, x: &[f32], mut per_block: impl FnMut(&mut Engine, f32)) -> (Vec<f32>, Vec<f32>) {
    let mut ol = vec![0.0; x.len()];
    let mut or = vec![0.0; x.len()];
    for (k, chunk) in x.chunks(256).enumerate() {
        let pos = k * 256;
        per_block(eng, pos as f32 / SR);
        let m = chunk.len();
        let (l, r) = (&mut ol[pos..pos + m], &mut or[pos..pos + m]);
        eng.process(chunk, chunk, l, r);
    }
    (ol, or)
}

fn sorted_targets(eng: &Engine, t_s: f32) -> Vec<f32> {
    let (_, v) = eng.debug_targets((t_s * SR) as f64);
    let mut v: Vec<f32> = v[..4].iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| b.total_cmp(a));
    v
}

#[test]
fn c_major_above_from_c3_gives_e_and_g() {
    let x = voice(1.5, |_| 48.0);
    let mut eng = Engine::new(SR);
    let p = EngineParams { mode: Mode::Chord, root: 0, quality: 0, voicing: Voicing::Above, voices: 2, humanize: 0.0, pure: false, ..Default::default() };
    run(&mut eng, &x, |e, _| e.set_params(&p));
    let v = sorted_targets(&eng, 1.0);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!((v[0] - 55.0).abs() < 0.3 && (v[1] - 52.0).abs() < 0.3, "{v:?}");
}

#[test]
fn chord_automation_switches_at_the_right_time() {
    // lead parado em C4; acorde C -> F em 1.0 s. A voz de cima deve ir de G4 (67) para A4 (69).
    let x = voice(2.0, |_| 60.0);
    let mut eng = Engine::new(SR);
    let base = EngineParams { mode: Mode::Chord, root: 0, quality: 0, voicing: Voicing::Above, voices: 2, humanize: 0.0, pure: false, ..Default::default() };
    run(&mut eng, &x, |e, t| {
        let mut p = base;
        if t >= 1.0 {
            p.root = 5;
        }
        e.set_params(&p);
    });
    let before = sorted_targets(&eng, 0.9);
    let after = sorted_targets(&eng, 1.15);
    assert!((before[0] - 67.0).abs() < 0.3, "antes {before:?}");
    assert!((after[0] - 69.0).abs() < 0.3, "depois {after:?}");
    // a mudanca acontece perto de 1.0 s (bloco de 256 amostras + glide de ~70 ms)
    let mid = sorted_targets(&eng, 0.97);
    assert!((mid[0] - 67.0).abs() < 0.3, "cedo demais {mid:?}");
}

#[test]
fn midi_notes_mode_follows_held_notes() {
    let x = voice(1.5, |_| 57.0);
    let mut eng = Engine::new(SR);
    let p = EngineParams { mode: Mode::MidiNotes, humanize: 0.0, pure: false, ..Default::default() };
    let mut sent = false;
    run(&mut eng, &x, |e, _| {
        e.set_params(&p);
        if !sent {
            e.note_on(0, 64, 100);
            e.note_on(0, 52, 100);
            sent = true;
        }
    });
    let v = sorted_targets(&eng, 1.0);
    assert!((v[0] - 64.0).abs() < 0.3 && (v[1] - 52.0).abs() < 0.3, "{v:?}");
}

#[test]
fn bypass_is_the_delayed_input() {
    let x = voice(0.5, |_| 50.0);
    let mut eng = Engine::new(SR);
    let p = EngineParams { bypass: true, ..Default::default() };
    let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&p));
    let lat = eng.latency();
    let err = (lat..x.len()).map(|i| (l[i] - x[i - lat]).abs()).fold(0.0f32, f32::max);
    assert!(err < 1e-5, "erro {err}");
}

#[test]
fn harmony_follows_lead_melody_in_scale_mode() {
    // C4 D4 E4 F4 em do maior, terca diatonica acima: E4 F4 G4 A4
    let notes = [60.0, 62.0, 64.0, 65.0];
    let x = voice(2.4, |t| notes[((t / 0.6) as usize).min(3)]);
    let mut eng = Engine::new(SR);
    let p = EngineParams { mode: Mode::Scale, root: 0, scale: 0, voicing: Voicing::Above, voices: 1, humanize: 0.0, pure: false, ..Default::default() };
    run(&mut eng, &x, |e, _| e.set_params(&p));
    for (k, want) in [64.0, 65.0, 67.0, 69.0].iter().enumerate() {
        let v = sorted_targets(&eng, 0.6 * k as f32 + 0.4);
        assert!((v[0] - want).abs() < 0.3, "nota {k}: {v:?}");
    }
}

