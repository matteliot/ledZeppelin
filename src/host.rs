//! Host-only demo renderer: `cargo sim [OUT.wav]`.
//!
//! Plays an original E-minor blues-rock riff, chords and a lead line at 96 bpm on a modelled electric guitar (see `guitar`: exact tuning, pick and pickup, palm mutes, strums, slides, bends,
//! vibrato and human timing), drives the patch's entry points the way the firmware does (128-sample blocks, footswitch and knob events at set times) and writes a stereo float WAV. Prints
//! per-section level stats, then a Gain-knob sweep on a held two-note chord showing how much it cleans up. Fails on NaN or clipping.

use crate::guitar::{Note, Player};
use crate::*;

const N: usize = 128;
const SRU: usize = 48_000;

const E2: f32 = 82.41;
const G2: f32 = 98.0;
const A2: f32 = 110.0;
const B2: f32 = 123.47;
const D3: f32 = 146.83;
const E3: f32 = 164.81;
const G3: f32 = 196.0;
const A3: f32 = 220.0;
const D4: f32 = 293.66;
const E4: f32 = 329.63;
const G4: f32 = 392.0;
const A4: f32 = 440.0;
const B4: f32 = 493.88;
const D5: f32 = 587.33;
const E5: f32 = 659.26;

enum Ev {
    /// Play these notes, strummed this many seconds apart; `ring` lets what's already sounding carry on.
    Play(Vec<Note>, f32, bool),
    /// The fretting hand lets go.
    Release,
    Press,
    Hold,
    Gain(f32),
}

fn score() -> (Vec<(f64, Ev, &'static str)>, f64) {
    let mut ev = vec![];
    let e = 60.0 / 96.0 / 2.0; // eighth note
    let mut t = 0.1;
    // Small, repeatable timing slop: nobody plays exactly on the grid.
    let mut seed: u32 = 0x1357_9bdf;
    let mut slop = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f64 / u32::MAX as f64 * 2.0 - 1.0) * 0.007
    };
    // Original riff: palm-muted chugs on the low E, accented notes ringing out, vibrato on the long ones. (note, length in eighths)
    let riff = || {
        [
            (Note::new(E2).pm(), 1.0),
            (Note::new(E2).pm(), 1.0),
            (Note::new(G2).vel(0.95), 1.0),
            (Note::new(A2).vel(0.9), 1.0),
            (Note::new(E2).pm(), 1.0),
            (Note::new(D3).vel(0.95), 1.0),
            (Note::new(B2).vel(0.9).vib(), 2.0),
            (Note::new(A2).vel(0.85), 1.0),
            (Note::new(G2).vel(0.85), 1.0),
            (Note::new(E2).pm(), 1.0),
            (Note::new(E2).pm(), 1.0),
            (Note::new(E2).vel(1.0).vib(), 4.0),
        ]
    };
    let mut play = |ev: &mut Vec<(f64, Ev, &'static str)>, t: &mut f64, times: usize| {
        for _ in 0..times {
            for (n, len) in riff() {
                ev.push((*t + slop(), Ev::Play(vec![n], 0.0, false), ""));
                *t += len * e;
            }
        }
    };
    play(&mut ev, &mut t, 2);
    ev.push((t, Ev::Gain(0.35), "Plexi, Gain backed off (chords)"));
    // Open-position chords, strummed down and left to ring, with a lighter upstroke on the "and" of 3.
    for c in [[E2, B2, E3], [A2, E3, A3], [D3, A3, D4], [G2, D3, G3]] {
        let chord = |v: f32| c.iter().map(|f| Note::new(*f).vel(v)).collect::<Vec<_>>();
        ev.push((t + 0.001, Ev::Play(chord(0.85), 0.014, false), ""));
        let mut up = chord(0.55);
        up.reverse();
        ev.push((t + 3.0 * e, Ev::Play(up, 0.01, false), ""));
        t += 4.0 * e;
    }
    ev.push((t, Ev::Gain(GAIN_DEFAULT), "Tone Bender fuzz (riff)"));
    ev.push((t + 0.001, Ev::Press, ""));
    play(&mut ev, &mut t, 1);
    ev.push((t, Ev::Press, "Plexi + Echoplex echo (lead)"));
    ev.push((t + 0.001, Ev::Hold, ""));
    // Lead: slide into the first note, a bend up a whole step, vibrato on everything held.
    let lead = [
        (Note::new(E4).vel(0.9).slide(2.0, 0.09), 2.0),
        (Note::new(G4).vel(0.85), 1.0),
        (Note::new(A4).vel(0.85), 1.0),
        (Note::new(B4).vel(0.95).vib(), 4.0),
        (Note::new(D5).vel(0.9), 2.0),
        (Note::new(B4).vel(0.85), 2.0),
        (Note::new(A4).vel(1.0).bend(0.06, 0.16, 200.0).vib(), 4.0),
        (Note::new(E5).vel(1.0).vib(), 8.0),
    ];
    for (n, len) in lead {
        ev.push((t + slop(), Ev::Play(vec![n], 0.0, false), ""));
        t += len * e;
    }
    ev.push((t - 0.05, Ev::Release, ""));
    ev.push((t, Ev::Hold, "echo off, repeats ring out"));
    ev.push((t + 0.001, Ev::Play([E2, B2, E3].iter().map(|f| Note::new(*f).vel(0.9).vib()).collect(), 0.016, false), ""));
    (ev, t + 3.5)
}

