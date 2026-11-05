use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use riff_dsp::{Chain, Remote, Runner, live};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::settings::{Backend, Settings};

const SCRATCH: usize = 4096;

/// Whether the audio thread runs with realtime priority: not known yet, yes, or no.
pub const REALTIME_YES: u8 = 1;
pub const REALTIME_NO: u8 = 2;

pub struct Audio {
    pub remote: Remote,
    pub rate: u32,
    pub buffer: u32,
    pub input: String,
    pub output: String,
    pub channel: usize,
    pub latency_ms: f32,
    pub dropouts: Arc<AtomicU32>,
    pub realtime: Arc<AtomicU8>,
    pub cpu: Arc<AtomicU32>,
    pub fast: Vec<usize>,
    pub(crate) _streams: Box<dyn std::any::Any>,
}

impl Audio {
    pub fn realtime(&self) -> Option<bool> {
        match self.realtime.load(Ordering::Relaxed) {
            REALTIME_YES => Some(true),
            REALTIME_NO => Some(false),
            _ => None,
        }
    }

    pub fn core(&self) -> Option<&'static str> {
        let cpu = self.cpu.load(Ordering::Relaxed);
        if self.fast.is_empty() || cpu == u32::MAX {
            return None;
        }
        Some(if self.fast.contains(&(cpu as usize)) {
            "P-core"
        } else {
            "E-core"
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub name: String,
    pub channels: usize,
}

/// Input devices with their channel counts, and output devices.
pub fn devices(backend: Backend) -> (Vec<Device>, Vec<String>) {
    #[cfg(target_os = "linux")]
    if backend == Backend::Jack {
        return crate::jack_audio::list();
    }
    let _ = backend;
    let host = cpal::default_host();
    let inputs = unique(
        host.input_devices()
            .map(|d| d.collect::<Vec<_>>())
            .unwrap_or_default()
            .iter()
            .filter(|d| listed(d))
            .map(|d| Device {
                name: name(d),
                channels: max_channels(d, true),
            })
            .collect(),
        |d| &d.name,
    );
    let outputs = unique(
        host.output_devices()
            .map(|d| d.collect::<Vec<_>>())
            .unwrap_or_default()
            .iter()
            .filter(|d| listed(d))
            .map(name)
            .collect(),
        |n| n,
    );
    (inputs, outputs)
}

fn unique<T>(mut items: Vec<T>, key: impl Fn(&T) -> &String) -> Vec<T> {
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert(key(i).clone()));
    items
}

fn alsa_id(device: &cpal::Device) -> Option<String> {
    let id = device.id().ok()?.to_string();
    id.strip_prefix("alsa:").map(str::to_string)
}

/// ALSA lists every card many times (sysdefault, front, surround, hdmi, hw, plughw). Keep the
/// PipeWire and default routes, plus one direct `hw:` entry per device.
fn listed(device: &cpal::Device) -> bool {
    match alsa_id(device) {
        Some(id) => id == "pipewire" || id == "default" || id.starts_with("hw:"),
        None => true,
    }
}

fn name(device: &cpal::Device) -> String {
    let base = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "unknown".into());
    match alsa_id(device) {
        Some(id) if id.starts_with("hw:") => format!("{base} ({id})"),
        _ => base,
    }
}

fn max_channels(device: &cpal::Device, input: bool) -> usize {
    let configs: Vec<cpal::SupportedStreamConfigRange> = if input {
        device
            .supported_input_configs()
            .map(|c| c.collect())
            .unwrap_or_default()
    } else {
        device
            .supported_output_configs()
            .map(|c| c.collect())
            .unwrap_or_default()
    };
    configs
        .iter()
        .map(|c| channel_rank(device, input, c.channels()))
        .max()
        .map(|(_, c)| c as usize)
        .unwrap_or(1)
}

/// Software devices (PipeWire's ALSA plugin, `default`) offer up to 64 channels that do not
/// exist. Treat them as stereo. Outputs also prefer stereo; real inputs keep every channel so
/// the guitar jack can be chosen.
fn channel_rank(device: &cpal::Device, input: bool, channels: u16) -> (bool, u16) {
    let virtual_device = alsa_id(device).is_some_and(|id| !id.starts_with("hw:"));
    if !input || virtual_device {
        (channels == 2, channels.min(2))
    } else {
        (true, channels)
    }
}

