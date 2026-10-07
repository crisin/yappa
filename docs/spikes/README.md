# Spikes

Each spike takes 1–3 days and ends with a markdown result here (`s1-poc.md`, …): what was
tried, measurements (latency per block, CPU, loopback-click latency), what worked, what
didn't, and the decision it feeds. Use the same measurements as variant A so the two can be
compared.

| Spike | Goal (variant B) |
| --- | --- |
| S0 | Repo skeleton: Cargo workspace, `cargo xtask check`, Tauri 2 + Svelte 5 shell, CI matrix (windows + macos) |
| S1 | PoC: two clients hear each other via local LiveKit — engine CLI (cpal → NativeAudioSource), RED, bitrate, reconnect; latency via loopback click |
| S2 | DSP chain offline: WAV → DeepFilterNet → gate → EQ → WAV; latency and CPU per block |
| S3 | Tauri: global PTT hotkey with a fullscreen game on Windows, tray, autostart |
| S4 | optional: clack-host loads a CLAP plugin and passes audio through |
| S6 | Control plane in Rust: axum + sqlx/SQLite, invite login, `POST /voice/token`, LiveKit webhook → presence; runs in the same compose as LiveKit |

S5 (UI stack analysis) is not run in this variant — ADR-001 decides Svelte 5.
