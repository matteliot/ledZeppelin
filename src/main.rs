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

#[cfg_attr(not(target_os = "none"), allow(dead_code))]
mod abi;
#[allow(dead_code)]
mod dsp;

use core::cell::UnsafeCell;
use core::f32::consts::TAU;

use abi::*;
use dsp::*;

// ---------------------------------------------------------------- voicing

/// Les Paul volume pot: level (dB) at Gain 0 (it's 0 dB at Gain 1), and its treble loss: the low-pass corner at Gain 0 and the extra at Gain 1 (Hz).
const POT_FLOOR_DB: f32 = -30.0;
const POT_LP_MIN: f32 = 6500.0;
const POT_LP_RANGE: f32 = 11000.0;
/// Below the default Gain, this much makeup (dB per unit of knob below it) keeps a backed-off guitar from dropping out of the mix while still cleaning up.
const GAIN_DEFAULT: f32 = 0.8;
const GAIN_MAKEUP_DB: f32 = 12.0;

/// Echoplex EP-3 preamp: gain, bias (asymmetry) and the top-end lift.
const EP_GAIN: f32 = 1.6;
const EP_BIAS: f32 = 0.15;

/// Tone Bender MkII: fuzz gain (dB), the second stage's bias, and its output level into the amp.
const TB_GAIN_DB: f32 = 32.0;
const TB_BIAS: f32 = 0.3;
const TB_OUT: f32 = 0.22;

/// Plexi: the two cascaded preamp triodes' gains and bias, the power amp's drive, and its sag: how much the supply droops under a loud note, above the knee.
const PRE1_GAIN: f32 = 8.0;
const PRE2_GAIN: f32 = 2.5;
const TRIODE_BIAS: f32 = 0.1;
const POWER_GAIN: f32 = 2.2;
const SAG: f32 = 1.0;
const SAG_KNEE: f32 = 0.5;

/// Echoplex tape echo: delay (s), feedback, wow (0.6 Hz) and flutter (7 Hz) depth as fractions of the delay, and its level.
const ECHO_S: f32 = 0.32;
const ECHO_FB: f32 = 0.38;
const WOW_HZ: f32 = 0.6;
const WOW_DEPTH: f32 = 0.0025;
const FLUTTER_HZ: f32 = 7.0;
const FLUTTER_DEPTH: f32 = 0.0006;
const ECHO_MIX: f32 = 0.5;
const ECHO_PAN: (f32, f32) = (0.45, 1.0);

/// Room mics: comb feedback and damping, early-reflection taps (ms, gain), pre-delay of the late room (ms), compressor amount, and the level at Room 1.
const ROOM_FEEDBACK: f32 = 0.76;
const ROOM_DAMP: f32 = 0.45;
const ER_TAPS: [(f32, f32); 6] = [(7.3, 0.55), (11.9, 0.5), (17.1, 0.42), (23.6, 0.36), (31.2, 0.3), (41.5, 0.24)];
const ROOM_PREDELAY_MS: f32 = 18.0;
const ROOM_COMP: f32 = 6.0;
const ROOM_GAIN: f32 = 4.5;
/// How much of the close mic the Room knob takes away at 1 (the room mics take over).
const ROOM_DRY_DUCK: f32 = 0.45;

/// Noise gate thresholds on the input envelope (linear), with hysteresis.
const GATE_OPEN: f32 = 0.0012;
const GATE_CLOSE: f32 = 0.0006;

/// Output trim: unity Level puts the Plexi riff around -12 dB RMS.
const OUT_TRIM: f32 = 0.42;

// ---------------------------------------------------------------- room parts

/// Lowpass-feedback comb filter (Freeverb style), length N samples.
struct Comb<const N: usize> {
    buf: [f32; N],
    i: usize,
    z: f32,
}

impl<const N: usize> Comb<N> {
    const NEW: Self = Self { buf: [0.0; N], i: 0, z: 0.0 };
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.buf[self.i];
        self.z = y + (self.z - y) * ROOM_DAMP;
        self.buf[self.i] = x + self.z * ROOM_FEEDBACK;
        self.i = if self.i + 1 == N { 0 } else { self.i + 1 };
        y
    }
}

/// Schroeder allpass, length N samples.
struct Allpass<const N: usize> {
    buf: [f32; N],
    i: usize,
}

