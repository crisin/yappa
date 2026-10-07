# apps/desktop — yAPPA desktop client

Tauri 2 shell (`src-tauri/`) + Svelte 5 UI (`src/`). The shell owns windows and OS
integration and forwards [`engine-protocol`](../../crates/engine-protocol) messages; all audio
lives in the Rust engine crates. The UI talks to the engine only through `src/lib/engine.ts`.

From the repo root, `cargo xtask dev` / `cargo xtask build` are the usual entry points. Inside
this folder the same works with npm:

| Script                            | Does                                                           |
| --------------------------------- | -------------------------------------------------------------- |
| `npm run app:dev`                 | desktop app with hot reload (`tauri dev`, Vite on :1420)       |
| `npm run app:build`               | production build + installers → `../../target/release/bundle/` |
| `npm run app:build:debug`         | installers from the debug profile                              |
| `npm run dev`                     | UI only in a browser on :1420; `engine.ts` answers with a mock |
| `npm run build` / `preview`       | Vite build of the UI into `dist/` / serve it                   |
| `npm run check`                   | svelte-check (types), warnings fail                            |
| `npm test` / `test:watch`         | vitest + Testing Library (jsdom)                               |
| `npm run format` / `format:check` | prettier                                                       |
| `npm run tauri -- <cmd>`          | raw Tauri CLI (`icon`, `info`, …)                              |

## Layout

```
src/
  App.svelte          one window: channels, stage, transmit/mute bar
  app.css             layout defaults — colors/spacing only via tokens
  lib/engine.ts       the only door to the engine (Tauri invoke, browser mock)
  lib/types/          GENERATED from Rust (cargo xtask gen-types) — do not edit
  lib/tokens.css      GENERATED from ui-tokens/ (cargo xtask tokens) — do not edit
  **/*.test.ts        vitest
src-tauri/
  src/lib.rs          Tauri commands: engine_info, get_settings, engine_command (stub until S1)
  tauri.conf.json     window, CSP, bundle (version comes from the workspace Cargo.toml)
  capabilities/       Tauri permissions (core only)
  icons/              placeholder icon set — regenerate with `npm run tauri -- icon <1024px.png>`
```

The app identifier is `dev.crisin.yappa-b`, so variant B installs next to variant A.
