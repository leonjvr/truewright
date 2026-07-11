# ai-browser (`aib`) — Proposal: An LLM-First Browser-Testing Engine

**Status:** Proposal — approved for Phase 0 spike
**Date:** 2026-07-10
**Author:** Leon (with Claude)

---

## 1. Problem Statement

Playwright and Puppeteer are excellent general-purpose automation tools, but they are a poor fit for LLM-agent-driven user testing for two structural reasons:

1. **Resource heaviness.** A Node.js driver process (~150–250 MB RSS), ~1 GB of downloaded browser binaries, and — under parallelism — a full browser instance per worker. The abstraction layers add protocol chatter and latency. An LLM agent running dozens of short exploratory sessions per hour pays this cost over and over.
2. **Input that doesn't mimic a user.** The mouse teleports to the target and clicks instantly. There are no movement curves, no approach timing, no overshoot, no typing rhythm. Pages that depend on hover paths, pointer telemetry, drag physics, or human-scale timing behave differently under Playwright than under a real user — which is exactly the gap "user testing" is supposed to close. Third-party widgets (OAuth screens, payment iframes) sometimes break outright under synthetic-instant input.

Secondary problems for LLM consumers specifically:

- **Page representation is an afterthought.** Agents get raw HTML dumps, screenshots, or bolted-on accessibility snapshots. Token cost per observation is high; there is no "what changed since my last action" primitive, so agents re-read whole pages.
- **Failure reporting is human-oriented.** A raw `TimeoutError` forces the agent to guess. Agents need structured causes ("click blocked: element occluded by [e3] cookie banner") they can self-correct from in one turn.
- **MCP is a wrapper, not a design center.** playwright-mcp wraps the existing engine; the engine's session model, waiting model, and output formats were never designed for a token-budgeted consumer.

**Goal:** a purpose-built engine that fixes the resource and realism problems, keeps Playwright-class deterministic testing via code injection, and treats LLM agents as the first-class consumer via native MCP.

### Goals

- Minimal-footprint driver: single static binary, no runtime, no browser downloads (drive the *installed* Chrome/Edge).
- Human-realistic input: movement curves, timing, typing cadence — reproducible via seeds.
- True OS-level input mode for maximum fidelity on third-party flows.
- Token-efficient, diff-based page observation for agents.
- Deterministic testing: init-script injection, network mock/record-replay, virtual clock, traces, a scripted test runner.
- Native MCP server (stdio + streamable HTTP).
- Agent exploration → exported deterministic regression test, as a first-class workflow.

### Non-Goals (v1)

- Firefox/Safari support (protocol layer is designed for WebDriver BiDi later; see §10).
- Cloud grid / distributed execution.
- Defeating bot-detection systems. Human-motion realism exists for *testing fidelity* (analytics, event handlers, UX timing, fragile third-party widgets on flows you are authorized to test) — not for evading anti-abuse systems.
- A general web-scraping framework.

---

## 2. Architecture Overview

**`aib`: one static Rust binary** with subcommands `daemon | mcp | run | trace | doctor`.

```
┌─────────────┐  stdio MCP   ┌──────────────────────────────┐   CDP (one WebSocket,
│ Claude Code ├─────────────►│ aib daemon (~15–30 MB RSS)   │   flatten sessions)
└─────────────┘  (aib mcp    │  ├ session registry           ├──────────────► installed
┌─────────────┐   shim)      │  ├ snapshot/diff engine       │                Chrome/Edge
│ other agent ├─────────────►│  ├ motion engine (seeded)     │   SendInput    (dedicated
└─────────────┘  HTTP MCP    │  ├ determinism layer          ├──────────────► profile dir)
┌─────────────┐              │  ├ MCP server (rmcp)          │   (true-user
│ aib run *.yaml ────────────►  └ trace writers              │    mode)
└─────────────┘              └──────────────────────────────┘
```

### 2.1 Process & session model