impl<const N: usize> Allpass<N> {
    const NEW: Self = Self { buf: [0.0; N], i: 0 };
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        self.buf[self.i] = x + b * 0.5;
        self.i = if self.i + 1 == N { 0 } else { self.i + 1 };
        b - x
    }
}

/// A live room heard from distant mics: early reflections off a pre-delay line, then a short Freeverb-style tail (about 0.55x Freeverb's lengths at 48 kHz; the right channel is spread by 23).
/// All zero, so it lives in .bss.
struct Room {
    pre: Delay<4096>,
    l: (Comb<677>, Comb<719>, Comb<773>, Comb<821>, Allpass<347>, Allpass<263>),
    r: (Comb<700>, Comb<742>, Comb<796>, Comb<844>, Allpass<370>, Allpass<286>),
}

impl Room {
    const NEW: Self = Self {
        pre: Delay::NEW,
        l: (Comb::NEW, Comb::NEW, Comb::NEW, Comb::NEW, Allpass::NEW, Allpass::NEW),
        r: (Comb::NEW, Comb::NEW, Comb::NEW, Comb::NEW, Allpass::NEW, Allpass::NEW),
    };

    #[inline]
    fn process(&mut self, x: f32) -> (f32, f32) {
        self.pre.push(x);
        // Early reflections alternate sides, each also heard (softer) on the other.
        let (mut el, mut er) = (0.0, 0.0);
        for (i, (ms, g)) in ER_TAPS.iter().enumerate() {
            let s = self.pre.read(ms * (SR / 1000.0)) * g;
            if i % 2 == 0 {
                el += s;
                er += 0.5 * s;
            } else {
                er += s;
                el += 0.5 * s;
            }
        }
        let x = self.pre.read(ROOM_PREDELAY_MS * (SR / 1000.0)) * 0.25;
        let l = &mut self.l;
        let yl = l.5.process(l.4.process(l.0.process(x) + l.1.process(x) + l.2.process(x) + l.3.process(x)));
        let r = &mut self.r;
        let yr = r.5.process(r.4.process(r.0.process(x) + r.1.process(x) + r.2.process(x) + r.3.process(x)));
        (0.5 * el + yl, 0.5 * er + yr)
    }
}

// ---------------------------------------------------------------- state

struct Lz {
    ready: bool,
    error: bool,
    // knobs, 0..1
    gain: f32,
    room: f32,
    level: f32,
    fuzz_on: bool,
    echo_on: bool,
    // smoothed values and targets
    pot: f32,
    pot_t: f32,
    mk: f32,
    mk_t: f32,
    out_g: f32,
    out_t: f32,
    wet: f32,
    wet_t: f32,
    fuzz_mix: f32,
    echo_send: f32,
    // guitar and Echoplex preamp
    hp_in: Biquad,
    pot_lp: Biquad,
    ep_lift: Biquad,
    ep_hp: Biquad,
    // Tone Bender
    tb_hp: Biquad,
    tb_lp: Biquad,
    tb_mid: Biquad,
    // Plexi
    bright: Biquad,
    cpl: [Biquad; 2],
    stack: [Biquad; 3],
    presence: Biquad,
    v1: f32,
    sag: f32,
    // cab
    cab: [Biquad; 6],
    // Echoplex echo (its tape lives in ECHO_TAPE)
    echo_hp: Biquad,
    echo_lp: Biquad,
    wow_ph: f32,
    flutter_ph: f32,
    // room mic compressor
    comp_env: f32,
    // dynamics
    env: f32,
    gate: f32,
    gate_open: bool,
    // coefficients
    c_param: f32,
    c_switch: f32,
    c_att: f32,
    c_rel: f32,
    c_gate_open: f32,
    c_gate_close: f32,
    c_sag: f32,
    c_comp_att: f32,
    c_comp_rel: f32,
}

