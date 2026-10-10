# Changelog

One line per finished task (definition of done). Newest first.

## Unreleased

- S1 PoC: `yappa-poc` (scratch crate `crates/spike-s1`) — cpal ↔ LiveKit Rust SDK, click latency, packet loss, reconnect; result in docs/spikes/s1-poc.md. Windows builds now use the static C runtime (`.cargo/config.toml`), needed by libwebrtc.
- S1 prep: local LiveKit in Docker (`infra/livekit/dev.yml`) and `cargo xtask livekit up|down|logs|loss <%>|cut <s>` — packet loss and a cut link via tc/netem; `doctor` checks for the MSVC 2022 toolset.
- Logo (for now): app icons, sidebar logo and favicon from the cat picture.
- Dev and production build: `cargo xtask dev [--ui]`, `cargo xtask build [--debug]` (installers), npm `app:*` scripts, release/dev profiles, apps/desktop README.
- Dev setup: `cargo xtask setup|doctor`, pinned toolchain, nextest/cargo-deny/insta/proptest/criterion, vitest + prettier, git hooks, Claude Code hooks/skills/agent — see docs/dev-setup.md.
- S0 CI: `.github/workflows/check.yml` — `cargo xtask check --ci` on Windows + macOS (active once pushed).
- S0 skeleton: Cargo workspace, `cargo xtask check|gen-types|tokens`, `engine-protocol` + `api-types` with generated TS, `dsp` biquad, ui-tokens, Tauri 2 + Svelte 5 desktop shell with engine stub.
- ADR-002: alternatives to Rust (C++/JUCE, Go, .NET, Electron; PocketBase, TS, Elixir for the control plane) checked — Rust stays.
- Repository created as variant B from the planning baseline (docs/00-grobstruktur.md); stack differences in ADR-001.
