# NAM performance plan

This plan removes the lags and crackles that occur when the amp slot runs a NAM capture. It tells what changes, why, and how to check each change.

## Problem

With a NAM capture in the amp slot, the sound crackles from time to time. The crackles occur at a buffer of 128 samples. A larger buffer reduces the crackles but adds latency, and the guitar feels slow.

## How riff runs a capture

1. PipeWire calls the riff audio callback one time for each buffer. At 48000 Hz and 128 samples, the callback must finish in 2.67 ms. This time is the budget.
2. The callback runs the chain: pedals, amp, cab and effects.
3. The amp slot calls `nam-rs` with blocks of up to 256 samples.
4. All the captures in `~/Music/riff/nam` are "A2" captures (`SlimmableContainer`). Each file has two models:
   - A small model: WaveNet, 3 channels, 23 layers.
   - A full model: WaveNet, 8 channels, 23 layers.
5. riff always runs the full model.

If one callback uses more than the budget, PipeWire has no audio for that buffer. You hear a crackle.

## Measurements

The test runs 10 seconds of a guitar-like signal in blocks of 128 samples. It measures each block as a percentage of the budget. "Worst" is the slowest block. A crackle occurs when one block goes above 100%, but PipeWire and the other programs also need time in each cycle. Thus, keep the worst block below approximately 50%.

The CPU is an Intel Core i7-1355U. It has 2 fast cores (P-cores, 5.0 GHz, CPUs 0 to 3) and 8 slow cores (E-cores, 3.7 GHz, CPUs 4 to 11).

Capture: Peavey 5150 Full Rig, no boost, SM57. Preset: Symbolic Rhythm with the capture in the amp slot.

| What runs | Core | Mean | p99 | Worst |
| --- | --- | --- | --- | --- |
| Built-in RECTO amp, full preset | P | 5.5% | | |
| Built-in RECTO amp, full preset | E | 7.2% | | |
| NAM small model only | P | 3.5% | 3.9% | 5.7% |
| NAM small model only | E | 7.7% | 20.2% | 25.8% |
| NAM full model only | P | 10.5% | 11.8% | 16.0% |
| NAM full model only | E | 30.4% | 47.1% | 55.4% |
| NAM full preset | P | 11.8% | 13.7% | 21.0% |
| NAM full preset | E | 33.5% | 52.0% | 83.2% |

When other programs used the CPU (a compile ran at the same time), single blocks went above 100% on both core types.

The power settings of the computer were:

- Platform profile: `quiet`.
- CPU governor: `powersave`.
- PipeWire default quantum: 1024. riff asks for 128.

## Causes

The causes are in order of effect.

1. **The audio thread can run on an E-core.** Linux can move the riff audio thread to any core. On an E-core, the full NAM model is 3 times slower. The worst block uses 83% of the budget. This is too much.
2. **The power profile limits the CPU.** The `quiet` profile and the `powersave` governor keep the clock low. The CPU needs time to speed up when you start to play. The first blocks after silence are slow.
3. **riff always runs the full model.** The small model of an A2 capture uses 3 times less CPU. For many tones, the difference in sound is small. riff gives no option to use it.
4. **A chain change cuts the sound.** When you change a preset, add a pedal or load a capture, riff builds a new chain and swaps it in one step. The swap does not fade. Also, the new NAM model starts with empty history. Both make a click. This click is not a CPU problem, but it sounds like a crackle.
5. **The build uses only basic CPU instructions.** riff builds for generic x86-64 (SSE2). `nam-rs` has no hand-written SIMD code. The compiler cannot use AVX2 and FMA, which make the matrix math faster.
6. **riff does not show the load.** The status line shows dropouts, but not how close each callback is to the budget. You cannot see whether a capture or a setting is the cause.

## Changes

Do the changes in this order. Measure after each change. Stop when the worst block stays below 50% on the system that you use.

### 1. Show the DSP load

Why: you must see the problem before you can fix it.

- In `crates/dsp/src/live.rs`, measure the time of each `Runner::process` call with `Instant::now()`. Divide by the budget (frames / rate). Keep the peak in `Meters` as an atomic value, like the input peak. `Instant::now()` reads the clock through the vDSO. It does not allocate or lock.
- In `crates/app/src/jack_audio.rs`, read the current CPU with `libc::sched_getcpu()` in the callback. Keep it in an atomic.
- In the status line, show `DSP 34% (E-core)`. Show the peak of the last second. Use the warning color above 50%.
- Add `crates/dsp/examples/nam.rs`. It loads a `.nam` file and prints mean, p99 and worst for each model of the file and for a full preset. This is the test that made the table above.

### 2. Run the audio thread on the fast cores

Why: this is the largest gain. The full model on a P-core uses 3 times less time than on an E-core.

- In `crates/app/src/jack_audio.rs`, implement `NotificationHandler::thread_init`. JACK calls it one time in the audio thread, before the first callback.
- In `thread_init`, read `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq`. Find the CPUs with the highest value. Call `libc::sched_setaffinity(0, ...)` with these CPUs.
- Do this only when the CPUs have different maximum clocks. On a CPU with equal cores, do nothing.
- Put the selection in a pure function, `fastest(cores: &[(usize, u32)]) -> Vec<usize>`, so that a test can check it.
- Add a SETUP option, FAST CORES, on by default. Save it in the settings.
- The status line from change 1 shows `P-core` when this works.

Risk: other programs can also use the P-cores. The riff thread has realtime priority, so it runs first. If you see more dropouts with this change, set FAST CORES to off.

