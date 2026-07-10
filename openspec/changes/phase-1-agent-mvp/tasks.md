# Tasks: Phase 1 — Agent MVP

## 1. `engine` crate scaffolding

- [x] 1.1 `crates/engine/Cargo.toml` + `src/lib.rs`; depends on `cdp`, `tokio`, `serde`, `serde_json`, `thiserror`, `tracing`
- [x] 1.2 `EngineError` taxonomy (`StaleRef`, `ActionTimeout`, `Cdp(cdp::CdpError)`, ...) with `From<cdp::CdpError>`

## 2. Injected walker (page-snapshot spec)

- [x] 2.1 `assets/walker.js`: idempotent `window.__aib` state, role/name computation, visibility check, tree walk skipping non-interactive/non-structural leaves, ref assignment
- [x] 2.2 `assets/resolve.js`: given a ref, return live center-point coordinates + visibility/size or a stale-ref marker
- [x] 2.3 Rust `snapshot::render(tree) -> String`: indented text renderer (role, quoted name, value, checked/disabled/hidden flags, `[ref]`)
- [x] 2.4 `Session::snapshot() -> Result<String>`: evaluate walker via `Runtime.evaluate`, deserialize tree, render

## 3. Actions (browser-actions spec)

- [x] 3.1 `Session::resolve_actionable(ref, timeout) -> Result<Coordinates>`: bounded-poll loop (100ms interval, 5s default deadline, two-stable-reads check) using `resolve.js`
- [x] 3.2 `Session::click(ref)`: resolve, dispatch `Input.dispatchMouseEvent` press+release at center point
- [x] 3.3 `Session::type_text(ref, text, submit: bool)`: click to focus, `Input.insertText`, optional Enter press
- [x] 3.4 `Session::press(key)`: named-key table (Enter, Tab, Escape, ArrowDown, ArrowUp, Backspace) → `Input.dispatchKeyEvent` keyDown/keyUp
- [x] 3.5 `Session::wait_for(text, timeout)`: poll `snapshot()` for substring at bounded interval (250ms default)
- [x] 3.6 `Session::screenshot()`: thin wrapper over `cdp::ops::Page::screenshot`
- [x] 3.7 `Session::navigate(url)`: wrap `cdp::ops::Page::navigate_and_wait`, return fresh snapshot
- [x] 3.8 `Session::close()`: teardown (page close, context dispose, browser shutdown)

## 4. `mcp` crate (mcp-server spec)

- [x] 4.1 `crates/mcp/Cargo.toml`: depends on `rmcp` (default features + `transport-io`), `engine`, `tokio`
- [x] 4.2 `AibTools` struct: `Arc<Mutex<Option<engine::Session>>>` + `ensure_session()` helper
- [x] 4.3 `#[tool_router]` impl: `browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`, `browser_press`, `browser_wait_for`, `browser_screenshot`, `browser_close`
- [x] 4.4 `EngineError -> rmcp::ErrorData` mapping (stale ref / timeout / internal)
- [x] 4.5 `#[tool_handler] impl ServerHandler`: `get_info()` with server name, instructions listing the tool set

## 5. CLI wiring

- [x] 5.1 `aib mcp [--headed]` subcommand in `src/main.rs`: `AibTools::new(headless).serve(rmcp::transport::stdio()).await?.waiting().await?`
- [x] 5.2 Ensure all logging goes to stderr (already the case from Phase 0's `tracing_subscriber` setup) so stdout stays clean MCP JSON-RPC

## 6. Verification

- [x] 6.1 Unit tests: walker role/name heuristics against representative HTML fixtures (via a headless browser, similar harness to Phase 0's integration test)
- [x] 6.2 Integration test: navigate to a local static HTML fixture with a labeled input + button, snapshot, click, type, wait_for, screenshot — skip-if-no-browser marker
- [x] 6.3 Manual end-to-end check: point an MCP-capable agent (or `npx @modelcontextprotocol/inspector`) at `aib mcp` and drive a real page through the tool set; record what worked / didn't in this change

## 7. Wrap-up

- [x] 7.1 Update README status section describing the MCP server and how to configure it in an agent host
- [ ] 7.2 `openspec validate phase-1-agent-mvp` clean; archive after implementation verified
