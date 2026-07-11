# Tasks: Browser Assert

## 1. Engine

- [ ] 1.1 `crates/engine/src/error.rs`: `EngineError::AssertionFailed { text, present, snapshot_excerpt }`
- [ ] 1.2 `Session::assert_text(text: &str, present: bool) -> Result<()>`: takes a fresh snapshot, checks containment, logs an action-trace entry (pass or fail) when a trace is active, returns `Ok(())` or the typed error

## 2. MCP integration

- [ ] 2.1 `browser_assert(text, present?)` tool; on `EngineError::AssertionFailed`, return `CallToolResult::error(...)` with a clear message instead of the usual `map_engine_err` -> `McpError` path

## 3. Verification

- [ ] 3.1 Host: full suite green
- [ ] 3.2 Integration test: a passing assertion succeeds; a failing assertion returns a tool-level error result (not a panic, not a protocol error) with a message identifying the expected text
- [ ] 3.3 Integration test: an assertion (pass and fail) is logged into an active trace as an action entry
- [ ] 3.4 Container: `bash docker/run-tests.sh` green

## 4. Wrap-up

- [ ] 4.1 README documents `browser_assert` and how it differs from `browser_wait_for`
- [ ] 4.2 `openspec validate browser-assert` clean; sync specs; archive
