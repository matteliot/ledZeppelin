# ledZeppelin

A [Polyend Endless](https://polyend.com/endless/) patch for the most Led Zeppelin guitar sound ever: Jimmy Page, 1969–1971. It's written in `no_std` Rust against the patch ABI of Polyend's
[FxPatchSDK](https://github.com/polyend/FxPatchSDK) (ABI 0x000B), so no ARM GCC is needed.

Signal chain (mono in from the left input, stereo out):

- **Les Paul volume pot**: the Gain knob is the guitar's volume knob. Backing it off drops the level into the amp (and a little treble), so the crunch cleans up the way Page rolled his guitar back.
- **Echoplex EP-3 preamp**: always in, like Page's. A slight boost with some sparkle and thickness.
- **Tone Bender MkII**: a germanium fuzz, thick and mid-heavy, that sputters as notes die (the first album's fuzz). Footswitch press.
- **Marshall Super Lead "Plexi"**: non-master-volume, so most of the crunch comes from the EL34 power amp, with supply sag for bloom on loud notes. Bright cap, Marshall tone stack, presence.
- **4x12 Greenback cab**: closed-back thump, upper-mid bark, soft top.
- **Echoplex tape echo**: a ~320 ms repeat with wow, flutter and tape saturation, panned right. Footswitch hold; repeats ring out when it's switched off.
- **Room mics**: "distance makes depth". Early reflections and a short live room, squashed by a compressor, blended in by the Room knob.

| Control | Function |
|---|---|
| Left knob | **Gain**: the guitar's volume knob. Full up for the riffs, back it off to clean up chords and rhythm |
| Mid knob | **Room**: close mic to distant room mics |
| Right knob | **Level**: output, unity at 3 o'clock |
| Footswitch press | **Tone Bender** fuzz on/off |
| Footswitch hold | **Echoplex** echo on/off |

| LED | Echo off | Echo on |
|---|---|---|
| Plexi | light yellow | light blue |
| Tone Bender | red | magenta |

## Build

```sh
rustup target add thumbv7em-none-eabihf --toolchain nightly
rustup component add llvm-tools --toolchain nightly
cargo build --release        # ELF for the pedal (default target is thumbv7em-none-eabihf)
./build-endl.sh              # build + make target/endless/led_zeppelin.endl, check f64 use and size
./build-endl.sh --deploy     # ...and copy to /Volumes/ENDLESS
```

## Audition on your computer

```sh
cargo sim                    # -> target/render/led_zeppelin_demo.wav
```

This renders an original E-minor blues-rock riff at 96 bpm through the same patch code. It goes Plexi crunch → Gain backed off for ringing chords → Tone Bender → a lead line with the Echoplex
echo → echo off with the repeats ringing out, then prints the level of each section and a Gain-knob sweep showing how much the amp cleans up. A render is in
[`samples/led_zeppelin_demo.wav`](samples/led_zeppelin_demo.wav).
