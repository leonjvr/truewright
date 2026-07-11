## Why

When a test run fails or an app behaves unexpectedly, the page's own `console.log`/`console.warn`/`console.error` output and any uncaught exceptions are often the fastest way to find out why — but today there's no way to see them at all; the only signals `aib` surfaces are the accessibility snapshot and screenshots. This is the first of Phase 3b-iii's remaining tooling pieces (structured trace/log capture, assertions, a YAML runner), and stands alone: capturing what the page logged is useful with or without the other pieces.

## What Changes

- New CDP surface: `Runtime.consoleAPICalled` (fires for every `console.*` call) and `Runtime.exceptionThrown` (uncaught exceptions) — both ride the `Runtime` domain already enabled on every page (from Phase 1's walker/resolve injection), no new `enable` call needed.
- `browser_console_start(name)`/`browser_console_stop()`: captures every console message and uncaught exception between start and stop, persisting them as a named JSON Lines (JSONL) trace — one JSON object per line, in chronological order — under `<data-dir>/aib/traces/<name>.jsonl`.

**Explicitly out of scope (follow-up changes):** a unified action trace (navigate/click/type/press also logged into the same JSONL stream) — a real feature, but a meaningfully bigger one (every `Session` action method needs to conditionally append an entry), kept separate so this change stays focused and independently verifiable; `browser_assert` and the YAML runner (built on top of trace data, not this change's concern); filtering by console level (v1 captures everything, unfiltered — the common "show me what happened" case, not a log-triage tool).

## Capabilities

### New Capabilities
- `console-capture`: capture a page's `console.*` output and uncaught exceptions to a named JSONL trace file.

## Impact

- `crates/cdp/src/protocol/runtime.rs`: `ConsoleApiCalled`/`ExceptionThrown` events (existing protocol module, `Runtime` domain already enabled).
- `crates/cdp/src/ops.rs`: no new command wrappers needed beyond the existing `events::<E>()` subscription mechanism — these are pure event subscriptions.
- `crates/engine/src/console.rs` (new): `ConsoleCapture` (mirrors `NetworkRecording`'s collector-task-plus-persist shape), JSONL trace entry format.
- `crates/engine/src/session.rs`: `Session::console_capture_start(name) -> Result<ConsoleCapture>`.
- `crates/mcp/src/lib.rs`: `browser_console_start(name)`, `browser_console_stop()` tools.
- New fixture (a page that logs at multiple levels and throws an uncaught exception) + integration test asserting the captured JSONL contains exactly what the fixture produced, in order.
