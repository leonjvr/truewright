# Tasks: Console Capture

## 1. CDP protocol

- [ ] 1.1 `crates/cdp/src/protocol/runtime.rs`: `ConsoleApiCalled` event (`kind`/`type`, `args: Vec<RemoteObject>`, `timestamp`), `ExceptionThrown` event (`timestamp`, `exception_details: {text, line_number, column_number, url}`)

## 2. Engine

- [ ] 2.1 `crates/engine/src/console.rs`: `TraceEntry` (console/exception variants, serde-tagged), `ConsoleCapture` (collector task subscribing to both event streams, mirrors `NetworkRecording`'s shape), `ConsoleCaptureSummary`
- [ ] 2.2 Best-effort string rendering of `RemoteObject` args (prefer `.value`, fall back to `.description`), joined with spaces
- [ ] 2.3 `Session::console_capture_start(name) -> Result<ConsoleCapture>`
- [ ] 2.4 Persist to `<data-dir>/aib/traces/<name>.jsonl`

## 3. MCP integration

- [ ] 3.1 `browser_console_start(name)`, `browser_console_stop()` tools; "already in progress" guard mirroring `browser_record_start`

## 4. Verification

- [ ] 4.1 Host: full suite green
- [ ] 4.2 Integration test: a fixture logging at multiple console levels and throwing an uncaught exception produces a JSONL trace with matching entries in chronological order
- [ ] 4.3 Container: `bash docker/run-tests.sh` green

## 5. Wrap-up

- [ ] 5.1 README documents `browser_console_start`/`browser_console_stop` and the JSONL trace format/location
- [ ] 5.2 `openspec validate console-capture` clean; sync specs; archive
