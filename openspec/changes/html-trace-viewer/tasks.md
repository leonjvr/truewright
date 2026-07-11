# Tasks: html-trace-viewer

## 1. `crates/engine/src/console.rs`
- [ ] 1.1 `TraceEntry::Screenshot { path: String, timestamp_ms: f64 }` variant
- [ ] 1.2 `ActionTraceSink`'s inner value becomes `ActiveTrace { name: String, entries: Arc<Mutex<Vec<TraceEntry>>> }` instead of a bare entry buffer
- [ ] 1.3 `save_screenshot(name, bytes) -> Result<PathBuf>`: writes to `<data-dir>/aib/traces/<name>-screenshots/<timestamp_ms>.png`, creating the dir as needed

## 2. `crates/engine/src/session.rs`
- [ ] 2.1 `log_action`/`ConsoleCapture::start` updated for the `ActiveTrace` shape
- [ ] 2.2 `screenshot()`: when a trace is active, save + log a `Screenshot` entry, best-effort (never fails the call)

## 3. `crates/engine/src/trace_view.rs` (new)
- [ ] 3.1 `render_html(entries: &[TraceEntry]) -> Result<String>`: sorts by `timestamp_ms`, renders a self-contained HTML page, color-coded rows per entry kind, screenshots embedded as base64 data URIs
- [ ] 3.2 `render_trace_html(name: &str) -> Result<PathBuf>`: loads the trace, renders it, writes `<name>.html` next to the `.jsonl`, returns the path
- [ ] 3.3 Export both from `crates/engine/src/lib.rs`

## 4. CLI + MCP
- [ ] 4.1 `src/main.rs`: new `Trace { View { name } }` subcommand (or similar shape) calling `engine::render_trace_html`
- [ ] 4.2 `crates/mcp/src/lib.rs`: new `browser_render_trace(name)` tool, returns the output path as text

## 5. Verification
- [ ] 5.1 Unit tests on `render_html` (no file I/O): entries render in sorted order, entry-kind styling present
- [ ] 5.2 Integration test: capture a trace with console/action/screenshot entries against a real fixture, render it, assert the HTML contains expected text and an embedded `data:image/png;base64,` screenshot
- [ ] 5.3 Manually inspect the rendered HTML visually (e.g. publish the generated file as a private Artifact during verification) to confirm it's actually legible, not just "contains the right substrings"
- [ ] 5.4 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container

## 6. Wrap-up
- [ ] 6.1 Update README with `aib trace view`/`browser_render_trace` usage
- [ ] 6.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 6.3 `openspec archive html-trace-viewer -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
