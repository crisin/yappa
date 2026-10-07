# yAPPA · variant B

> yAPPA: Appa aus Avatar, und yappen ist, was wir da tun.

Self-hosted gaming voice chat as a replacement for Discord and Steam voice — for one group of
up to 20, on own hardware, with voice quality and robustness first.

yAPPA is built twice from the same plan. This repository is **variant B**:

| | Variant A | Variant B (this repo) |
| --- | --- | --- |
| Where | `App-Hub/projects/yappa`, GitHub `crisin/yappa` | `D:\Projekte\dev\YAPPA`, local |
| Workflow | App Hub board, worktree per item, review lane | Claude Code directly, `TASKS.md`, branch per task |
| Client | Tauri 2, native Rust audio engine, UI by spike S5 | Tauri 2, native Rust audio engine, **Svelte 5** |
| Control plane | NestJS + Prisma on Railway | **Rust (axum + sqlx + SQLite)** next to LiveKit |
| Media plane | self-hosted LiveKit | self-hosted LiveKit |
| Tooling | npm workspaces + Node scripts | **Cargo workspace + `cargo xtask`** |

Why and what follows from it: [ADR-001](docs/adr/001-variante-b-stack.md).

## Docs

- [docs/00-grobstruktur.md](docs/00-grobstruktur.md) — the planning baseline (German, ADR-000)
- [docs/adr/](docs/adr/README.md) — architecture decisions
- [docs/spikes/](docs/spikes/README.md) — spike goals and results
- [docs/dev-setup.md](docs/dev-setup.md) — tooling, tests, Claude Code setup
- [TASKS.md](TASKS.md) — what is being worked on
- [CHANGELOG.md](CHANGELOG.md) · [CLAUDE.md](CLAUDE.md) — rules for coding agents

## Status

S0 skeleton (2026-10-07): Cargo workspace, contracts with generated TS, `dsp` biquad,
Tauri 2 + Svelte 5 shell with an engine stub. No audio yet — that is S1. Local repo only.
Why Rust and not something else: [ADR-002](docs/adr/002-language-alternatives.md).

## Quickstart

Prerequisites: Rust via rustup (the version is pinned in `rust-toolchain.toml` and installed
automatically), Node 20+, the Tauri 2 prerequisites (Windows: WebView2 + MSVC Build Tools;
macOS: Xcode Command Line Tools), Docker Desktop for local LiveKit from S1 on.

```bash
cargo xtask setup     # once per machine: dev tools, npm deps, git hooks, then a tool report
cargo xtask check     # the gate: format, lint, types, all tests, licenses
```

## Develop

```bash
cargo xtask dev       # desktop app with hot reload (Vite for the UI, Rust rebuilds on change)
cargo xtask dev --ui  # UI only in the browser (http://localhost:1420), engine is mocked
cargo xtask test      # Rust + UI tests
```

## Build (production)

```bash
cargo xtask build          # release build + installers for the OS you are on
cargo xtask build --debug  # same, debug profile (faster, for testing the installer)
```

Windows produces `target/release/bundle/nsis/*-setup.exe` (per-user install, no admin) and
`target/release/bundle/msi/*.msi`; macOS produces `target/release/bundle/macos/*.app` and
`target/release/bundle/dmg/*.dmg`. Builds are unsigned for now — SmartScreen / Gatekeeper
warn once. Signing, update channels (nightly/beta/stable) and the updater come with phase 2.

All commands, tools, test kinds and the Claude Code setup: [docs/dev-setup.md](docs/dev-setup.md).
The same commands also exist as npm scripts in [apps/desktop](apps/desktop/README.md).
