# ai-browser (`aib`)

An LLM-first browser-testing engine: a single Rust binary that drives your installed Chrome/Edge over CDP, with human-realistic (and seed-reproducible) mouse/keyboard input, a true OS-level input mode, token-efficient accessibility snapshots for agents, Playwright-style deterministic injection (network mocks, virtual clock, init scripts), and a native MCP server.

**Why:** Playwright/Puppeteer are resource-heavy (Node driver + ~1 GB bundled browsers + browser-per-worker) and their input doesn't mimic a real user (teleporting mouse, instant clicks). `aib` fixes both while keeping deterministic testing.

📄 **Read the full design: [PROPOSAL.md](./PROPOSAL.md)** — architecture, MCP tool surface, strengths/weaknesses, risks, and the phased roadmap.

## Status

- **Phase 0 (CDP spike) — done.** A minimal hand-rolled CDP client (`crates/cdp`) attaches to installed Chrome/Edge, creates an isolated browser context and page, navigates, evaluates JS, and captures a screenshot — all with typed commands over a flatten-session WebSocket connection.
- **Phase 1 (agent MVP) — done.** `crates/engine` adds a session layer with an injected DOM/ARIA walker (token-efficient, ref-addressable snapshots), bounded-poll actionability, and click/type/press/wait_for/screenshot. `crates/mcp` + `aib mcp` expose it all as a stdio MCP server (`browser_navigate`, `browser_snapshot`, `browser_click`, `browser_type`, `browser_press`, `browser_wait_for`, `browser_screenshot`, `browser_close`), verified end-to-end against a real page. This is a scoped-down slice of PROPOSAL.md's full vision — see `openspec/changes/archive/*-phase-1-agent-mvp/design.md` for exactly what's deferred (isolated-world injection, MutationObserver-driven actionability, human motion, multi-session daemon).
- **Browser efficiency — done.** Headless launches get memory-reduction flags and an auto-downloaded, cached `chrome-headless-shell` (installed-browser fallback; `--browser installed` opt-out); `aib doctor` reports full process-tree memory per browser. Measured: headless-shell ~170–350 MB vs installed Chrome/Edge headless ~450–1550 MB for the same page, on both Windows and Linux.
- **Screencast capture — done.** `browser_record_start`/`browser_record_stop` record the page via CDP's `Page.startScreencast`, capped at 30s, and assemble a small animated GIF (JPEG frame sequence + timestamped manifest also saved to disk) — for verifying moving parts, not just static screenshots. GIF only in this phase; no WebM/MP4 (see below).
- **Human motion (synthetic) — done.** `browser_click`/`browser_type` take an optional `human_like` mode: the mouse follows a seeded, Fitts's-law-timed Bezier path (with jitter and occasional overshoot+correction) from wherever the cursor last was instead of teleporting, and typing dispatches one character at a time with persona-shaped, non-uniform pauses instead of a bulk insert. Three built-in presets (`careful`/`average`/`fast`); an explicit `seed` reproduces the exact same motion and timing on a later call, and the response always reports the seed actually used.
- **Human motion (trained) — done.** `browser_train_start(name)`/`browser_train_stop()` capture a real human's mouse/keyboard input (genuinely trusted DOM events, not this engine's own dispatch — see below) while they physically use the browser, fit a `Persona` from it (Fitts's-law constants, jitter, overshoot probability, typing cadence), and save it by name. `browser_click`/`browser_type`'s `trained_profile` param replays that persona through the exact same synthesis/dispatch path the synthetic presets use, so omitting `seed` already gives fresh, non-identical motion/timing on every call — never a literal replay of the recorded demonstration. Requesting a `trained_profile` name that was never trained fails with a clear error, never a silent fallback to a synthetic persona.
- **Network mocking (record/replay) — done.** `browser_network_record_start(name)`/`browser_network_record_stop()` passively capture real request/response pairs to a named cassette; `browser_network_replay_start(name)`/`browser_network_replay_stop()` intercept every request and fulfill it from the cassette instead, so a later test run has zero live-backend dependency. Verified end-to-end: recorded against a real local server, replayed with that server shut down, rendered identically. A request with no matching cassette entry fails as a network error rather than silently reaching the real network.
- **Deterministic init (init scripts + seeded randomness) — done.** `browser_add_init_script(source)` registers JS that runs before any of a page's own scripts, on every subsequent navigation — unlike every other injected script in this project (walker/resolve/train), which runs via `Runtime.evaluate` *after* load. `browser_seed_randomness(seed)` is a convenience built on the same mechanism: overrides `Math.random` with a deterministic PRNG, so an app's own use of randomness during page initialization becomes reproducible run to run. Verified: an init script's value is visible to a page's own first-run inline script (not just to a later agent action), and the same seed reproduces an identical `Math.random()` sequence across separate navigations while different seeds diverge.
- **Virtual clock — done.** `browser_set_clock(time_ms)` installs a virtual clock frozen at a given epoch time (built on the same init-script mechanism), overriding `Date`/`performance.now`/`setTimeout`/`setInterval`/`requestAnimationFrame` so time never moves on its own. `browser_advance_clock(ms)` moves it forward and synchronously fires every due callback in chronological order, including callbacks newly scheduled by other callbacks firing within the same advance. Verified: `Date.now()` reflects the installed time and stays fixed without an advance; a 5-second-delayed callback stays pending until advanced past its delay; a chain of callbacks scheduled with 0ms follow-up delays all fire within a single advance call.
- **Console capture — done.** `browser_console_start(name)`/`browser_console_stop()` capture the page's `console.log`/`warn`/`error` output and any uncaught exceptions between start and stop, saved as a named JSONL trace (`<data-dir>/aib/traces/<name>.jsonl`) — the fastest way to see why a test failed or an app behaved unexpectedly. Verified: a fixture logging at multiple levels and throwing produced a trace with matching entries in the same chronological order.
- **Action trace — done.** While a console trace is active, `browser_navigate`/`browser_click`/`browser_type`/`browser_press` each append a one-line summary into that same trace, interleaved chronologically with console/exception entries — one JSONL stream showing both what the agent did and what the page said in response. Zero overhead when no trace is active. Verified: a navigate + type + click sequence produced action entries in the correct order relative to each other.
- **Next:** true OS-level input mode (Phase 4) and the remaining Phase 3 tooling (`browser_assert`, YAML runner) — see PROPOSAL.md's roadmap.

## MCP server

Configure `aib mcp` as a stdio MCP server in an agent host (e.g. Claude Code, Claude Desktop). It lazily launches a browser on the first tool call and exposes: `browser_navigate(url)`, `browser_snapshot()`, `browser_click(ref, human_like?, persona?, trained_profile?, seed?)`, `browser_type(ref, text, submit?, human_like?, persona?, trained_profile?, seed?)`, `browser_press(key)`, `browser_wait_for(text, timeout_ms?)`, `browser_screenshot()`, `browser_record_start(max_duration_ms?, quality?)`, `browser_record_stop()`, `browser_train_start(name)`, `browser_train_stop()`, `browser_network_record_start(name)`, `browser_network_record_stop()`, `browser_network_replay_start(name)`, `browser_network_replay_stop()`, `browser_add_init_script(source)`, `browser_seed_randomness(seed)`, `browser_set_clock(time_ms)`, `browser_advance_clock(ms)`, `browser_console_start(name)`, `browser_console_stop()`, `browser_close()`. Refs come from the snapshot text (e.g. `[e6]`).

`human_like` (default `false`) switches `browser_click`/`browser_type` from instant dispatch to synthesized human-like motion: a curved, timed mouse path (`browser_click`) plus per-character typing pauses (`browser_type`). Setting `persona` or `trained_profile` implies `human_like` even if left at its default. `seed` fixes the RNG for reproducible motion/timing across calls; omit it for a fresh random seed each time. The result text always reports the seed used, e.g. `clicked e6 (human-like, seed=1234567890)`.

- `persona` selects a synthetic timing profile — `careful`, `average` (default), or `fast`. An unknown name is a tool error, not a silent fallback.
- `trained_profile` selects a profile learned from a real human via `browser_train_start`/`stop` instead of a synthetic preset. `persona` and `trained_profile` are mutually exclusive — passing both is a tool error.

**Training a profile:**

```
browser_train_start(name: "alice")   # physically click/type/move the mouse in the browser window now
...  a human interacts with the page directly ...
browser_train_stop()                 # fits and saves the profile; fails clearly if too little was captured
                                      # (needs at least 3 distinct mouse movements and 5 keystrokes)

browser_type(ref: "e2", text: "...", trained_profile: "alice")   # replays alice's fitted timing, freshly varied
browser_click(ref: "e3", trained_profile: "nobody")               # error: "no trained profile named ..."
```

Only genuinely trusted (`event.isTrusted`) input is captured, filtered by an additional suppression flag this engine's own click/type/press dispatch sets around itself while training is active — CDP-dispatched input is itself `isTrusted` in Chrome, so `isTrusted` alone can't tell this engine's own synthetic dispatch apart from the human physically using the window. One training session at a time; it auto-stops after 5 minutes if `browser_train_stop` is never called. Trained profiles persist under `<data-dir>/aib/profiles/human/<name>.json` and are reused across sessions.

**Network mocking (record/replay):**

```
browser_network_record_start(name: "checkout-flow")   # drive the app normally; real requests/responses are captured
...  browser_navigate/click/type against the real backend ...
browser_network_record_stop()                         # saves the cassette; reports how many requests were recorded

browser_network_replay_start(name: "checkout-flow")    # every request now intercepted, no live network involved
...  replay the same navigate/click/type sequence -- identical responses, live backend can be down ...
browser_network_replay_stop()                          # back to normal (live) network behavior
```

Recording is passive (the `Network` domain observes real traffic); replay actively intercepts every request (the `Fetch` domain) and fulfills it from the cassette, matched by `(method, URL)` and served in original recorded order for repeated calls to the same endpoint (handles polling/pagination without needing to match request bodies). A request during replay with no matching cassette entry fails as a network error — never a silent passthrough to the live network, so an incomplete recording is obvious immediately instead of quietly flaky. Record and replay are mutually exclusive with each other, one at a time; cassettes persist under `<data-dir>/aib/network/<name>.json`.

**Init scripts and seeded randomness:**

```
browser_add_init_script(source: "window.__buildFlag = 'test';")   # runs before the page's own scripts
browser_seed_randomness(seed: 42)                                  # Math.random becomes deterministic
browser_navigate(url: "...")                                       # both take effect starting here
```

Call `browser_add_init_script`/`browser_seed_randomness` *before* `browser_navigate` — like CDP's underlying mechanism, they only affect loads that happen after registration, not whatever's already loaded. Multiple init scripts (including repeated `browser_seed_randomness` calls with different seeds) accumulate and run in registration order on every subsequent navigation for the rest of the session; there's no removal API in this phase. `browser_seed_randomness` overrides `Math.random` with a small, explicitly non-cryptographic PRNG (mulberry32) — same seed produces the same sequence across separate navigations, useful when the app under test generates IDs, picks variants, or seeds animations with `Math.random()` during its own initialization.

**Virtual clock:**

```
browser_set_clock(time_ms: 1700000000000)   # register before navigating, like init scripts
browser_navigate(url: "...")                 # Date/performance.now/timers now read the virtual clock
...                                            # e.g. click something that starts a 30s session-timeout warning
browser_advance_clock(ms: 30000)             # fires the timeout instantly instead of waiting 30 real seconds
```

`browser_set_clock` is built on the same init-script mechanism, so it also has to be called before `browser_navigate`. Once installed, time never moves on its own: `Date.now()`/`new Date()`/`performance.now()` keep returning the same value, and `setTimeout`/`setInterval`/`requestAnimationFrame` callbacks are queued rather than firing for real. `browser_advance_clock(ms)` is the only thing that moves time forward — it fires every callback whose scheduled (virtual) time falls within the advance, in chronological order, including a callback newly scheduled by another callback firing within that same advance (so a `setTimeout` chain resolves correctly in one call). There's no "resume real time" — a session that never installs a clock is unaffected and simply uses real time, as before.

**Console capture:**

```
browser_console_start(name: "checkout-flow")   # captures console.*/uncaught exceptions from here
...  browser_navigate/click/type as normal ...
browser_console_stop()                          # saves the JSONL trace; reports how many entries
```

Each line of the saved trace is one JSON object: `{"type":"console","level":"log"|"warning"|"error"|..., "text":"...", "timestamp_ms":...}` for a `console.*` call (note `console.warn` reports as level `"warning"`, matching Chrome's own `Runtime.consoleAPICalled` type), `{"type":"exception","text":"...","timestamp_ms":...}` for an uncaught exception, or `{"type":"action","text":"click e6","timestamp_ms":...}` for an agent action (`browser_navigate`/`browser_click`/`browser_type`/`browser_press`) taken while the trace is active — all three kinds share the same trace, interleaved in the order they actually happened, so you can see both what the agent did and what the page said in response, in one place. Capture is unfiltered (every level, no truncation) — the "see what happened" case, not a log-triage tool. One capture at a time; it auto-stops after 5 minutes if `browser_console_stop` is never called. Traces persist under `<data-dir>/aib/traces/<name>.jsonl`.

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

## Recording (`browser_record_start` / `browser_record_stop`)

Captures a short video of the page via CDP's screencast API — for verifying animations, transitions, or drag interactions actually happened, not just that one frame looks right:

- One recording at a time per session; `browser_record_start` fails if one is already active.
- Hard-capped at 30 seconds regardless of the requested `max_duration_ms`, so a forgotten `browser_record_stop` can't grow unbounded.
- Frames are captured at 480×360, every 3rd repaint, JPEG quality 60 by default — kept small on purpose; this is a "did it move correctly" check, not pixel-perfect capture.
- `browser_record_stop` writes the JPEG frame sequence and a `manifest.json` (real per-frame timestamps) to `<data-dir>/aib/recordings/<id>/`, assembles `clip.gif` from them (delays derived from the real timestamps, not a fixed rate), and returns the directory path, frame count, duration, and one preview frame as inline image content.
- **GIF only — no WebM/MP4.** Neither the host nor `docker/Dockerfile`'s test image has `ffmpeg` available, and this project doesn't ship encoding paths it hasn't actually run (see `openspec/changes/archive/*-screencast-capture/design.md`).

## Testing in Docker

Browser-launching tests spawn real Chrome/Chromium processes and can leave orphans if a test panics before cleanup — safer to run them disposably than against your host's real browser session:

```
bash docker/run-tests.sh
```

Builds a Debian + Chromium image, runs `cargo test --workspace` and `aib doctor --json` inside a container, and discards everything when it exits — nothing touches your host's browser or its profile directories. `crates/cdp`'s browser discovery works on both Windows (registry + `%LOCALAPPDATA%`) and Linux (`/usr/bin/chromium` etc. + `$XDG_DATA_HOME`/`~/.local/share`) for this reason.
