//! The audio path must not allocate: a real-time thread cannot wait on the system allocator.

use heaven::dsp::harmony::{Mode, Voicing};
use heaven::dsp::{Engine, EngineParams};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

fn count() {
    let _ = COUNT.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call forwards to the system allocator; counting does not touch the memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        System.alloc(l)
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        count();
        System.alloc_zeroed(l)
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
        System.realloc(p, l, n)
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const SR: f32 = 48_000.0;

/// A melody that changes note every 150 ms, so the voicing search runs often.
fn melody() -> Vec<f32> {
    let notes = [57.0f32, 60.3, 62.0, 64.2, 65.0, 67.0, 69.0, 72.0];
    let mut phase = 0.0f32;
    (0..(SR * 3.0) as usize)
        .map(|i| {
            let note = notes[(i as f32 / (SR * 0.15)) as usize % notes.len()];
            phase = (phase + 440.0 * ((note - 69.0) / 12.0).exp2() / SR).fract();
            (1..=8).map(|h| (std::f32::consts::TAU * phase * h as f32).sin() / h as f32).sum::<f32>() * 0.2
        })
        .collect()
}

fn allocations(params: EngineParams, block: usize, x: &[f32]) -> u64 {
    let mut engine = Engine::new(SR);
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    let before = COUNT.with(Cell::get);
    for (k, chunk) in x.chunks_exact(block).enumerate() {
        engine.set_params(&params);
        if params.mode == Mode::MidiNotes && k % 20 == 0 {
            engine.note_on(0, 60 + (k / 20 % 12) as u8, 100);
            engine.note_off(0, 60 + ((k / 20 + 6) % 12) as u8);
        }
        engine.process(chunk, chunk, &mut l, &mut r);
    }
    COUNT.with(Cell::get) - before
}

#[test]
fn audio_path_does_not_allocate() {
    let x = melody();
    let base = EngineParams { voices: 4, double: true, voicing: Voicing::Around, ..Default::default() };
    let cases = [
        ("chord", base),
        ("scale", EngineParams { mode: Mode::Scale, ..base }),
        ("midi chord", EngineParams { mode: Mode::MidiChord, ..base }),
        ("midi notes", EngineParams { mode: Mode::MidiNotes, ..base }),
        ("tune", EngineParams { tune: true, ..base }),
        ("tune only", EngineParams { tune: true, harmonies: false, ..base }),
        ("everything off", EngineParams { harmonies: false, ..base }),
        ("bypass", EngineParams { bypass: true, ..base }),
    ];
    for (name, params) in cases {
        for block in [64, 512, 2048] {
            assert_eq!(allocations(params, block, &x), 0, "{name}, block {block}");
        }
    }
}