- **Long-lived daemon**, not per-invocation binaries. It owns the CDP WebSocket, event subscriptions, snapshot caches, and the true-user input lock. Per-invocation processes could not hold the continuous event streams that diff snapshots and event-driven waiting depend on. Idle auto-shutdown after N minutes.
- **Attach to the installed browser.** Registry lookup (`App Paths\chrome.exe` / `msedge.exe`) with well-known-path fallback; launch with `--remote-debugging-port` and a **dedicated `--user-data-dir`** under `%LOCALAPPDATA%\aib\profiles\` — never the user's live profile.
- **One browser, many contexts.** Each session gets an isolated browser context (`Target.createBrowserContext`): own cookies/storage/cache/proxy, zero extra browser processes. This is Playwright's `newContext()` model without the per-worker browser duplication.
- **Session** = { browserContextId, pages, active page, persona, input mode, determinism config, seed, trace handle }. MCP connections bind to a session and can create/switch.
- **Crash handling:** WebSocket close / `Target.targetCrashed` → typed errors, sessions declared dead; agents re-create. No transparent state restoration in v1.

### 2.2 Resource profile (targets, to be validated by the Phase 1 benchmark)

| | Playwright + playwright-mcp | **aib** |
|---|---|---|
| Driver process | ~150–250 MB Node | ~15–30 MB Rust daemon |
| Per parallel session | new browser: +150–400 MB | new *context*: ~0 browser-side (+ per-page renderer, which Chrome spawns in both worlds) |
| Disk | ~1 GB browsers + node_modules | one ~15 MB binary, no downloads |
| Cold start | node + browser launch | daemon attach (browser often already running) |

Honest caveat: renderer memory is identical in both worlds. The savings are the driver process, per-worker browser duplication, and disk — not Chrome itself.

---

## 3. CDP Layer

**Hand-rolled minimal CDP client (~2 kLOC) over `tokio-tungstenite`** — not chromiumoxide (sparsely maintained; codegens the entire protocol, inflating compile times and churn surface). We need ~10 domains and ~60 commands.

- **Flatten-session mode** (`Target.attachToTarget { flatten: true }`): one WebSocket; a reader task demuxes by `sessionId`, then routes command responses via `oneshot` channels and events via bounded per-session `broadcast` channels (lag detection so slow consumers can't OOM the daemon).
- **Typed/raw hybrid:** hand-written `serde` structs for the commands and events we actually use (Input, Fetch, Accessibility get compile-time safety); `execute_raw(method, params)` escape hatch, optionally exposed as a config-gated MCP tool.
- **Domains (v1):** Browser, Target (incl. `setAutoAttach` for popups/OOPIFs — critical for OAuth), Page, Runtime (incl. isolated worlds), Input, DOM, Accessibility, Network (observe), Fetch (intercept), Emulation, Log, Security (opt-in cert-ignore for local dev).
- **Churn mitigation:** pin minimum Chrome at stable−2 with a startup check; `deny_unknown_fields` OFF everywhere; weekly CI against Chrome/Edge stable and beta.
- **`BrowserProtocol` trait** — coarse-grained operations (`create_context`, `navigate`, `snapshot_ax`, `dispatch_input`, `intercept`, …), not CDP-method-shaped, so a WebDriver BiDi implementation can slot in later without pretending BiDi has CDP's shape. CDP-only features (virtual time) may downcast; they are documented as Chromium-only.

Crates: `tokio`, `tokio-tungstenite`, `serde`/`serde_json`, `futures`, `thiserror`, `tracing`, `dashmap`, `rand_pcg`, `rmcp`, `windows`, `clap`, `serde_yaml`.

---

## 4. LLM-Facing Page Representation

The key ergonomics problem: what does the agent *see*?

**Primary source: an injected ARIA/DOM walker running in an isolated world** (`Page.createIsolatedWorld`), with the CDP Accessibility domain as validator/fallback. One `Runtime.callFunctionOn` round-trip returns roles, names, values, states, visibility, and stable refs in a single compact payload — something the raw CDP AX tree needs several round-trips to assemble.

Snapshot format — YAML-ish indented text with stable refs:

```yaml
- page: "Checkout — Acme" url: https://shop.example/checkout [e1]
  - main [e4]
    - heading "Payment details" (h2) [e5]
    - textbox "Card number" value="4242 4242 4242" invalid [e6]
    - combobox "Country" value="South Africa" [e7]
    - button "Pay R499" disabled [e8]
    - alert "Card number is incomplete" [e9]
