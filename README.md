# ai-browser (`aib`)

An LLM-first browser-testing engine: a single Rust binary that drives your installed Chrome/Edge over CDP, with human-realistic (and seed-reproducible) mouse/keyboard input, a true OS-level input mode, token-efficient accessibility snapshots for agents, Playwright-style deterministic injection (network mocks, virtual clock, init scripts), and a native MCP server.

**Why:** Playwright/Puppeteer are resource-heavy (Node driver + ~1 GB bundled browsers + browser-per-worker) and their input doesn't mimic a real user (teleporting mouse, instant clicks). `aib` fixes both while keeping deterministic testing.

📄 **Read the full design: [PROPOSAL.md](./PROPOSAL.md)** — architecture, MCP tool surface, strengths/weaknesses, risks, and the phased roadmap.

## Status

- **Phase 0 (CDP spike) — done.** A minimal hand-rolled CDP client (`crates/cdp`) attaches to installed Chrome/Edge, creates an isolated browser context and page, navigates, evaluates JS, and captures a screenshot — all with typed commands over a flatten-session WebSocket connection.
- **Phase 1 (agent MVP) — done.** `crates/engine` adds a session layer with an injected DOM/ARIA walker (token-efficient, ref-addressable snapshots), bounded-poll actionability, and click/type/press/wait_for/screenshot. `crates/mcp` + `aib mcp` expose it all as a stdio MCP server (`browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`, `browser_press`, `browser_wait_for`, `browser_screenshot`, `browser_close`), verified end-to-end against a real page. This is a scoped-down slice of PROPOSAL.md's full vision — see `openspec/changes/archive/*-phase-1-agent-mvp/design.md` for exactly what's deferred (isolated-world injection, MutationObserver-driven actionability, human motion, multi-session daemon).
- **Browser efficiency — done.** Headless launches get memory-reduction flags and an auto-downloaded, cached `chrome-headless-shell` (installed-browser fallback; `--browser installed` opt-out); `aib doctor` reports full process-tree memory per browser. Measured: headless-shell ~170–350 MB vs installed Chrome/Edge headless ~450–1550 MB for the same page, on both Windows and Linux.
- **Next: screencast capture** (re-prioritized 2026-07-11 after the external-research review in `.research/REVIEW.md`): `Page.startScreencast`-based video recording (`browser_record_start`/`browser_record_stop`). The human-motion engine (original Phase 2) follows after that.

## MCP server

Configure `aib mcp` as a stdio MCP server in an agent host (e.g. Claude Code, Claude Desktop). It lazily launches a browser on the first tool call and exposes: `browser_navigate(url)`, `browser_snapshot()`, `browser_click(ref)`, `browser_type(ref, text, submit?)`, `browser_press(key)`, `browser_wait_for(text, timeout_ms?)`, `browser_screenshot()`, `browser_close()`. Refs come from the snapshot text (e.g. `[e6]`).

```
aib mcp                        # headless, managed chrome-headless-shell (auto-downloaded/cached)
aib mcp --headed                # show the browser window (installed browser)
aib mcp --browser installed     # headless, but always the installed browser (no shell download)
```

## Building

```
cargo build --release
```

Produces `target/release/aib.exe`.

## `aib doctor`

Runs the full attach→navigate→evaluate→screenshot→teardown cycle against every installed Chromium browser plus the managed headless-shell (headless runs), and reports command round-trip latency and process-tree memory (`tree_rss_mb`):

```
aib doctor                     # human-readable report; shell + installed browsers, headless
aib doctor --headed            # show the browser windows (installed browsers only — shell is headless-only)
aib doctor --browser installed # skip the managed shell, installed browsers only
aib doctor --json              # machine-readable report for CI
```

Exits non-zero if any step fails on any browser. See `openspec/specs/doctor-cli/spec.md` and `openspec/specs/browser-attach/spec.md` for the full specs, and `openspec/changes/archive/*-browser-efficiency/` for recorded memory-comparison evidence.

### Managed `chrome-headless-shell`

Headless runs auto-download and cache the stripped, headless-only `chrome-headless-shell` binary (from Chrome for Testing) on first use — a deliberate, documented exception to the "no downloads" principle, made because the browser binary, not the driver, dominates memory cost (see `.research/REVIEW.md`). It's cached under `<data-dir>/aib/browsers/<version>/` and reused offline afterwards; if resolution or download fails, `aib` falls back to the installed browser automatically. Headed runs, and `--browser installed`, always use the installed browser and never touch the network.

## Testing in Docker

Browser-launching tests spawn real Chrome/Chromium processes and can leave orphans if a test panics before cleanup — safer to run them disposably than against your host's real browser session:

```
bash docker/run-tests.sh
```

Builds a Debian + Chromium image, runs `cargo test --workspace` and `aib doctor --json` inside a container, and discards everything when it exits — nothing touches your host's browser or its profile directories. `crates/cdp`'s browser discovery works on both Windows (registry + `%LOCALAPPDATA%`) and Linux (`/usr/bin/chromium` etc. + `$XDG_DATA_HOME`/`~/.local/share`) for this reason.
