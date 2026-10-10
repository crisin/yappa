//! Spike S1 — the proof of concept: the audio engine as a CLI, no UI, no control plane.
//!
//!   yappa-poc devices
//!   yappa-poc join --identity <name> [--source mic|click|silence|none] [--sink device|null]
//!                  [--seconds N] [--detect]
//!   yappa-poc measure [--seconds 30] [--acoustic] [--label text]
//!
//! Common: --url ws://localhost:7880  --room poc  --bitrate 64000  --no-red  --dtx
//!         --period-ms 500  --threshold 0.1  --noise 0.03 (noise floor under the clicks)
//!
//! `join` is one participant: two of them in a room hear each other. `measure` runs two
//! participants in this process and times clicks from one to the other on one clock:
//!   default      A publishes synthetic clicks, B detects them in the decoded audio —
//!                everything between the two device buffers (encoder, network, jitter
//!                buffer, decoder).
//!   --acoustic   B plays clicks on the speaker, A's microphone picks them up and
//!                publishes them, B detects them in what it receives — one full output
//!                stage, the air, one full capture stage and the network: mouth to ear.
//!
//! Needs `cargo xtask livekit` running. Scratch code: the result is docs/spikes/s1-poc.md.

mod audio;
mod click;
mod session;

use audio::PlayoutRing;
use click::{ClickGen, BLOCK};
use session::{Client, ClientConfig, Log, NetStats, Sink, Source};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Result<T = ()> = std::result::Result<T, String>;

