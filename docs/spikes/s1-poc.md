# S1: Proof of concept — result

2026-10-10 · Windows PC (AMD Ryzen 7 7700X, Windows 11 Home 10.0.26300) · branch
`task/s1-poc`, on top of `6ca4d16`

**Short version:** the Rust SDK path works. Two clients hear each other through the local
LiveKit, the custom source (`NativeAudioSource`) and RED do what the plan assumes, and the
SDK reconnects on its own after 20 s and 45 s without a link. The digital path costs
50–60 ms. Three things are not answered yet: the real mouth-to-ear figure (no microphone
here that hears the speaker), a physically pulled cable, and echo cancellation.

## Question

From the planning baseline ("Hauptrisiko und Fallback"): is the LiveKit Rust SDK good enough
to build the engine on, or do we fall back to the JS SDK in the webview?

1. Do two clients hear each other with cpal → `NativeAudioSource` and no UI?
2. Do `red`, `dtx` and the bitrate option do what they say?
3. What does the path cost in latency, measured with a click?
4. How does it sound at 5 / 10 / 20 % packet loss?
5. Does the session come back after the link was gone?

## Setup

- **Server:** `livekit/livekit-server:v1.13.8` in Docker Desktop (`cargo xtask livekit`),
  `--dev`, one muxed UDP port (7882), node IP 127.0.0.1. Clients on the same machine, so
  the network itself adds about 1 ms (ICE round trip 0–2 ms).
- **Client:** `crates/spike-s1` (`yappa-poc`), `livekit` 0.9.4 / `libwebrtc` 0.3.51,
  `cpal` 0.18.2 (WASAPI shared mode), `rtrb` 0.4. Built with `--release` (thin LTO).
- **Publish options:** mono 48 kHz, `red: true`, `dtx: false`, `max_bitrate` 64 000 unless
  noted, `NativeAudioSource` with `queue_size_ms = 0` (10 ms blocks straight to the encoder).
- **Devices** (only in the device run): capture "Kopfhörermikrofon (DualSense Wireless
  Controller)", 48 kHz, 480-frame callbacks; playout "Lautsprecher (Focusrite USB Audio)",
  48 kHz, callbacks up to 1056 frames (22 ms). Playout cushion 20 ms per speaker.
- **Click measurement** (`yappa-poc measure`): two participants in one process, one clock.
  A publishes a 5 ms 2 kHz burst at the start of a 10 ms block every 500 ms over a −30 dBFS
  noise floor (so the encoder has something to encode); B finds the onset in the decoded
  audio. This is the **digital path**: encoder, network, SFU, jitter buffer, decoder. It
  contains **no device buffer**, and the click is stamped when its block is handed over — a
  real microphone would have recorded it up to 10 ms earlier.
- **Impairments:** `cargo xtask livekit loss <p>` drops p % in the server container in
  **each direction** (uplink of the speaker and downlink of the listener). End to end that is
  about 2p — the "lost" column below is what the listener actually saw. `cut <s>` drops
  everything for s seconds. Both hit the signalling connection too.

## Measurements

### Latency, no impairment

| What | Value | Method |
| --- | --- | --- |
| Click latency, digital path, 60 s | min 45.8 · median 53.2 · p95 60.6 · max 97.4 ms (n=119) | `measure --seconds 60 --noise 0.03` |
| Same, other runs | median 50.8 – 62.0 ms | six further runs of 12–60 s; the median follows the jitter buffer |
| Jitter buffer (mean per run) | 30 – 40 ms | WebRTC stats, `jitterBufferDelay / emittedCount` |
| The max of ≈ 97 ms | first click after the join | jitter buffer still settling |
| Capture block ahead of the path | + up to 10 ms | by construction, not measured |
| Mouth to ear (speaker → air → microphone → network) | **not measured** | `measure --acoustic` ran, but the controller microphone does not pick up the speakers: 2 of 18 clicks crossed the threshold, both noise |
| Output device latency | **not measured** | only the callback size is known (up to 22 ms) plus the 20 ms cushion |

### Cost on the send side

| What | Value | Method |
| --- | --- | --- |
| `capture_frame` per 10 ms block | mean 10 – 13 µs, max 79 – 328 µs | timed around the call, 1 200 – 6 000 blocks per run |
| Source pacing, longest block interval | 10.6 – 12.4 ms | pump thread, `std::thread::sleep` |
| CPU, whole process | 2.20 CPU-s in 60.7 s ≈ 3.6 % of one core | `TotalProcessorTime`; two participants, encoder with RED, decoder, both connections |
| DSP per block | not applicable | no DSP in this spike (that is S2) |

### RED and bitrate

| Option | Sent (payload) | Note |
| --- | --- | --- |
| 64 kbit/s, RED | ≈ 128 – 130 kbit/s | server log: track published as `audio/red` |
| 64 kbit/s, no RED | ≈ 63 kbit/s | |
| 96 kbit/s, RED | ≈ 192 kbit/s | |
| 32 kbit/s, RED | ≈ 55 kbit/s | |
| 64 kbit/s, RED, clicks without the noise floor | ≈ 58 kbit/s | Opus is VBR: `max_bitrate` is a cap, not a fixed rate |

The WebRTC stats name the codec `audio/opus` on both ends even with RED; the proof for RED
is the server log and the doubled payload.

### Packet loss (30 s each, 64 kbit/s)

"Concealed" is the share of played samples the decoder had to invent — the number that
tracks what you hear.

