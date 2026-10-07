# Changelog

One line per finished task (definition of done). Newest first.

## Unreleased

- Dev and production build: `cargo xtask dev [--ui]`, `cargo xtask build [--debug]` (installers), npm `app:*` scripts, release/dev profiles, apps/desktop README.
- Dev setup: `cargo xtask setup|doctor`, pinned toolchain, nextest/cargo-deny/insta/proptest/criterion, vitest + prettier, git hooks, Claude Code hooks/skills/agent — see docs/dev-setup.md.
- S0 CI: `.github/workflows/check.yml` — `cargo xtask check --ci` on Windows + macOS (active once pushed).
- S0 skeleton: Cargo workspace, `cargo xtask check|gen-types|tokens`, `engine-protocol` + `api-types` with generated TS, `dsp` biquad, ui-tokens, Tauri 2 + Svelte 5 desktop shell with engine stub.
- ADR-002: alternatives to Rust (C++/JUCE, Go, .NET, Electron; PocketBase, TS, Elixir for the control plane) checked — Rust stays.
- Repository created as variant B from the planning baseline (docs/00-grobstruktur.md); stack differences in ADR-001.