/// Most remote speakers mixed at once (a full channel is 19 others; the spike needs one).
const PLAYOUT_SLOTS: usize = 8;
/// Playout cushion per speaker before the device starts reading: 20 ms.
const PLAYOUT_PREFILL: usize = 2 * BLOCK;
/// Ring size per speaker: 200 ms. A fuller ring means drift or a stall — frames are dropped.
const PLAYOUT_RING: usize = 20 * BLOCK;
/// Capture ring: 100 ms between the device callback and the pump.
const CAPTURE_RING: usize = 10 * BLOCK;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args(std::env::args().skip(1).collect());
    let result = match args.0.first().map(String::as_str) {
        Some("devices") => audio::list_devices(),
        Some("join") => join(&args).await,
        Some("measure") => measure(&args).await,
        _ => Err(
            "usage: yappa-poc <devices | join --identity <name> | measure> — \
                  options at the top of crates/spike-s1/src/main.rs"
                .into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("yappa-poc: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Args(Vec<String>);

impl Args {
    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        let i = self.0.iter().position(|a| a == name)?;
        self.0.get(i + 1).map(String::as_str)
    }

    fn parsed<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T> {
        match self.value(name) {
            Some(v) => v.parse().map_err(|_| format!("{name}: cannot read '{v}'")),
            None => Ok(default),
        }
    }

    fn client(&self, identity: &str) -> Result<ClientConfig> {
        Ok(ClientConfig {
            url: self.value("--url").unwrap_or("ws://localhost:7880").into(),
            room: self.value("--room").unwrap_or("poc").into(),
            identity: identity.into(),
            bitrate: self.parsed("--bitrate", 64_000)?,
            red: !self.flag("--no-red"),
            dtx: self.flag("--dtx"),
        })
    }
}

/// The playout mix: rings for remote speakers plus optional extra inputs.
fn playout(extra: Vec<PlayoutRing>) -> Result<(audio::Device, Sink)> {
    let mut producers = Vec::new();
    let mut rings = extra;
    for _ in 0..PLAYOUT_SLOTS {
        let (producer, ring) = rtrb::RingBuffer::new(PLAYOUT_RING);
        producers.push(producer);
        rings.push(PlayoutRing {
            ring,
            prefill: PLAYOUT_PREFILL,
            continuous: true,
        });
    }
    let device = audio::start_playout(rings)?;
    let sink = Sink {
        detect: None,
        playout: Arc::new(Mutex::new(producers)),
    };
    Ok((device, sink))
}

async fn join(args: &Args) -> Result {
    let identity = args.value("--identity").ok_or("join needs --identity")?;
    let cfg = args.client(identity)?;
    let period_ms = args.parsed("--period-ms", 500)?;
    let seconds: u64 = args.parsed("--seconds", 0)?;

    let mut capture = None;
    let source = match args.value("--source").unwrap_or("mic") {
        "mic" => {
            let (producer, ring) = rtrb::RingBuffer::new(CAPTURE_RING);
            let device = audio::start_capture(producer)?;
            println!("capture: {} — {}", device.name, device.config);
            capture = Some(device);
            Source::Mic(ring)
        }
        "click" => Source::Click {
            period_ms,
            noise: args.parsed("--noise", 0.0)?,
        },
        "silence" => Source::Silence,
        "none" => Source::Nothing,
        other => return Err(format!("--source {other}: mic | click | silence | none")),
    };
    let (output, mut sink) = match args.value("--sink").unwrap_or("device") {
        "device" => {
            let (device, sink) = playout(Vec::new())?;
            println!("playout: {} — {}", device.name, device.config);
            (Some(device), sink)
        }
        "null" => (None, Sink::default()),
        other => return Err(format!("--sink {other}: device | null")),
    };
    if args.flag("--detect") {
        sink.detect = Some(args.parsed("--threshold", 0.1)?);
    }

    let log = Log::new();
    let client = Client::start(cfg.clone(), source, sink, log.clone()).await?;
    println!("in the room — Ctrl-C to leave");
    let ticks = if seconds == 0 { u64::MAX } else { seconds };
    for tick in 1..=ticks {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            _ = tokio::signal::ctrl_c() => break,
        }
        if tick % 5 == 0 {
            if let Ok(s) = NetStats::sample(&client.room).await {
                println!(
                    "[{:8.3} s] {identity}: jitter buffer {} ms, lost {}/{} packets, \
                     concealed {} %, uplink rtt {:.1} ms",
                    log.now(),
                    fmt(s.jitter_buffer_ms(), 1),
                    s.packets_lost,
                    s.packets_received,
                    fmt(s.concealed_percent(), 2),
                    s.uplink_rtt_ms,
                );
            }
        }
    }
    let elapsed = log.now();
    let stats = NetStats::sample(&client.room).await.ok();
    println!("\n## {identity}");
    print_config(&cfg);
    if let Some(s) = &stats {
        print_net("this client", s, elapsed);
    }
    print_pump(&client);
    print_device("capture", capture.as_ref());
    print_device("playout", output.as_ref());
    print_receive(&client);
    client.close().await;
    Ok(())
}

async fn measure(args: &Args) -> Result {
    let seconds: u64 = args.parsed("--seconds", 30)?;
    let period_ms: u32 = args.parsed("--period-ms", 500)?;
    let acoustic = args.flag("--acoustic");
    // The microphone hears a click far below full scale; the digital path does not.
    let threshold = args.parsed("--threshold", if acoustic { 0.02 } else { 0.1 })?;
    let label = args.value("--label").unwrap_or("measure");
    let log = Log::new();
    let stop = Arc::new(AtomicBool::new(false));

    let mut capture = None;
    let mut output = None;
    let mut injector = None;
    let (source, mut sink) = if acoustic {
        // B's speaker plays only the injected clicks — playing what it receives too would
        // close the loop and howl.
        let (mut producer, ring) = rtrb::RingBuffer::new(PLAYOUT_RING);
        let inject = PlayoutRing {
            ring,
            prefill: 1,
            continuous: false,
        };
        output = Some(audio::start_playout(vec![inject])?);
        let (log, stop) = (log.clone(), stop.clone());
        injector = Some(std::thread::spawn(move || {
            let mut gen = ClickGen::new(period_ms);
            let mut block = [0i16; BLOCK];
            while !stop.load(Relaxed) {
                if gen.fill(&mut block) {
                    log.sent.lock().unwrap().push(log.now());
                    for s in block {
                        let _ = producer.push(s);
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }));
        let (producer, ring) = rtrb::RingBuffer::new(CAPTURE_RING);
        capture = Some(audio::start_capture(producer)?);
        (Source::Mic(ring), Sink::default())
    } else {
        let noise = args.parsed("--noise", 0.0)?;
        (Source::Click { period_ms, noise }, Sink::default())
    };
    sink.detect = Some(threshold);

    let cfg_a = args.client("poc-a")?;
    let listener = Client::start(args.client("poc-b")?, Source::Nothing, sink, log.clone()).await?;
    let speaker = Client::start(cfg_a.clone(), source, Sink::default(), log.clone()).await?;

    for tick in 1..=seconds {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if tick % 5 == 0 {
            if let Ok(s) = NetStats::sample(&listener.room).await {
                println!(
                    "[{:8.3} s] poc-b: {} clicks heard, jitter buffer {} ms, lost {}/{} \
                     packets, concealed {} %",
                    log.now(),
                    log.detected.lock().unwrap().len(),
                    fmt(s.jitter_buffer_ms(), 1),
                    s.packets_lost,
                    s.packets_received,
                    fmt(s.concealed_percent(), 2),
                );
            }
        }
    }
    let elapsed = log.now();
    let heard = NetStats::sample(&listener.room).await;
    let spoken = NetStats::sample(&speaker.room).await;
    stop.store(true, Relaxed);

    let mode = if acoustic {
        "acoustic loop: speaker → air → microphone → network (mouth to ear)"
    } else {
        "digital: click source → network → decoded audio (no device buffers)"
    };
    println!("\n## {label}");
    println!("mode             {mode}");
    print_config(&cfg_a);
    println!("run              {seconds} s, one click every {period_ms} ms, threshold {threshold}");
    print_clicks(&log, f64::from(period_ms) / 1000.0, elapsed);
    match &heard {
        Ok(s) => print_net("listener poc-b", s, elapsed),
        Err(e) => println!("listener stats   not available: {e}"),
    }
    match &spoken {
        Ok(s) => print_net("speaker poc-a", s, elapsed),
        Err(e) => println!("speaker stats    not available: {e}"),
    }
    print_pump(&speaker);
    print_device("capture", capture.as_ref());
    print_device("playout", output.as_ref());
    print_events(&log);

    speaker.close().await;
    listener.close().await;
    if let Some(t) = injector {
        let _ = t.join();
    }
    Ok(())
}

/// The reconnect timeline.
fn print_events(log: &Log) {
    let events = log.events.lock().unwrap();
    for (t, line) in events.iter().filter(|(_, l)| {
        l.contains("RECONNECT") || l.contains("DISCONNECTED") || l.contains("connection state")
    }) {
        println!("event            {t:.3} s  {line}");
    }
}

fn print_config(cfg: &ClientConfig) {
    println!(
        "publish options  {} bit/s max, red={}, dtx={}, url {}",
        cfg.bitrate, cfg.red, cfg.dtx, cfg.url
    );
}

fn print_clicks(log: &Log, period: f64, end: f64) {
    let detected = log.detected.lock().unwrap();
    let sent = log.sent.lock().unwrap();
    // Clicks sent before the listener was subscribed cannot arrive, and the last one may
    // still be on its way when the run ends: count from the first that arrived, and stop
    // one window before the end.
    let window = period * 0.9;
    let first = detected.first().copied().unwrap_or(f64::MAX);
    let counted: Vec<f64> = sent
        .iter()
        .copied()
        .filter(|t| *t > first - window && *t < end - window)
        .collect();
    let (latencies, missed) = click::match_clicks(&counted, &detected, window);
    println!(
        "clicks           {} sent, {} arrived, {} missed (of {} sent in total)",
        counted.len(),
        latencies.len(),
        missed,
        sent.len()
    );
    match click::summarize(&latencies) {
        Some(s) => println!(
            "latency ms       min {:.1} · median {:.1} · p95 {:.1} · max {:.1}  (n={})",
            s.min * 1e3,
            s.median * 1e3,
            s.p95 * 1e3,
            s.max * 1e3,
            s.n
        ),
        None => println!("latency ms       not measured — no click arrived"),
    }
    if let Some((start, length)) = click::longest_gap(&detected) {
        println!("longest silence  {length:.2} s between clicks, from {start:.2} s");
    }
}

fn print_net(who: &str, s: &NetStats, elapsed: f64) {
    if s.packets_received > 0 {
        let total = s.packets_received as f64 + s.packets_lost as f64;
        println!(
            "{who:<16} received: codec {}, {} packets, {} lost ({:.2} %), jitter {:.1} ms, \
             jitter buffer {} ms",
            s.codec_received,
            s.packets_received,
            s.packets_lost,
            s.packets_lost as f64 / total * 100.0,
            s.jitter_ms,
            fmt(s.jitter_buffer_ms(), 1),
        );
        println!(
            "{:<16} concealed {} % of samples in {} events, {} NACKs sent, {} \
             retransmissions, {} FEC packets",
            "",
            fmt(s.concealed_percent(), 2),
            s.concealment_events,
            s.nack_count,
            s.retransmitted_received,
            s.fec_packets_received,
        );
    }
    if s.packets_sent > 0 {
        println!(
            "{who:<16} sent: codec {}, {} packets, ≈{:.0} kbit/s payload, ICE rtt {:.1} ms, RTCP rtt {:.1} ms, \
             uplink loss {:.1} %",
            s.codec_sent,
            s.packets_sent,
            s.bytes_sent as f64 * 8.0 / elapsed / 1000.0,
            s.ice_rtt_ms,
            s.uplink_rtt_ms,
            s.uplink_fraction_lost * 100.0,
        );
    }
}

fn print_pump(client: &Client) {
    let blocks = client.pump.blocks.load(Relaxed);
    if blocks > 0 {
        println!(
            "send handoff     {} blocks, capture_frame mean {:.1} µs · max {:.1} µs per 10 ms \
             block, longest block interval {:.1} ms",
            blocks,
            client.pump.handoff_ns_sum.load(Relaxed) as f64 / blocks as f64 / 1e3,
            client.pump.handoff_ns_max.load(Relaxed) as f64 / 1e3,
            client.pump.interval_ns_max.load(Relaxed) as f64 / 1e6,
        );
    }
}

fn print_receive(client: &Client) {
    let frames = client.receive.frames.load(Relaxed);
    if frames > 0 {
        println!(
            "receive          {} frames, {} dropped at the playout ring",
            frames,
            client.receive.dropped_frames.load(Relaxed)
        );
    }
}

fn print_device(what: &str, device: Option<&audio::Device>) {
    if let Some(d) = device {
        println!(
            "{what:<16} {} — {}; largest callback {} frames, {} xruns",
            d.name,
            d.config,
            d.stats.max_callback_frames.load(Relaxed),
            d.stats.xruns.load(Relaxed)
        );
    }
}

fn fmt(value: Option<f64>, decimals: usize) -> String {
    value.map_or_else(|| "–".into(), |v| format!("{v:.decimals$}"))
}
