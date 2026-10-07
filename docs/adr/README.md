# Architecture Decision Records

One page per decision: **Context** (forces, options), **Decision**, **Consequences**. Number
them consecutively (`003-short-title.md`). A decision in the "Offene Entscheidungen" table of
the planning doc gets an ADR when it switches to "Entschieden".

| ADR | Title | Status |
| --- | --- | --- |
| 000 | [Grobstruktur v0](../00-grobstruktur.md) — planning baseline (shared with variant A) | accepted (2026-10-01) |
| 001 | [Variant B — same core, Rust-first stack, outside App Hub](001-variante-b-stack.md) | accepted (2026-10-07) |
| 002 | [Rust stays — alternatives checked](002-language-alternatives.md) | accepted (2026-10-07) |

Decided in ADR-000 already: project name yAPPA, public repo with nightly via GitHub Releases
(for this variant: once a remote exists). ADR-001 overrides ADR-000 where it says so (control
plane, UI framework, scripts, work tracking).
