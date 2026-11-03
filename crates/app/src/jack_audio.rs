//! Linux audio through the JACK API. On a normal desktop this is PipeWire's JACK layer
//! (`pipewire-jack`), so no JACK server is needed. One duplex client runs the chain directly in
//! the process callback: input and output share one clock, so there is no ring buffer and no
//! drift between them.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use jack::{
    AudioIn, AudioOut, Client, ClientOptions, Control, LatencyType, PortFlags, ProcessScope,
};
use riff_dsp::{Chain, Runner, live};

use crate::audio::{Audio, Device, REALTIME_NO, REALTIME_YES, guess_channel, guitar_input};
use crate::settings::Settings;

const CLIENT: &str = "riff";
const AUDIO: &str = "32 bit float mono audio";
const SCRATCH: usize = 4096;

/// Device ports grouped by device, in channel order. `capture` lists the ports we read from.
fn devices(client: &Client, capture: bool) -> Vec<(String, Vec<String>)> {
    let (flags, prefix) = if capture {
        (PortFlags::IS_OUTPUT, "capture_")
    } else {
        (PortFlags::IS_INPUT, "playback_")
    };
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for port in client.ports(None, Some(AUDIO), flags) {
        let Some((device, short)) = port.split_once(':') else {
            continue;
        };
        if !short.starts_with(prefix) {
            continue;
        }
        match out.iter_mut().find(|(d, _)| d == device) {
            Some((_, ports)) => ports.push(port),
            None => out.push((device.to_string(), vec![port])),
        }
    }
    out
}

fn open(name: &str) -> Result<Client, String> {
    Client::new(name, ClientOptions::NO_START_SERVER)
        .map(|(c, _)| c)
        .map_err(|_| {
            "Cannot reach PipeWire through JACK. Install pipewire-jack, or select the ALSA backend."
                .to_string()
        })
}

pub fn list() -> (Vec<Device>, Vec<String>) {
    let Ok(client) = open("riff-scan") else {
        return (Vec::new(), Vec::new());
    };
    let inputs = devices(&client, true)
        .into_iter()
        .map(|(name, ports)| Device {
            name,
            channels: ports.len(),
        })
        .collect();
    let outputs = devices(&client, false)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    (inputs, outputs)
}

/// PipeWire takes its realtime limits from the desktop portal. If the portal reports no budget
/// (it does when rtkit was installed after login), the kernel kills the process as soon as the
/// realtime audio thread runs. Fail with a message before that can happen.
fn realtime_budget_ok() -> bool {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit only writes to the struct we pass.
    let ok = unsafe { libc::getrlimit(libc::RLIMIT_RTTIME, &mut limit) } == 0;
    !ok || limit.rlim_cur != 0
}

struct Process {
    runner: Runner,
    input: jack::Port<AudioIn>,
    left: jack::Port<AudioOut>,
    right: jack::Port<AudioOut>,
    output: Vec<[f32; 2]>,
    realtime: Arc<AtomicU8>,
}

impl jack::ProcessHandler for Process {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
        if self.realtime.load(Ordering::Relaxed) == 0 {
            // SAFETY: sched_getscheduler(0) only queries the calling thread.
            let policy = unsafe { libc::sched_getscheduler(0) };
            let rt = policy == libc::SCHED_FIFO || policy == libc::SCHED_RR;
            let flag = if rt { REALTIME_YES } else { REALTIME_NO };
            self.realtime.store(flag, Ordering::Relaxed);
        }
        let input = self.input.as_slice(ps);
        let left = self.left.as_mut_slice(ps);
        let right = self.right.as_mut_slice(ps);
        let mut at = 0;
        while at < input.len() {
            let n = (input.len() - at).min(SCRATCH);
            let out = &mut self.output[..n];
            self.runner.process(&input[at..at + n], out);
            for (i, frame) in out.iter().enumerate() {
                left[at + i] = frame[0];
                right[at + i] = frame[1];
            }
            at += n;
        }
        Control::Continue
    }
}

