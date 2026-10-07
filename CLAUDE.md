# yAPPA (variant B) — Claude Code Instructions

Self-hosted gaming voice chat for a fixed group of up to 20 people, replacing Discord and
Steam voice. Voice quality and robustness come before everything else.

This repo is **variant B**: same idea as variant A (`App-Hub/projects/yappa`, driven by the
App Hub board), but a Rust-first stack and developed directly with Claude Code, without the
hub. Do not copy code between the two repos unasked — they are meant to be compared.

**Read before architectural work:**
1. [`docs/00-grobstruktur.md`](docs/00-grobstruktur.md) — planning baseline (German), ADR-000.
2. [`docs/adr/001-variante-b-stack.md`](docs/adr/001-variante-b-stack.md) — where this
   variant deviates. ADR-001 wins over ADR-000 where they differ.

## Architecture in one breath

- **Desktop client** (`apps/desktop`, Tauri 2 + Svelte 5 + Vite + TS): login, channels,
  voice, settings. **All audio processing is native Rust** in the engine; the webview only
  sees levels, switches and settings.
- **Control plane** (`crates/control`, Rust: axum + sqlx + SQLite): invites/identity,
  channels, LiveKit tokens, presence from webhooks, a tiny bootstrap/emergency admin page.
  Runs next to LiveKit in the same `docker compose`. Never in the voice path — a running call
  survives a control-plane restart (long-lived tokens, client cache).
- **Media plane** (self-hosted LiveKit): routes the Opus streams. UDP direct, TURN fallback.

## Repo layout (target — built up by the S0 tasks)

```
apps/desktop/            Tauri 2 shell (window, tray, hotkeys, updater) + Svelte 5 UI
  src/                   UI — talks only engine-protocol + api-types (generated TS)
  src-tauri/             Rust shell, embeds crates/audio-engine
crates/engine-protocol/  commands, events, settings schema; versioned; TS via ts-rs
crates/api-types/        client <-> control-plane DTOs; TS via ts-rs
crates/dsp/              pure DSP blocks (biquad EQ, gate, compressor, effects) — no I/O
crates/audio-engine/     sources, pipeline, mixer, output, platform traits — no Tauri, no LiveKit
crates/transport/        Transport trait + LiveKit implementation, reconnect, publish options
crates/control/          control-plane server binary (axum, sqlx/SQLite, migrations)
crates/platform-windows/, crates/platform-macos/   one implementation per OS behind traits
crates/plugin-host/      CLAP/VST3 (phase 3)
xtask/                   repo tasks: check, livekit, gen-types, …  (`cargo xtask <cmd>`)
ui-tokens/               design tokens + themes as JSON -> CSS variables
infra/livekit/           compose files dev/home/vps (LiveKit + control + Caddy), livekit.yaml
docs/adr/, docs/spikes/  decisions and spike results
TASKS.md                 the board of this variant
```

## Rules

1. **Layering is the rule, not the folder structure.** `dsp` knows no I/O, `audio-engine`
   knows no network and no Tauri, `transport` knows no DSP, `control` knows no audio. Every
   seam is a contract crate with tests; no side imports the other directly.
2. **Real-time audio thread:** no allocations, no locks (use `rtrb`), no logging in the
   callback.
3. **Windows is the reference platform.** macOS gets what comes along. Platform code lives
   behind traits (`AudioDevice`, `AppCapture`, `GlobalHotkey`, `RealtimeThread`), one module
   per OS via `cfg`. A missing implementation hides or degrades the feature — it never breaks
   the build or the call.
4. **Cargo first.** Repo tasks are `xtask` subcommands (Rust), never bash- or
   PowerShell-only scripts. npm exists only inside `apps/desktop`. The only shell files are
   the one-line git hooks in `.githooks/` (git runs them with its own `sh` everywhere).
5. **Anything touching the send chain** gets a latency and CPU measurement before and after.
   Latency budget mouth-to-ear: 70–120 ms.
6. **No secrets in the repo** (it may go public). LiveKit keys and signing keys only in
   `.env` (gitignored) or CI secrets; `.env.example` documents them.
7. **Generated TS types are committed** together with the Rust change that produced them.

## Working without the hub

- Pick the top task from `TASKS.md` "Now"/"Next". Work on a branch `task/<short-name>`,
  never directly on `main`.
- When done: `cargo xtask check` green, `TASKS.md` and `CHANGELOG.md` updated, then ask the
  user to review before merging (`git merge --no-ff`, logbook-format merge message).
  No auto-merge, no push unless asked.
- New requirements go into `TASKS.md` under "Later" with three answers: Which problem while
  gaming does it solve? Which plane changes, and does the client/control/media separation
  stay clean? What is deliberately not built for it?

## Definition of done (per task)

- `cargo xtask check` green (Windows and macOS in CI, once CI exists)
- Reviewed by the user before merge
- One line in `CHANGELOG.md` (Unreleased), task moved to "Done" in `TASKS.md`
- Architecture change → an ADR in `docs/adr/` (context, decision, consequences — one page)

## Commands and tests

Full reference: [`docs/dev-setup.md`](docs/dev-setup.md). The daily loop:

```bash
cargo xtask check          # the gate — must be green before every commit you propose
cargo xtask check --fast   # quick subset (also the git pre-commit hook)
cargo xtask test           # Rust (nextest + doctests) + UI (vitest) only
cargo xtask gen-types      # after changing engine-protocol / api-types
cargo xtask tokens         # after changing ui-tokens/
cargo insta review         # after an intended snapshot change
cargo xtask bench          # DSP cost per 10 ms block (send-chain changes: before/after)
npm --prefix apps/desktop run tauri dev   # the desktop app
npm --prefix apps/desktop run dev         # UI only in a browser; lib/engine.ts mocks the engine
```

Which test for what: unit tests next to the code; `proptest` for invariants; `insta`
snapshots (rounded, platform-stable) for responses and tables; `criterion` benches for the
send chain; vitest + Testing Library for UI. Generated files (`apps/desktop/src/lib/types/*`
incl. `defaultSettings.json`, `tokens.css`) are written by xtask and committed — never edit
them; Claude Code is denied write access to them.

Claude Code setup in `.claude/`: a SessionStart hook prints the current tasks, a PostToolUse
hook formats edited files, skills `task` / `adr` / `spike`, agent `realtime-reviewer` (run it
after touching engine, DSP or transport code).

## Git & Logbook

The git history is this project's logbook. Every change is a commit with a detailed message
(`git config commit.template .gitmessage`):

```
<type>(<scope>): <what changed — imperative, max 72 chars>

Why:       the problem or motivation
What:      the change, key decisions, rejected alternatives
Verified:  how it was checked — or "not verified" plus the reason
Follow-up: open ends (omit if none)
```

Types: feat fix refactor docs chore test perf build ci. One logical change per commit.
Never rewrite pushed history.