```

Token-efficiency rules:

- Only interactive, named, or structural elements; pure wrappers collapsed.
- Text truncated (default 200 chars) with `…(+412 chars, browser_read ref=e12)`.
- Off-viewport subtrees elided by default; `snapshot(full=true)` or `snapshot(ref=eN)` on demand.
- Budget: median page ≤ 1.5k tokens; hard cap with pagination.

**Stable refs:** the walker keeps a `ref → Element` map inside the isolated world and reports `backendNodeId` mappings to the daemon. Refs are snapshot-epoch scoped; acting on a stale ref triggers cheap revalidation (still connected? same role/name?) and, on failure, a typed `StaleRef` error carrying a fresh diff — the agent self-corrects in one turn. Coordinates are resolved at action time (`DOM.getContentQuads`), never cached.

**Diff-based updates:** the injected script maintains a dirty-set via `MutationObserver` + `IntersectionObserver` + input/scroll listeners. Every action response embeds `changes:` — re-rendered dirty subtrees, removals, console errors since the last action, navigation events. Agents rarely call `snapshot` explicitly; the action loop is self-feeding. Navigation resets the epoch (`-- new page, refs reset --`).

**Screenshots on demand only** (`browser_screenshot`, viewport/full/element-crop, JPEG q≈70) — images are the token-budget killer, never automatic.

---

## 5. Actionability & Auto-Waiting (event-driven, no polling)

Replicates Playwright's checks — attached, visible, stable, enabled, receives events — but event-driven and with structured failure output:

1. Resolve ref → element; `DOM.scrollIntoViewIfNeeded`.
2. Injected `awaitActionable(el)` resolves when, across **two consecutive animation frames**, the element is connected, visible (non-zero rects, `display`/`visibility` ok), positionally stable (identical bounding box — Playwright's rAF-pair trick), and enabled. Built on MutationObserver + rAF, no polling loops.
3. **Hit test** from the daemon: `getContentQuads` → persona-weighted target point (not always dead center) → `elementFromPoint` chain check through open shadow roots. If occluded, report *what* occludes it.
4. **Post-action settling:** race { `frameNavigated` + `lifecycleEvent(networkAlmostIdle)`, DOM mutation quiet-window (~150 ms), timeout } and report which condition released the gate: `settled: network-idle after 640ms`.
5. Timeouts return a **structured report**: which check failed, last known element state, occluder ref if any. This is the single biggest agent-UX differentiator over raw Playwright errors.

`browser_wait_for` (text/ref/role+name appear/disappear, URL match) rides the same MutationObserver infrastructure.

---

## 6. Human-Motion Engine

A `motion` crate producing **backend-agnostic timelines**: `Vec<TimedInputEvent { at_ms, event }>`, computed entirely up-front from a seeded RNG, then replayed through either input backend.

- **Mouse paths:** minimum-jerk baseline with Bezier control-point perturbation; total duration from Fitts's law (`MT = a + b·log2(D/W + 1)`); probabilistic overshoot + damped correction for distant targets; low-amplitude noise jitter. The cursor position is session state — every path starts from the last position; the mouse never teleports.
- **Typing:** log-normal per-character delays classed by bigram (same-hand/alternating/repeat), burst-pause rhythm, optional typo+backspace injection (off by default, never in deterministic runs unless explicitly seeded on).
- **Scrolling:** wheel-event trains with momentum curve and settle jitter.
- **Personas** (TOML, user-extensible): `careful`, `average` (default), `fast`, `elderly` — parameterizing Fitts coefficients, jitter, dwell, cadence, error rate.
- **Reproducible realism:** per-action RNG = `Pcg64::seed_from(hash(session_seed, action_index, action_kind))` — same seed ⇒ byte-identical timelines, and inserting one action doesn't reshuffle subsequent paths. The seed is always reported in traces and tool output.
- **Speed knob orthogonal to persona:** `speed: 0` = instant Playwright-style dispatch (still passing actionability gates) for fast deterministic runs; `speed: 1.0` = real-time human.

### Input backends

| | **Synthetic-human (default)** | **True-user mode** |
|---|---|---|
| Mechanism | CDP `Input.dispatchMouseEvent`/`dispatchKeyEvent`, timeline-paced at ~60–125 Hz | Windows `SendInput` (via `windows` crate); trait allows macOS `CGEvent` / Linux `uinput` later |
| Headless | yes | no — headed, focused, unoccluded window |
| Parallel | yes | no — one OS cursor ⇒ daemon-wide lock, actions queue (`queued_behind: n` reported) |
| Event provenance | `isTrusted: true`, but debugger-dispatched | indistinguishable from a physical user at the OS level |
| Coordinate mapping | CSS viewport coords | **calibration probe**: dispatch an OS move to a predicted point, read back `event.screenX/clientX` in-page, solve the affine viewport→screen mapping — robust across browser chrome, zoom, and per-monitor DPI; recalibrate on window-bounds/zoom change |

True-user mode keeps CDP attached for *observation* (snapshots, settling, assertions) while input arrives from the OS. Focus is enforced (`SetForegroundWindow` + verification) with typed errors if the desktop is locked. CI use requires an interactive desktop session (autologon VM); it is positioned as a local/pre-release fidelity tool, not a scale tool.

---

## 7. Deterministic-Testing Layer

All shims are **per-context opt-in and OFF by default**, so third-party widgets (OAuth, payment sandboxes) see an untouched page in default mode.

- **Init scripts:** `Page.addScriptToEvaluateOnNewDocument`, ordered bundle: [clock shim?, seeded `Math.random` shim?, user scripts].
- **Network:** `Fetch` domain exclusively for mutation (mock/block/delay/rewrite by URL pattern + method), `Network` events purely for observation/HAR — separating the two avoids a classic CDP bug farm. **Record/replay:** capture paused responses to a `.aibhar` (JSONL + bodies dir); replay matches on (method, normalized URL, optional body hash) with configurable fallback (error | passthrough).
- **Clock:** two modes — (a) `Emulation.setVirtualTimePolicy` (renderer virtual time; powerful, fully sealed tests) or (b) sinon-style JS shim of `Date`/timers/rAF via init script (surgical, safe on third-party pages).
- **Randomness:** `Math.random` ← seeded xorshift via init script; `crypto.getRandomValues` shim opt-in.
- **Capture:** console API calls, exceptions, browser log entries → ring buffer + trace.
- **Trace:** JSONL of `{ts, kind: action|snapshot|console|network|screenshot|assert, seed, persona, …}` with screenshots as files; `aib trace export` renders a static HTML viewer (later phase).
- **Assertions** evaluate against the snapshot model (role/name/value/state), not raw DOM — the same abstraction the agent reasons in.

---

## 8. MCP Surface

**One `browser` MCP server** (agents handle one coherent toolset better), built on the official **`rmcp`** Rust SDK. Transports: stdio (`aib mcp` shim auto-spawns the daemon) and streamable HTTP on loopback with a bearer token.

| Group | Tools |
|---|---|
| Session/nav | `browser_navigate {url, wait_until?}` · `browser_back/forward/reload` · `browser_tabs {action, index?}` · `browser_session {action, persona?, seed?, mode?: synthetic\|true_user}` |
| Observe | `browser_snapshot {full?, ref?, max_tokens?}` · `browser_read {ref}` · `browser_screenshot {ref?, full_page?}` · `browser_console` · `browser_network_log {filter?}` |
| Act (each returns `{ok, settled, changes, console_errors}`) | `browser_click {ref, button?, count?, modifiers?}` · `browser_type {ref, text, submit?, clear?}` · `browser_press {keys}` · `browser_select {ref, values[]}` · `browser_hover` · `browser_drag {from_ref, to_ref}` · `browser_scroll` · `browser_upload {ref, paths[]}` · `browser_dialog {action, text?}` |
| Wait/assert | `browser_wait_for {text?\|ref?\|url?\|role_name?, state, timeout_ms?}` · `browser_assert {ref?, assertion}` → pass/fail + actual |
| Determinism | `browser_mock_network {rules[]}` · `browser_record` / `browser_replay` · `browser_clock {install, now?}` / `browser_clock_advance {ms}` · `browser_init_script {source}` · `browser_trace {action, path?}` · `browser_evaluate {expression, ref?}` (config-gated) |

### Scripted tests vs agent exploration — the keystone workflow

Runner mode (`aib run tests/*.yaml`) executes YAML specs whose steps map **1:1 to the MCP tool names/args**. Because the mapping is exact, an agent's exploration trace exports directly as a runnable regression test:

> **explore with agent → freeze into deterministic test → run in CI forever.**

```yaml
name: checkout-happy-path
session: { persona: average, seed: 42, mode: synthetic }
mocks: [{ pattern: "*/api/quote", respond: { status: 200, body_file: quote.json } }]
steps:
  - navigate: { url: "http://localhost:3000/checkout" }
  - click:    { ref: { role: button, name: "Pay R499" } }   # role+name selectors in runner mode
  - wait_for: { text: "Order confirmed", timeout_ms: 10000 }
  - assert:   { assertion: { kind: url, matches: "/orders/*" } }
