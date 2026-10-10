//! cpal capture and playout at 48 kHz. The callbacks only move samples between the device
//! buffer and `rtrb` rings and bump atomics — no allocation, no locks, no logging.
//!
//! Spike shortcuts, on purpose: no resampler (a device that cannot do 48 kHz is an error),
//! first channel only on capture, mono duplicated to every output channel.

use crate::click::SAMPLE_RATE;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, StreamConfig, SupportedStreamConfig};
use rtrb::{Consumer, Producer};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

type Result<T> = std::result::Result<T, String>;

/// Counters the callbacks write and the report reads.
#[derive(Default)]
pub struct DeviceStats {
    /// Largest callback buffer seen, in frames — the effective device buffer size.
    pub max_callback_frames: AtomicU32,
    /// Capture: samples dropped because the ring was full. Playout: how often a speaker's
    /// ring ran dry (each time costs one prefill of silence).
    pub xruns: AtomicU64,
}

/// Keeps the stream alive; dropping it stops the device.
pub struct Device {
    _stream: cpal::Stream,
    pub name: String,
    pub config: String,
    pub stats: Arc<DeviceStats>,
}

pub fn list_devices() -> Result<()> {
    let host = cpal::default_host();
    println!("host: {}", host.id().name());
    let describe = |d: &cpal::Device, cfg: Result<SupportedStreamConfig>| {
        let cfg = match cfg {
            Ok(c) => describe_config(&c),
            Err(e) => format!("no default config ({e})"),
        };
        println!("  {} — {cfg}", device_name(d));
    };
    println!("inputs:");
    for d in host.input_devices().map_err(err)? {
        describe(&d, d.default_input_config().map_err(err));
    }
    println!("outputs:");
    for d in host.output_devices().map_err(err)? {
        describe(&d, d.default_output_config().map_err(err));
    }
    Ok(())
}

/// Default input device → mono i16 at 48 kHz into `ring`.
pub fn start_capture(ring: Producer<i16>) -> Result<Device> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("no input device")?;
    let supported = device.supported_input_configs().map_err(err)?;
    let config = pick_48k(supported, "input")?;
    let stats = Arc::new(DeviceStats::default());
    let stream = match config.sample_format() {
        SampleFormat::F32 => capture_stream::<f32>(&device, &config, ring, stats.clone()),
        SampleFormat::I16 => capture_stream::<i16>(&device, &config, ring, stats.clone()),
        SampleFormat::I32 => capture_stream::<i32>(&device, &config, ring, stats.clone()),
        other => Err(format!("unsupported input sample format {other}")),
    }?;
    stream.play().map_err(err)?;
    Ok(Device {
        _stream: stream,
        name: device_name(&device),
        config: describe_config(&config),
        stats,
    })
}

/// One input of the playout mix.
pub struct PlayoutRing {
    pub ring: Consumer<i16>,
    /// Samples that must be queued before the ring starts (again) — the cushion against
    /// the 10 ms frames from the network and the device callback drifting past each other.
    pub prefill: usize,
    /// Whether running dry counts as an underrun (a speaker) or is normal (injected clicks).
    pub continuous: bool,
}

/// Sums every ring (one per remote speaker, plus anything injected locally) and plays the
/// result on the default output device.
pub fn start_playout(rings: Vec<PlayoutRing>) -> Result<Device> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output device")?;
    let supported = device.supported_output_configs().map_err(err)?;
    let config = pick_48k(supported, "output")?;
    let stats = Arc::new(DeviceStats::default());
    let stream = match config.sample_format() {
        SampleFormat::F32 => playout_stream::<f32>(&device, &config, rings, stats.clone()),
        SampleFormat::I16 => playout_stream::<i16>(&device, &config, rings, stats.clone()),
        SampleFormat::I32 => playout_stream::<i32>(&device, &config, rings, stats.clone()),
        other => Err(format!("unsupported output sample format {other}")),
    }?;
    stream.play().map_err(err)?;
    Ok(Device {
        _stream: stream,
        name: device_name(&device),
        config: describe_config(&config),
        stats,
    })
}

fn capture_stream<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut ring: Producer<i16>,
    stats: Arc<DeviceStats>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    i16: FromSample<T>,
{
    let channels = usize::from(config.channels());
    let callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let frames = (data.len() / channels) as u32;
        stats.max_callback_frames.fetch_max(frames, Relaxed);
        let mut dropped = 0;
        for frame in data.chunks_exact(channels) {
            if ring.push(i16::from_sample(frame[0])).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            stats.xruns.fetch_add(dropped, Relaxed);
        }
    };
    let config: StreamConfig = config.config();
    device
        .build_input_stream(config, callback, stream_error, None)
        .map_err(err)
}

fn playout_stream<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut rings: Vec<PlayoutRing>,
    stats: Arc<DeviceStats>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<i16>,
{
    let channels = usize::from(config.channels());
    let mut playing = vec![false; rings.len()];
    let callback = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        let frames = (data.len() / channels) as u32;
        stats.max_callback_frames.fetch_max(frames, Relaxed);
        let mut starved = 0;
        for frame in data.chunks_exact_mut(channels) {
            let mut sum = 0i32;
            for (input, playing) in rings.iter_mut().zip(playing.iter_mut()) {
                if !*playing && input.ring.slots() >= input.prefill {
                    *playing = true;
                }
                if *playing {
                    match input.ring.pop() {
                        Ok(s) => sum += i32::from(s),
                        Err(_) => {
                            *playing = false;
                            starved += u64::from(input.continuous);
                        }
                    }
                }
            }
            let mixed = sum.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            frame.fill(T::from_sample(mixed));
        }
        if starved > 0 {
            stats.xruns.fetch_add(starved, Relaxed);
        }
    };
    let config: StreamConfig = config.config();
    device
        .build_output_stream(config, callback, stream_error, None)
        .map_err(err)
}

/// The config with the fewest channels that runs at 48 kHz in a format we convert.
fn pick_48k(
    configs: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
    direction: &str,
) -> Result<SupportedStreamConfig> {
    configs
        .filter(|c| {
            matches!(
                c.sample_format(),
                SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32
            )
        })
        .filter_map(|c| c.try_with_sample_rate(SAMPLE_RATE))
        .min_by_key(|c| c.channels())
        .ok_or_else(|| {
            format!("default {direction} device has no 48 kHz mode (the spike has no resampler)")
        })
}

fn describe_config(c: &SupportedStreamConfig) -> String {
    format!(
        "{} Hz, {} ch, {}, buffer {:?}",
        c.sample_rate(),
        c.channels(),
        c.sample_format(),
        c.buffer_size()
    )
}

fn device_name(d: &cpal::Device) -> String {
    d.description()
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "unknown device".into())
}

// Runs on a cpal-owned thread, not in the data callback.
fn stream_error(e: cpal::Error) {
    eprintln!("audio stream: {e}");
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
