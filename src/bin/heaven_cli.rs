//! Render offline do Heaven (mesmo motor do plugin) e benchmark.
//!
//! heaven_cli <entrada.wav> <saida.wav> [chave=valor ...]
//!   mode=chord|scale|midichord|midinotes  root=C..B  chord=Maj|Min|7|...  scale=major|minor|...
//!   voicing=above|below|around|wide  voices=1..4  harmony=dB  lead=dB  humanize=0..1
//!   width=0..1  formant=-1..1  pure=0|1  double=0|1  block=512
//!   chords="0:Am,4:F,8:C"         linha do tempo de acordes (segundos:acorde), como automacao
//!   midi="0-4:57,60,64;4-8:53,57" notas MIDI seguradas por intervalo de tempo (segundos)
//!   compensate=1                  remove a latencia do arquivo de saida (padrao 1)

use heaven::dsp::harmony::{Mode, Voicing, CHORD_NAMES, NOTE_NAMES, SCALE_NAMES};
use heaven::dsp::{Engine, EngineParams};
use std::time::Instant;

fn parse_note(s: &str) -> Option<(u8, usize)> {
    let b = s.as_bytes();
    let base = match b.first()?.to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    match b.get(1) {
        Some(b'#') => Some(((base + 1) % 12, 2)),
        Some(b'b') => Some(((base + 11) % 12, 2)),
        _ => Some((base, 1)),
    }
}

/// "F#m7" -> (raiz, qualidade)
fn parse_chord(s: &str) -> Option<(u8, u8)> {
    let (root, used) = parse_note(s)?;
    let rest = &s[used..];
    let q = match rest {
        "" | "maj" | "M" => "Maj",
        "m" | "min" => "Min",
        "maj7" | "M7" => "Maj7",
        "dim" | "o" => "dim",
        "dim7" | "o7" => "dim7",
        "aug" | "+" => "aug",
        "m7b5" | "ø" => "m7b5",
        other => other,
    };
    let qi = CHORD_NAMES.iter().position(|&n| n.eq_ignore_ascii_case(q))?;
    Some((root, qi as u8))
}

