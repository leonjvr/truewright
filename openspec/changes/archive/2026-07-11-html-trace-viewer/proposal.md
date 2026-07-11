## Why

Saved traces (`browser_console_start`/`stop`) are JSONL -- accurate and greppable, but not something a human wants to read directly to understand "what happened during this test run." `aib`'s own originally-envisioned CLI surface (`daemon | mcp | run | trace | doctor`) named `trace` as a first-class subcommand; today only `doctor` and `mcp` exist. This change adds it: a way to render an already-captured trace as a single, self-contained HTML page.

## What Changes

- `TraceEntry` gains a `Screenshot { path, timestamp_ms }` variant. `Session::screenshot()` now saves the PNG to `<data-dir>/aib/traces/<name>-screenshots/<timestamp_ms>.png` and logs it into the active trace, when one is active -- best-effort (a save/log failure never fails the `screenshot()` call itself, matching this project's existing "tracing costs nothing when it can't help" posture for the action trace). Screenshots were the one entry kind genuinely worth adding for a *viewer* specifically: console/exception/action entries are already useful as plain text, but "what did the page actually look like at this moment" is something only an image answers.
- `ActionTraceSink`'s inner value gains the trace's own `name` alongside its entry buffer (previously bare `Arc<Mutex<Vec<TraceEntry>>>`), since `screenshot()` needs to know which trace's screenshot directory to save into.
- New `crates/engine/src/trace_view.rs`: renders a trace's entries as one self-contained HTML file -- chronologically sorted (explicit sort by `timestamp_ms`, not relying on push order, since console/exception entries arrive via an async collector task racing against directly-called action/screenshot logging), color-coded by entry kind, screenshots embedded inline as base64 data URIs (not `file://` references -- self-contained, portable, works regardless of how the HTML file is later opened or moved).
- `engine::render_trace_html(name)`: loads a saved trace, renders it, writes `<name>.html` next to the `.jsonl` file, returns the output path -- the one function both the CLI and the MCP tool below call into.
- New `aib trace view <name>` CLI subcommand.
- New MCP tool `browser_render_trace(name)`, returning the output path as text (not the HTML content itself -- keeps agent context usage in line with this project's existing screenshot/recording tools, which return a path/one-frame rather than a large blob).

**Explicitly out of scope (deferred), and why:**
- **Network/snapshot entry types.** Network traffic is already covered by the separate network-mocking cassette feature (recording it again into the console/action trace would be redundant); full DOM snapshots are large and not clearly more useful in an HTML view than the existing action-text summaries already are. Screenshots earn their place because they answer a question nothing else in the trace can (what did it *look* like).
- **A live/streaming viewer.** This renders an already-completed, saved trace after the fact -- not a real-time dashboard for a trace still being captured.
- **Trace-entry pruning/rotation for the screenshot directory.** Screenshots accumulate on disk per trace, same as the trace file itself already does; no cleanup mechanism exists for either today, and this change doesn't add one for screenshots specifically.

## Capabilities

### New Capabilities
- `html-trace-viewer`: renders a saved console/action trace as a single self-contained HTML timeline, with `browser_screenshot` calls captured inline when a trace is active.

## Impact

- `crates/engine/src/console.rs`: new `TraceEntry::Screenshot` variant; `ActionTraceSink` restructured to carry the trace name.
- `crates/engine/src/session.rs`: `screenshot()` saves + logs when a trace is active.
- `crates/engine/src/trace_view.rs` (new): HTML rendering.
- `crates/engine/src/lib.rs`: export `render_trace_html`.
- `src/main.rs`/new `src/trace.rs`: `aib trace view <name>` subcommand.
- `crates/mcp/src/lib.rs`: new `browser_render_trace(name)` tool.
- No CDP protocol changes, no real-OS side effects -- headless verification.
