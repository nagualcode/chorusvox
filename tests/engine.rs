//! Testes de integracao do motor com uma voz sintetica (trem de pulsos glotais filtrado).

use heaven::dsp::harmony::{Mode, Voicing};
use heaven::dsp::pitch::PitchTracker;
use heaven::dsp::{Engine, EngineParams};

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


// ------------------------------------------------------------------------------ correcao do lead

const LAT: usize = (SR * 0.08) as usize;

fn tuned_only() -> EngineParams {
    EngineParams { tune: true, harmonies: false, retune_ms: 0.0, chromatic: true, ..Default::default() }
}

/// Altura (MIDI) detectada no sinal em cada quadro de 4 ms entre `a` e `b` segundos.
fn track(x: &[f32], a: f32, b: f32) -> Vec<f32> {
    let mut pt = PitchTracker::new(SR);
    pt.push(x);
    let (mut t, mut v) = (a, Vec::new());
    while t < b {
        let fr = pt.at((t * SR) as f64);
        if fr.voiced() {
            v.push(69.0 + 12.0 * (fr.f0 / 440.0).log2());
        }
        t += 0.004;
    }
    v
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

fn rms(x: &[f32]) -> f32 { (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt() }

#[test]
fn sharp_singer_lands_on_the_note() {
    for sung in [36.3, 40.35, 48.3, 60.35, 69.2, 79.3, 84.2] {
        let x = voice(1.5, |_| sung);
        let mut eng = Engine::new(SR);
        let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&tuned_only()));
        let got = median(track(&l, 0.5, 1.4));
        assert!((got - sung.round()).abs() < 0.03, "cantou {sung}, saiu {got}");
    }
}

#[test]
fn correction_works_at_other_sample_rates() {
    // o mesmo sinal lido a outra taxa soa em outra altura: basta conferir a nota resultante
    let x = voice(2.0, |_| 60.0);
    for sr in [44_100.0f32, 96_000.0, 192_000.0] {
        let mut eng = Engine::new(sr);
        let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&tuned_only()));
        let mut pt = PitchTracker::new(sr);
        pt.push(&l);
        let at = |q: f32| pt.at((q * x.len() as f32) as f64);
        let v: Vec<f32> = (0..200).map(|k| at(0.5 + k as f32 * 0.002)).filter(|f| f.voiced()).map(|f| 69.0 + 12.0 * (f.f0 / 440.0).log2()).collect();
        let got = median(v);
        assert!((got - got.round()).abs() < 0.04, "{sr} Hz: saiu {got}");
    }
}

#[test]
fn tuned_lead_has_flat_level() {
    // se o trecho vozeado virasse consoante por falta de marcas, ou os graos desalinhassem,
    // o nivel oscilaria aqui
    for sung in [36.3, 40.35, 60.35, 79.3, 84.2] {
        let x = voice(1.5, |_| sung);
        let mut eng = Engine::new(SR);
        let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&tuned_only()));
        let win = (SR * 0.03) as usize;
        let ripple = |s: &[f32]| {
            let (lo, hi) = s.chunks_exact(win).map(rms).fold((f32::MAX, 0.0f32), |(a, b), v| (a.min(v), b.max(v)));
            20.0 * (hi / lo).log10()
        };
        let (a, b) = ((SR * 0.4) as usize, (SR * 1.4) as usize);
        let extra = ripple(&l[a..b]) - ripple(&x[a..b]);
        let level = 20.0 * (rms(&l[a..b]) / rms(&x[a..b])).log10();
        assert!(extra < 0.5, "midi {sung}: ondulacao {extra} dB alem do seco");
        assert!(level.abs() < 1.0, "midi {sung}: nivel {level} dB em relacao ao seco");
    }
}

#[test]
fn in_tune_singer_passes_almost_unchanged() {
    let x = voice(1.5, |_| 60.0);
    let mut eng = Engine::new(SR);
    let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&tuned_only()));
    let (a, b) = ((SR * 0.5) as usize, (SR * 1.3) as usize);
    let err: Vec<f32> = (a..b).map(|i| l[i] - x[i - LAT]).collect();
    let snr = 20.0 * (rms(&x[a..b]) / rms(&err)).log10();
    assert!(snr > 30.0, "SNR {snr} dB");
}

#[test]
fn level_stays_flat_across_voiced_unvoiced_edges() {
    // vogal, ruido de consoante, vogal: a emenda nao pode criar buraco nem pico
    let mut x = voice(1.6, |_| 57.3);
    let mut rng = heaven::dsp::util::Rng::new(7);
    let (a, b) = ((SR * 0.7) as usize, (SR * 0.85) as usize);
    let level = rms(&x[..a]);
    for v in &mut x[a..b] {
        *v = (rng.uniform() - 0.5) * 3.4 * level;
    }
    let mut eng = Engine::new(SR);
    let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&tuned_only()));
    let win = (SR * 0.005) as usize;
    for k in (((SR * 0.5) as usize)..((SR * 1.2) as usize)).step_by(win) {
        let dry = rms(&x[k..k + 3 * win]);
        let out = rms(&l[k + LAT..k + LAT + 3 * win]);
        let db = 20.0 * (out / dry).log10();
        assert!(db.abs() < 3.0, "t={:.3}s: {db} dB em relacao ao seco", k as f32 / SR);
    }
}

