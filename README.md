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

Prerequisites: Rust stable (rustup), Node 20+, Tauri 2 prerequisites (Windows: WebView2 +
MSVC Build Tools; macOS: Xcode CLT); Docker Desktop for local LiveKit from S1 on.

```bash
cargo xtask setup                         # dev tools, npm deps, git hooks (once per machine)
cargo xtask check                         # everything green?
npm --prefix apps/desktop run tauri dev   # the desktop app
```

Tooling, tests and the Claude Code setup: [docs/dev-setup.md](docs/dev-setup.md).
