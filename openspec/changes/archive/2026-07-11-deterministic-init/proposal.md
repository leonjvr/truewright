## Why

Everything injected into a page so far (`walker.js`, `resolve.js`, `train.js`) runs via `Runtime.evaluate` — *after* the page has already loaded and its own scripts have already run. That's fine for reading page state, but it's the wrong tool for making a page's own behavior deterministic: an app that calls `Math.random()` while initializing (generating an ID, picking a variant, seeding an animation) has already done so by the time any `Runtime.evaluate` call could intervene. PROPOSAL.md's Phase 3 names "init scripts" and "seeded randomness" as separate determinism primitives; this change delivers both together because seeded randomness is really just the first (and simplest) real consumer of the init-script delivery mechanism.

## What Changes

- New CDP surface: `Page.addScriptToEvaluateOnNewDocument`, which registers a script that runs before any of a page's own scripts, on every subsequent navigation — not used anywhere in this codebase today (everything so far runs after load, via `Runtime.evaluate`).
- `browser_add_init_script(source)`: registers arbitrary JS to run before every subsequent page load in this session.
- `browser_seed_randomness(seed)`: a convenience built on `add_init_script` that overrides `Math.random` with a small deterministic PRNG seeded from the given value, so an app's own use of `Math.random()` during page initialization becomes reproducible run to run.

**Explicitly out of scope (follow-up changes):** the virtual clock (`Date`/`setTimeout`/`setInterval`/`requestAnimationFrame` overrides plus an explicit time-advance API) — its own change, substantially more complex than an init script or a PRNG override since it effectively means reimplementing part of the JS timer event loop in an injected script, and bundling it here risks a change too large to finish and verify cleanly in one pass; console capture, JSONL traces, `browser_assert`, and the YAML runner (observability/tooling built on top of a deterministic environment, not the environment itself); removing/clearing a previously registered init script (not a stated need for v1 — scripts persist for the session's lifetime, same as the load-once walker/resolve scripts already do).

## Capabilities

### New Capabilities
- `deterministic-init`: register JS that runs before every page load in a session (not after, unlike existing `Runtime.evaluate`-based injection); a built-in seeded-`Math.random` override as the first concrete use of it.

## Impact

- `crates/cdp/src/protocol/page.rs`: `AddScriptToEvaluateOnNewDocument`/`AddScriptToEvaluateOnNewDocumentParams`/`Response` (new command, existing protocol module).
- `crates/cdp/src/ops.rs`: `Page::add_init_script(source) -> Result<String>` (returns the CDP-assigned identifier).
- `crates/engine/assets/seeded_random.js`: the injected PRNG override script (small, deterministic, seed-parameterized).
- `crates/engine/src/session.rs`: `Session::add_init_script(source)`, `Session::seed_randomness(seed)`.
- `crates/mcp/src/lib.rs`: `browser_add_init_script(source)`, `browser_seed_randomness(seed)` tools.
- New fixture + integration test proving injection order (an init script's effect is visible to the page's own first-run code, not just observable afterward) and seeded-randomness reproducibility (same seed -> identical `Math.random()` sequence across separate navigations; different seeds -> different sequences).