| Loss per direction | Lost at the listener | RED: concealed | no RED: concealed | RED: click latency median / p95 | Clicks missed (RED / no RED) |
| --- | --- | --- | --- | --- | --- |
| 5 % | 10.2 % / 9.0 % | 1.1 % | 8.4 % | 70.3 / 82.5 ms | 1 of 59 / 3 of 59 |
| 10 % | 19.4 % / 17.9 % | 3.2 % | 16.1 % | 75.0 / 86.4 ms | 0 of 59 / 0 of 58 |
| 20 % | 34.5 % / 35.0 % | 10.8 % | 32.5 % | 74.8 / 101.7 ms | 3 of 59 / 5 of 41 |

- With RED the jitter buffer grows to 52 – 54 ms (from 30 – 40), hence the 15 – 20 ms more
  latency. Without RED it stays lower and the losses are simply concealed.
- No NACKs and no retransmissions for audio in any run; recovery is RED and Opus FEC only.
- A 60 s repeat of 20 % with a 2 s click period gave the same picture (36.1 % lost, 12.9 %
  concealed, median 70.9 ms, 27 of 27 clicks) — the latencies are not a pairing artefact.
- Judged by counters and clicks only. **Nobody listened to speech** under loss.

### Link cut (cut starts 15 s into the run)

| Cut | Client notices | Back in the room | Audio back after the link returned | Silence in total |
| --- | --- | --- | --- | --- |
| 5 s | never — no reconnect event | stayed connected | ≈ 1.5 s | 6.5 s |
| 20 s | after ≈ 14.3 s (`Reconnecting`) | 1.6 s after the link returned | ≈ 2.0 s | 22.0 s |
| 45 s | after ≈ 14.5 s | 5.2 s after the link returned | ≈ 6.0 s | 51.0 s |

- After 20 s and 45 s the server had already dropped both participants; the SDK rejoined
  with the same token and re-subscribed without any help from our code. Clicks were normal
  again afterwards (median 68 – 70 ms).
- The cut is a black hole in the server container. The PC's own network interface stays up,
  so this resembles a dead router or uplink. **A cable pulled at the PC and a Wi-Fi switch
  were not measured** — there the OS reports the link going down, which may behave
  differently.

### Two separate clients with devices

`join --identity alice --source click --sink null` and `join --identity bob --source mic
--sink device`, two processes, 12 s: bob played alice's clicks on the Focusrite output and
published the controller microphone, alice received it. 0 packets lost, 0 playout underruns,
0 frames dropped at the playout ring, 0 capture overruns. Nobody confirmed by ear in this
session that the clicks were audible — the counters say they were played.

## What worked / what did not

**Worked**

- The custom source path from the plan: cpal callback → `rtrb` → pump thread →
  `NativeAudioSource::capture_frame`. The handoff costs microseconds.
- `red`, `dtx: false` and `audio_encoding.max_bitrate` are honoured.
- Reconnect, including a full rejoin after the server gave up on us.
- The audio callbacks only touch `rtrb` and atomics (checked by the realtime reviewer).

**Did not work, or cost time**

- **Build on Windows:** the SDK's prebuilt libwebrtc needs the **MSVC 2022** toolset (2019
  fails in abseil) and the **static C runtime**. `.cargo/config.toml` now sets
  `+crt-static` for the Windows targets — for the whole workspace, desktop app included.
  `cargo xtask doctor` checks for the toolset.
- **Outage detection is slow:** about 14 s until the client says `Reconnecting`. For 14 s a
  user would see "connected" and hear nothing.
- **Bitrate is a cap, not fixed** — the plan says "feste Bitrate". Opus CBR is not exposed
  by `TrackPublishOptions`.
- **The RTCP round-trip figure is useless here:** 280 – 317 ms on localhost and rising from
  run to run. The diagnostics panel should use the ICE candidate-pair round trip.
- The acoustic measurement needs a microphone that hears the speaker — see follow-up.

**Warnings for the engine design** (from the code and the realtime review)

- No drift compensation between the network's 10 ms frames and the output device clock; the
  spike drops a frame when its 200 ms ring is full. The engine needs a real answer.
- A capture backlog must be dropped, not sent as a burst (it becomes standing delay at the
  receiver). The spike keeps the newest block.
- `NativeAudioStream` does not end when a track is unsubscribed: track lifetime has to be
  driven by room events. The spike leaks one playout slot per resubscribe (8 slots).
- The SDK's receive path allocates and locks per frame — it must stay behind a ring, never
  in the output callback. Its internal queue (10 frames, drop-oldest) should be set
  explicitly.
- `capture_frame` with `queue_size_ms = 0` runs the encoder hand-off on the calling thread:
  dedicated non-real-time thread only.

## Decision it feeds

- **Main risk of the baseline (Rust SDK vs. JS-SDK fallback): go with the Rust SDK.** The
  fallback is not needed on the evidence so far. No ADR-001 change.
- **Echo cancellation: still open.** Nothing in this spike tested speakers plus microphone.
  The headset assumption stands until someone tries it.
- **Latency budget (70 – 120 ms):** the digital path uses 50 – 60 ms of it, 70 – 75 ms under
  loss with RED. Whether the total fits depends on the two device stages, which are not
  measured.
- **`+crt-static` on Windows** is a workspace-wide build setting forced by libwebrtc;
  recorded here and in `docs/dev-setup.md`, no ADR (no layer or contract changes).

## Follow-up tasks (also in TASKS.md)

- Mouth-to-ear with a cable from the Focusrite output to its input (or a microphone at the
  speaker): `yappa-poc measure --acoustic`.
- Second machine in the LAN (`LIVEKIT_NODE_IP`): pull the cable at the PC, switch Wi-Fi;
  listen to speech at 5 / 10 / 20 % loss; speakers instead of headset for the AEC question.
- Find out where the 14 s outage detection comes from and whether the engine can detect
  silence on the wire sooner.
- Build and run the spike on the MacBook.
- `lk load-test` with 20 publishers (needs the `lk` CLI).
