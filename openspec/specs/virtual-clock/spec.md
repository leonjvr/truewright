# virtual-clock Specification

## Purpose
An agent-controlled virtual clock, so time-dependent behavior (session timeouts, debounced/throttled handlers, staggered animations) becomes both instant and deterministic instead of either slow (waiting for real time) or untested (skipped entirely). Time is frozen unless explicitly advanced, and advancing fires every due callback in chronological order — one mechanism that covers both "leave time frozen" and "tick time forward on demand" usage.
## Requirements
### Requirement: Agent-controlled virtual clock
The engine SHALL support installing a virtual clock frozen at a caller-supplied time, overriding `Date`, `Date.now`, and `performance.now` to read from it, such that time only appears to pass when explicitly advanced.

#### Scenario: Date.now reflects the installed virtual time
- **WHEN** the clock is set to a specific epoch time and the page reads `Date.now()`
- **THEN** it observes exactly that time, not the real wall-clock time

#### Scenario: Time does not pass on its own
- **WHEN** the clock is installed and real wall-clock time elapses without any explicit advance
- **THEN** `Date.now()` continues to report the same, unchanged virtual time

### Requirement: Explicit clock advancement fires due timers in order
The engine SHALL support advancing the virtual clock by a caller-supplied duration, synchronously firing every `setTimeout`/`setInterval`/`requestAnimationFrame` callback whose scheduled time falls within the advanced window, in chronological order, including callbacks newly scheduled by other callbacks firing within the same advance.

#### Scenario: A delayed callback does not fire before its time
- **WHEN** a `setTimeout` is scheduled with a delay longer than any subsequent clock advance
- **THEN** its callback has not fired

#### Scenario: Advancing past a delay fires the callback
- **WHEN** the clock is advanced by at least as much as a pending `setTimeout`'s delay
- **THEN** that callback fires exactly once

#### Scenario: A callback chain scheduled within the same advance still fires
- **WHEN** a callback fires during an advance and itself schedules a new `setTimeout` with a delay that still falls within the same advance's window
- **THEN** the newly scheduled callback also fires during that same advance

