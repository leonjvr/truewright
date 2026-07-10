# ai-browser (`aib`)

An LLM-first browser-testing engine: a single Rust binary that drives your installed Chrome/Edge over CDP, with human-realistic (and seed-reproducible) mouse/keyboard input, a true OS-level input mode, token-efficient accessibility snapshots for agents, Playwright-style deterministic injection (network mocks, virtual clock, init scripts), and a native MCP server.

**Why:** Playwright/Puppeteer are resource-heavy (Node driver + ~1 GB bundled browsers + browser-per-worker) and their input doesn't mimic a real user (teleporting mouse, instant clicks). `aib` fixes both while keeping deterministic testing.

📄 **Read the full design: [PROPOSAL.md](./PROPOSAL.md)** — architecture, MCP tool surface, strengths/weaknesses, risks, and the phased roadmap.

## Status

Phase 0 (CDP spike) complete: a minimal hand-rolled CDP client (`crates/cdp`) attaches to installed Chrome/Edge, creates an isolated browser context and page, navigates, evaluates JS, and captures a screenshot — all with typed commands over a flatten-session WebSocket connection. Next: Phase 1 (agent MVP — daemon, MCP server, accessibility snapshots).

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
