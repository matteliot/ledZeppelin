//! The `cargo sim` demo: its score (the rendering itself is `pedalkit::host`).
//!
//! Plays an original E-minor blues-rock riff, chords and a lead line at 96 bpm on a modelled electric guitar (see `guitar`: exact tuning, pick and pickup, palm mutes, strums, slides, bends,
//! vibrato and human timing), drives the patch's entry points the way the firmware does (128-sample blocks, footswitch and knob events at set times) and writes a stereo float WAV. Prints
//! per-section level stats, then a Gain-knob sweep on a held two-note chord showing how much it cleans up. Fails on NaN or clipping.

use crate::pedalkit::guitar::Note;
use crate::led_zeppelin::presets::GAIN_DEFAULT;
use crate::pedalkit::host::{Ev, Score};

const E2: f32 = 82.41;
const G2: f32 = 98.0;
pub const A2: f32 = 110.0;
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

enum Local {
    /// Play these notes, strummed this many seconds apart; `ring` lets what's already sounding carry on.
    Play(Vec<Note>, f32, bool),
    /// The fretting hand lets go.
    Release,
    Press,
    Hold,
    Gain(f32),
}

fn local_score() -> (Vec<(f64, Local, &'static str)>, f64) {
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
    let mut play = |ev: &mut Vec<(f64, Local, &'static str)>, t: &mut f64, times: usize| {
        for _ in 0..times {
            for (n, len) in riff() {
                ev.push((*t + slop(), Local::Play(vec![n], 0.0, false), ""));
                *t += len * e;
            }
        }
    };
    play(&mut ev, &mut t, 2);
    ev.push((t, Local::Gain(0.35), "Plexi, Gain backed off (chords)"));
    // Open-position chords, strummed down and left to ring, with a lighter upstroke on the "and" of 3.
    for c in [[E2, B2, E3], [A2, E3, A3], [D3, A3, D4], [G2, D3, G3]] {
        let chord = |v: f32| c.iter().map(|f| Note::new(*f).vel(v)).collect::<Vec<_>>();
        ev.push((t + 0.001, Local::Play(chord(0.85), 0.014, false), ""));
        let mut up = chord(0.55);
        up.reverse();
        ev.push((t + 3.0 * e, Local::Play(up, 0.01, false), ""));
        t += 4.0 * e;
    }
    ev.push((t, Local::Gain(GAIN_DEFAULT), "Tone Bender fuzz (riff)"));
    ev.push((t + 0.001, Local::Press, ""));
    play(&mut ev, &mut t, 1);
    ev.push((t, Local::Press, "Plexi + Echoplex echo (lead)"));
    ev.push((t + 0.001, Local::Hold, ""));
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
        ev.push((t + slop(), Local::Play(vec![n], 0.0, false), ""));
        t += len * e;
    }
    ev.push((t - 0.05, Local::Release, ""));
    ev.push((t, Local::Hold, "echo off, repeats ring out"));
    ev.push((t + 0.001, Local::Play([E2, B2, E3].iter().map(|f| Note::new(*f).vel(0.9).vib()).collect(), 0.016, false), ""));
    (ev, t + 3.5)
}

/// The score as pedalkit events.
pub fn score() -> (Score, f64) {
    let (ev, total) = local_score();
    (ev.into_iter().map(|(t, e, l)| (t as f64, e.into(), l)).collect(), total as f64)
}

impl From<Local> for Ev {
    fn from(e: Local) -> Ev {
        match e {
            Local::Play(notes, strum, ring) => Ev::Play(notes, strum, ring),
            Local::Press => Ev::Press,
            Local::Hold => Ev::Hold,
            Local::Release => Ev::Release,
            Local::Gain(v) => Ev::Knob(0, v),
        }
    }
}