### 3. Add a QUALITY knob for A2 captures

Why: the small model uses 3 times less CPU. It is a good choice for a slow computer or for many effects.

- In `crates/dsp/src/nam.rs`, add the parameter `QUALITY` with two values: FULL and LITE.
- In `NamAmp::set`, call `Slimmable::select(0)` for LITE and `select(len - 1)` for FULL. Use `Model::as_slimmable_mut`. The call does not allocate.
- After a change, call `Model::reset` on the new model, then fade in over 10 ms. This stops a click.
- Show the knob only when the capture is an A2 capture.
- Do not change the model automatically when the load is high. An automatic change would change the tone while you play.
- Save the value in `nam_values`. Old presets have no value. Use FULL for them.

### 4. Fade between the old and the new chain

Why: this removes the click on a preset change, a pedal change and a capture change.

- In `crates/dsp/src/live.rs`, keep the old chain after `Command::Swap`. For 10 ms, run both chains and mix them: the old chain fades out and the new chain fades in.
- After the fade, send the old chain to the trash ring, as now.
- During the fade, the CPU use is the sum of the two chains. The fade is short, so this is acceptable. Measure it with the test from change 1.
- Before `Command::Swap`, warm the new chain on the UI thread. Process `receptive_field()` samples of silence, plus one block. Then the NAM history is full and the first audio block is not a cold start. Add `Chain::warm()` for this.
- Add a test in `live.rs`: a swap during play does not allocate on the audio thread, and the output has no jump bigger than a set limit.

### 5. Build for the CPU of this computer

Why: AVX2 and FMA can make the NAM math faster. The measured gain on a P-core was small and the results were not stable. Measure again on an idle computer before you keep this change.

- In `scripts/install.sh`, set `RUSTFLAGS="-C target-cpu=native"` for the Linux build. Do not use this for a build that you give to other people, because it can fail on an older CPU.
- In `Cargo.toml`, set `codegen-units = 1` in `[profile.release]`.
- Keep the change only if the full model is 15% faster or more.

A better long-term solution: add SIMD code with runtime CPU detection to `nam-rs` (a fork or an upstream pull request). Then every build is fast. This is more work. Do it only if changes 2 to 4 are not sufficient.

### 6. Set up the computer

Why: the power profile limits the CPU for all programs, also riff.

Add these steps to the README, in "Scarlett Solo on Linux":

1. Set the power profile to performance while you play:

   ```sh
   powerprofilesctl set performance
   ```

2. Run `pw-top` while you play. The ERR column must stay at 0 for riff.
3. If ERR goes up, look at the DSP value in the riff status line.

Do not change the power profile from riff. It is a system setting.

## Files that change

| File | Change |
| --- | --- |
| `crates/dsp/src/live.rs` | Load meter, fade on swap |
| `crates/dsp/src/chain.rs` | `Chain::warm()` |
| `crates/dsp/src/nam.rs` | QUALITY parameter, model select, reset and fade |
| `crates/dsp/src/rig.rs` | Default QUALITY for old presets |
| `crates/dsp/examples/nam.rs` | New benchmark |
| `crates/app/src/jack_audio.rs` | `thread_init`, `fastest()`, CPU read |
| `crates/app/src/settings.rs` | FAST CORES setting |
| `crates/app/src/setup.rs` | FAST CORES option |
| `crates/app/src/app.rs`, `host.rs` | Load in the status line, warm before swap |
| `crates/ui/src/mod.rs` | QUALITY knob on the NAM amp |
| `scripts/install.sh`, `Cargo.toml` | Build flags (only if change 5 is kept) |
| `README.md` | Power profile and load steps |

## How to check the result

1. Run `cargo run --release -p riff-dsp --example nam -- <file.nam>` before and after each change. Write the numbers in the table above.
2. Run `cargo test` and `cargo clippy --workspace --all-targets -- -D warnings`.
3. Open riff with the 5150 capture at a buffer of 128 samples.
4. Play for 10 minutes. Change presets and pedals while you play.
5. The result is good when:
   - `pw-top` shows ERR 0 for riff.
   - The riff status line shows 0 dropouts.
   - DSP stays below 50%.
   - A preset change makes no click.

## Results

Status on 2026-11-05:

| Change | Status |
| --- | --- |
| 1. Show the DSP load | Done. The status line shows `DSP 34% P-core`. A message shows above 60%. |
| 2. Fast cores | Done. FAST CORES in SETUP, on by default. riff pins the audio thread in the first callback. |
| 3. QUALITY knob | Done. A change rebuilds the chain, so change 4 removes the click. |
| 4. Fade on swap | Done. 15 ms equal-power fade. `Chain::warm()` runs before each swap. |
| 5. Native build | Not done. The gain was 12% to 19%, approximately 14% on average. This is less than the 15% limit. |
| 6. Computer setup | Done in the README. |

Measurements after the changes, with `cargo run --release -p riff-dsp --example nam`. Capture: Peavey 5150 Full Rig, no boost, SM57. Block: 128 samples.

| What runs | Core | Mean | p99 | Worst |
| --- | --- | --- | --- | --- |
| Preset, FULL | P | 13.0% | 15.7% | 19.5% |
| Preset, LITE | P | 4.9% | 5.6% | 7.3% |
| Preset, FULL | E | 31.1% | 33.4% | 38.4% |
| Preset, LITE | E | 8.6% | 9.1% | 9.8% |

With FAST CORES on, riff uses the P-core values. The live test with the Scarlett is not done yet.
