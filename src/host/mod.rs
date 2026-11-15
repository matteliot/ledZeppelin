//! Host-only demo: `cargo sim [OUT.wav]` plays `score` on the modelled
//! guitar through the patch (see `pedalkit::host`), then sweeps Gain on a
//! held dyad.

mod score;

use crate::entry::{ENTRY, pedal, set_param, update_buffers};
use crate::led_zeppelin::presets::GAIN_DEFAULT;
use crate::pedalkit::abi::PatchEnv;
use crate::pedalkit::host::stats::to_db;
use crate::pedalkit::host::{self, BLOCK, Options, SRU};

const PARAM_GAIN: i32 = 0;
const PARAM_ROOM: i32 = 1;

pub fn main() {
    let out = host::out_path("target/render/led_zeppelin_demo.wav");
    let (events, total) = score::score();
    let o = Options { first: "Plexi crunch (riff)", input_trim: 1.0, working: 16 };
    let r = host::render(&ENTRY, events, total, o);
    let env = PatchEnv { abi_version: r.env.abi_version, user_ctx: r.env.user_ctx };
    let ok = host::stats::report(&r.l, &r.r, r.sections.clone());
    println!("final LED colour index: {}", unsafe { (ENTRY.get_state_idx)(&r.env) });
    host::wav::write_wav(&out, &r.l, &r.r);
    println!("wrote {out} ({:.1} s)", r.l.len() as f64 / SRU as f64);
    gain_sweep(&env);
    if !ok {
        eprintln!("error: NaN or full-scale output");
        std::process::exit(1);
    }
}

/// Holds an A2 + E3 power-chord (two sines, like a ringing dyad) through the Plexi (no fuzz, echo or room) at several Gain settings, and prints the output level and how much of the output's power is
/// distortion: the level of everything that isn't the two input notes (harmonics and intermodulation) relative to the notes, as an amplitude ratio (THD+IMD). Backing Gain off should clean it up without the level collapsing.
fn gain_sweep(env: &PatchEnv) {
    pedal().set_switches(false, false);
    unsafe { set_param(env, PARAM_ROOM, 0.0) };
    let tones = [score::A2, 165.0]; // E3 rounded to 165 Hz so both notes fit the 1 s window exactly (no leakage)
    println!("\nGain sweep, held A2+E3 dyad through the Plexi:\n{:>6} {:>8} {:>12}", "Gain", "rms dB", "distortion %");
    for gain in [1.0, GAIN_DEFAULT, 0.6, 0.4, 0.2, 0.0] {
        unsafe { set_param(env, PARAM_GAIN, gain) };
        let mut out = vec![];
        let mut n = 0usize;
        while out.len() < SRU * 3 / 2 {
            let mut l = [0.0f32; BLOCK];
            let mut r = [0.0f32; BLOCK];
            for x in l.iter_mut() {
                let t = n as f32 / SRU as f32;
                *x = tones.iter().map(|f| 0.08 * (core::f32::consts::TAU * f * t).sin()).sum();
                n += 1;
            }
            unsafe { update_buffers(env, l.as_mut_ptr(), r.as_mut_ptr(), BLOCK as i32) };
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
