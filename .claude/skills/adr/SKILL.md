---
name: adr
description: Write an Architecture Decision Record for yAPPA variant B in docs/adr/. Use when a decision changes a layer, a contract, a dependency with license impact, or resolves an entry in the "Offene Entscheidungen" table of the planning doc.
---

# Write an ADR

1. Next number: highest `docs/adr/NNN-*.md` + 1. File `docs/adr/NNN-short-kebab-title.md`.
2. One page, this shape:

   ```markdown
   # ADR-NNN: <decision as a statement>

   Status: proposed | accepted (YYYY-MM-DD) · amends ADR-xxx (if so)

   ## Context
   Forces, constraints, options considered (a small table if more than two).

   ## Decision
   What we do, concretely (crates, versions, seams).

   ## Consequences
   What gets easier, what gets harder, revisit triggers.
   ```

3. Add a row to the table in `docs/adr/README.md`.
4. If it answers an open decision of `docs/00-grobstruktur.md`, do **not** edit the baseline
   (it is shared with variant A) — the ADR is the record. Update `TASKS.md` if it closes an
   open question.
5. Status `accepted` only when the user agreed in this conversation; otherwise `proposed`.
6. Commit as `docs(adr): ADR-NNN — <title>` in logbook format.
