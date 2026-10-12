//! CLI de validacao e processamento WAV para o ChorusVox / DecaBuddy.

use decabuddy::dsp::FastRng;
use decabuddy::scales::ScaleId;
use decabuddy::DecaBuddyProcessor;
use std::env;
use std::f32::consts::PI;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

const SR: u32 = 44_100;

#[derive(Clone, Copy, Default)]
struct FormantResonator {
    b0: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl FormantResonator {
    fn set(&mut self, freq_hz: f32, bw_hz: f32, sr: f32) {
        let r = (-PI * bw_hz / sr).exp();
        let theta = 2.0 * PI * freq_hz / sr;
        self.b0 = 1.0 - r;
        self.a1 = -2.0 * r * theta.cos();
        self.a2 = r * r;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x - self.a1 * self.z1 - self.a2 * self.z2;
        self.z2 = self.z1;
        self.z1 = y;
        y
    }
}

fn vowel_formants(v: char) -> [f32; 3] {
    match v {
        'a' => [800.0, 1250.0, 2800.0],
        'e' => [500.0, 1900.0, 2600.0],
        'i' => [320.0, 2300.0, 3000.0],
        'o' => [500.0, 900.0, 2700.0],
        'u' => [350.0, 800.0, 2600.0],
        _ => [600.0, 1500.0, 2700.0],
    }
}

fn synth_formant_voice(sr: f32) -> Vec<f32> {
    let phrase: &[(char, char, f32, f32)] = &[
        ('s', 'a', 60.0, 0.55),
        ('t', 'e', 64.0, 0.50),
        (' ', 'o', 67.0, 0.55),
        ('s', 'i', 65.0, 0.45),
        ('t', 'e', 64.0, 0.45),
        (' ', 'a', 62.0, 0.50),
        ('t', 'o', 60.0, 0.80),
        ('s', 'u', 55.0, 0.85),
        (' ', 'a', 57.0, 0.50),
        ('t', 'i', 59.0, 0.50),
        (' ', 'o', 60.0, 1.10),
    ];

    let bws = [90.0_f32, 110.0, 160.0];
    let mut rng = FastRng::new(42);
    let mut out = Vec::new();
    let mut phase = 0.0_f32;
    let mut glottal_lp = 0.0_f32;
    let mut hp_prev_x = 0.0_f32;
    let mut hp_prev_y = 0.0_f32;

    for &(cons, vow, midi, dur) in phrase {
        let n_cons = match cons {
            's' => (0.08 * sr) as usize,
            't' => (0.03 * sr) as usize,
            _ => 0,
        };
        for i in 0..n_cons {
            let noise = rng.next_bipolar();
            let hp = noise - hp_prev_x + 0.85 * hp_prev_y;
            hp_prev_x = noise;
            hp_prev_y = hp;
            let env = if cons == 's' {
                (PI * (i as f32) / (n_cons as f32)).sin().sqrt() * 0.22
            } else {
                (-(i as f32) / (0.006 * sr)).exp() * 0.35
            };
            out.push(hp * env);
        }

        let n_vow = (dur * sr) as usize;
        let freqs = vowel_formants(vow);
        let mut res = [FormantResonator::default(); 3];
        for k in 0..3 {
            res[k].set(freqs[k], bws[k], sr);
        }
        let detune = rng.next_bipolar() * 0.12;
        let vib_phase_off = (rng.next_bipolar() + 1.0) * PI;

        for i in 0..n_vow {
            let t = i as f32 / sr;
            let vib_depth = 0.22 * ((t - 0.14).max(0.0) / 0.22).min(1.0);
            let vib = vib_depth * (2.0 * PI * 5.6 * t + vib_phase_off).sin();
            let scoop = -0.85 * (-t / 0.045).exp();
            let m = midi + detune + vib + scoop;
            let f0 = 440.0 * 2.0_f32.powf((m - 69.0) / 12.0);

            let prev_floor = phase.floor();
            phase += f0 / sr;
            let pulse = if phase.floor() > prev_floor { 1.0 } else { 0.0 };

            glottal_lp = pulse + 0.93 * glottal_lp;
            let mut sig = glottal_lp + 0.015 * rng.next_bipolar();

            for r in &mut res {
                sig = r.process(sig);
            }

            let attack = (t / 0.03).min(1.0);
            let release = ((dur - t) / 0.06).min(1.0).max(0.0);
            out.push(sig * attack * release);
        }
    }

    let peak = out.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
    if peak > 1e-6 {
        let scale = 0.8 / peak;
        for x in &mut out {
            *x *= scale;
        }
    }
    out
}

fn write_wav_stereo_i16(path: &str, l: &[f32], r: &[f32], sr: u32) -> std::io::Result<()> {
    let n = l.len().min(r.len());
    let data_bytes = (n * 4) as u32;
    let mut f = BufWriter::new(File::create(path)?);

    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&sr.to_le_bytes())?;
    f.write_all(&(sr * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_bytes.to_le_bytes())?;

    for i in 0..n {
        let sl = (l[i].clamp(-0.98, 0.98) * 32767.0) as i16;
        let sr_s = (r[i].clamp(-0.98, 0.98) * 32767.0) as i16;
        f.write_all(&sl.to_le_bytes())?;
        f.write_all(&sr_s.to_le_bytes())?;
    }
    Ok(())
}

fn read_wav_mono(path: &str) -> std::io::Result<(Vec<f32>, u32)> {
    let mut buf = Vec::new();
    BufReader::new(File::open(path)?).read_to_end(&mut buf)?;
    if buf.len() < 44 || &buf[0..4] != b"RIFF" || &buf[8..12] != b"WAVE" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Cabecalho RIFF/WAVE invalido",
        ));
    }

    let mut audio_format = 1u16;
    let mut channels = 1usize;
    let mut sr = SR;
    let mut bits_per_sample = 16u16;
    let mut pos = 12;

    while pos + 8 <= buf.len() {
        let tag = &buf[pos..pos + 4];
        let sz =
            u32::from_le_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]) as usize;
        let chunk_start = pos + 8;
        let chunk_end = (chunk_start + sz).min(buf.len());

        if tag == b"fmt " && sz >= 16 {
            audio_format = u16::from_le_bytes([buf[chunk_start], buf[chunk_start + 1]]);
            channels = u16::from_le_bytes([buf[chunk_start + 2], buf[chunk_start + 3]]) as usize;
            sr = u32::from_le_bytes([
                buf[chunk_start + 4],
                buf[chunk_start + 5],
                buf[chunk_start + 6],
                buf[chunk_start + 7],
            ]);
            bits_per_sample = u16::from_le_bytes([buf[chunk_start + 14], buf[chunk_start + 15]]);
        } else if tag == b"data" {
            let raw = &buf[chunk_start..chunk_end];
            let bytes_per_sample = (bits_per_sample as usize / 8).max(1);
            let frame_bytes = bytes_per_sample * channels.max(1);
            let mut out = Vec::with_capacity(raw.len() / frame_bytes);

            for frame in raw.chunks_exact(frame_bytes) {
                let s = match (audio_format, bits_per_sample) {
                    (1, 16) => i16::from_le_bytes([frame[0], frame[1]]) as f32 / 32768.0,
                    (1, 24) => {
                        let v = ((frame[2] as i8 as i32) << 16)
                            | ((frame[1] as i32) << 8)
                            | (frame[0] as i32);
                        v as f32 / 8_388_608.0
                    }
                    (1, 32) => {
                        i32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as f32
                            / 2_147_483_648.0
                    }
                    (3, 32) => f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]),
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            format!(
                                "Formato WAV nao suportado: format={}, bits={}",
                                audio_format, bits_per_sample
                            ),
                        ))
                    }
                };
                out.push(s);
            }
            return Ok((out, sr));
        }
        pos = chunk_start + ((sz + 1) & !1);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Chunk data nao encontrado no WAV",
    ))
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let (input, sr) = if args.len() >= 2 {
        println!("Lendo arquivo de entrada: {}", args[1]);
        read_wav_mono(&args[1])?
    } else {
        println!("Gerando voz cantada sintetica com formantes e consoantes (44.1 kHz)...");
        let dry = synth_formant_voice(SR as f32);
        write_wav_stereo_i16("00_dry_voice.wav", &dry, &dry, SR)?;
        (dry, SR)
    };

    let out_path = if args.len() >= 3 {
        args[2].as_str()
    } else {
        "decabuddy_rust_output.wav"
    };

    let mut proc = DecaBuddyProcessor::new(sr as f32);
    proc.params.key = 0;
    proc.params.harmonies[0].scale = ScaleId::LowOctave;
    proc.params.harmonies[1].scale = ScaleId::Unison;
    proc.params.harmonies[1].enabled = true;
    proc.params.harmonies[2].scale = ScaleId::High3rdModal1;
    proc.params.harmonies[3].scale = ScaleId::High5thModal1;

    let mut out_l = vec![0.0_f32; input.len()];
    let mut out_r = vec![0.0_f32; input.len()];

    for ((in_blk, l_blk), r_blk) in input
        .chunks(128)
        .zip(out_l.chunks_mut(128))
        .zip(out_r.chunks_mut(128))
    {
        proc.process_block(in_blk, l_blk, r_blk);
    }
    write_wav_stereo_i16(out_path, &out_l, &out_r, sr)?;
    println!(
        "[Scale Mode] Concluido ({:.2}s, latencia = {} amostras) -> {}",
        input.len() as f32 / sr as f32,
        proc.latency_samples(),
        out_path
    );
    Ok(())
}
