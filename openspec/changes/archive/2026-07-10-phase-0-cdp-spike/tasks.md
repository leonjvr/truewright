# Tasks: Phase 0 — CDP Spike

## 1. Workspace setup

- [x] 1.1 Create Cargo workspace: root `Cargo.toml`, `crates/cdp`, `src/main.rs` (`aib` bin with `clap`, `doctor` subcommand stub)
- [x] 1.2 Add dependencies: tokio (rt-multi-thread, macros, time, process, fs), tokio-tungstenite, serde, serde_json, futures, thiserror, tracing, tracing-subscriber, dashmap, clap
- [x] 1.3 CI-ready basics: rustfmt.toml, clippy clean, `cargo test` green on empty suite

## 2. Browser discovery & launch (browser-attach spec)

- [x] 2.1 Registry lookup for `chrome.exe`/`msedge.exe` App Paths with well-known-path fallback; `NoBrowserFound` error listing checked locations
- [x] 2.2 Launch with `--remote-debugging-port=0`, dedicated `--user-data-dir` under `%LOCALAPPDATA%\aib\profiles\<name>`, plus first-run-suppression flags
- [x] 2.3 Resolve WebSocket URL by polling `DevToolsActivePort` with 10 s deadline (`AttachTimeout` on expiry)
- [x] 2.4 Teardown: dispose created contexts; kill process only if we launched it; test no orphaned processes

## 3. CDP client core (cdp-client spec)

- [x] 3.1 `Transport` trait + WebSocket impl; `Connection` with reader task, id-based pending map (oneshot), send loop
- [x] 3.2 Flatten-session demux: `Target.attachToTarget {flatten:true}`, route by sessionId → session registry
- [x] 3.3 Bounded broadcast event streams per session with lag signaling; typed `events::<E>()` filter
- [x] 3.4 `CdpError` taxonomy; per-command timeout; disconnect fails all in-flight commands (test)
- [x] 3.5 Typed protocol subset: Browser.getVersion, Target.{createBrowserContext, disposeBrowserContext, createTarget, attachToTarget, closeTarget}, Page.{enable, navigate, setLifecycleEventsEnabled, captureScreenshot}, Runtime.{enable, evaluate}; `execute_raw` escape hatch; tolerant deserialization
- [x] 3.6 Navigation-complete semantics: race lifecycleEvent(load) / loadEventFired with timeout

## 4. End-to-end operations

- [x] 4.1 High-level helper: connect → create context → create page → navigate(url) → evaluate(expr) → screenshot → teardown
- [x] 4.2 Integration test against installed browser: example.com title assertion + non-empty screenshot bytes (skip-if-no-browser marker for CI)

## 5. Doctor command (doctor-cli spec)

- [x] 5.1 `aib doctor`: run full cycle per discovered browser; per-step pass/fail; one browser's failure doesn't abort others; non-zero exit on any failure
- [x] 5.2 Latency: ≥100 × `Runtime.evaluate("1+1")`, report p50/p95, warn at p50 ≥ 5 ms
- [x] 5.3 `--json` mode emitting the full machine-readable report
- [x] 5.4 Run on this machine against Chrome and Edge; record results in the change (exit-criteria evidence)

## 6. Wrap-up

- [x] 6.1 Update README status section; document `aib doctor` usage
- [x] 6.2 `openspec validate phase-0-cdp-spike` clean; archive change after implementation is verified