fn find(wanted: Option<&str>, input: bool) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if let Some(w) = wanted {
        let devices = if input {
            host.input_devices().ok().map(|d| d.collect::<Vec<_>>())
        } else {
            host.output_devices().ok().map(|d| d.collect::<Vec<_>>())
        };
        if let Some(d) = devices
            .unwrap_or_default()
            .into_iter()
            .find(|d| listed(d) && name(d) == w)
        {
            return Some(d);
        }
    }
    if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    }
}

pub fn guess_channel(device: &str, channels: usize) -> usize {
    let lower = device.to_lowercase();
    let solo = lower.contains("scarlett") && lower.contains("solo");
    if solo && !lower.contains("4th gen") && channels >= 2 {
        1
    } else {
        0
    }
}

pub fn guitar_input(device: &str) -> bool {
    device.to_lowercase().contains("inst")
}

fn pick(
    device: &cpal::Device,
    input: bool,
    rate: u32,
) -> Result<(cpal::SampleFormat, usize), String> {
    let configs: Vec<cpal::SupportedStreamConfigRange> = if input {
        device
            .supported_input_configs()
            .map_err(|e| e.to_string())?
            .collect()
    } else {
        device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .collect()
    };
    let fits = |c: &&cpal::SupportedStreamConfigRange| {
        c.min_sample_rate() <= rate && c.max_sample_rate() >= rate
    };
    let best = configs
        .iter()
        .filter(fits)
        .max_by_key(|c| {
            (
                channel_rank(device, input, c.channels()),
                c.sample_format() == cpal::SampleFormat::F32,
            )
        })
        .ok_or(format!("{} does not support {rate} Hz", name(device)))?;
    Ok((best.sample_format(), best.channels() as usize))
}

pub fn start(settings: &Settings, build: impl FnOnce(f32) -> Chain) -> Result<Audio, String> {
    #[cfg(target_os = "linux")]
    if settings.backend == Backend::Jack {
        return crate::jack_audio::start(settings, build);
    }
    let rate = settings.rate;
    let in_dev = find(settings.input.as_deref(), true).ok_or("no input device")?;
    let out_dev = find(settings.output.as_deref(), false).ok_or("no output device")?;
    let (in_fmt, in_channels) = pick(&in_dev, true, rate)?;
    let (out_fmt, out_channels) = pick(&out_dev, false, rate)?;
    let channel = settings.channel.min(in_channels.saturating_sub(1));
    let buffer = cpal::BufferSize::Fixed(settings.buffer);
    let in_config = cpal::StreamConfig {
        channels: in_channels as u16,
        sample_rate: rate,
        buffer_size: buffer,
    };
    let out_config = cpal::StreamConfig {
        channels: out_channels as u16,
        sample_rate: rate,
        buffer_size: buffer,
    };
    let (producer, consumer) = RingBuffer::new(rate as usize);
    let (runner, remote) = live(build(rate as f32));
    let dropouts = Arc::new(AtomicU32::new(0));
    let input = match in_fmt {
        cpal::SampleFormat::F32 => build_input::<f32>(&in_dev, &in_config, producer, channel),
        cpal::SampleFormat::I16 => build_input::<i16>(&in_dev, &in_config, producer, channel),
        cpal::SampleFormat::I32 => build_input::<i32>(&in_dev, &in_config, producer, channel),
        other => return Err(format!("unsupported input format {other:?}")),
    }?;
    let feed = Feed {
        runner,
        ring: consumer,
        buffer: settings.buffer as usize,
        primed: false,
        dropouts: dropouts.clone(),
        input: vec![0.0; SCRATCH],
        output: vec![[0.0; 2]; SCRATCH],
    };
    let output = match out_fmt {
        cpal::SampleFormat::F32 => build_output::<f32>(&out_dev, &out_config, feed, out_channels),
        cpal::SampleFormat::I16 => build_output::<i16>(&out_dev, &out_config, feed, out_channels),
        cpal::SampleFormat::I32 => build_output::<i32>(&out_dev, &out_config, feed, out_channels),
        other => return Err(format!("unsupported output format {other:?}")),
    }?;
    input.play().map_err(|e| e.to_string())?;
    output.play().map_err(|e| e.to_string())?;
    let latency_ms = 3.0 * settings.buffer as f32 / rate as f32 * 1000.0;
    Ok(Audio {
        remote,
        rate,
        buffer: settings.buffer,
        input: name(&in_dev),
        output: name(&out_dev),
        channel,
        latency_ms,
        dropouts,
        realtime: Arc::new(AtomicU8::new(0)),
        cpu: Arc::new(AtomicU32::new(u32::MAX)),
        fast: Vec::new(),
        _streams: Box::new((input, output)),
    })
}