```

(Runner mode resolves role+name → element internally; ephemeral `eN` refs are an agent-mode concept.)

---

## 9. Comparison

| | Puppeteer | Playwright | playwright-mcp | **aib** |
|---|---|---|---|---|
| Driver footprint | Node (~150 MB+) | Node (~150–250 MB) | Node + Playwright | ~15–30 MB single binary |
| Browser binaries | downloads Chromium | downloads ~1 GB (3 engines) | via Playwright | **none — installed Chrome/Edge** |
| Parallel isolation | context or browser | browser per worker (test runner) | single session focus | context per session, one browser |
| Human-like input | no | no | no | **curves, timing, personas, seeded** |
| OS-level true input | no | no | no | **yes (SendInput mode)** |
| Agent page representation | none native | ARIA snapshot (retrofit) | ARIA snapshot | **AX snapshot + refs + action-embedded diffs** |
| Structured failure causes | no | partial | no | **yes (occluder refs, settle reasons)** |
| Deterministic injection | yes | yes (mature) | via Playwright | yes (init scripts, mocks, clock, seeds) |
| Explore→regression export | no | codegen (recorder) | no | **trace → YAML test, 1:1 tool mapping** |
| Browsers | Chromium (+FF via BiDi) | Chromium/FF/WebKit | = Playwright | **Chromium only (v1)**; BiDi trait for later |
| Maturity | very high | very high | high | **zero — greenfield** |

---

## 10. Strengths & Weaknesses of This Rewrite

### Strengths

1. **~10–20× smaller driver footprint**, zero browser downloads, single-binary deploy — an agent host can ship it trivially.
2. **Cheap parallelism** via browser contexts instead of browser processes.
3. **Input realism no existing tool offers** — a shared motion model feeding both CDP and true OS input, with personas.
4. **Reproducible realism**: seeded timelines make human-like runs replayable — a property neither "instant synthetic" nor "actual human" testing has.
5. **Agent-native observation**: token-budgeted snapshots, stable refs, diffs embedded in action responses — fewer tokens and fewer round-trips per agent step.
6. **Structured failure reports** agents can self-correct from in one turn.
7. **Explore→freeze workflow**: agent sessions become deterministic CI tests with no translation layer.
8. **Owned stack**: no dependency on Playwright's release cadence or design priorities.

### Weaknesses / Risks

1. **Reimplementing Playwright's actionability edge cases is the highest risk** — years of accumulated handling for pointer-event chains, cross-origin iframes (OOPIFs), `<label>` delegation, contenteditable, `<select multiple>`. Mitigation: port their checks conceptually, test against a corpus of gnarly real pages, ship a documented "known unsupported" list in v1.
2. **CDP churn under auto-updating browsers.** Using the installed Chrome/Edge means the browser updates beneath us. Mitigation: tolerant deserialization, pinned minimum version with startup check, weekly CI against stable + beta channels.
3. **Chromium-only v1.** The `BrowserProtocol` trait is the BiDi escape hatch, but some features (virtual time, isolated-world semantics) have no BiDi equivalent and will remain Chromium-only.
4. **Realism vs determinism is a real tension.** Seeded timelines reproduce *inputs* exactly, but page behavior under real timing varies run-to-run unless the virtual clock is installed — which sacrifices the timing realism you wanted. These are two modes; the proposal never claims both simultaneously.
5. **True-user mode is inherently serialized and desktop-bound** — one OS cursor, focus stealing by notifications, mixed-DPI edge cases, no headless. It is a fidelity tool, not a CI-scale tool.
6. **Some third-party widgets fingerprint CDP attachment itself** (e.g., `Runtime.enable` detection). Motion quality cannot guarantee such widgets behave; true-user mode with a quiet debugger helps but is not guaranteed. Documented honestly.
7. **Renderer memory is unchanged** — savings are driver/duplication/disk, not Chrome's rendering cost.
8. **Maintenance vs the cheap counterfactual.** playwright-mcp plus a motion plugin would deliver perhaps 70% of the value for 5% of the effort. This project is justified by the footprint, true-user mode, and owning the agent-ergonomics loop — and it dies of a thousand cuts without scope discipline. The phased plan and per-phase exit criteria are the discipline mechanism.
9. **Isolated-world invisibility is imperfect** — observers still run on the page's task queue; pathological pages can perturb timing-sensitive behavior.

---

## 11. Workspace Layout

```
ai-browser/
  Cargo.toml               # workspace
  crates/
    cdp/                   # minimal CDP client: transport, connection demux, typed protocol subset, raw escape hatch
    engine/                # sessions, contexts, pages, snapshot model + diff engine, actionability, BrowserProtocol trait
    motion/                # rng, personas, path/typing/scroll synthesis, Timeline
    os-input/              # OsInput trait; windows.rs (SendInput), focus.rs, coords.rs (calibration probe)
    determinism/           # init_scripts, clock, rand, netmock, record_replay, console, trace
    mcp/                   # rmcp server, tool schemas/handlers, stdio shim + HTTP transport
    runner/                # YAML test runner (1:1 MCP tool mapping), trace→YAML export
    injected/              # TypeScript isolated-world agent script (walker, observers, awaitActionable);
                           #   built with esbuild in build.rs, embedded via include_str!
  src/main.rs              # aib CLI: daemon | mcp | run | trace | doctor
