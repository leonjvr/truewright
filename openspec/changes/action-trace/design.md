# Design: Action Trace

## Context

Extends `console-capture` (archived) rather than introducing a new capability — an action trace and a console/exception trace are the same underlying idea ("what happened, in order"), so they belong in one stream, not two separately-started captures a caller would have to correlate by hand.

## Goals / Non-Goals

**Goals:** `Session`'s own action methods append a one-line summary into the active trace, if any, interleaved correctly by real time with console/exception entries; zero overhead when no trace is active.

**Non-Goals:** capturing an action's full effect (before/after snapshot diff) — v1 logs that an action happened and its key argument(s), not its consequences; a way to start an action-only trace without console/exception capture (no stated need — `browser_console_start` already captures everything relevant with one call).

## Decisions

1. **`Session` holds a swappable `Arc<Mutex<Option<Arc<Mutex<Vec<TraceEntry>>>>>>`** — a reference to the currently-active trace's shared entry buffer, or `None`. `console_capture_start` sets it (to the same buffer `ConsoleCapture`'s own collector task writes to); the `ConsoleCapture` value's `stop()` clears it. Action methods lock-and-check this cell before appending, so tracing costs nothing when inactive (one lock check, then done).
2. **`stop()` clears `Session`'s reference, not just its own collector.** Without this, an action logged after `stop()` has already drained (`std::mem::take`) the buffer would append into empty space nobody ever reads again -- clearing the reference stops that from mattering (a post-stop action call sees `None` and simply doesn't log).
3. **A narrow race at the exact stop boundary is accepted, not engineered away.** An action call racing exactly against `stop()` could still land its entry in the buffer moments before `mem::take` runs (fine, captured) or moments after (silently dropped, since the fresh post-take buffer is never read again). This is a debugging/observability trace, not correctness-critical data, and eliminating the race entirely would need a full stop-the-world barrier for marginal benefit -- same "no more sophistication than obviously needed" stance every other capture in this project has taken.
4. **Timestamps are epoch-milliseconds on both sides, no conversion needed.** CDP's `Runtime.consoleAPICalled`/`exceptionThrown` timestamps are already "milliseconds since epoch" per the CDP protocol's own definition; `Session`'s action entries use `SystemTime::now()` converted the same way, so both sources sort correctly by real wall-clock time without any offset reconciliation.
5. **Action entries are one-line text summaries, not structured argument objects.** `click e6`, `type e6 "hello@example.com"` -- readable at a glance in the JSONL, and consistent with how console-capture already renders console arguments as best-effort strings rather than preserving full structure.

## Risks / Trade-offs

- [The stop-boundary race (Decision #3) could silently drop one action entry in rare timing] → acceptable; documented rather than engineered away, matching every other capture's stance in this project.
- [Action summaries don't include the action's *result*] → acceptable for v1; a caller who needs that can call `browser_snapshot` after the traced action, same as today.

## Migration Plan

Purely additive to the existing `console-capture` capability -- no new MCP tools, no new parameters. A `browser_console_start` call today automatically starts including action entries once this ships.

## Open Questions

None blocking.