fn read_wav(path: &str) -> (Vec<f32>, Vec<f32>, u32) {
    let mut r = hound::WavReader::open(path).expect("nao abriu o wav de entrada");
    let spec = r.spec();
    let ch = spec.channels as usize;
    let data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.unwrap() as f32 * scale).collect()
        }
    };
    let frames = data.len() / ch;
    let mut l = Vec::with_capacity(frames);
    let mut rr = Vec::with_capacity(frames);
    for f in 0..frames {
        l.push(data[f * ch]);
        rr.push(data[f * ch + (ch - 1).min(1)]);
    }
    (l, rr, spec.sample_rate)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("uso: heaven_cli <entrada.wav> <saida.wav> [chave=valor ...]");
        std::process::exit(2);
    }
    let (in_l, in_r, sr) = read_wav(&args[1]);
    let mut p = EngineParams::default();
    let mut block = 512usize;
    let mut chords: Vec<(f64, u8, u8)> = Vec::new();
    let mut midi: Vec<(f64, f64, Vec<u8>)> = Vec::new();
    let mut compensate = true;
    let mut dump: Option<String> = None;
    for a in &args[3..] {
        let Some((k, v)) = a.split_once('=') else { continue };
        match k {
            "mode" => {
                p.mode = match v {
                    "scale" => Mode::Scale,
                    "midichord" => Mode::MidiChord,
                    "midinotes" => Mode::MidiNotes,
                    _ => Mode::Chord,
                }
            }
            "root" => p.root = parse_note(v).expect("root").0,
            "chord" => p.quality = CHORD_NAMES.iter().position(|&n| n.eq_ignore_ascii_case(v)).expect("chord") as u8,
            "scale" => {
                p.scale = SCALE_NAMES
                    .iter()
                    .position(|&n| n.to_lowercase().replace(['.', ' '], "") == v.to_lowercase().replace(['.', ' '], ""))
                    .expect("scale") as u8
            }
            "voicing" => {
                p.voicing = match v {
                    "below" => Voicing::Below,
                    "around" => Voicing::Around,
                    "wide" => Voicing::Wide,
                    _ => Voicing::Above,
                }
            }
            "voices" => p.voices = v.parse().unwrap(),
            "harmony" => p.harmony_db = v.parse().unwrap(),
            "lead" => p.lead_db = v.parse().unwrap(),
            "humanize" => p.humanize = v.parse().unwrap(),
            "width" => p.width = v.parse().unwrap(),
            "formant" => p.formant = v.parse().unwrap(),
            "pure" => p.pure = v != "0",
            "double" => p.double = v != "0",
            "block" => block = v.parse().unwrap(),
            "compensate" => compensate = v != "0",
            "dump" => dump = Some(v.to_string()),
            "chords" => {
                for item in v.split(',') {
                    let (t, c) = item.split_once(':').expect("chords t:acorde");
                    let (r, q) = parse_chord(c).unwrap_or_else(|| panic!("acorde invalido: {c}"));
                    chords.push((t.parse().unwrap(), r, q));
                }
            }
            "midi" => {
                for item in v.split(';') {
                    let (span, notes) = item.split_once(':').expect("midi a-b:notas");
                    let (a, b) = span.split_once('-').expect("midi a-b");
                    midi.push((a.parse().unwrap(), b.parse().unwrap(), notes.split(',').map(|n| n.parse().unwrap()).collect()));
                }
            }
            _ => eprintln!("parametro desconhecido: {k}"),
        }
    }

    let mut eng = Engine::new(sr as f32);
    let lat = eng.latency();
    let n = in_l.len();
    let total = n + if compensate { lat } else { 0 };
    let pad = |v: &[f32]| {
        let mut x = v.to_vec();
        x.resize(total, 0.0);
        x
    };
    let (in_l, in_r) = (pad(&in_l), pad(&in_r));
    let mut out_l = vec![0.0f32; total];
    let mut out_r = vec![0.0f32; total];
    let mut held: Vec<u8> = Vec::new();
    let t_start = Instant::now();
    let mut pos = 0;
    while pos < total {
        let m = block.min(total - pos);
        let t = pos as f64 / sr as f64;
        let mut bp = p;
        if let Some(&(_, r, q)) = chords.iter().rev().find(|(ct, _, _)| *ct <= t) {
            bp.root = r;
            bp.quality = q;
        }
        eng.set_params(&bp);
        let want: Vec<u8> = midi.iter().find(|(a, b, _)| *a <= t && t < *b).map(|x| x.2.clone()).unwrap_or_default();
        if want != held {
            for &nn in &held {
                if !want.contains(&nn) {
                    eng.note_off(0, nn);
                }
            }
            for &nn in &want {
                if !held.contains(&nn) {
                    eng.note_on(0, nn, 100);
                }
            }
            held = want;
        }
        let (ol, or) = (&mut out_l[pos..pos + m], &mut out_r[pos..pos + m]);
        eng.process(&in_l[pos..pos + m], &in_r[pos..pos + m], ol, or);
        pos += m;
    }
    let el = t_start.elapsed().as_secs_f64();
    let dur = n as f64 / sr as f64;
    eprintln!(
        "render: {:.2}s de audio em {:.3}s ({:.0}x tempo real), latencia {} amostras ({:.1} ms)",
        dur,
        el,
        dur / el,
        lat,
        1000.0 * lat as f64 / sr as f64
    );
    let skip = if compensate { lat } else { 0 };
    let spec = hound::WavSpec { channels: 2, sample_rate: sr, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(&args[2], spec).expect("nao criou o wav de saida");
    let mut peak = 0.0f32;
    for i in skip..total {
        w.write_sample(out_l[i]).unwrap();
        w.write_sample(out_r[i]).unwrap();
        peak = peak.max(out_l[i].abs()).max(out_r[i].abs());
    }
    w.finalize().unwrap();
    if let Some(path) = dump {
        // t, f0 do rastreador, confianca, aperiodicidade, hf, nota do lead, alvos das vozes
        use std::io::Write;
        let mut f = std::fs::File::create(&path).expect("dump");
        let pt = eng.pitch_tracker();
        let step = sr as f64 * 0.004;
        let mut t = 0.0;
        while t < n as f64 {
            let fr = pt.at(t);
            let (lead, v) = eng.debug_targets(t);
            let vs: Vec<String> = v.iter().map(|x| format!("{x:.3}")).collect();
            writeln!(f, "{:.4},{:.2},{:.3},{:.3},{:.3},{:.3},{}", t / sr as f64, fr.f0, fr.conf, fr.aper, fr.hf, lead, vs.join(",")).unwrap();
            t += step;
        }
    }
    eprintln!("pico {:.3} -> {}", peak, &args[2]);
    let _ = NOTE_NAMES;
}
