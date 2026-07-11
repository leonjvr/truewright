# Design: YAML Runner

## Context

The last piece of Phase 3 (Determinism). Everything else shipped so far makes a single test run reproducible; this makes a test *definable* as a standalone artifact and *replayable* from a captured trace, tying the whole phase together.

## Goals / Non-Goals

**Goals:** a readable, minimal YAML step format covering the existing action set; fail-fast execution reporting which step failed; converting a captured action trace back into a runnable script.

**Non-Goals:** `human_like`/trained-profile/seeded parameters on YAML steps (v1 covers instant dispatch only); a standalone CLI runner (MCP tool is the primary interface, consistent with every other capability); branching, retries, or loops in the script format (a flat ordered step list, not a scripting language).

## Decisions

1. **Steps are externally-tagged, single-key YAML maps** (`- navigate: "..."`, `- type: {ref: e6, text: "..."}`) — this is serde's *default* enum representation for a `#[serde(rename_all = "snake_case")]` enum with a mix of tuple and struct variants, so no custom (de)serialization code is needed; it also reads naturally and matches established step-list conventions (GitHub Actions, Ansible) rather than inventing new YAML idioms.
2. **Fail-fast, not collect-all-failures.** A script is a sequence of steps building on each other's state (a later `click` typically depends on an earlier `navigate` having succeeded) — continuing past a failed step would mostly just produce a cascade of confusing secondary failures. `run_yaml` stops at the first failing step and reports which one and why, matching how a normal imperative test would behave on an unhandled assertion failure.
3. **`serde_yaml`, despite no longer being actively maintained upstream, chosen anyway.** It remains the most widely-used YAML crate in the Rust ecosystem for exactly this straightforward structural (de)serialization use case, with a large surface of real-world usage and no outstanding correctness concerns for the feature set this needs (no custom tags, no exotic YAML 1.1 features). A maintained fork (`serde_yml`) exists if this ever needs revisiting; not switching preemptively for a dependency that already does the job.
4. **The exporter reads `action` entries only, skipping `console`/`exception`.** Console output and exceptions are observability, not steps to replay — exporting them as script steps would be meaningless (there's no "do a console.log" action). The exporter maps each action entry's already-existing text summary (e.g. `"click e6"`, `"type e6 \"hello\""`) back into a `Step`, reusing the exact same summary format `action-trace` already produces rather than inventing a second serialization for the same data.
5. **No ref-stability guarantee beyond what already exists.** A YAML script (whether hand-written or exported) references walker-assigned refs (`e6`, etc.), exactly like a live `browser_click` call already does — refs are deterministic for a structurally-stable page (assigned by tree-walk order) but aren't guaranteed stable across a page's own code changes. This is an existing, already-documented property of ref-based actions (Phase 1), not a new limitation this change introduces.

## Risks / Trade-offs

- [`serde_yaml` is unmaintained upstream] → accepted; still the most stable, widely-deployed option for this exact use case, and the risk is low for straightforward structural parsing with no custom tag/exotic-feature needs. Revisit only if a real problem surfaces.
- [Exported YAML re-uses ref values captured at record time, which could drift if the app's DOM structure changes] → inherent to ref-based actions generally, not specific to export; same caveat already applies to a human hand-writing a YAML script with refs from a snapshot they looked at once.

## Migration Plan

Purely additive — two new MCP tools, one new dependency (engine crate only, doesn't touch the release binary's other dependency surfaces), no existing tool's behavior changes.

## Open Questions

None blocking.
