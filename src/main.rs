//! led_zeppelin: a Polyend Endless patch for the most Led Zeppelin guitar sound ever: Jimmy Page, 1969-1971.
//!
//! Signal chain (mono in from the left input, stereo out):
//!   Les Paul volume pot (the Gain knob: level and a little treble loss, so backing it off cleans up like rolling back the guitar)
//!   -> Echoplex EP-3 preamp (always in: a slight JFET-ish boost, sparkle and thickness)
//!   -> Tone Bender MkII germanium fuzz (footswitch press; off = no fuzz, just the amp)
//!   -> non-master-volume Marshall Super Lead "Plexi": bright cap, two cascaded triode stages, tone stack, EL34 power amp with sag, presence
//!   -> 4x12 Greenback cab
//!   + Echoplex tape echo (footswitch hold): ~320 ms slap with wow, flutter and tape saturation, panned right
//!   + room mics ("distance makes depth"): early reflections and a short room, squashed by a compressor, blended by the Room knob
//!
//! Controls
//!   Left knob   Gain   (the guitar's volume knob: 1 = full up, back it off to clean up)
//!   Mid knob    Room   (close mic -> distant room mics)
//!   Right knob  Level  (output; unity at 3 o'clock)
//!   Footswitch press  Tone Bender fuzz on/off
//!   Footswitch hold   Echoplex echo on/off (repeats ring out when it's switched off)
//!   LED  Plexi: light yellow | Plexi+echo: light blue | Tone Bender: red | Tone Bender+echo: magenta
//!
//! Host builds (`cargo sim`) render a demo through the same code; see `host`.

#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

/// Reusable blocks (effects, DSP, the Endless ABI); see its docs.
mod pedalkit;

/// The pedal itself.
mod led_zeppelin;

endless_patch!(kit = crate::pedalkit, pedal = crate::led_zeppelin::Patch);

#[cfg(not(target_os = "none"))]
mod host;

#[cfg(not(target_os = "none"))]
fn main() {
    host::main();
}