impl Lz {
    const NEW: Self = Self {
        ready: false,
        error: false,
        gain: GAIN_DEFAULT,
        room: 0.4,
        level: 0.75,
        fuzz_on: false,
        echo_on: false,
        pot: 1.0,
        pot_t: 1.0,
        mk: 1.0,
        mk_t: 1.0,
        out_g: 0.0,
        out_t: 0.0,
        wet: 0.0,
        wet_t: 0.0,
        fuzz_mix: 0.0,
        echo_send: 0.0,
        hp_in: Biquad::NEW,
        pot_lp: Biquad::NEW,
        ep_lift: Biquad::NEW,
        ep_hp: Biquad::NEW,
        tb_hp: Biquad::NEW,
        tb_lp: Biquad::NEW,
        tb_mid: Biquad::NEW,
        bright: Biquad::NEW,
        cpl: [Biquad::NEW; 2],
        stack: [Biquad::NEW; 3],
        presence: Biquad::NEW,
        v1: 0.0,
        sag: 0.0,
        cab: [Biquad::NEW; 6],
        echo_hp: Biquad::NEW,
        echo_lp: Biquad::NEW,
        wow_ph: 0.0,
        flutter_ph: 0.0,
        comp_env: 0.0,
        env: 0.0,
        gate: 0.0,
        gate_open: false,
        c_param: 1.0,
        c_switch: 1.0,
        c_att: 1.0,
        c_rel: 1.0,
        c_gate_open: 1.0,
        c_gate_close: 1.0,
        c_sag: 1.0,
        c_comp_att: 1.0,
        c_comp_rel: 1.0,
    };

    fn init(&mut self) {
        self.hp_in.highpass(70.0, 0.707);
        // Echoplex preamp: a touch of sparkle up top; the coupling cap clears the bias DC.
        self.ep_lift.peak(5000.0, 0.7, 2.5);
        self.ep_hp.highpass(25.0, 0.707);
        // Tone Bender MkII: tight lows, a thick mid hump, soft top.
        self.tb_hp.highpass(110.0, 0.707);
        self.tb_lp.lowpass(4500.0, 0.707);
        self.tb_mid.peak(800.0, 0.8, 3.0);
        // Plexi: bright cap on the input, coupling caps between stages, the Marshall tone stack (lean bass, scooped low mids, treble up), presence in the power amp.
        self.bright.peak(3000.0, 0.7, 2.5);
        self.cpl[0].highpass(25.0, 0.707);
        self.cpl[1].highpass(25.0, 0.707);
        self.stack[0].peak(100.0, 0.8, -2.0);
        self.stack[1].peak(650.0, 0.8, -3.5);
        self.stack[2].peak(2500.0, 0.9, 2.0);
        self.presence.peak(3500.0, 0.9, 3.0);
        // 4x12 Greenbacks: closed-back resonance, upper-mid bark, steep roll-off above 5 kHz.
        self.cab[0].highpass(75.0, 0.9);
        self.cab[1].peak(110.0, 1.2, 3.0);
        self.cab[2].peak(2200.0, 1.5, 4.0);
        self.cab[3].peak(4200.0, 2.0, -3.0);
        self.cab[4].lowpass(5200.0, 0.8);
        self.cab[5].lowpass(7500.0, 0.6);
        // Echoplex tape: each repeat loses lows and highs.
        self.echo_hp.highpass(150.0, 0.707);
        self.echo_lp.lowpass(3200.0, 0.707);
        self.c_param = coef(0.02);
        self.c_switch = coef(0.012);
        self.c_att = coef(0.001);
        self.c_rel = coef(0.08);
        self.c_gate_open = coef(0.002);
        self.c_gate_close = coef(0.06);
        self.c_sag = coef(0.05);
        self.c_comp_att = coef(0.005);
        self.c_comp_rel = coef(0.15);
        self.retarget();
        self.pot = self.pot_t;
        self.mk = self.mk_t;
        self.out_g = self.out_t;
        self.wet = self.wet_t;
        self.fuzz_mix = if self.fuzz_on { 1.0 } else { 0.0 };
        self.echo_send = if self.echo_on { 1.0 } else { 0.0 };
        self.ready = true;
    }

    fn retarget(&mut self) {
        self.pot_t = db(POT_FLOOR_DB * (1.0 - self.gain));
        self.mk_t = db(GAIN_MAKEUP_DB * (GAIN_DEFAULT - self.gain).max(0.0));
        self.pot_lp.lowpass(POT_LP_MIN + POT_LP_RANGE * self.gain, 0.6);
        self.wet_t = self.room;
        // Square law: 0 at minimum, unity at 3 o'clock, +5 dB at maximum.
        let v = self.level / 0.75;
        self.out_t = v * v * OUT_TRIM;
    }

    /// Echoplex EP-3 preamp: always in the chain. A small, slightly asymmetric boost.
    #[inline]
    fn echoplex_pre(&mut self, x: f32) -> f32 {
        let y = self.ep_lift.process(x) * EP_GAIN;
        self.ep_hp.process(tanh(y + EP_BIAS) - tanh(EP_BIAS))
    }

