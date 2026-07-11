## Why

Time-dependent UI (session-timeout warnings, debounced/throttled handlers, staggered animations, "5 seconds ago"-style relative timestamps) is a routine source of test flakiness: a real-wall-clock test either waits (slow, and still racy near the boundary) or skips the behavior entirely. `deterministic-init` already delivered init-script injection and seeded randomness as Phase 3b's first two determinism primitives; this change delivers the third — a virtual clock the agent explicitly controls, so time-dependent behavior becomes both instant and deterministic instead of either slow or untested.

## What Changes

- `browser_set_clock(time_ms)`: installs a virtual clock frozen at the given epoch time, via the same `add_init_script` mechanism `deterministic-init` introduced. Overrides `Date`/`Date.now`/`performance.now` to read the virtual clock, and `setTimeout`/`setInterval`/`requestAnimationFrame` to queue callbacks against virtual time instead of firing on real wall-clock time.
- `browser_advance_clock(ms)`: moves the virtual clock forward by `ms` and synchronously fires every timer/interval/animation-frame callback whose scheduled (virtual) time falls within the advance, in chronological order — including callbacks newly scheduled by earlier callbacks firing within the same advance (e.g. a `setTimeout` chain).
- PROPOSAL.md's "both clock modes" turns out to be one mechanism, not two: a session that never calls `browser_advance_clock` behaves as "frozen mode" (time never moves), and one that calls it repeatedly behaves as "tick mode" (time moves exactly when and how much the agent says) — no separate implementation needed for each; see design.md.

**Explicitly out of scope (follow-up changes):** console capture, JSONL traces, `browser_assert`, and the YAML runner (Phase 3b-ii's remaining, independent pieces); resuming *real* wall-clock time after installing a virtual clock (not a stated need — a session that wants real time simply never calls `browser_set_clock`); sub-millisecond timer ordering guarantees beyond "fires in scheduled order" (browsers themselves don't guarantee tighter than that).

## Capabilities

### New Capabilities
- `virtual-clock`: an agent-controlled virtual clock (`Date`/`performance.now`/`setTimeout`/`setInterval`/`requestAnimationFrame`) that only advances when explicitly told to, firing due callbacks in chronological order.

## Impact

- `crates/engine/assets/virtual_clock.js`: the injected clock/timer override, installed via `add_init_script` (reuses `deterministic-init`'s delivery mechanism, no new CDP surface).
- `crates/engine/src/session.rs`: `Session::set_clock(time_ms)`, `Session::advance_clock(ms)` (the latter calls the init script's exposed `window.__aibAdvanceClock` via `Runtime.evaluate` — an active, at-any-time call, not just at page-load).
- `crates/mcp/src/lib.rs`: `browser_set_clock(time_ms)`, `browser_advance_clock(ms)` tools.
- New fixture + integration test: a `setTimeout`-scheduled DOM change that doesn't appear until the clock is advanced past its delay, and a `setInterval` chain that fires the expected number of times for a given advance.
