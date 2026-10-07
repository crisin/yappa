---
name: spike
description: Run and document a spike (S1–S6) for yAPPA variant B — a time-boxed experiment with measurements, result in docs/spikes/. Use together with the task skill when the task is a spike.
---

# Spike

A spike answers a question; it does not build the product. Code may live in a scratch crate
(`crates/spike-<name>`, `publish = false`) and be deleted later — the **result document** is
what must last.

1. Write the question(s) first, from `docs/spikes/README.md` and the planning doc.
2. Time-box: 1–3 days of work. If the box is blown, stop and report — no gold-plating.
3. Measure the same things variant A measures, so both can be compared:
   - latency per 10 ms block (criterion or instrumented timing), CPU per stage;
   - S1: mouth-to-ear via loopback click (two clients), behaviour at 5/10/20 % packet loss,
     reconnect after cable pull / Wi-Fi switch;
   - always record: OS, CPU, build profile (`--release`), audio device, buffer size.
4. Result in `docs/spikes/sN-<name>.md`:

   ```markdown
   # SN: <name> — result

   Date · machine · commit

   ## Question
   ## Setup
   ## Measurements        (table: what, value, method)
   ## What worked / what did not
   ## Decision it feeds   (→ ADR-NNN if any)
   ## Follow-up tasks     (also added to TASKS.md)
   ```

5. Never invent or extrapolate numbers. A measurement that could not run (no second machine,
   no microphone) is written down as "not measured" with the reason.
