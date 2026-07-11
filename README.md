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
- **Browser assert — done.** `browser_assert(text, present?)` checks the current snapshot immediately (no polling, unlike `browser_wait_for`) and fails as a genuine tool-level failure result (not a protocol error) an agent should treat as a test failure — the explicit pass/fail check a test actually needs, distinct from `browser_wait_for`'s "poll until true" shape. Logged into the active trace (pass or fail) when one is active. Verified: a passing check succeeds, a failing one returns a clear error, and both are correctly logged.
- **YAML runner — done (Phase 3 complete).** `browser_run_yaml(source)` executes a declarative YAML script (`navigate`/`click`/`type`/`press`/`wait_for`/`assert` steps) against the current session, stopping at the first failing step; `browser_export_yaml(name)` converts an already-captured trace's actions back into a runnable script of that same format — record a flow once, then replay it as a checked-in test. Verified: a hand-written script runs end to end; a deliberately-failing script stops at the right step without running later ones; a real captured trace exports to YAML and replaying it reproduces the same end state.
- **True OS-level input (Phase 4) — done.** `browser_click`/`browser_type`'s new `true_input` flag dispatches via real Windows `SendInput` instead of CDP — the actual system cursor moves and clicks, and real keystrokes are sent, indistinguishable from a human at the OS level (unlike every other input mode in this project, which is CDP-synthesized, even though Chrome itself marks those events `isTrusted`). Reuses the exact same `human_like` timing synthesis; only the delivery mechanism changes. Windows-only, headed sessions only for v1 — rejected with a clear error otherwise, never a silent CDP fallback. Verified live: real cursor movement focused a form field, real keystrokes typed into it, a real click submitted the form — confirmed via a mid-test screenshot and a passing integration test.
- **Popup auto-attach (Phase 5) — done.** A new top-level page opened as a side effect of interacting with the current one (a `window.open()` OAuth-login popup, a `target="_blank"` link) now attaches automatically instead of being invisible to the engine. `browser_list_pages()` shows every attached page and which one is active; `browser_switch_page(page_id)` changes which page subsequent actions target — no auto-switching, an agent has to explicitly notice and pick. If the active page closes itself (e.g. an OAuth redirect finishing), the active page falls back to the original one automatically. Cross-origin OOPIF (iframe) attach is out of scope for this slice — top-level popups/new tabs only. Verified: a fixture opens a real popup via `window.open()`, the popup is listed but not active, switching to it lets actions target its own content, and closing it via its own `window.close()` falls back to the opener; full suite green on host and in the Docker container.
- **Streamable-HTTP MCP transport (Phase 5) — done.** `aib mcp --http` serves the identical tool surface stdio does over a loopback-only HTTP listener guarded by a bearer token, for MCP clients that connect over HTTP instead of spawning `aib` as a subprocess. Always binds `127.0.0.1` — no flag exposes it beyond loopback. Each HTTP session gets its own independent browser (a uniquely-suffixed profile directory), so concurrent sessions never collide. Verified: real requests with no/wrong token get `401`; a correctly authenticated session completes the MCP handshake and lists the real tool set; two concurrent sessions each navigate a real browser without a profile-directory collision — confirmed both via the automated suite and manually against the compiled binary with `curl`.
- **Same-origin iframes (Phase 5) — done.** `browser_snapshot` now sees into same-origin `<iframe>` content instead of silently omitting it — real-world apps embed same-origin iframes constantly (rich-text editors, embedded widgets, app-internal micro-frontends), and previously the walker showed nothing where one existed. Nested content appears under an `iframe` entry with usable refs, and `browser_click`/`browser_type` on those refs land at the correct on-screen coordinates (accounting for the iframe's own position and any ancestor iframes' positions). A cross-origin iframe is shown as an explicit "not inspectable" leaf rather than vanishing — full cross-origin OOPIF support (a real CDP target-attach problem) remains out of scope. Verified: a fixture with both a same-origin (`srcdoc`) and cross-origin (`data:` URL) iframe shows the same-origin button in the snapshot, clicking it via its ref actually fires the iframe's own click handler (proving the coordinate math, not just that the CDP call didn't error), and the cross-origin frame renders as the explicit boundary text.
- **Shadow-DOM-aware walker (Phase 5) — done.** `browser_snapshot` now sees into open shadow roots — web components (design-system libraries, framework-agnostic widgets) previously showed as empty leaves regardless of how much interactive content they rendered. Shadow content is spliced seamlessly into the tree (no wrapper node, unlike iframes — an open shadow boundary has no functional consequence once walked); a `<slot>`'s actually-projected light-DOM content shows up in its rendered position, not omitted or duplicated. No coordinate-translation changes were needed at all — shadow DOM affects tree structure only, confirmed by testing rather than assumed. Closed shadow roots aren't distinguishably surfaced (genuinely undetectable from script, not a scoping choice). Verified: a custom-element fixture with a directly-rendered shadow button and a slotted heading shows both in the snapshot, and clicking the shadow-nested button's ref actually fires its handler.
- **HTML trace viewer (Phase 5) — done.** `aib trace view <name>` and the `browser_render_trace(name)` MCP tool render an already-saved trace as one self-contained HTML file — console/exception/action entries in chronological order, color-coded by kind. `browser_screenshot` calls made while a trace is active are now captured into it too (a new trace entry kind, the one thing a text-only trace could never show) and appear inline in the rendered page as embedded images, not external file references — the HTML file needs nothing alongside it to be readable. Verified: a captured trace with console/exception/action/screenshot entries renders correctly, with the screenshot visible inline as a base64-embedded image; visually confirmed legible.
- **Next:** more Phase 5 hardening (see PROPOSAL.md's roadmap) — no specific slice committed yet.

## MCP server

Configure `aib mcp` as a stdio MCP server in an agent host (e.g. Claude Code, Claude Desktop), or run `aib mcp --http` to serve over loopback HTTP instead (see below). It lazily launches a browser on the first tool call and exposes: `browser_navigate(url)`, `browser_snapshot()`, `browser_click(ref, human_like?, persona?, trained_profile?, seed?, true_input?)`, `browser_type(ref, text, submit?, human_like?, persona?, trained_profile?, seed?, true_input?)`, `browser_press(key)`, `browser_wait_for(text, timeout_ms?)`, `browser_screenshot()`, `browser_record_start(max_duration_ms?, quality?)`, `browser_record_stop()`, `browser_train_start(name)`, `browser_train_stop()`, `browser_network_record_start(name)`, `browser_network_record_stop()`, `browser_network_replay_start(name)`, `browser_network_replay_stop()`, `browser_add_init_script(source)`, `browser_seed_randomness(seed)`, `browser_set_clock(time_ms)`, `browser_advance_clock(ms)`, `browser_console_start(name)`, `browser_console_stop()`, `browser_assert(text, present?)`, `browser_run_yaml(source)`, `browser_export_yaml(name)`, `browser_render_trace(name)`, `browser_list_pages()`, `browser_switch_page(page_id)`, `browser_close()`. Refs come from the snapshot text (e.g. `[e6]`).

**Streamable-HTTP transport:**

```
aib mcp --http                              # listens on http://127.0.0.1:8787/mcp, prints a random bearer token
aib mcp --http --port 9000 --token my-secret  # explicit port + token instead of a random one
```

Point an MCP-HTTP-capable client at `http://127.0.0.1:<port>/mcp` with `Authorization: Bearer <token>`. Always binds `127.0.0.1` only — there's no flag to bind anywhere else, since the threat model is "single trusted machine, bearer token," not "reachable over the network." Every concurrent client session gets its own independent browser (its own profile directory), same cost model as running multiple stdio `aib mcp` processes. Stdio remains the default when `--http` isn't passed; nothing about existing stdio usage changes.

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

**True OS-level input:**

```
browser_click(ref: "e3", true_input: true)                             # real SendInput click, instant
browser_type(ref: "e2", text: "...", true_input: true, human_like: true, persona: "average")  # real keystrokes, human-paced
```

`true_input: true` dispatches through the real Windows input pipeline (`SendInput`) instead of CDP's `Input.dispatch*Event` — the actual system mouse cursor moves and clicks, and real keystrokes are sent to whatever window has OS focus, not just inside the browser tab. This is the only input mode in this project that isn't CDP-synthesized end to end; every other mode (including `human_like`) is still tooling-dispatched, even though Chrome itself marks those events `isTrusted`. `true_input` composes with `human_like`/`persona`/`trained_profile`/`seed` unchanged — the same synthesized curved-path/typing-cadence timeline is used either way, only the final delivery mechanism differs.

**Requirements and failure modes:** Windows only, and the session must be headed (`aib mcp --headed`) — a headless renderer has no real OS window to receive input, and there's no macOS/Linux backend yet. Both cases fail with a clear tool error, never a silent fallback to CDP dispatch. Because it's real OS input, it briefly takes over your actual mouse cursor and window focus — expect the browser window to visibly pop to the foreground for the duration of the action, same as if you'd clicked it yourself.

**Popups and new tabs:**

```
browser_click(ref: "e3")                      # opens a "Sign in with Google" popup as a side effect
browser_list_pages()                          # * [e...primary-id] https://app.example.com -- App
                                               #   [e...popup-id]   https://accounts.google.com/... -- Sign in
browser_switch_page(page_id: "e...popup-id")  # subsequent actions now target the popup
...  browser_snapshot/click/type against the popup's own content ...
# once the popup completes its flow and closes itself, the active page
# automatically falls back to the original one -- no extra call needed
browser_snapshot()                            # back to the original page's content
```

A popup or new tab opened as a side effect of an action (`window.open()`, a `target="_blank"` link) attaches automatically, but never becomes active on its own — call `browser_list_pages()` to see it (marked with its `page_id`) and `browser_switch_page(page_id)` to start driving it. If that page later closes itself, the active page reverts to the original automatically. Cross-origin iframes (OOPIFs) aren't covered by this — only top-level popups/new tabs.

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

Each line of the saved trace is one JSON object: `{"type":"console","level":"log"|"warning"|"error"|..., "text":"...", "timestamp_ms":...}` for a `console.*` call (note `console.warn` reports as level `"warning"`, matching Chrome's own `Runtime.consoleAPICalled` type), `{"type":"exception","text":"...","timestamp_ms":...}` for an uncaught exception, `{"type":"action","text":"click e6","timestamp_ms":...}` for an agent action (`browser_navigate`/`browser_click`/`browser_type`/`browser_press`/`browser_assert`) taken while the trace is active, or `{"type":"screenshot","path":"...","timestamp_ms":...}` for a `browser_screenshot` call taken while the trace is active — all four kinds share the same trace, interleaved in the order they actually happened, so you can see both what the agent did and what the page said (and looked like) in response, in one place. Capture is unfiltered (every level, no truncation) — the "see what happened" case, not a log-triage tool. One capture at a time; it auto-stops after 5 minutes if `browser_console_stop` is never called. Traces persist under `<data-dir>/aib/traces/<name>.jsonl`; screenshots taken during a trace persist alongside it under `<data-dir>/aib/traces/<name>-screenshots/`.

**Trace viewer:**

```
aib trace view checkout-flow            # renders <data-dir>/aib/traces/checkout-flow.html, prints its path
browser_render_trace(name: "checkout-flow")   # same thing, callable by an agent -- returns the path
```

Renders an already-saved trace as one self-contained HTML file: every console/exception/action/screenshot entry in chronological order, color-coded by kind, with screenshots embedded inline as base64 images so the file needs nothing alongside it to be readable anywhere you open it. Both the CLI and the MCP tool call the same renderer and produce the same output; neither returns the HTML content itself, just where it was written.

**Assertions:**

```
browser_assert(text: "Account created")                  # passes if the text is present right now
browser_assert(text: "Loading...", present: false)        # passes if the text is absent
```

Unlike `browser_wait_for` (which polls until the text appears or a timeout elapses, returning the snapshot), `browser_assert` checks the current snapshot once, immediately, and is pass/fail: it returns a normal success result when the check holds, and an MCP tool-level error result — a genuine failure signal, not a protocol error — with a message identifying what was expected when it doesn't. Treat a `browser_assert` failure as a real test failure. `present` defaults to `true`.

**YAML scripts:**

```yaml
name: signup flow
steps:
  - navigate: "https://example.com/signup"
  - type:
      ref: e2
      text: "hello@example.com"
  - click: e3
  - assert:
      text: "Account created"
```

```
browser_run_yaml(source: "<the YAML above>")   # runs each step in order, stopping at the first failure

browser_console_start(name: "signup-flow")     # record a flow once...
...  browser_navigate/type/click as normal ...
browser_console_stop()
browser_export_yaml(name: "signup-flow")       # ...then get it back as a runnable script
```

Steps are single-key maps, matching common step-list conventions (GitHub Actions, Ansible): `navigate`, `click`, and `press` take a plain string; `type` takes `{ref, text, submit?}`; `wait_for` takes `{text, timeout_ms?}`; `assert` takes `{text, present?}`. `browser_run_yaml` executes them via the exact same underlying methods a live tool call would use and stops at the first failing step, reporting which one. `browser_export_yaml` reads an already-saved trace's `action` entries (from `browser_console_start`/`stop`) and reconstructs the equivalent script — console/exception entries aren't included, since there's no "step" to replay for a log message. v1 covers the instant-dispatch action set only (no `human_like`/persona/trained-profile steps).

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
