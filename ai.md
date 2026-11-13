# ai.md — Project Requirements Log

This file records every requirement given for the **ledZeppelin** project, so the whole project can be recreated from scratch by handing this file to an AI assistant as the prompt.

It is kept up to date as new requirements are given. Requirements are listed in the order they were given; later entries override earlier ones where they conflict (and the earlier entry is marked as superseded).

---

## Meta / Process Requirements

1. Maintain this `ai.md` file at the repo root. Its purpose is to keep a list of all requirements so the entire project can be recreated from this file alone. Update it as work progresses, whenever a new requirement is given.

## Project Requirements

2. **Do exactly what the sibling project `../ironButterfly` (https://github.com/matteliot/ironButterfly) does, but for Led Zeppelin: a Polyend Endless patch binary for "the most Led Zeppelin guitar sound ever."** Build it with `cargo build --release`.
   - The target sound is Jimmy Page, 1969–1971: a Les Paul on the bridge pickup with its volume knob around 7–8, through an always-on Echoplex EP-3 preamp into a cranked, non-master-volume Marshall Super Lead (Plexi) and a 4x12, recorded with distant room mics ("distance makes depth").
   - It's lower gain than people expect: the crunch is mostly power-amp breakup, and the guitar's volume knob is the gain control.
   - From the first album: a Tone Bender MkII fuzz, and the Echoplex used as a tape echo.
3. Push the project to its GitHub remote (`origin`, https://github.com/matteliot/ledZeppelin).
4. On `master`, add the demo render (`cargo sim` output) as `samples/led_zeppelin_demo.wav`. Commit with the message "ai" and push.
5. Keep `ai.md` lines unwrapped: only wrap where needed, or at a natural word break around 200 characters.
6. As in ironButterfly (its requirement 6), turning the drive knob (here Gain) down must actually clean up chords and ringing notes, like rolling back the guitar, while the volume across the knob stays roughly level.

7. The demo's guitar sounded fake ("like MIDI"): plain Karplus-Strong noise plucks, out of tune up high, robotic timing. Make the demo guitar realistic by modelling it better (not by using samples), then re-render `samples/led_zeppelin_demo.wav`.

### Implementation decisions (needed to recreate requirement 2)

- **Toolchain and packaging (copied from `../ironButterfly`).**
  - Language and target: `no_std` Rust, edition 2024, nightly toolchain, `thumbv7em-none-eabihf`.
  - Copied files: `src/abi.rs` (FxPatchSDK ABI 0x000B header), `patch.ld`, `build.rs`, `.cargo/config.toml` (pedal is the default target, `-fp64`, `cargo sim` alias), `rust-toolchain.toml`, `.gitignore` and `src/dsp.rs` (f32-only sin/cos/exp2/tanh, biquads, one-pole, delay lines).
  - `build-endl.sh` makes `target/endless/led_zeppelin.endl`. It checks for f64 instructions and the 512 KB limit; `--deploy` copies the file to `/Volumes/ENDLESS`.
- **Input.** HP 70 Hz. A gate follows the input envelope (1 ms attack, 80 ms release; opens at 0.0012, closes at 0.0006, 2 ms open / 60 ms close) and is applied to the Tone Bender only.
- **Les Paul volume pot (Gain knob, default 0.8).** Level `-30 dB × (1 - gain)`, and a LP (Q 0.6) at 6.5 kHz + 11 kHz × gain for the pot's treble loss. Makeup after the cab: `+12 dB × max(0, 0.8 - gain)`.
- **Echoplex EP-3 preamp (always on).** +2.5 dB peak at 5 kHz (Q 0.7), ×1.6, then `tanh(y + 0.15) - tanh(0.15)` and HP 25 Hz.
- **Tone Bender MkII (footswitch press, crossfaded over 12 ms, starts off).**
  1. HP 110 Hz, then +32 dB.
  2. Stage 1: `tanh(u)` for positive input, `1.2·tanh(0.6u)` for negative (soft, asymmetric germanium).
  3. Stage 2: `tanh(2a + 0.3) - tanh(0.3)` (biased, so it sputters as notes die).
  4. LP 4.5 kHz, +3 dB at 800 Hz (Q 0.8), ×0.22 into the amp. It's gated.
- **Marshall Super Lead (Plexi).**
  1. Bright cap: +2.5 dB at 3 kHz (Q 0.7).
  2. Triode 1: ×8, then `tanh(v + 0.1) - tanh(0.1)`, 2x oversampled (the average of the stage at the half-sample midpoint and at the sample). HP 25 Hz coupling.
  3. Triode 2: ×2.5, the same curve, HP 25 Hz coupling.
  4. Tone stack: -2 dB at 100 Hz (Q 0.8), -3.5 dB at 650 Hz (Q 0.8), +2 dB at 2.5 kHz (Q 0.9).
  5. EL34 power amp: ×2.2, then `tanh(p / (1 + sag))`. Sag follows `max(0, |p| - 0.5)` with a 50 ms one-pole, so only a hard-working power amp sags and quiet playing stays clean.
  6. Presence: +3 dB at 3.5 kHz (Q 0.9).
- **4x12 Greenback cab.** HP 75 Hz (Q 0.9), +3 dB at 110 Hz (Q 1.2), +4 dB at 2.2 kHz (Q 1.5), -3 dB at 4.2 kHz (Q 2), LP 5.2 kHz (Q 0.8), LP 7.5 kHz (Q 0.6).
- **Measured cleanup (requirement 6).** Held A2 + 165 Hz sine dyad (0.08 each), no fuzz/echo/room, THD+IMD as an amplitude ratio: Gain 1.0 → 30%, 0.8 → 21%, 0.6 → 13%, 0.4 → 8%, 0.2 → 4.5%, 0 → 2%. Output level stays within -8.7 to -13.2 dB RMS across the knob.
- **Echoplex tape echo (footswitch hold, starts off).**
  - The send is switched (12 ms crossfade), so repeats ring out after it's turned off.
  - Delay 320 ms, modulated by wow (0.6 Hz, ±0.25%) and flutter (7 Hz, ±0.06%), read with linear interpolation from a 32768-sample tape.
  - Feedback 0.38 through HP 150 Hz and LP 3.2 kHz, written to tape through `tanh` (tape saturation).
  - Level ×0.5, panned L 0.45, R 1.0. The dry guitar is centred.
- **Room mics (Room knob, default 0.4).**
  - Fed the mono sum of guitar and echo. A 4096-sample pre-delay line gives six early reflections (7.3/11.9/17.1/23.6/31.2/41.5 ms, gains 0.55/0.5/0.42/0.36/0.3/0.24), alternating sides with half on the other side, at ×0.5.
  - The late room is Freeverb-style, fed from the 18 ms tap × 0.25. Four LP-feedback combs per channel (L 677/719/773/821 samples; R is +23 on each), feedback 0.76, damping 0.45. Then two allpasses (L 347/263, R 370/286) with gain 0.5.
  - A compressor squashes the room (5 ms attack, 150 ms release on max(|L|, |R|)): gain `4.5 / (1 + 6·env)`.
  - Mix: dry × (1 - 0.45·room) + room signal × room.
  - The echo tape and the room are all-zero statics, so they're in .bss, not the file.
- **Output.** Level (square law, unity at 0.75) × 0.42 trim, then `tanh` × 0.98. Knob changes are smoothed over 20 ms.
- **Controls.**
  - Knobs: Gain (default 0.8), Room (default 0.4), Level (default 0.75).
  - Press: Tone Bender on/off. Hold: Echoplex echo on/off. Both start off (plain Plexi).
  - LED: Plexi light yellow, Plexi+echo light blue, Tone Bender red, Tone Bender+echo magenta. Red also means the working buffer is missing.
- **Host audition.** `cargo sim` renders an *original* E-minor blues-rock riff at 96 bpm, not any actual Led Zeppelin riff, on the modelled guitar below.
  - It plays Plexi riff ×2 (palm-muted chugs, accented notes ringing, vibrato on long notes) → Gain 0.35 with strummed open chords (downstroke, lighter upstroke) → Gain back to default with the Tone Bender riff → Plexi + Echoplex lead line (a slide in, a whole-step bend, vibrato) → echo off with a chord as the repeats ring out.
  - Every event's timing is nudged by up to ±7 ms, repeatably.
  - It writes `target/render/led_zeppelin_demo.wav`, prints per-section stats and the Gain sweep above, and fails on NaN or full-scale output.

### Implementation decisions (needed to recreate requirement 7): the guitar model, `src/guitar.rs` (host only)

- **String.** Extended Karplus-Strong per note: a 4096-sample delay line read with 4-point Lagrange interpolation, so the loop delay is fractional. The loop has a one-zero loss filter `g·((1-s)·y + s·y₋₁)`, and the read delay is `SR/f - s`, so every note is in tune (measured within 0.2 cents from E2 to B5; the old integer-length string was up to ~16 cents flat).
  - Loop gain per period `g = 10^(-3 / (T60·f))`. Held T60 = `6·sqrt(82.41/f)` s, clamped to 1.8–6 s. Palm mute: T60 0.18 s, s = 0.5. Otherwise s = 0.18 + 0.12·(1 - velocity).
  - Releasing a note (the next note, unless it's allowed to ring) switches T60 to 0.06 s.
  - Two polarizations: the second is detuned +0.7 cents, decays at 0.8× the T60, and is mixed at 0.45.
- **Pick.** The excitation is one period of one-pole low-passed noise at 1.2 kHz + 6 kHz × velocity (×0.45 when palm muted), combed at the picking position (0.16 ± 0.04 of the string, random per note), with the mean removed and normalized to 0.12 × velocity RMS. Plus a click: 96 samples of differentiated noise × 0.015 × velocity, decaying over 20 samples.
  - Attack pitch: +7 cents × velocity, decaying with a 50 ms time constant.
- **Pitch movement (cents).** Vibrato 5.3 Hz, ±18 cents, fading in after 0.22 s over 0.35 s. Bend: smoothstep to the target over its duration. Slide: from N semitones below, smoothstep over its duration.
- **Pickup.** Each polarization is heard as `y - y(0.075·period)` (a bridge humbucker comb). The sum is ×0.55, then a 4.3 kHz low-pass with Q 1.3 (pickup and cable resonance) and HP 60 Hz.
- **Player.** A note can override its T60 (a fret-hand-muted "chk", or a chord sustained in front of a loud amp). Velocity varies ±12% per note, and onsets by up to 1.5 ms. Chords strum low to high (14 ms per string down, 10 ms up). The voice is dropped 0.4 s after release, or after 1.5 × T60.

8. **Buffers are zero-initialized in `.bss` and explicitly zeroed in `patch_init` before use** (rather than shipped as zeros inside the patch image). The reverb/echo buffers were already separate all-zero statics in `.bss`. `patch_init` then clears each buffer static in place with `core::ptr::write_bytes` (no heap or `calloc` in `no_std`, and a zeroed copy would be a stack temporary) before `init`: the loader should already have cleared `.bss`, but nothing promises it, and a NaN left in a feedback loop never decays. The firmware working buffer stays unused.

9. **Ship the built patch in a `release/` directory at the repo root** (`release/led_zeppelin.endl`, a copy of `target/endless/led_zeppelin.endl` from `./build-endl.sh`), committed to git, so a user can download it from GitHub and copy it straight to the pedal without building. README has an Install section saying so. Rebuild and recopy it whenever the patch changes.