#[test]
fn vibrato_survives_a_natural_retune_and_is_flattened_at_zero() {
    let sung = |t: f32| 60.3 + 0.4 * (std::f32::consts::TAU * 5.5 * t).sin();
    let x = voice(2.5, sung);
    let spread = |retune_ms: f32| {
        let mut eng = Engine::new(SR);
        let p = EngineParams { retune_ms, ..tuned_only() };
        let (l, _) = run(&mut eng, &x, |e, _| e.set_params(&p));
        let v = track(&l, 1.0, 2.3);
        let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), &m| (a.min(m), b.max(m)));
        (hi - lo, (lo + hi) / 2.0)
    };
    let (natural, center) = spread(60.0);
    let (flat, _) = spread(0.0);
    assert!(natural > 0.45, "vibrato com retune natural: {natural}");
    assert!((center - 60.0).abs() < 0.1, "centro {center}");
    assert!(flat < 0.2, "vibrato com retune 0: {flat}");
}

#[test]
fn correction_follows_the_scale_and_the_harmony_follows_the_correction() {
    // cantor em C#4 + 20 cents, em do maior: o lead vai para D4 e a harmonia nasce de D4
    let x = voice(1.5, |_| 61.2);
    let mut eng = Engine::new(SR);
    let p = EngineParams {
        tune: true,
        retune_ms: 0.0,
        mode: Mode::Scale,
        root: 0,
        scale: 0,
        voices: 1,
        humanize: 0.0,
        pure: false,
        ..Default::default()
    };
    run(&mut eng, &x, |e, _| e.set_params(&p));
    let (lead, _) = eng.debug_targets((1.0 * SR) as f64);
    assert!((lead - 62.0).abs() < 0.05, "lead {lead}");
    let harmony = sorted_targets(&eng, 1.0);
    assert!(harmony.len() == 1 && (harmony[0] - 65.0).abs() < 0.05, "terca diatonica acima de D4: {harmony:?}");
}

#[test]
fn harmonies_off_tune_off_is_the_delayed_input() {
    let x = voice(1.0, |_| 60.0);
    let mut eng = Engine::new(SR);
    let p = EngineParams { harmonies: false, ..Default::default() };
    let (l, r) = run(&mut eng, &x, |e, _| e.set_params(&p));
    for i in (SR * 0.3) as usize..x.len() {
        assert!((l[i] - x[i - LAT]).abs() < 1e-6 && (r[i] - x[i - LAT]).abs() < 1e-6, "amostra {i}");
    }
}

#[test]
fn tuned_lead_keeps_the_stereo_image() {
    let x = voice(1.2, |_| 60.3);
    let half: Vec<f32> = x.iter().map(|v| v * 0.5).collect();
    let mut eng = Engine::new(SR);
    let (mut ol, mut or) = (vec![0.0; x.len()], vec![0.0; x.len()]);
    for (k, (a, b)) in x.chunks(256).zip(half.chunks(256)).enumerate() {
        eng.set_params(&tuned_only());
        let pos = k * 256;
        let (l, r) = (&mut ol[pos..pos + a.len()], &mut or[pos..pos + a.len()]);
        eng.process(a, b, l, r);
    }
    let (s, e) = ((SR * 0.5) as usize, (SR * 1.1) as usize);
    let db = 20.0 * (rms(&or[s..e]) / rms(&ol[s..e])).log10();
    assert!((db + 6.02).abs() < 0.2, "L/R {db} dB");
}

#[test]
fn toggling_tune_does_not_click() {
    let x = voice(2.0, |_| 60.3);
    let mut eng = Engine::new(SR);
    let (l, _) = run(&mut eng, &x, |e, t| e.set_params(&EngineParams { tune: (1.0..1.5).contains(&t), harmonies: false, retune_ms: 0.0, chromatic: true, ..Default::default() }));
    let steady = (((SR * 0.5) as usize)..((SR * 0.9) as usize)).map(|i| (l[i + 1] - l[i]).abs()).fold(0.0f32, f32::max);
    let edge = (((SR * 0.95) as usize)..((SR * 1.8) as usize)).map(|i| (l[i + 1] - l[i]).abs()).fold(0.0f32, f32::max);
    assert!(edge < steady * 1.6, "degrau {edge} contra {steady}");
}

