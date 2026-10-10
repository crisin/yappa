# Tasks

Variant B has no App Hub board — this file is the board. One task = one branch
(`task/<short-name>`), reviewed before it is merged into `main`. Move a line when its state
changes; finished tasks get a line in `CHANGELOG.md`.

States: **now** (in progress, max. 2) · **next** (planned, ordered) · **later** · **done**.

## Now

- [ ] S0 rest: on the MacBook `cargo xtask setup` + `check` + `build` (.app/.dmg); click
      through `cargo xtask dev` there

## Next

- [ ] First evening with the gang (user): home server up (`docs/homeserver.md`), installer
      + invites out (`docs/mittesten.md`), collect diagnostics files; write down what broke
- [ ] Homeserver walkthrough (user, hands-on): follow `docs/homeserver.md` on the mini PC
      behind the FRITZ!Box — DS-Lite check first; note in the guide what did not match
- [ ] S1 rest: mouth-to-ear with a loopback cable (Focusrite out → in) via
      `yappa-poc measure --acoustic`; second machine in the LAN: cable pulled at the PC,
      Wi-Fi switch, speech by ear at 5/10/20 % loss, speakers for the AEC question; spike on
      the MacBook → add to `docs/spikes/s1-poc.md`
- [ ] S6: Control plane in Rust — axum + sqlx/SQLite, invite login, token issuing, webhook
      presence → `docs/spikes/s6-control.md`
- [ ] S2: Offline DSP chain WAV → DeepFilterNet → gate → EQ → WAV, latency + CPU per block
- [ ] S3: Global PTT with a fullscreen game on Windows, tray, autostart

## Later

- [ ] Engine follow-ups from the test client (ADR-003, realtime review 2026-10-10):
      drift compensation instead of jumping at the high-water mark; resampling for devices
      that are not at 48 kHz; transport events carry a session number so a late event of a
      replaced join cannot touch the new one; a "nothing arrives" warning before the SDK
      notices (the per-second stats show a dead link at once, the SDK after ≈ 14 s)
- [ ] Echo cancellation for people on speakers (S1 left it open; testers are told to use
      a headset)
- [ ] Outage detection: the client needs ≈ 14 s to notice a dead link (S1) — find the
      source (signal ping timeout?) and detect silence on the wire sooner
- [ ] `lk load-test` with 20 publishers against the dev server (needs the `lk` CLI)
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

- [x] Test client: `crates/transport` (trait + LiveKit), `crates/audio-engine` (devices,
      pump, 24-slot mixer, levels, rejoin), shell with logging and diagnostics export, UI
      with invite join, participants, per-person volume, debug panel; headless example;
      ADR-003; tester guide `docs/mittesten.md` (2026-10-10)
- [x] Home server (stage 1) prepared: `infra/livekit/home.yml` (LiveKit + Caddy, host
      network), `.env.example`, `cargo xtask livekit token <name>`, `yappa-poc --token`,
      guide `docs/homeserver.md` for FRITZ!Box + Linux VM (2026-10-10)
- [x] S1: PoC — `crates/spike-s1` (`yappa-poc`): two clients hear each other via local
      LiveKit, RED/bitrate verified, click latency 50–60 ms (digital path), loss and
      reconnect measured → `docs/spikes/s1-poc.md`; Rust SDK stays (2026-10-10)
- [x] S1 prep: `infra/livekit/dev.yml` (LiveKit v1.13.8 + netem sidecar) and
      `cargo xtask livekit up|down|logs|loss|cut` (2026-10-10)
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
