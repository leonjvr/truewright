# Design: Phase 1 — Agent MVP

## Context

Phase 0 delivered a hand-rolled CDP client (`crates/cdp`) with a validated attach→navigate→evaluate→screenshot cycle. Phase 1 turns that into something an LLM agent can actually drive: an MCP server exposing navigate/snapshot/click/type/wait_for. PROPOSAL.md's full vision (isolated-world injection, MutationObserver-driven actionability, diff-based `changes:`, daemon+multi-session architecture) is intentionally not all built here — this change scopes down to the smallest slice that is genuinely end-to-end and honestly documents what's deferred, consistent with how Phase 0 scoped itself down from the full CDP domain surface.

## Goals / Non-Goals

**Goals**
- A real MCP server an agent host can point at (`aib mcp`, stdio) that can navigate a page, get a usable snapshot, and act on it (click/type/press/wait_for), with a screenshot escape hatch.
- Token-efficient, ref-addressable snapshots good enough to actually drive a form, not a proof-of-concept stub.
- Structured errors (stale ref, actionability timeout) instead of opaque failures.

**Non-Goals (explicitly deferred, per PROPOSAL.md's phase list)**
- Isolated-world injection (main world is used; a page's own JS could in principle observe our walker's globals — acceptable for Phase 1, revisited in hardening).
- Event-driven (MutationObserver/rAF-pair) actionability — Phase 1 uses bounded polling.
- Diff-based `changes:` embedded in action responses — Phase 1 requires an explicit `browser_snapshot` call after actions that might change the page.
- Multi-tab / multi-session daemon with browser contexts per session — Phase 1 is one browser, one context, one page, one MCP server process.
- Human-motion timing/curves — clicks and typing are dispatched immediately (Phase 2).

## Decisions

1. **Single-process MCP server owns the browser directly; no separate daemon.**
   PROPOSAL.md's daemon+stdio-shim split exists to let multiple MCP clients share one browser and to survive individual client disconnects. Neither matters yet with one agent driving one page. The MCP server process itself is the long-lived process holding the CDP connection — simplest thing that could possibly work, and the `engine::Session` API is written so a future daemon can wrap multiple `Session`s without changing this crate's shape.

2. **Walker runs in the main world via `Runtime.evaluate`, re-injected (idempotently) on every call.**
   True isolated-world injection (`Page.createIsolatedWorld` + tracking its `executionContextId` across the page's lifetime, recreated per navigation) is real work for marginal Phase 1 benefit — nothing yet needs immunity from page script. The walker script is idempotent (`window.__aib = window.__aib || {...}`), so re-evaluating it every snapshot/action call is both simple and correct; the ref map persists across calls because it lives on `window`, and naturally resets on navigation because navigation replaces `window`.

3. **Actionability is bounded polling (100ms interval, 5s deadline, two-stable-reads check), not MutationObserver.**
   The full spec (PROPOSAL.md §5) computes stability across two `requestAnimationFrame`s via an injected observer with zero polling. Building that plus the ref/action machinery in one change is too much surface for Phase 1. Bounded polling is honest, testable, and correct for the vast majority of pages; the design explicitly flags this as a Phase 5 hardening item rather than silently under-delivering on the original design.

4. **Typing uses `Input.insertText`, not per-keystroke `Input.dispatchKeyEvent`.**
   Per-key dispatch is what Phase 2's human-motion engine will drive (timed, curved, persona-based). Until that exists, `Input.insertText` after a real click-to-focus is the correct level of realism for Phase 1: it's a real CDP input command (not `element.value = ...` JS assignment, which some frameworks' change-detection ignores), just without keystroke-level timing.

5. **rmcp (official Rust MCP SDK) via `#[tool_router]`/`#[tool]`/`#[tool_handler]` macros, stdio transport.**
   Confirmed API shape (from `modelcontextprotocol/rust-sdk` examples): a struct implementing tool methods annotated `#[tool(description = "...")]` inside a `#[tool_router]` impl block, parameters via `Parameters<T: JsonSchema + Deserialize>`, returning `Result<CallToolResult, ErrorData>`; `#[tool_handler] impl ServerHandler` wires it together; `service.serve(rmcp::transport::stdio()).await?.waiting().await?` runs it. Dependencies: `rmcp` with `default` features (`base64`, `macros`, `server`) plus `transport-io`.

6. **Session state: `Arc<Mutex<Option<engine::Session>>>` inside the tool-router struct.**
   Lazily populated on first use; tools take the lock, `.get_or_insert_with` semantics via an explicit `ensure_session()` async helper (can't use `Option::get_or_insert_with` directly with async init). `browser_close` takes the session out and tears it down.

## Risks / Trade-offs

- [Main-world injection is observable/tamperable by page script] → acceptable now (own-app and semi-trusted third-party testing per PROPOSAL.md's stated targets); isolated-world upgrade tracked for hardening, not silently promised here.
- [Bounded polling adds latency (up to one interval) vs event-driven waiting] → interval is small (100ms) relative to typical action timescales; documented as a known gap, not a hidden regression from the original design.
- [Re-evaluating the walker script every call has a JS-parse cost] → script is small (~150 lines); acceptable at Phase 1 scale, revisit if profiling shows otherwise.
- [Single global session means no parallelism] → matches PROPOSAL.md's own Phase 1 exit criteria ("agent completes a flow via snapshots only"), not a regression — parallel sessions are explicitly a later-phase concern (browser contexts already exist in `cdp::ops` from Phase 0 for when that's built).

## Migration Plan

Additive: new crates (`engine`, `mcp`), new `aib mcp` subcommand. `aib doctor` and `crates/cdp`'s public API are untouched. No rollback concerns beyond reverting the new crates/subcommand.

## Open Questions

- Exact set of role/name heuristics in the walker will need iteration against real pages during implementation; the spec fixes the contract (role+name+state+ref) not the exact heuristic table.
- Whether `browser_navigate` should auto-close a previous session's page or reuse the same page object across navigations — resolved during implementation as "reuse the same page, `Page.navigate` handles it," consistent with `cdp::ops::Page::navigate_and_wait` from Phase 0.