```

---

## 12. Phased Roadmap

> **Revised 2026-07-11** after the external-research review (`.research/REVIEW.md`): the browser binary, not the driver, is the dominant cost, and motion-video capture is an explicit product requirement. Two browser-side phases (1.5a/1.5b) were inserted ahead of the human-motion engine, and the zero-download principle was deliberately relaxed for headless runs (auto-downloaded `chrome-headless-shell`, cached, with installed-Chrome fallback; headed runs still use the installed browser).

| Phase | Scope | Duration | Exit criteria |
|---|---|---|---|
| **0 — CDP spike** ✅ | Hand-rolled client: attach to installed Chrome *and* Edge, create context, navigate, `Runtime.evaluate`, screenshot | 1–2 wks | `aib doctor` passes on both browsers; command round-trip p50 < 5 ms |
| **1 — Agent MVP** ✅ | Stdio MCP; injected walker + refs; navigate/snapshot/click/type/press/screenshot/wait_for; instant input through a bounded-poll actionability gate (daemon + diff snapshots deferred) | 3–4 wks | An agent completes a form flow using snapshots only; measured driver footprint vs Playwright (8 MB vs 120 MB RSS, 0.03 s vs 0.80 s CPU) |
| **1.5a — Browser efficiency** ✅ | Memory-reduction launch flags; auto-downloaded, cached `chrome-headless-shell` as the default headless binary with installed-Chrome fallback; process-tree memory measurement in doctor | 1 wk | Measured tree-RSS: shell ~170–350 MB vs installed Chrome/Edge headless ~450–1550 MB, on Windows and Linux; suite green on host + container |
| **1.5b — Screencast capture** ✅ | `Page.startScreencast` recording: JPEG frame sequence + manifest, animated GIF (pure Rust); `browser_record_start/stop` MCP tools. WebM/ffmpeg explicitly deferred — no verified environment has ffmpeg | 1 wk | 59-frame recording of an animated fixture (sliding box) produced a playable GIF, verified headed by the user watching it happen live and opening the resulting clip; integration + unit tests pass on Windows and in the Docker container |
| **2 — Human motion** | motion crate, personas, seeded timelines, CDP backend; diff-based `changes:` in action responses | 2–3 wks | Same seed ⇒ identical timelines (golden-file test); drag-and-drop and hover menus work; 8 parallel motion sessions without cross-talk |
| **3 — Determinism** | Fetch mocking + record/replay, init scripts, both clock modes, seeded randomness, console capture, JSONL traces, `browser_assert`, YAML runner + trace→YAML export | 3–4 wks | A recorded agent exploration replays green 20/20 against mocked network; a 15-test suite runs headless in CI |
| **4 — True-user mode** | SendInput backend, calibration probe, focus management, serialization queue | 2–3 wks | Third-party OAuth login completes via OS input; calibration correct at 100/150/200 % DPI and 80/100/125 % zoom; mixed-DPI dual-monitor test passes |
| **5 — Hardening** | OOPIF/popup auto-attach, shadow-DOM edge corpus, weekly Chrome-beta CI, HTML trace viewer, streamable-HTTP polish, **BiDi spike** to validate the protocol trait | ongoing | — |