struct Notify {
    dropouts: Arc<AtomicU32>,
}

impl jack::NotificationHandler for Notify {
    fn xrun(&mut self, _: &Client) -> Control {
        self.dropouts.fetch_add(1, Ordering::Relaxed);
        Control::Continue
    }
}

fn latency(client: &Client, port: &str, mode: LatencyType) -> u32 {
    client
        .port_by_name(port)
        .map(|p| p.get_latency_range(mode).1)
        .unwrap_or(0)
}

pub fn start(settings: &Settings, build: impl FnOnce(f32) -> Chain) -> Result<Audio, String> {
    let client = open(CLIENT)?;
    if !realtime_budget_ok() {
        return Err(
            "Realtime audio is not set up yet: the desktop portal grants no realtime time. \
                    Log out and back in (or run: systemctl --user restart xdg-desktop-portal)."
                .into(),
        );
    }
    let captures = devices(&client, true);
    let playbacks = devices(&client, false);
    let pick = |list: &[(String, Vec<String>)], wanted: Option<&str>, capture: bool| {
        wanted
            .and_then(|w| list.iter().find(|(d, _)| d == w))
            .or_else(|| {
                capture
                    .then(|| list.iter().find(|(d, _)| guitar_input(d)))
                    .flatten()
            })
            .or_else(|| list.first())
            .cloned()
    };
    let (in_name, in_ports) =
        pick(&captures, settings.input.as_deref(), true).ok_or("no input device")?;
    let (out_name, out_ports) =
        pick(&playbacks, settings.output.as_deref(), false).ok_or("no output device")?;
    let channel = if settings.input.as_deref() == Some(in_name.as_str()) {
        settings.channel
    } else {
        guess_channel(&in_name, in_ports.len())
    }
    .min(in_ports.len() - 1);

    // Ask PipeWire for our buffer size. This sets a quantum for this node only; PipeWire picks
    // the smallest one asked for and goes back to normal when riff exits.
    let _ = client.set_buffer_size(settings.buffer);
    let rate = client.sample_rate();

    let input = client
        .register_port("in", AudioIn::default())
        .map_err(|e| e.to_string())?;
    let left = client
        .register_port("out_L", AudioOut::default())
        .map_err(|e| e.to_string())?;
    let right = client
        .register_port("out_R", AudioOut::default())
        .map_err(|e| e.to_string())?;
    let names = [&input.name(), &left.name(), &right.name()].map(|n| n.clone().unwrap_or_default());

    let (runner, remote) = live(build(rate as f32));
    let dropouts = Arc::new(AtomicU32::new(0));
    let realtime = Arc::new(AtomicU8::new(0));
    let process = Process {
        runner,
        input,
        left,
        right,
        output: vec![[0.0; 2]; SCRATCH],
        realtime: realtime.clone(),
    };
    let notify = Notify {
        dropouts: dropouts.clone(),
    };
    let active = client
        .activate_async(notify, process)
        .map_err(|e| e.to_string())?;
    let c = active.as_client();
    let connect = |from: &str, to: &str| {
        c.connect_ports_by_name(from, to)
            .map_err(|e| format!("cannot connect {from} to {to}: {e}"))
    };
    connect(&in_ports[channel], &names[0])?;
    let out_l = &out_ports[0];
    let out_r = out_ports.get(1).unwrap_or(out_l);
    connect(&names[1], out_l)?;
    connect(&names[2], out_r)?;

    let buffer = c.buffer_size();
    let frames = buffer
        + latency(c, &in_ports[channel], LatencyType::Capture)
        + latency(c, out_l, LatencyType::Playback);
    Ok(Audio {
        remote,
        rate,
        buffer,
        input: in_name,
        output: out_name,
        channel,
        latency_ms: frames as f32 / rate as f32 * 1000.0,
        dropouts,
        realtime,
        _streams: Box::new(active),
    })
}
