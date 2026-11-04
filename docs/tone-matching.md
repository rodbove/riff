# Tone matching

This guide tells you how to tune riff against a reference tone, for example a render from AmpliTube. It also lists the tests to do next.

## How the sound goes through riff

riff processes the guitar in this order:

1. INPUT gain.
2. Noise gate.
3. Pedals (for example OD-9).
4. Amp.
5. Cabinet.
6. Effects (for example doubler, delay, reverb).
7. OUTPUT level.

The amp (`crates/dsp/src/amp.rs`) has these parts, in order:

1. TIGHT filter. It removes low bass before the distortion.
2. Preamp stages. Each stage is a tube model with filters before and after it. The stages make the distortion. More stages give more gain and more compression.
3. Tone stack: BASS, MID, TREBLE. Each model has a mid frequency and a mid cut (`scoop_db`). The MID knob adds or removes 10 dB from that cut.
4. PRESENCE and RESONANCE.
5. Power amp with sag. MASTER sets how hard the power amp works.
6. Speaker load: a bass bump at 110 Hz (`thump_db`) and a top shelf (`air_hz`, `air_db`).

The cabinet (`crates/dsp/src/cab.rs`) is a calculated impulse response. It has a speaker curve, small peaks and dips for speaker breakup, two mics, a MIC BLEND, a LOW CUT and a HIGH CUT. You can also load your own IR files.

## What you must understand

### A high-gain amp is fully saturated

With high gain, the GAIN and MASTER knobs change the feel, but they change the frequency balance very little. The filters after the distortion (tone stack, speaker load, cabinet) set the frequency balance. To change how the tone sounds, change these filters first.

The filters before the distortion (TIGHT, pedal TONE, stage filters) change the feel: how tight the palm mutes are and how clean the chords stay.

### Frequency regions

| Region | When there is too much | When there is too little | Main controls |
| --- | --- | --- | --- |
| 60-100 Hz | boomy, flubby palm mutes | thin, no chug | BASS, RESONANCE, `thump_db`, cab LOW CUT |
| 200-300 Hz | muddy | cold | BASS, cab curve |
| 400-1000 Hz | boxy, honky, "cheap pedal" | scooped, guitar is lost in a mix | MID, `scoop_db`, `mid_hz` |
| 2-4 kHz | harsh, ice pick | dull, no bite | TREBLE, PRESENCE, `air_db` |
| 5-9 kHz | fizz, "bees in a can" | muffled | cab HIGH CUT, PRESENCE |
| above 10 kHz | hiss | closed | cab HIGH CUT |

### Louder always sounds better

When you compare two tones, set the same loudness first. If one tone is 2 dB louder, your ears prefer it, even if it is not better.

## How to tune by ear

1. Record a DI take in STUDIO. Play the parts in "Tests to do next".
2. Click EXPORT DI.
3. On the Mac, run the DI through AmpliTube. Export a WAV file with the full length, at 48 kHz, 24-bit or 32-bit float, without clipping.
4. In riff STUDIO, click IMPORT to put the AmpliTube file on a second track.
5. Use REAMP TRACK on the DI track to play it through the current riff tone.
6. Use S (solo) to switch between the two tracks. Set the track volumes so that the two tracks have the same loudness.
7. Change one knob at a time. Then click REAMP TRACK again and compare.
8. When the tone is good, click SAVE to keep it as a user preset.

## How to measure

The measurement tools compare riff renders with the reference render of the same DI.

Set up the tools one time:

1. Run `python3 -m venv tools/tone/.venv`.
2. Run `tools/tone/.venv/bin/pip install numpy scipy`.

Use the tools:

1. To list the factory presets, run `cargo run --release -p riff-dsp --example render -- list`.
2. To write a preset to a file that you can change, run `cargo run --release -p riff-dsp --example render -- dump "Symbolic Rhythm" > my.json`.
3. Change the values in `my.json`. The order of the amp values is GAIN, BASS, MID, TREBLE, PRESENCE, RESONANCE, MASTER, BRIGHT. The order of the cab values is MIC BLEND, LOW CUT, HIGH CUT, LEVEL.
4. To compare, run:

   ```sh
   tools/tone/.venv/bin/python tools/tone/compare.py \
     --di "take DI.wav" --ref "amplitube.wav" \
     --preset "Symbolic Rhythm" --rig my.json --keep /tmp/renders
   ```

5. Listen to the renders in the `--keep` folder.

The tool aligns the two files and prints these values for each render:

- **distance**: the mean difference in dB from the reference, from 80 Hz to 8 kHz. A smaller value is closer. Below 2 dB is very close.
- **vs ref**: the difference in each third-octave band. A positive value means riff has more in that band. Use the "Frequency regions" table to find the control to change.
- **noise**: the spectral flatness in three regions. A high value means noise-like sound, for example fizz or mush. A low value means clear notes. Compare it with the reference value.
- **dynamics**: how much the level moves while you play. A small value means more compression.

To change an amp model itself (for all presets), change the values in the model (`const RECTO: Voicing`, and so on) in `crates/dsp/src/amp.rs`. Then run the compare tool again. Use `--rig` files that use that model.

## Tests to do next

Do these tests in this order. For each test, use the same DI file in riff and in AmpliTube.

1. **Cab only.** Make a sine sweep file (this file does not exist yet). Play it through AmpliTube with only the cabinet on (MH-412SL, Condenser 87 and Dynamic 57). This measures the AmpliTube cabinet, so riff's V30 can be tuned to match it. Use the result only as a tuning reference. Do not put it in riff.
2. **One technique per part.** Record a take of about 10 seconds for each part, with a pause between the parts:
   - one open low E, until the sound stops (decay, fizz, gate)
   - palm-muted chugs on low E (tightness, attack)
   - open low chords, for example "As I Am" (mud)
   - single notes high on the neck (lead fizz)
   - the same riff with soft picking, then hard picking (dynamics)
   - the guitar volume knob at about 5 (clean-up)
3. **Amp without cab.** Run the DI through the same AmpliTube rig with the cabinet off. This separates amp problems from cabinet problems.
4. **Pedal off.** Run the same rig without the overdrive. This shows how the riff OD-9 is different from the AmpliTube overdrive.
5. **A second reference tone**, for example a Rectifier or 5150 type of amp. More references stop riff from fitting one amp only.

## Results so far

The first comparison used "Symbolic Rhythm" and an AmpliTube rig: MH-500 Metalhead, MH-412SL cabinet with a Condenser 87 and a Dynamic 57, and a yellow overdrive (DRIVE 0.98, TONE 7.43, LEVEL 10). The amp settings were DRIVE 5.2, VOLUME 6.6, PRESENCE 8.5, TREBLE 3.3, MID 5.1 and BASS 7.1.

- Before the change, all riff amps had 7 to 13 dB too much at 400-1000 Hz. This was the "boxy" sound.
- The new amp values cut more mids, add bass at 110 Hz and add a top shelf. RECTO with the AmpliTube knob settings is now 1.9 dB from the reference.
- riff has 3 to 5 dB more at 6-8 kHz than the reference. A cab HIGH CUT of about 6000 Hz is closest (1.7 dB) and sounds less fizzy.