    /// Tone Bender MkII: germanium, soft and asymmetric, then a biased second stage that sputters as notes die.
    #[inline]
    fn tone_bender(&mut self, x: f32) -> f32 {
        let u = self.tb_hp.process(x) * db(TB_GAIN_DB);
        let a = if u > 0.0 { tanh(u) } else { tanh(0.6 * u) * 1.2 };
        let b = tanh(2.0 * a + TB_BIAS) - tanh(TB_BIAS);
        self.tb_mid.process(self.tb_lp.process(b)) * TB_OUT
    }

    /// Non-master-volume Marshall Super Lead: two triode stages (the first 2x oversampled), tone stack, then the EL34s, where most of the crunch lives.
    #[inline]
    fn plexi(&mut self, x: f32) -> f32 {
        let v = self.bright.process(x) * PRE1_GAIN;
        let a = 0.5 * (triode(0.5 * (v + self.v1)) + triode(v));
        self.v1 = v;
        let a = self.cpl[0].process(a);
        let b = self.cpl[1].process(triode(a * PRE2_GAIN));
        let mut t = b;
        for s in self.stack.iter_mut() {
            t = s.process(t);
        }
        // The supply sags under loud notes (only once the power amp is working hard), squeezing it: the "bloom". Quiet playing never sags, so a backed-off guitar stays clean.
        let p = t * POWER_GAIN;
        self.sag += ((p.abs() - SAG_KNEE).max(0.0) - self.sag) * self.c_sag;
        let y = tanh(p / (1.0 + SAG * self.sag));
        self.presence.process(y)
    }

    /// Echoplex tape echo: one repeat per pass, each a little darker and more saturated, with wow and flutter.
    #[inline]
    fn echo(&mut self, tape: &mut Delay<32768>, x: f32) -> f32 {
        self.wow_ph += WOW_HZ / SR;
        if self.wow_ph >= 1.0 {
            self.wow_ph -= 1.0;
        }
        self.flutter_ph += FLUTTER_HZ / SR;
        if self.flutter_ph >= 1.0 {
            self.flutter_ph -= 1.0;
        }
        let d = ECHO_S * SR * (1.0 + WOW_DEPTH * sin(TAU * self.wow_ph) + FLUTTER_DEPTH * sin(TAU * self.flutter_ph));
        let y = tape.read(d);
        let fb = self.echo_lp.process(self.echo_hp.process(y));
        tape.push(tanh(x + fb * ECHO_FB));
        y
    }

    #[inline]
    fn tick(&mut self, tape: &mut Delay<32768>, room: &mut Room, x: f32) -> (f32, f32) {
        self.pot += (self.pot_t - self.pot) * self.c_param;
        self.mk += (self.mk_t - self.mk) * self.c_param;
        self.out_g += (self.out_t - self.out_g) * self.c_param;
        self.wet += (self.wet_t - self.wet) * self.c_param;
        let ft = if self.fuzz_on { 1.0 } else { 0.0 };
        self.fuzz_mix += (ft - self.fuzz_mix) * self.c_switch;
        let et = if self.echo_on { 1.0 } else { 0.0 };
        self.echo_send += (et - self.echo_send) * self.c_switch;

        let x = self.hp_in.process(x);
        let a = x.abs();
        self.env += (a - self.env) * if a > self.env { self.c_att } else { self.c_rel };
        if self.env > GATE_OPEN {
            self.gate_open = true;
        } else if self.env < GATE_CLOSE {
            self.gate_open = false;
        }
        let (gt, gc) = if self.gate_open { (1.0, self.c_gate_open) } else { (0.0, self.c_gate_close) };
        self.gate += (gt - self.gate) * gc;

        // Guitar volume pot, then the Echoplex preamp; the Tone Bender is crossfaded in front of the amp.
        let x = self.pot_lp.process(x) * self.pot;
        let x = self.echoplex_pre(x);
        let f = self.tone_bender(x) * self.gate;
        let x = x + (f - x) * self.fuzz_mix;
        let mut g = self.plexi(x);
        for b in self.cab.iter_mut() {
            g = b.process(g);
        }
        let g = g * self.mk;

        // The echo's send is switched, so repeats ring out after it's turned off.
        let e = self.echo(tape, g * self.echo_send) * ECHO_MIX;
        let dl = g + e * ECHO_PAN.0;
        let dr = g + e * ECHO_PAN.1;

        // Room mics hear everything, squashed hard by a compressor.
        let (rl, rr) = room.process(0.5 * (dl + dr));
        let m = rl.abs().max(rr.abs());
        self.comp_env += (m - self.comp_env) * if m > self.comp_env { self.c_comp_att } else { self.c_comp_rel };
        let cg = ROOM_GAIN / (1.0 + ROOM_COMP * self.comp_env);
        let dry = 1.0 - ROOM_DRY_DUCK * self.wet;
        let l = dl * dry + rl * cg * self.wet;
        let r = dr * dry + rr * cg * self.wet;
        (limit(l * self.out_g), limit(r * self.out_g))
    }

