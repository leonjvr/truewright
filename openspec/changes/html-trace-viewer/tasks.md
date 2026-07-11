# Tasks: html-trace-viewer

## 1. `crates/engine/src/console.rs`
- [x] 1.1 `TraceEntry::Screenshot { path: String, timestamp_ms: f64 }` variant
- [x] 1.2 `ActionTraceSink`'s inner value becomes `ActiveTrace { name: String, entries: Arc<Mutex<Vec<TraceEntry>>> }` instead of a bare entry buffer
- [x] 1.3 `save_screenshot(name, bytes) -> Result<PathBuf>`: writes to `<data-dir>/aib/traces/<name>-screenshots/<timestamp_ms>.png`, creating the dir as needed

## 2. `crates/engine/src/session.rs`
- [x] 2.1 `log_action`/`ConsoleCapture::start` updated for the `ActiveTrace` shape
- [x] 2.2 `screenshot()`: when a trace is active, save + log a `Screenshot` entry, best-effort (never fails the call)

## 3. `crates/engine/src/trace_view.rs` (new)
- [x] 3.1 `render_html(entries: &[TraceEntry]) -> Result<String>`: sorts by `timestamp_ms`, renders a self-contained HTML page, color-coded rows per entry kind, screenshots embedded as base64 data URIs
- [x] 3.2 `render_trace_html(name: &str) -> Result<PathBuf>`: loads the trace, renders it, writes `<name>.html` next to the `.jsonl`, returns the path
- [x] 3.3 Export both from `crates/engine/src/lib.rs`

## 4. CLI + MCP
- [x] 4.1 `src/main.rs`: new `Trace { View { name } }` subcommand calling `engine::render_trace_html`
- [x] 4.2 `crates/mcp/src/lib.rs`: new `browser_render_trace(name)` tool, returns the output path as text

## 5. Verification
- [x] 5.1 Unit tests on `render_html` (no file I/O): entries render in sorted order, entry-kind styling present, HTML-escaping, failed-assert styling
- [x] 5.2 Integration test: capture a trace with console/exception/action/screenshot entries against the existing console fixture, render it, assert the HTML contains expected text and an embedded `data:image/png;base64,` screenshot
- [x] 5.3 Manually inspected the rendered HTML visually (published the generated file as a private Artifact) -- confirmed legible dark-theme timeline with correctly color-coded rows and a properly embedded screenshot
- [x] 5.4 `cargo test --workspace` on host (fully green, 3x repeated for the new test) and `bash docker/run-tests.sh` in the container (green)
- [x] 5.5 Manually smoke-tested `aib trace view <name>` against the compiled binary

## 6. Wrap-up
- [x] 6.1 Update README with `aib trace view`/`browser_render_trace` usage
- [x] 6.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 6.3 `openspec archive html-trace-viewer -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