fn build_input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut producer: Producer<f32>,
    channel: usize,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                for frame in data.chunks(channels) {
                    let v: f32 = frame[channel.min(frame.len() - 1)].to_sample();
                    let _ = producer.push(v);
                }
            },
            |e| eprintln!("input error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

struct Feed {
    runner: Runner,
    ring: Consumer<f32>,
    buffer: usize,
    primed: bool,
    dropouts: Arc<AtomicU32>,
    input: Vec<f32>,
    output: Vec<[f32; 2]>,
}

impl Feed {
    fn fill(&mut self, frames: usize) {
        let have = self.ring.slots();
        if !self.primed {
            if have < self.buffer {
                self.input[..frames].fill(0.0);
                return;
            }
            self.primed = true;
        }
        let excess = have.saturating_sub(frames + self.buffer * 3);
        for _ in 0..excess {
            let _ = self.ring.pop();
        }
        let mut short = false;
        for slot in &mut self.input[..frames] {
            *slot = match self.ring.pop() {
                Ok(v) => v,
                Err(_) => {
                    short = true;
                    0.0
                }
            };
        }
        if short {
            self.dropouts.fetch_add(1, Ordering::Relaxed);
            self.primed = false;
        }
    }
}

fn build_output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut feed: Feed,
    channels: usize,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = channels.max(1);
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| {
                for chunk in data.chunks_mut(SCRATCH * channels) {
                    let frames = chunk.len() / channels;
                    feed.fill(frames);
                    let (input, output) = (&feed.input[..frames], &mut feed.output[..frames]);
                    feed.runner.process(input, output);
                    for (dst, src) in chunk.chunks_mut(channels).zip(output.iter()) {
                        match dst.len() {
                            1 => dst[0] = T::from_sample((src[0] + src[1]) * 0.5),
                            _ => {
                                dst[0] = T::from_sample(src[0]);
                                dst[1] = T::from_sample(src[1]);
                                for d in &mut dst[2..] {
                                    *d = T::from_sample(0.0);
                                }
                            }
                        }
                    }
                }
            },
            |e| eprintln!("output error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_guessing_the_guitar_input {
        use super::*;

        mod with_a_3rd_gen_scarlett_solo {
            use super::*;

            #[test]
            fn uses_input_2() {
                assert_eq!(guess_channel("Scarlett Solo USB", 2), 1);
                assert_eq!(
                    guess_channel("Scarlett Solo USB, USB Audio (hw:CARD=USB,DEV=0)", 2),
                    1
                );
            }
        }

        mod with_a_4th_gen_scarlett_solo {
            use super::*;

            #[test]
            fn uses_input_1() {
                assert_eq!(guess_channel("Scarlett Solo 4th Gen Pro", 4), 0);
                assert_eq!(
                    guess_channel("Scarlett Solo 4th Gen, USB Audio (hw:CARD=Gen,DEV=0)", 4),
                    0
                );
                assert_eq!(
                    guess_channel("Scarlett Solo 4th Gen Input 1 Inst/Line", 1),
                    0
                );
            }
        }

        mod with_another_device {
            use super::*;

            #[test]
            fn uses_input_1() {
                assert_eq!(guess_channel("Scarlett 2i2 USB", 2), 0);
                assert_eq!(guess_channel("MacBook Pro Microphone", 1), 0);
                assert_eq!(guess_channel("Built-in Audio Analog Stereo", 2), 0);
            }
        }
    }

    mod when_choosing_the_default_input {
        use super::*;

        #[test]
        fn prefers_the_instrument_source() {
            assert!(guitar_input("Scarlett Solo 4th Gen Input 1 Inst/Line"));
            assert!(!guitar_input("Scarlett Solo 4th Gen Input 2 Mic"));
            assert!(!guitar_input("Built-in Audio Analog Stereo"));
        }
    }
}
