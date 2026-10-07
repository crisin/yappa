# ADR-001: Variant B — same core, Rust-first stack, developed outside App Hub

Status: accepted (2026-10-07)

## Context

yAPPA is developed twice, in parallel, from the same planning baseline
([ADR-000](../00-grobstruktur.md)):

- **Variant A** lives in `App-Hub/projects/yappa` (GitHub `crisin/yappa`). It follows the
  baseline literally and is driven through the App Hub board (worktree per item, review lane,
  debate intake).
- **Variant B** is this repository (`D:\Projekte\dev\YAPPA`). It is developed "externally":
  Claude Code directly in the repo, no hub.

Two copies of the same idea only teach something if they differ where the baseline itself is
unsure. The baseline already names its seams and open decisions (UI framework, where the
control plane lives, client shell, scripts). Variant B takes the other branch at a few of those
seams and keeps everything that *is* the idea identical, so the two can be compared on voice
quality, robustness, effort and speed of iteration.

## Decision

**Identical to variant A (the core idea — not up for variation here):**

- Desktop client on **Tauri 2**, Windows as reference platform, macOS builds along.
- **All audio native in Rust**: cpal capture/output, denoiser behind a `Denoiser` trait
  (DeepFilterNet / RNNoise), gate/VAD/PTT, EQ, compressor, effect slot; real-time thread
  without allocations and locks (`rtrb`).
- **LiveKit** as media plane (Rust SDK, `NativeAudioSource`, RED on, DTX off, fixed bitrate),
  self-hosted in the stages Dev → Home → VPS → own hardware.
- The engine has a message-based API (`engine-protocol`) and contains no Tauri code.
- Layering: `dsp` knows no I/O, `audio-engine` knows no network, `transport` knows no DSP.
- Feature list, latency budget (70–120 ms mouth-to-ear), spikes S1–S4, update channels,
  diagnostics — as in the baseline.

**Different in variant B:**

| Seam | Variant A (hub) | Variant B (this repo) | Why try it |
| --- | --- | --- | --- |
| Control plane | NestJS + Prisma on Railway (`apps/api`) | **Rust: axum + sqlx + SQLite** (`crates/control`), one binary | One language end to end; DTOs come from Rust via ts-rs, no second schema |
| Hosting of control plane | Railway, separate from media | **Same host as LiveKit**, one `docker compose` (LiveKit + control + Caddy) | No Railway account; a small group needs one box. Trade-off below |
| Shared contracts | `packages/shared` (TS DTOs) + `engine-protocol` | **Rust crates only** (`engine-protocol`, `api-types`), TS generated | Contracts stay testable in one place |
| Desktop UI | decided by spike S5 | **Svelte 5 + Vite + TypeScript**, decided now | No spike needed to start; same stack as App Hub; tokens keep it reversible |
| Scripts / task runner | Node scripts in `infra/scripts`, npm root | **`cargo xtask`**; npm only inside `apps/desktop` | Cargo-first repo, still no bash/PowerShell-only scripts |
| Work tracking | App Hub board, review lane, debate | **`TASKS.md`** + branch per task, review before merge | Measures what the hub adds by its absence |

## Consequences

- **Railway-outage argument changes shape.** In A, control and media fail independently. In B
  they share a host: if the box dies, both are gone — but then there is no voice anyway. The
  failover story (fallback media server, channel reassignment) needs the control plane to run
  on the fallback host too, or as a second instance with replicated SQLite (Litestream). To be
  settled when stage 2 (VPS) is built.
- Long-lived LiveKit tokens (≈ 12 h) and the client-side cache stay, so a restart of the
  control process still never ends a running call.
- No existing NestJS AuthModule to reuse — auth is invite links + session tokens in the
  control crate (argon2 only if passwords are ever added). Small for one group, but new code.
- Spike S5 is dropped for B; S0 here means: Cargo workspace, xtask, Tauri + Svelte shell,
  `cargo xtask check`, CI matrix.
- Results from B are written so they can be compared with A: spike results in `docs/spikes/`
  use the same measurements (latency per block, CPU, loopback-click latency).
- Any of these choices may be reverted towards A by a later ADR; the seams make that local.
