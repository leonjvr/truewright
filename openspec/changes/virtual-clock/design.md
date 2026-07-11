# Design: Virtual Clock

## Context

Third of Phase 3b's determinism primitives (`deterministic-init` shipped init scripts + seeded randomness first). The virtual clock reuses `deterministic-init`'s init-script delivery mechanism entirely — no new CDP surface — but is a meaningfully bigger piece of injected JS because it has to reimplement enough of the browser's own timer scheduling (`setTimeout`/`setInterval`/`requestAnimationFrame`) to be a correct, if intentionally simple, fake-timers implementation.

## Goals / Non-Goals

**Goals:** `Date`/`performance.now`/timers all read from one agent-controlled virtual clock; `browser_advance_clock` fires every due callback in correct chronological order, including callbacks scheduled by other callbacks firing within the same advance; verified against a real fixture, not just unit-tested JS logic.

**Non-Goals:** resuming real wall-clock time after installing a virtual clock; sub-millisecond ordering guarantees; `requestIdleCallback`, `queueMicrotask`, or `MutationObserver` timing (real browsers don't tie these to timer scheduling in a way a test would usually need to control); a separate "modes" concept (see Decision #1).

## Decisions

1. **"Both clock modes" is one mechanism: frozen-unless-advanced.** The clock only moves when `browser_advance_clock` is called. A session that never calls it gets frozen-time behavior for free (every `Date.now()` call returns the same value, no timer ever fires); a session that calls it whenever it wants a UI transition to happen gets tick-mode behavior. Building these as two separate code paths would duplicate the entire timer-queue logic for no behavioral gain — a single "advance by N, fire what's due" primitive already expresses both usage patterns.
2. **`setTimeout`/`setInterval` are replaced with a queue, not left running against real time.** Real timers firing on real wall-clock time while `Date.now()` reports a frozen/virtual value would immediately desync app behavior from what it reads (a `setTimeout(() => console.log(Date.now()), 1000)` firing for real after 1 real second but logging a virtual timestamp that hasn't moved). Timers must be queued against virtual time and fired only by `browser_advance_clock`.
3. **`browser_advance_clock` fires callbacks by repeatedly finding-and-firing the single earliest still-due timer, not by firing all currently-queued due timers in one pass.** A one-pass approach would miss a `setTimeout(fn2, 0)` scheduled by `fn1` firing earlier in the *same* advance, even though `fn2`'s target time is still within the advanced window. The find-earliest-fire-repeat loop naturally picks up newly-queued timers as long as their target time is `<= ` the advance's end time, matching real timer-chain behavior (e.g. a recursive `setTimeout(tick, 0)` loop advances correctly).
4. **`Date` is replaced with a plain wrapper function (`FakeDate`), not a `Proxy`.** A `Proxy` handles the general case more "automatically" but adds real complexity (trap semantics for `construct`/`get`/`set` all need to be correct) for no benefit here — a wrapper function using the well-known `Function.prototype.bind.apply` idiom to forward arbitrary constructor arguments to the real `Date`, plus `FakeDate.prototype = Date.prototype` (so `instanceof Date` still works against either reference), covers every case an app actually exercises with far less surface area to get wrong.
5. **`requestAnimationFrame` is modeled as a ~16.67ms (60fps) timer, not tied to actual paint.** There's no real paint to tie it to in a virtual-time world; treating it as "fires roughly once per simulated frame interval" is the simplest model that still lets `browser_advance_clock` drive frame-based animation logic deterministically.

## Risks / Trade-offs

- [An app that reads real wall-clock time through an API this change doesn't override (e.g. a `Date` reference captured *before* the init script runs, or a Web Worker's own timer scope) stays on real time] → inherent to any injected-override approach; documented, not silently pretended away. Init scripts run before the *main-world* page scripts, which covers the overwhelming majority of real app code.
- [`requestAnimationFrame`'s fixed ~16.67ms model doesn't match a real display's actual refresh rate] → acceptable; the goal is deterministic, controllable animation-driven logic for testing, not frame-perfect timing fidelity.

## Migration Plan

Purely additive — two new MCP tools, no existing tool's behavior changes. A session that never calls `browser_set_clock` is completely unaffected (real `Date`/timers, as today).

## Open Questions

None blocking.
