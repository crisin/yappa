# Tasks

Variant B has no App Hub board — this file is the board. One task = one branch
(`task/<short-name>`), reviewed before it is merged into `main`. Move a line when its state
changes; finished tasks get a line in `CHANGELOG.md`.

States: **now** (in progress, max. 2) · **next** (planned, ordered) · **later** · **done**.

## Now

- [ ] S0: Skeleton — Cargo workspace (`crates/*`), `xtask` with `check` (fmt, clippy -D
      warnings, test, UI typecheck), `engine-protocol` + `dsp` crates, `.cargo/config.toml`
      alias `xtask`

## Next

- [ ] S0: `apps/desktop` — Tauri 2 + Svelte 5 + Vite + TS, ui-tokens → CSS variables, empty
      window with channel list / speakers / mic bar
- [ ] S0: CI — GitHub Actions matrix windows-latest + macos-latest running `cargo xtask check`
      (needs a remote, see open questions)
- [ ] S1 prep: `infra/livekit/dev.yml` + `cargo xtask livekit` (local LiveKit, dev keys)
- [ ] S1: PoC — engine CLI, two clients hear each other via local LiveKit; loopback-click
      latency, packet loss, reconnect with pulled cable → `docs/spikes/s1-poc.md`
- [ ] S6: Control plane in Rust — axum + sqlx/SQLite, invite login, token issuing, webhook
      presence → `docs/spikes/s6-control.md`
- [ ] S2: Offline DSP chain WAV → DeepFilterNet → gate → EQ → WAV, latency + CPU per block
- [ ] S3: Global PTT with a fullscreen game on Windows, tray, autostart

## Later

- [ ] S4 (optional): clack-host loads a CLAP plugin
- [ ] Phase 1 · Playable core · Phase 2 · Quality · Phase 3 · Extensions (see
      `docs/00-grobstruktur.md`, "Projekt-Vorgehen")

## Open questions

- Remote: separate GitHub repo (e.g. `crisin/yappa-b`) or keep local until S1 works?
- License: MIT/Apache (no VST3) — same open decision as variant A.

## Done

- [x] Repository created: git, planning baseline, ADR-001 (variant B stack), docs skeleton
      (2026-10-07)
