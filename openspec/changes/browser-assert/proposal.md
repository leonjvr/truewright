## Why

`browser_wait_for` polls for a substring and returns the snapshot once found (or times out) — useful for waiting on an async change, but the wrong shape for "check this is true right now and fail clearly if it isn't," which is what a test's actual pass/fail assertions need. Today an agent has to call `browser_snapshot` and eyeball the text itself to decide pass/fail, with no explicit, structured failure signal. `browser_assert` is that explicit check: immediate (no polling), pass/fail (not text-or-timeout), and fails as a genuine tool-level failure an agent can branch on.

## What Changes

- `browser_assert(text, present?)`: checks the current snapshot for `text` immediately (`present: true`, the default) or checks it's absent (`present: false`); returns a normal success result on pass, and an MCP tool-level error result (`CallToolResult::error`, `is_error: true` — a real failure signal the calling agent sees directly, distinct from a protocol-level error) with a clear expected-vs-actual message on failure.
- When a console/action trace is active, each `browser_assert` call is logged as an action entry recording the assertion and its outcome (pass/fail) — reuses `action-trace`'s existing mechanism, no new tracing infrastructure.

**Explicitly out of scope (follow-up changes):** assertions beyond text presence/absence (element state like `checked`/`disabled`, attribute values, counts) — v1 covers the single most common case; richer assertion kinds are a natural, additive follow-up once real usage shows which ones matter. The YAML runner (consumes `browser_assert` as one of its step kinds, not this change's concern).

## Capabilities

### New Capabilities
- `browser-assert`: an immediate, pass/fail text-presence assertion against the current page snapshot, distinct from `browser_wait_for`'s poll-and-return-text shape.

## Impact

- `crates/engine/src/session.rs`: `Session::assert_text(text, present) -> Result<()>`, returning a typed error with expected-vs-actual detail on failure; logs an action-trace entry either way when a trace is active.
- `crates/engine/src/error.rs`: `EngineError::AssertionFailed { text, present, snapshot_excerpt }`.
- `crates/mcp/src/lib.rs`: `browser_assert(text, present?)` tool, mapping an assertion failure to `CallToolResult::error` rather than a protocol-level `McpError`.
- New integration test: an assertion that should pass does; one that should fail returns a tool-level error result with a clear message, not a panic or a protocol error.