    fn led(&self) -> Color {
        if self.error {
            return Color::Red;
        }
        match (self.fuzz_on, self.echo_on) {
            (false, false) => Color::LightYellow,
            (false, true) => Color::LightBlue,
            (true, false) => Color::Red,
            (true, true) => Color::Magenta,
        }
    }
}

/// A triode stage: soft and slightly asymmetric (bias shifted), DC removed.
#[inline]
fn triode(v: f32) -> f32 {
    tanh(v + TRIODE_BIAS) - tanh(TRIODE_BIAS)
}

/// Output soft limiter, never reaching ±1.
#[inline]
fn limit(x: f32) -> f32 {
    tanh(x) * 0.98
}

/// All patch state. The firmware calls every entry point from the audio thread (SDK: "no need to synchronize"). The echo tape and the room are all zero, so they live in .bss and don't bloat the
/// patch file.
struct Global<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Global<T> {}
static STATE: Global<Lz> = Global(UnsafeCell::new(Lz::NEW));
static ECHO_TAPE: Global<Delay<32768>> = Global(UnsafeCell::new(Delay::NEW));
static ROOM: Global<Room> = Global(UnsafeCell::new(Room::NEW));

#[allow(clippy::mut_from_ref)]
fn state() -> &'static mut Lz {
    // SAFETY: single-threaded access from the firmware's audio thread.
    unsafe { &mut *STATE.0.get() }
}

#[allow(clippy::mut_from_ref)]
fn tape() -> &'static mut Delay<32768> {
    // SAFETY: as `state`; only `update_buffers` uses it.
    unsafe { &mut *ECHO_TAPE.0.get() }
}

#[allow(clippy::mut_from_ref)]
fn room() -> &'static mut Room {
    // SAFETY: as `state`; only `update_buffers` uses it.
    unsafe { &mut *ROOM.0.get() }
}

/// Zeroes a buffer static in place. The loader should already have cleared
/// .bss, but nothing promises it, and a NaN left in a feedback loop never
/// decays. (In place: a zeroed copy would be a large stack temporary.)
///
/// SAFETY: `T` must be made only of f32 and usize fields, so all-zero bits
/// are a valid value; single-threaded as `state`.
unsafe fn clear<T>(g: &Global<T>) {
    unsafe { core::ptr::write_bytes(g.0.get(), 0, 1) }
}

// ---------------------------------------------------------------- entry points

const PARAM_GAIN: i32 = 0;
const PARAM_ROOM: i32 = 1;
const PARAM_LEVEL: i32 = 2;

unsafe extern "C" fn patch_init(env: *const PatchEnv) {
    if env.is_null() || unsafe { (*env).abi_version } != PATCH_ABI_VERSION as u32 {
        return;
    }
    unsafe {
        clear(&ECHO_TAPE);
        clear(&ROOM);
    }
    state().init();
}

unsafe extern "C" fn update_buffers(_: *const PatchEnv, left: *mut f32, right: *mut f32, samples: i32) -> f32 {
    let Ok(n) = usize::try_from(samples) else { return 0.0 };
    if left.is_null() || right.is_null() || n == 0 {
        return 0.0;
    }
    // SAFETY: the firmware passes two buffers of `samples` floats.
    let (l, r) = unsafe { (core::slice::from_raw_parts_mut(left, n), core::slice::from_raw_parts_mut(right, n)) };
    let s = state();
    if !s.ready {
        return 0.0;
    }
    let (tape, room) = (tape(), room());
    for (a, b) in l.iter_mut().zip(r.iter_mut()) {
        let (ol, or) = s.tick(tape, room, *a);
        *a = ol;
        *b = or;
    }
    0.0
}

