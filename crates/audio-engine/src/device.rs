//! Audio devices behind the [`AudioIo`] trait, and the cpal implementation.
//!
//! The data callbacks only move samples between the device buffer and `rtrb` rings and bump
//! atomics — no allocation, no locks, no logging. Not built yet, on purpose: resampling (a
//! device that cannot run at 48 kHz is an error) and channel selection (capture takes the
//! first channel, playout puts the mono mix on every channel).

use crate::mixer::MixerRt;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, SupportedStreamConfig};
use engine_protocol::AudioDevice;
use rtrb::Producer;
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc;
use std::sync::Arc;
use transport::SAMPLE_RATE;

/// Largest callback the playout path handles in one go; bigger ones are rendered in pieces.
const MAX_CALLBACK_FRAMES: usize = 8192;

/// Written by the callbacks, read by the engine's control thread.
#[derive(Default)]
pub struct DeviceCounters {
    /// Largest callback buffer seen, in frames.
    pub callback_frames: AtomicU32,
    /// Capture: samples dropped because the ring was full.
    pub overruns: AtomicU32,
    /// Glitches the driver reported (buffer over- or underrun in the device itself).
    pub driver_xruns: AtomicU32,
    /// Set by the stream's error callback: the device is gone or broken.
    pub failed: AtomicBool,
}

/// An open stream. Dropping it closes the device.
pub struct OpenDevice {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub counters: Arc<DeviceCounters>,
    /// Whatever keeps the stream alive.
    pub keep: Box<dyn Any + Send>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceList {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
}

/// The engine's view of the sound hardware. `None` as id means the system default.
pub trait AudioIo: Send {
    fn devices(&self) -> DeviceList;
    /// Mono 48 kHz samples into `ring`.
    fn open_capture(&mut self, id: Option<&str>, ring: Producer<i16>)
        -> Result<OpenDevice, String>;
    /// Plays what `mixer` renders. The mixer comes back through `back` when the stream is
    /// closed — or right away if opening fails — so the next device continues with the same
    /// speakers.
    fn open_playout(
        &mut self,
        id: Option<&str>,
        mixer: MixerRt,
        back: mpsc::Sender<MixerRt>,
    ) -> Result<OpenDevice, String>;
}

/// Gives its content back when dropped — how the mixer leaves a closed stream's callback.
struct Returning<T> {
    value: Option<T>,
    back: mpsc::Sender<T>,
}

impl<T> Drop for Returning<T> {
    // Runs when the stream is torn down, not in the data callback.
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            let _ = self.back.send(value);
        }
    }
}

#[derive(Default)]
pub struct CpalIo;

impl AudioIo for CpalIo {
    fn devices(&self) -> DeviceList {
        let host = cpal::default_host();
        let default_in = host.default_input_device().and_then(|d| device_id(&d));
        let default_out = host.default_output_device().and_then(|d| device_id(&d));
        fn list(
            devices: Result<impl Iterator<Item = cpal::Device>, cpal::Error>,
            default: &Option<String>,
        ) -> Vec<AudioDevice> {
            match devices {
                Ok(all) => all
                    .filter_map(|d| {
                        let id = device_id(&d)?;
                        Some(AudioDevice {
                            is_default: Some(&id) == default.as_ref(),
                            name: device_name(&d),
                            id,
                        })
                    })
                    .collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "could not list audio devices");
                    Vec::new()
                }
            }
        }
        DeviceList {
            inputs: list(host.input_devices(), &default_in),
            outputs: list(host.output_devices(), &default_out),
        }
    }

    fn open_capture(
        &mut self,
        id: Option<&str>,
        ring: Producer<i16>,
    ) -> Result<OpenDevice, String> {
        let host = cpal::default_host();
        let device = match id {
            Some(id) => find(&host, id)?,
            None => host.default_input_device().ok_or("no microphone found")?,
        };
        let config = pick_48k(device.supported_input_configs().map_err(text)?)
            .ok_or_else(|| no_48k(&device))?;
        let counters = Arc::new(DeviceCounters::default());
        let stream = match config.sample_format() {
            SampleFormat::F32 => capture::<f32>(&device, &config, ring, counters.clone()),
            SampleFormat::I16 => capture::<i16>(&device, &config, ring, counters.clone()),
            SampleFormat::I32 => capture::<i32>(&device, &config, ring, counters.clone()),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(text)?;
        Ok(opened(&device, &config, counters, stream))
    }

    fn open_playout(
        &mut self,
        id: Option<&str>,
        mixer: MixerRt,
        back: mpsc::Sender<MixerRt>,
    ) -> Result<OpenDevice, String> {
        // From here on the holder owns the mixer: whatever happens, dropping it (directly,
        // or inside a callback cpal drops) sends the mixer back.
        let holder = Returning {
            value: Some(mixer),
            back,
        };
        let host = cpal::default_host();
        let device = match id {
            Some(id) => find(&host, id)?,
            None => host
                .default_output_device()
                .ok_or("no output device found")?,
        };
        let config = pick_48k(device.supported_output_configs().map_err(text)?)
            .ok_or_else(|| no_48k(&device))?;
        let counters = Arc::new(DeviceCounters::default());
        let stream = match config.sample_format() {
            SampleFormat::F32 => playout::<f32>(&device, &config, holder, counters.clone()),
            SampleFormat::I16 => playout::<i16>(&device, &config, holder, counters.clone()),
            SampleFormat::I32 => playout::<i32>(&device, &config, holder, counters.clone()),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(text)?;
        Ok(opened(&device, &config, counters, stream))
    }
}

fn capture<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut ring: Producer<i16>,
    counters: Arc<DeviceCounters>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    i16: FromSample<T>,
{
    let channels = usize::from(config.channels());
    let on_error = error_flag(counters.clone());
    let callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let frames = (data.len() / channels) as u32;
        counters.callback_frames.fetch_max(frames, Relaxed);
        let mut dropped = 0;
        for frame in data.chunks_exact(channels) {
            if ring.push(i16::from_sample(frame[0])).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            counters.overruns.fetch_add(dropped, Relaxed);
        }
    };
    device
        .build_input_stream(config.config(), callback, on_error, None)
        .map_err(text)
}

