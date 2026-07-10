# ai-browser (`aib`)

An LLM-first browser-testing engine: a single Rust binary that drives your installed Chrome/Edge over CDP, with human-realistic (and seed-reproducible) mouse/keyboard input, a true OS-level input mode, token-efficient accessibility snapshots for agents, Playwright-style deterministic injection (network mocks, virtual clock, init scripts), and a native MCP server.

**Why:** Playwright/Puppeteer are resource-heavy (Node driver + ~1 GB bundled browsers + browser-per-worker) and their input doesn't mimic a real user (teleporting mouse, instant clicks). `aib` fixes both while keeping deterministic testing.

📄 **Read the full design: [PROPOSAL.md](./PROPOSAL.md)** — architecture, MCP tool surface, strengths/weaknesses, risks, and the phased roadmap.

## Status

- **Phase 0 (CDP spike) — done.** A minimal hand-rolled CDP client (`crates/cdp`) attaches to installed Chrome/Edge, creates an isolated browser context and page, navigates, evaluates JS, and captures a screenshot — all with typed commands over a flatten-session WebSocket connection.
- **Phase 1 (agent MVP) — done.** `crates/engine` adds a session layer with an injected DOM/ARIA walker (token-efficient, ref-addressable snapshots), bounded-poll actionability, and click/type/press/wait_for/screenshot. `crates/mcp` + `aib mcp` expose it all as a stdio MCP server (`browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`, `browser_press`, `browser_wait_for`, `browser_screenshot`, `browser_close`), verified end-to-end against a real page. This is a scoped-down slice of PROPOSAL.md's full vision — see `openspec/changes/archive/*-phase-1-agent-mvp/design.md` for exactly what's deferred (isolated-world injection, MutationObserver-driven actionability, human motion, multi-session daemon).
- **Next: Phase 2 (human motion)** — seeded, persona-based mouse/keyboard timing shared between the CDP backend and (later) a true OS-input backend.

## MCP server

Configure `aib mcp` as a stdio MCP server in an agent host (e.g. Claude Code, Claude Desktop). It lazily launches a browser on the first tool call and exposes: `browser_navigate(url)`, `browser_snapshot()`, `browser_click(ref)`, `browser_type(ref, text, submit?)`, `browser_press(key)`, `browser_wait_for(text, timeout_ms?)`, `browser_screenshot()`, `browser_close()`. Refs come from the snapshot text (e.g. `[e6]`).

```
aib mcp            # headless
aib mcp --headed   # show the browser window
```

## Building

```
cargo build --release
```

Produces `target/release/aib.exe`.

## `aib doctor`

Runs the full attach→navigate→evaluate→screenshot→teardown cycle against every installed Chromium browser (Chrome, Edge) and reports command round-trip latency:

```
aib doctor            # human-readable report, headless browsers
aib doctor --headed   # show the browser windows
aib doctor --json     # machine-readable report for CI
```

Exits non-zero if any step fails on any browser. See `openspec/changes/phase-0-cdp-spike/` for the full spec and `doctor-evidence.json` for a recorded passing run (both browsers, p50 < 5ms).
