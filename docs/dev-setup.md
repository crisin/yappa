# Dev setup

Everything a machine (Windows PC, MacBook, CI) and a coding agent needs, and why. One entry
point: `cargo xtask`. No shell scripts — the only shell files are the two one-line git hooks,
which git itself runs (Git for Windows ships `sh`).

## New machine

```bash
git clone <repo> && cd yappa      # or: the local folder
cargo xtask setup                 # tools, npm deps, git hooks, commit template, then doctor
cargo xtask check                 # all green?
```

`rust-toolchain.toml` pins the compiler (currently 1.97.1 + clippy, rustfmt, llvm-tools), so
both machines and CI lint with the same clippy. Bump it in its own commit.

## The toolbox

`cargo xtask doctor` prints this table live, with install hints for what is missing.

| Tool | Need | What for |
| --- | --- | --- |
| Rust (pinned) | required | everything |
| Node 20+ | required | UI build, svelte-check, vitest, prettier |
| cargo-nextest | required | test runner — parallel, isolated, readable output |
| cargo-deny | required | licenses (no GPL/AGPL by accident — the VST3 question), advisories, sources |
| cargo-insta | required | reviewing snapshot changes (`cargo insta review`) |
| cargo-llvm-cov | optional | `cargo xtask coverage` (HTML report) |
| bacon | optional | clippy/tests continuously in a terminal (`bacon`, `bacon test`) |
| Docker | from S1 | local LiveKit |
| `lk` (LiveKit CLI) | from S1 | tokens, rooms, `lk load-test` with 20 publishers |
| gh | optional | once the repo is on GitHub |

`cargo xtask setup` installs the cargo tools via `cargo-binstall` (prebuilt binaries). Node,
Docker, `lk` and `gh` come from the OS package manager (winget / brew).

## Commands

| Command | Does |
| --- | --- |
| `cargo xtask dev` | desktop app with hot reload (`tauri dev`) |
| `cargo xtask dev --ui` | UI only in the browser on :1420, engine mocked |
| `cargo xtask build` | production build: release profile (thin LTO, stripped) + installers, lists them with size |
| `cargo xtask build --debug` | installer from the debug profile |
| `cargo xtask check` | the gate: fmt, tokens, prettier, svelte-check, clippy `-D warnings`, nextest + doctests, vitest, cargo-deny |
| `cargo xtask check --fast` | pre-commit subset: fmt, tokens, prettier, clippy |
| `cargo xtask check --ci` | full + `npm ci` + fails if generated files are not committed |
| `cargo xtask test` | Rust + UI tests only |
| `cargo xtask gen-types` | Rust contracts → `apps/desktop/src/lib/types` (TS + `defaultSettings.json`) |
| `cargo xtask tokens` | `ui-tokens/` → `apps/desktop/src/lib/tokens.css` |
| `cargo xtask bench` | criterion: DSP cost per 10 ms block |
| `cargo xtask coverage` | line coverage, opens the HTML report |
| `cargo xtask doctor` / `setup` | see above |

Build profiles (root `Cargo.toml`): in **dev**, all dependencies and the `dsp` crate are
optimised (`opt-level` 2/3) while our own crates stay debuggable — debug-speed DSP would make
audio impossible to judge by ear. **release** uses `codegen-units = 1`, thin LTO and strips
symbols. The app version has one source: `[workspace.package] version` in `Cargo.toml`
(`tauri.conf.json` has no version of its own).

Git hooks (`.githooks`, activated by `setup` via `core.hooksPath`): **pre-commit** runs
`check --fast`, **pre-push** the full `check`.

## Tests — which kind for what

| Kind | Tool | Where | Use for |
| --- | --- | --- | --- |
| Unit | `#[test]` + nextest | next to the code | logic, small DSP properties |
| Property | proptest | `crates/*/tests/` | invariants over random input: filters stay stable, no NaN |
| Snapshot | insta | `crates/*/tests/snapshots/` | frequency responses, tables, serialized formats — rounded so Windows/macOS agree |
| Contract | ts-rs export tests | engine-protocol, api-types | Rust is the source of truth; TS + default settings are generated and committed, CI fails on drift |
| Benchmark | criterion | `crates/*/benches/` | anything in the send chain; budget DSP < 5 ms per 10 ms block |
| UI | vitest + Testing Library (jsdom) | `apps/desktop/src/**/*.test.ts` | components and `lib/engine.ts` in browser mode |
| Offline audio | WAV in → WAV out (S2) | `crates/audio-engine/tests/` | the whole chain without audio devices, headless in CI |
| Integration | LiveKit in Docker (S1) | `crates/transport/tests/` | connect, publish, reconnect |

The last two rows arrive with their spikes.

## Claude Code in this repo

Checked in under `.claude/`, so it works the same on both machines:

- **`settings.json`**
  - *allow*: cargo/xtask/test/lint commands, `npm run`, read-only and local git (status,
    diff, log, switch, add, commit) — no prompts for the daily loop.
  - *ask*: `git push/merge/rebase/reset`, `cargo add`, `npm install` — anything that
    publishes, rewrites, or adds dependencies.
  - *deny*: editing generated files (`src/lib/types/**`, `tokens.css`), reading `.env*`.
  - *env*: no ANSI colors in tool output (`CARGO_TERM_COLOR=never`, `NO_COLOR=1`).
  - *hooks*: **SessionStart** prints branch + "Now"/"Next" from `TASKS.md` into the
    session's context; **PostToolUse** formats every edited file (rustfmt / prettier) via
    `cargo xtask hook post-edit` — Rust, so it runs on Windows and macOS alike.
- **Skills** (`.claude/skills/`): `task` (one TASKS.md item end to end: branch → tests →
  gate → bookkeeping → logbook commit → stop for review), `adr`, `spike` (measurement
  protocol comparable with variant A).
- **Agent** (`.claude/agents/realtime-reviewer.md`): reviews diffs for allocations/locks in
  the audio path, layering violations, contract drift, missing measurements.

Personal overrides go to `.claude/settings.local.json` (gitignored).

## Possible later additions

- `sccache` as global `rustc-wrapper` (in `~/.cargo/config.toml`, not the repo) if Claude Code
  worktrees make rebuilds of Tauri painful.
- Tauri WebDriver e2e (`tauri-driver`, Windows only) once there are real flows to click.
- An MCP server for LiveKit/`lk` if the agent should drive rooms during S1 tests.