fn playout<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut mixer: Returning<MixerRt>,
    counters: Arc<DeviceCounters>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels());
    let on_error = error_flag(counters.clone());
    let mut mono = vec![0f32; MAX_CALLBACK_FRAMES];
    let callback = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        counters
            .callback_frames
            .fetch_max((data.len() / channels) as u32, Relaxed);
        let Some(mixer) = mixer.value.as_mut() else {
            return;
        };
        for piece in data.chunks_mut(MAX_CALLBACK_FRAMES * channels) {
            let mono = &mut mono[..piece.len() / channels];
            mono.fill(0.0);
            mixer.render(mono);
            for (frame, sample) in piece.chunks_exact_mut(channels).zip(mono.iter()) {
                frame.fill(T::from_sample(*sample));
            }
        }
    };
    device
        .build_output_stream(config.config(), callback, on_error, None)
        .map_err(text)
}

/// cpal reports glitches through this callback too, and those come from the audio thread
/// itself: count them, nothing else. Only an error that ends the stream is logged and
/// flagged, for the control thread to reopen the device.
fn error_flag(counters: Arc<DeviceCounters>) -> impl FnMut(cpal::Error) + Send + 'static {
    move |e| match e.kind() {
        cpal::ErrorKind::Xrun => {
            counters.driver_xruns.fetch_add(1, Relaxed);
        }
        // The stream goes on: rerouted to another endpoint, or no priority boost.
        cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied => {}
        _ => {
            tracing::warn!(error = %e, "audio stream ended with an error");
            counters.failed.store(true, Relaxed);
        }
    }
}

fn opened(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    counters: Arc<DeviceCounters>,
    stream: cpal::Stream,
) -> OpenDevice {
    OpenDevice {
        name: device_name(device),
        sample_rate: config.sample_rate(),
        channels: config.channels(),
        counters,
        keep: Box::new(stream),
    }
}

fn find(host: &cpal::Host, id: &str) -> Result<cpal::Device, String> {
    let parsed = id
        .parse()
        .map_err(|_| format!("'{id}' is not a device id"))?;
    host.device_by_id(&parsed)
        .ok_or_else(|| "the selected device is not connected".to_string())
}

/// The mode with the fewest channels that runs at 48 kHz in a format we convert.
fn pick_48k(
    configs: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
) -> Option<SupportedStreamConfig> {
    configs
        .filter(|c| {
            matches!(
                c.sample_format(),
                SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32
            )
        })
        .filter_map(|c| c.try_with_sample_rate(SAMPLE_RATE))
        .min_by_key(|c| c.channels())
}

fn no_48k(device: &cpal::Device) -> String {
    format!(
        "'{}' has no 48 kHz mode — set it to 48000 Hz in the system sound settings",
        device_name(device)
    )
}

fn device_id(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "unknown device".into())
}

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}
