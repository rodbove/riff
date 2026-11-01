# riff

riff is a guitar amp and effects app for metal. It plays your guitar from an audio interface through pedals, an amp and a speaker cabinet. It runs as a standalone app and as a VST3 or CLAP plugin.

## Features

- Six amp models. The names are generic. Each model is like a famous amp:

  | Model | Like |
  | --- | --- |
  | CLEAN TWIN | Fender Twin clean |
  | BRIT 800 | Marshall JCM800 |
  | MARK IIC | Mesa Mark IIC+ lead channel |
  | RECTO | Mesa Dual Rectifier |
  | BLOCK 51 | Peavey 5150 |
  | UBER | Bogner Uberschall or Engl |

- NAM captures. Load a `.nam` file (Neural Amp Modeler) into the amp slot.
- Four cabinet models with two mics and a mic blend:
  - 4x12 V30
  - 4x12 Greenback
  - 2x12 open back
  - 1x12
- Load your own impulse responses (WAV) into each mic.
- Pedals: noise gate, boost, compressor, OD-9 (tube screamer style), RODENT (RAT style), SWEDISH (HM-2 style), wah, 10-band EQ and octave down.
- Effects: chorus, phaser, flanger, delay (digital, analog, ping-pong), reverb (room, plate, hall), stereo doubler and a post EQ.
- Factory presets in the style of Metallica, Dream Theater, Death, Avenged Sevenfold and Arch Enemy, plus some extras. The presets are a start point, not exact copies.
- Save your own presets, compare two tones with A/B, and use the tuner.
- MIDI: program change selects a preset. CC 11 (expression pedal) moves the wah.

## Build and run

1. Install Rust.
2. Run `./scripts/bundle.sh`.
3. Open `target/riff.app`.
4. Allow microphone access when macOS asks. riff needs this access to read the audio interface.

For development, run `cargo run --release -p riff-app`.

## Set up a Focusrite Scarlett Solo

1. Connect the guitar to the INST input (input 2) on the front of the Scarlett.
2. Push the INST button, so that the input is in instrument mode.
3. Play hard and turn the GAIN knob on the Scarlett until the ring flashes green, not red.
4. In riff, click SETUP:
   - Set INPUT to the Scarlett.
   - Set GUITAR ON to input 2. riff selects input 2 for a Scarlett Solo automatically.
   - Set OUTPUT to the Scarlett, and connect headphones or monitors to the Scarlett.
   - Set RATE to 48000 Hz and BUFFER to 128 samples.
5. Click APPLY. The status line shows the latency and the dropouts.
6. If you hear crackles, set BUFFER to 256 samples.
7. Turn off DIRECT MONITOR on the Scarlett. If you do not, you hear the clean guitar too.

## Use

- Click the preset name to see the presets. Use `<` and `>`, or the left and right arrow keys, to go to the previous or next preset.
- Drag a knob up or down. Hold Shift for small changes. Double-click a knob to reset it. You can also use the mouse wheel on a knob.
- Click the footswitch of a pedal to turn the pedal on or off.
- Drag a pedal left or right to change the order. Right-click a pedal to remove it.
- Click `+ PEDAL` or `+ EFFECT` to add one.
- Click the name of an effect in the rack to turn it on or off. Right-click it to remove it.
- Push T, or click TUNER, to tune. The tuner mutes the output.
- Click SAVE to save the tone as a user preset. User presets go to `~/Music/riff/presets`.
- Click A/B one time to store the current tone as A. Change the tone, then click A/B to switch between the two tones.

## NAM captures and impulse responses

riff does not include captures or IRs. You can download free ones:

- NAM captures: [ToneHunt](https://tonehunt.org). Search for 5150, Rectifier, Mark IIC+ or JCM800. Put the files in `~/Music/riff/nam`.
- Cabinet IRs: put WAV files in `~/Music/riff/ir`.

To use a capture:

1. Click NAM on the amp. Select the `.nam` file.
2. Use INPUT to set the drive into the capture, and OUTPUT to set the level.
3. If the capture includes a cabinet, turn the CAB off.
4. Most captures are made at 48 kHz. Set RATE to 48000 Hz in SETUP. riff shows a warning if the rates are different.

Right-click NAM to select a different file. Click an amp model to go back to the built-in amps.

## Plugin

1. Run `cargo xtask bundle riff-plugin --release`.
2. Copy the bundles:
   - `target/bundled/riff.vst3` to `~/Library/Audio/Plug-Ins/VST3/`
   - `target/bundled/riff.clap` to `~/Library/Audio/Plug-Ins/CLAP/`
3. Open your DAW and rescan the plugins.

The plugin has the same screen as the app. The host can automate these parameters: Input, Output, Gate, Amp Gain, Wah, Delay Mix and Reverb Mix.

The plugin has no AU format. Logic Pro and GarageBand cannot load it. Use Reaper, Ableton Live, Bitwig, Studio One or another VST3 or CLAP host.

## Tests

Run `cargo test` and `cargo clippy --workspace --all-targets -- -D warnings`.

To measure the CPU use of each factory preset, run `cargo run --release -p riff-dsp --example bench`.

## License notes

The plugin uses nih-plug. The VST3 export of nih-plug is under the GPLv3.