unsafe extern "C" fn set_buffer(_: *const PatchEnv, buffer: *mut f32) {
    // All state is static; the working buffer isn't needed.
    if buffer.is_null() {
        state().error = true;
    }
}

unsafe extern "C" fn get_buffer_size(_: *const PatchEnv) -> i32 {
    WORKING_BUFFER_SIZE
}

unsafe extern "C" fn get_param_min(_: *const PatchEnv, _: i32) -> f32 {
    0.0
}

unsafe extern "C" fn get_param_max(_: *const PatchEnv, _: i32) -> f32 {
    1.0
}

unsafe extern "C" fn get_param_default(_: *const PatchEnv, idx: i32) -> f32 {
    match idx {
        PARAM_GAIN => GAIN_DEFAULT,
        PARAM_ROOM => 0.4,
        _ => 0.75,
    }
}

unsafe extern "C" fn is_param_enabled(_: *const PatchEnv, idx: i32, source_id: i32) -> u8 {
    // Knobs only (source 0), like the SDK wrapper.
    (source_id == 0 && (0..PARAM_COUNT).contains(&idx)) as u8
}

unsafe fn write_c_str(s: &str, out: *mut u8, len: usize) {
    if out.is_null() || len == 0 {
        return;
    }
    let n = s.len().min(len - 1);
    // SAFETY: the firmware provides `len` writable bytes at `out`.
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), out, n);
        *out.add(n) = 0;
    }
}

unsafe extern "C" fn get_param_name(_: *const PatchEnv, idx: i32, out: *mut u8, len: usize) {
    let name = match idx {
        PARAM_GAIN => "Gain",
        PARAM_ROOM => "Room",
        PARAM_LEVEL => "Level",
        _ => "",
    };
    unsafe { write_c_str(name, out, len) }
}

unsafe extern "C" fn get_param_unit(_: *const PatchEnv, _: i32, out: *mut u8, len: usize) {
    unsafe { write_c_str("", out, len) }
}

unsafe extern "C" fn set_param(_: *const PatchEnv, idx: i32, value: f32) {
    let s = state();
    let v = value.clamp(0.0, 1.0);
    match idx {
        PARAM_GAIN => s.gain = v,
        PARAM_ROOM => s.room = v,
        PARAM_LEVEL => s.level = v,
        _ => return,
    }
    if s.ready {
        s.retarget();
    }
}

unsafe extern "C" fn special_action(_: *const PatchEnv, idx: i32) {
    let s = state();
    match idx {
        ACTION_FOOTSWITCH_PRESS => s.fuzz_on = !s.fuzz_on,
        ACTION_FOOTSWITCH_HOLD => s.echo_on = !s.echo_on,
        _ => {}
    }
}

unsafe extern "C" fn get_state_idx(_: *const PatchEnv) -> i32 {
    state().led() as i32
}

// ---------------------------------------------------------------- header

#[cfg(target_os = "none")]
unsafe extern "C" {
    static __patch_bss_start: u8;
    static patch_image_size: u8;
    static patch_bss_size: u8;
}

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".patch_header")]
#[used]
pub static patch_header: PatchHeader = PatchHeader {
    magic: PATCH_MAGIC,
    abi_version: PATCH_ABI_VERSION,
    flags: PATCH_FLAG_NONE,
    init: patch_init,
    agent_update_buffers: update_buffers,
    agent_set_buffer: set_buffer,
    agent_get_buffer_size: get_buffer_size,
    agent_get_param_min: get_param_min,
    agent_get_param_max: get_param_max,
    agent_get_param_default: get_param_default,
    agent_is_param_enabled: is_param_enabled,
    agent_get_param_name: get_param_name,
    agent_get_param_unit: get_param_unit,
    agent_set_param: set_param,
    agent_special_action: special_action,
    agent_get_state_idx: get_state_idx,
    image_size: &raw const patch_image_size,
    bss_begin: &raw const __patch_bss_start,
    bss_size: &raw const patch_bss_size,
    reserved: [0; 16],
};

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(target_os = "none"))]
#[allow(dead_code)] // each demo uses only some of the playing techniques
mod guitar;
#[cfg(not(target_os = "none"))]
mod host;

#[cfg(not(target_os = "none"))]
fn main() {
    host::main();
}
