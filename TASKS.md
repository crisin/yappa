# Tasks

Variant B has no App Hub board — this file is the board. One task = one branch
(`task/<short-name>`), reviewed before it is merged into `main`. Move a line when its state
changes; finished tasks get a line in `CHANGELOG.md`.

States: **now** (in progress, max. 2) · **next** (planned, ordered) · **later** · **done**.

## Now

- [ ] S0 rest: on the MacBook `cargo xtask setup` + `check` + `build` (.app/.dmg); click
      through `cargo xtask dev` there

## Next

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

- Remote: stays local for now (2026-10-07); the user moves it to GitHub later. CI
  (`.github/workflows/check.yml`) starts working then.
- Logo: the cat (2026-10-07) is a "for now" logo of unknown origin — clarify the rights or
  replace it before the repo goes public.
- License: MIT/Apache (no VST3) — same open decision as variant A. Cargo metadata says
  `MIT OR Apache-2.0` provisionally.

## Done

- [x] Logo "for now": cat as app icon set, sidebar logo and favicon; `cargo xtask dev`
      checked on Windows by the user (2026-10-07)
- [x] Dev/prod build commands: `cargo xtask dev|build`, npm `app:*` scripts, release profile,
      bundle config, READMEs (2026-10-07)
- [x] Dev setup: pinned toolchain, `cargo xtask setup|doctor|test|bench|coverage|hook`,
      nextest, cargo-deny, insta + proptest + criterion, vitest + Testing Library + prettier,
      git hooks, Claude Code settings/hooks/skills/agent, `docs/dev-setup.md` (2026-10-07)
- [x] S0: CI workflow — windows + macos matrix running `cargo xtask check --ci` (dormant
      until a remote exists) (2026-10-07)
- [x] S0: `apps/desktop` — Tauri 2 + Svelte 5 + Vite + TS shell, engine stub, ui-tokens →
      tokens.css, one-window UI (2026-10-07)
- [x] S0: Cargo workspace, `cargo xtask check|gen-types|tokens`, `engine-protocol`,
      `api-types`, `dsp` (biquad) (2026-10-07)
- [x] ADR-002: alternatives to Rust checked — Rust stays (2026-10-07)
- [x] Repository created: git, planning baseline, ADR-001 (variant B stack), docs skeleton
      (2026-10-07)