pub fn main() {
    let out = std::env::args().nth(1).unwrap_or("target/render/led_zeppelin_demo.wav".into());
    let env = PatchEnv { abi_version: PATCH_ABI_VERSION as u32, user_ctx: core::ptr::null_mut() };
    let mut work = vec![0.0f32; 16];
    unsafe {
        patch_init(&env);
        set_buffer(&env, work.as_mut_ptr());
        for i in 0..PARAM_COUNT {
            set_param(&env, i, get_param_default(&env, i));
        }
    }

    let (mut events, total_s) = score();
    events.sort_by(|a, b| b.0.total_cmp(&a.0));
    let total = (total_s * SRU as f64) as usize;
    let mut guitar = Player::new();

    let mut lo = vec![];
    let mut ro = vec![];
    let mut sections: Vec<(usize, &str)> = vec![(0, "Plexi crunch (riff)")];
    let mut t = 0;
    while t < total {
        while events.last().is_some_and(|(et, _, _)| (*et * SRU as f64) as usize <= t) {
            let (_, e, label) = events.pop().unwrap();
            match e {
                Ev::Press => unsafe { special_action(&env, ACTION_FOOTSWITCH_PRESS) },
                Ev::Hold => unsafe { special_action(&env, ACTION_FOOTSWITCH_HOLD) },
                Ev::Gain(v) => unsafe { set_param(&env, PARAM_GAIN, v) },
                Ev::Release => guitar.release_all(),
                Ev::Play(notes, strum, ring) => guitar.play(&notes, strum, ring),
            }
            if !label.is_empty() {
                sections.push((t, label));
            }
        }
        let mut l = [0.0f32; N];
        let mut r = [0.0f32; N];
        for x in l.iter_mut() {
            *x = guitar.tick();
        }
        unsafe { update_buffers(&env, l.as_mut_ptr(), r.as_mut_ptr(), N as i32) };
        lo.extend_from_slice(&l);
        ro.extend_from_slice(&r);
        t += N;
    }

    let mut bad = false;
    sections.push((lo.len(), ""));
    println!("{:<32} {:>7} {:>8} {:>8} {:>8}", "section", "peak", "rms dB", "L-R dB", "L/R corr");
    for w in sections.windows(2) {
        let (a, name) = w[0];
        let b = w[1].0;
        let (l, r) = (&lo[a..b], &ro[a..b]);
        let peak = l.iter().chain(r).fold(0.0f32, |m, v| m.max(v.abs()));
        let side: Vec<f32> = l.iter().zip(r).map(|(a, b)| a - b).collect();
        let dot: f64 = l.iter().zip(r).map(|(a, b)| *a as f64 * *b as f64).sum();
        let corr = dot / (rms(l) * rms(r) * l.len() as f64).max(1e-12);
        println!("{name:<32} {peak:>7.3} {:>8.1} {:>8.1} {corr:>8.2}", to_db(rms(l)), to_db(rms(&side)));
        if l.iter().chain(r).any(|v| !v.is_finite()) || peak >= 1.0 {
            bad = true;
        }
    }
    println!("final LED colour index: {}", unsafe { get_state_idx(&env) });

    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    write_wav(&out, &lo, &ro);
    println!("wrote {out} ({:.1} s)", lo.len() as f64 / SRU as f64);
    gain_sweep(&env);
    if bad {
        eprintln!("error: NaN or full-scale output");
        std::process::exit(1);
    }
    let _ = (get_buffer_size, get_param_min, get_param_max, is_param_enabled, get_param_name, get_param_unit);
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

fn to_db(v: f64) -> f64 {
    20.0 * v.max(1e-9).log10()
}

/// Holds an A2 + E3 power-chord (two sines, like a ringing dyad) through the Plexi (no fuzz, echo or room) at several Gain settings, and prints the output level and how much of the output's power is
/// distortion: the level of everything that isn't the two input notes (harmonics and intermodulation) relative to the notes, as an amplitude ratio (THD+IMD). Backing Gain off should clean it up without the level collapsing.
fn gain_sweep(env: &PatchEnv) {
    let s = state();
    s.fuzz_on = false;
    s.echo_on = false;
    unsafe { set_param(env, PARAM_ROOM, 0.0) };
    let tones = [A2, 165.0]; // E3 rounded to 165 Hz so both notes fit the 1 s window exactly (no leakage)
    println!("\nGain sweep, held A2+E3 dyad through the Plexi:\n{:>6} {:>8} {:>12}", "Gain", "rms dB", "distortion %");
    for gain in [1.0, GAIN_DEFAULT, 0.6, 0.4, 0.2, 0.0] {
        unsafe { set_param(env, PARAM_GAIN, gain) };
        let mut out = vec![];
        let mut n = 0usize;
        while out.len() < SRU * 3 / 2 {
            let mut l = [0.0f32; N];
            let mut r = [0.0f32; N];
            for x in l.iter_mut() {
                let t = n as f32 / SRU as f32;
                *x = tones.iter().map(|f| 0.08 * (core::f32::consts::TAU * f * t).sin()).sum();
                n += 1;
            }
            unsafe { update_buffers(env, l.as_mut_ptr(), r.as_mut_ptr(), N as i32) };
            out.extend_from_slice(&l);
        }
        // Skip the first half second (knob smoothing, filters settling), then measure one second.
        let w = &out[SRU / 2..SRU / 2 + SRU];
        let total: f64 = w.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / w.len() as f64;
        let tone_power: f64 = tones
            .iter()
            .map(|f| {
                let (mut c, mut si) = (0.0f64, 0.0f64);
                for (i, v) in w.iter().enumerate() {
                    let ph = std::f64::consts::TAU * *f as f64 * (SRU / 2 + i) as f64 / SRU as f64;
                    c += *v as f64 * ph.cos();
                    si += *v as f64 * ph.sin();
                }
                let (a, b) = (2.0 * c / w.len() as f64, 2.0 * si / w.len() as f64);
                (a * a + b * b) / 2.0
            })
            .sum();
        println!("{gain:>6.2} {:>8.1} {:>12.1}", to_db(total.sqrt()), 100.0 * ((total - tone_power).max(0.0) / tone_power).sqrt());
    }
}

fn write_wav(path: &str, l: &[f32], r: &[f32]) {
    let bytes = (l.len() * 8) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + bytes).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&(48_000u32 * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&bytes.to_le_bytes());
    for (a, c) in l.iter().zip(r) {
        b.extend_from_slice(&a.to_le_bytes());
        b.extend_from_slice(&c.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}
