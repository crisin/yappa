# ADR-003: Engine seams as built, pasted invites, logging

Status: accepted (2026-10-10) · builds on ADR-001, uses the result of spike S1

## Context

Spike S1 showed the LiveKit Rust SDK carries the plan. The gang should now test over the
internet with a real window instead of a command line, and every odd sound should be
explainable afterwards. That forced the first real cut of the crates the plan only names
(`transport`, `audio-engine`) and three decisions the plan leaves open or defers.

## Decision

**1. The `Transport` trait lives in `crates/transport`; LiveKit sits behind the cargo feature
`livekit`.** The engine depends on the crate without the feature and sees only the trait:
10 ms blocks of 48 kHz mono in both directions (`VoiceOut`, `AudioReceiver`/`FrameSink`) and
plain events. The shell turns the feature on and hands the engine a `LiveKitTransport`.
Rejected: a separate contract crate for one trait (a fourth crate to keep in step), and the
trait inside `audio-engine` (then `transport` would depend on the engine).

**2. Sound hardware is the `AudioIo` trait inside `audio-engine`, implemented with cpal
there.** The planned `platform-*` crates take over what cpal cannot do (process loopback,
hotkeys); plain capture and playout do not need them. Tests run the whole engine with a fake
`AudioIo` and a fake `Transport`.

**3. Three kinds of threads.** Control thread: commands, transport events, timers — may
block and log. Pump thread: capture ring → 10 ms blocks → transport; not real-time, because
the transport encodes on it. Device callbacks: `rtrb` and atomics only. The receive mix has a
fixed number of speaker slots (24) so the output callback never allocates; a slot's lifetime
is the lifetime of the sink the transport holds for that speaker.

**4. Until the control plane exists (S6), access is a pasted invite.** `api_types::Invite`
(server address + LiveKit token) as one line of text, issued with `cargo xtask livekit
token`. The engine command is the planned `Join { url, token }`; only where the two values
come from will change.

**5. Reconnect policy lives in the engine, not in the UI.** The SDK's own reconnect comes
first. If a session that was established ends anyway, the engine joins again with the same
token after 3 s, 6 s, … (capped at 30 s) until the user leaves. Not when the transport says
a retry cannot help: thrown out, same identity joined elsewhere, room deleted, token refused.
A join that never worked is not repeated.

**6. Logging is `tracing`, set up by the shell, with three sinks.** JSON lines in a daily
file (seven days kept; our crates at debug, including one stats line per second in a call),
info and above to the UI as `Event::Log` for the timeline, and readable text on stderr.
"Export diagnostics" writes one text file with version, settings and the log files. Tokens
and invites are never logged.

## Consequences

- `engine-protocol` grew additively (participants, stats, log entries, resync); the
  protocol version stays 1.
- The engine runs headless with the same code as in the app
  (`apps/desktop/src-tauri/examples/headless.rs`) — the measuring tool the plan asks for.
- An invite is a bearer secret with a long life. Acceptable for a handful of testers,
  replaced by S6. Revoking one means new server keys for everybody.
- Known gaps, accepted for the test client and visible in the debug panel instead of
  hidden: no resampling (48 kHz devices only), no drift compensation between a speaker's
  clock and our output (playout jumps when a buffer runs over, counted as "skips"), the
  transmit mode is stored but the microphone is always on, no DSP chain yet (S2).
- cpal's `realtime` feature is on (thread priority for the audio threads); it brings
  `audio_thread_priority` into the tree.
- Windows builds link the static C runtime (found in S1) — unchanged.
