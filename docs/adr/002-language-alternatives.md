# ADR-002: Rust stays — alternatives to Rust (and the rest of the stack) checked

Status: accepted (2026-10-07) · confirms ADR-001

## Context

ADR-001 made variant B "Rust-first". Before building the skeleton, the question was asked
seriously: is Rust actually the best choice, or only the obvious one? Rust shows up in two
places that need separate answers:

1. **Client audio engine** — shared with variant A, so this is really a question about the
   core idea.
2. **Control plane** — variant B's own choice (A uses NestJS).

Criteria, in order: voice quality and real-time safety · LiveKit client support · denoiser and
plugin ecosystem · Windows-first OS hooks (PTT, MMCSS, process loopback) · how well coding
agents work in it · license fit (MIT/Apache wanted).

## 1. Client audio engine

| Candidate | LiveKit client | Real-time audio | Denoiser / plugins | Verdict |
| --- | --- | --- | --- | --- |
| **Rust** (cpal, livekit 0.9.4) | native: the Rust SDK *is* LiveKit's native core | no GC, `rtrb`, no-alloc callback enforceable | DeepFilterNet is a Rust crate (`deep_filter`), `nnnoiseless`, `clack-host` for CLAP | **chosen** |
| C++ (JUCE) | C++ SDK exists, but wraps the Rust core via FFI | excellent, industry standard | best plugin hosting (VST3/AU/CLAP) | JUCE 8 is AGPLv3 or commercial; memory safety in the RT path; agents slower. Runner-up if plugin hosting becomes central |
| Go (pion) | pion is the best non-browser WebRTC; LiveKit server is Go | GC + cgo in the audio callback | Opus, denoiser and audio I/O only via cgo; no plugin hosting | good for servers, wrong for the engine |
| C# / .NET | no native client (Unity SDK sits on the Rust FFI) | GC, NAudio is Windows-only | thin | no |
| Electron + JS SDK + AudioWorklet | first-class JS SDK | browser audio stack, no hard latency control | no VST, PTT needs a native module | already the baseline's documented fallback, not a target |

The deciding fact: every non-JS LiveKit client SDK (Python, Node, Unity, C++) is a wrapper
around the Rust core (`livekit-ffi`). Any native path ends up on Rust anyway — writing the
engine in Rust removes a layer instead of adding one. Discord solves the same problem with an
Electron UI over a native C++ engine; Rust is that architecture with memory safety and without
JUCE's license.

## 2. Control plane (variant B)

| Candidate | For | Against |
| --- | --- | --- |
| **Rust: axum 0.8 + sqlx 0.9 + SQLite** | `livekit-api` 0.8 does token signing and webhook verification; DTOs shared with the client as a crate (ts-rs); one toolchain, one CI cache, one language for agents | auth, invites and the emergency admin page are handwritten; slower compiles |
| Go + PocketBase | auth, SQLite, realtime subscriptions (presence push) and an admin UI out of the box; official LiveKit Go server SDK | a second compiled language next to Rust; PocketBase is pre-1.0 with breaking changes; DTOs not shared |
| TypeScript (Hono/Fastify on Node or Bun) | first-class LiveKit server SDK, fast to write | too close to variant A (NestJS) — the comparison would teach little |
| Elixir / Phoenix | `Phoenix.Presence` is built for exactly this | fourth language, no official LiveKit SDK |

**Rust stays.** The control plane is small (tokens, invites, channels, presence), so the
"handwritten" cost is bounded, and sharing the DTO crate with the client is worth more than a
generated admin UI. **Fallback:** if S6 takes more than ~3 days, switch the control plane to
PocketBase + Go hooks (new ADR) — the `api-types` seam keeps that local.

## 3. Rest of the stack — sanity check

- **Shell: Tauri 2** (2.12). Tauri 3 is in alpha; stay on 2 until it is stable. Electron
  would add Node between UI and engine; native Rust GUIs (Slint, egui, iced, Dioxus) would
  complete "one language" but cost UI speed and the "a theme is a file" token approach.
- **UI: Svelte 5** (5.57) + Vite 8 + TypeScript — unchanged from ADR-001.
- **Media plane: LiveKit** stays. Mumble/murmur is the proven gaming-voice alternative with
  very low latency, but has no FEC/RED, no TURN fallback, its own protocol and a Qt/C++
  client — building on it would mean C++. mediasoup (Node/C++) has no native client SDK. An
  own pion-based SFU is a project of its own.

## Consequences

- S0 proceeds as planned in ADR-001 (Cargo workspace, `xtask`, Tauri 2 + Svelte 5).
- Pinned starting versions: tauri 2.12, livekit 0.9.4, livekit-api 0.8.2, cpal 0.18, axum
  0.8, sqlx 0.9, ts-rs 12, svelte 5.57, vite 8.
- Revisit triggers: plugin hosting becomes a must-have → look at C++/JUCE for `plugin-host`
  only (out-of-process host, so the license stays contained); S6 overruns → PocketBase.
