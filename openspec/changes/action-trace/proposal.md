## Why

`console-capture` records what the page said (console output, exceptions) but not what the agent *did* to provoke it — reconstructing a failure from a console trace alone still means guessing which action happened at which point in the timeline. Interleaving the agent's own actions (navigate/click/type/press/wait_for) into the same chronological JSONL stream turns it into a genuine "what happened" trace: both sides of the interaction, in one place, in order.

## What Changes

- `Session`'s action methods (`navigate`, `click`/`click_with`, `type_text`/`type_text_with`, `press`) log a one-line summary entry (e.g. `click e6`, `type e6 "hello@example.com"`, `press Enter`, `navigate https://...`) into the currently-active console trace, if one is active — a no-op with no overhead when no trace is active, same as today.
- `TraceEntry` (from `console-capture`) gains an `Action` variant, tagged `"action"` in the JSONL output, using the same epoch-millisecond timestamp convention as the existing `console`/`exception` entries (CDP's own timestamps are already epoch-ms, so both sources interleave correctly by real time without any conversion).
- This is additive to the existing `browser_console_start`/`browser_console_stop` tools — no new MCP surface, no new parameters. A capture started today already starts including actions once this change ships.

**Explicitly out of scope (follow-up changes):** `browser_assert` and the YAML runner (build on top of this trace format, not this change's concern); capturing action *results* (e.g. the resulting snapshot diff) — v1 logs that an action happened and its arguments, not its full before/after effect, matching `console-capture`'s own "no more sophistication than obviously needed" scope.

## Capabilities

### Modified Capabilities
- `console-capture`: adds an `Action` trace-entry kind, populated by `Session`'s own action methods when a trace is active, interleaved chronologically with the existing console/exception entries.

## Impact

- `crates/engine/src/console.rs`: `TraceEntry::Action { text, timestamp_ms }`; `ConsoleCapture` exposes its entry buffer as a shareable sink so `Session` can append to it directly.
- `crates/engine/src/session.rs`: `Session` holds a swappable reference to "the active trace's sink, if any," set by `console_capture_start` and cleared by the returned `ConsoleCapture`'s `stop()`; `navigate`/`click_with`/`type_text_with`/`press` append a summary entry when a sink is set.
- New integration test: start a trace, perform a `navigate` + `click` + `type`, stop the trace, assert the JSONL contains action entries for each, correctly interleaved by timestamp with any console output the actions provoked.
