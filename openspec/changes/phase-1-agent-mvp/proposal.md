## Why

Phase 0 proved the CDP layer works (attach, navigate, evaluate, screenshot, p50 < 5ms on this machine). ai-browser's actual value proposition — an LLM agent driving a real browser through MCP, without Playwright's footprint — doesn't exist yet: there is no MCP server, no page representation an agent can reason over, and no way to act on a page (click/type) at all. Phase 1 builds the smallest end-to-end slice that makes ai-browser usable by an agent: navigate, see a token-efficient snapshot of the page, click and type by reference, wait for a condition, and screenshot on demand — all as native MCP tools over stdio.

## What Changes

- New `crates/engine`: a session layer on top of `cdp` that owns one browser + one active page, injects a DOM/ARIA walker script (main world, not yet isolated-world — see design.md for the scoped-down tradeoff) to produce accessibility-style snapshots with stable `eN` refs, and exposes navigate/snapshot/click/type/press/wait_for/screenshot/close operations.
- New `crates/mcp`: an MCP server (built on the official `rmcp` crate) exposing `browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`, `browser_press`, `browser_wait_for`, `browser_screenshot`, `browser_close` as tools over stdio, backed by a single lazily-created `engine::Session`.
- New `aib mcp` CLI subcommand that runs the MCP server on stdio (auto-launches/attaches a browser on first tool call).
- Ref-based actionability: before acting on a ref, the engine resolves it to live coordinates and retries (bounded poll, not yet the fully event-driven MutationObserver gate from PROPOSAL.md §5) until visible and stable, or times out with a structured error.

**Explicitly out of scope for this change** (later phases per PROPOSAL.md roadmap): human-motion input timing/curves (Phase 2), network mocking/clock/record-replay/trace export/YAML runner (Phase 3), true-user OS input (Phase 4), multi-tab/multi-session daemon architecture, isolated-world injection, diff-based `changes:` in action responses, and full MutationObserver-based actionability. These are named explicitly so this change isn't held to a bar it doesn't claim to meet.

## Capabilities

### New Capabilities
- `page-snapshot`: injected walker producing a token-efficient, ref-addressable text snapshot of a page (role/name/state), plus ref resolution and staleness handling.
- `browser-actions`: click/type/press/wait_for/screenshot operations addressed by ref, with bounded-poll actionability and structured failure reasons.
- `mcp-server`: the stdio MCP server surface — tool set, session lifecycle (lazy create, explicit close), and error mapping from engine errors to MCP tool errors.

### Modified Capabilities
_None — `browser-attach` and `cdp-client` are consumed as-is from Phase 0; no requirement changes._

## Impact

- New crates: `crates/engine`, `crates/mcp` (workspace members).
- New dependency: `rmcp` 2.x (`server`, `macros`, `transport-io` features) — the official Rust MCP SDK.
- `src/main.rs` gains an `mcp` subcommand alongside `doctor`.
- Builds on `cdp::launch` and `cdp::ops` from Phase 0 unchanged.
- No breaking changes to Phase 0 surfaces.
