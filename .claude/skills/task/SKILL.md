---
name: task
description: Work one task from TASKS.md end to end in yAPPA variant B — branch, implement with tests, run the gate, update TASKS/CHANGELOG, commit in logbook format, stop for review. Use when the user says "next task", "mach weiter", names a task from TASKS.md, or asks to start a spike.
---

# Work a task (variant B has no board — TASKS.md is the board)

1. **Pick.** The task the user named, else the top line of "Now", else the top of "Next".
   Read the matching section of `docs/00-grobstruktur.md` and any ADR it touches. If the task
   is a spike, also load the `spike` skill.
2. **Branch.** `git switch -c task/<short-name>` from `main` (or from the branch the user
   names). Never commit to `main`. Move the task line to "Now" in `TASKS.md`.
3. **Plan briefly** (3–6 bullets in chat) when the task touches more than one crate or a
   contract (`engine-protocol`, `api-types`). Contract changes are additive unless an ADR says
   otherwise; bump `PROTOCOL_VERSION` / `API_VERSION` only for breaking ones.
4. **Implement test-first where it is cheap:**
   - pure logic / DSP → unit tests next to the code, property tests (`proptest`) for
     invariants, snapshots (`insta`) for responses and tables;
   - anything in the send chain → a `criterion` bench in `crates/<crate>/benches/` and the
     numbers before/after in the commit message (budget: DSP < 5 ms per 10 ms block);
   - UI → vitest + Testing Library next to the component (`*.test.ts`).

   Generated files (`apps/desktop/src/lib/types`, `tokens.css`) are never edited by hand —
   change the Rust/JSON source and run `cargo xtask gen-types` / `cargo xtask tokens`.
5. **Gate.** `cargo xtask check` must pass. Snapshot diffs: inspect them, then
   `cargo insta accept` only if the change is intended — say so in the commit.
   After changes to engine/DSP/transport crates, run the `realtime-reviewer` agent.
6. **Bookkeeping.** Task line → "Done" with the date; one line in `CHANGELOG.md`
   (Unreleased); an ADR if an architecture decision was made (`adr` skill).
7. **Commit** in logbook format (`.gitmessage`): Why / What / Verified / Follow-up, one
   logical change per commit. "Verified" lists what actually ran — never claim a platform or
   a measurement that did not run.
8. **Stop.** Summarise for review: what changed, what was verified, what was not (e.g.
   macOS, real audio devices). Do not merge or push — the user reviews and merges.
