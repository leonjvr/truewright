# Tasks: Phase 0 — CDP Spike

## 1. Workspace setup

- [ ] 1.1 Create Cargo workspace: root `Cargo.toml`, `crates/cdp`, `src/main.rs` (`aib` bin with `clap`, `doctor` subcommand stub)
- [ ] 1.2 Add dependencies: tokio (rt-multi-thread, macros, time, process, fs), tokio-tungstenite, serde, serde_json, futures, thiserror, tracing, tracing-subscriber, dashmap, clap
- [ ] 1.3 CI-ready basics: rustfmt.toml, clippy clean, `cargo test` green on empty suite

## 2. Browser discovery & launch (browser-attach spec)

- [ ] 2.1 Registry lookup for `chrome.exe`/`msedge.exe` App Paths with well-known-path fallback; `NoBrowserFound` error listing checked locations
- [ ] 2.2 Launch with `--remote-debugging-port=0`, dedicated `--user-data-dir` under `%LOCALAPPDATA%\aib\profiles\<name>`, plus first-run-suppression flags
- [ ] 2.3 Resolve WebSocket URL by polling `DevToolsActivePort` with 10 s deadline (`AttachTimeout` on expiry)
- [ ] 2.4 Teardown: dispose created contexts; kill process only if we launched it; test no orphaned processes

## 3. CDP client core (cdp-client spec)

- [ ] 3.1 `Transport` trait + WebSocket impl; `Connection` with reader task, id-based pending map (oneshot), send loop
- [ ] 3.2 Flatten-session demux: `Target.attachToTarget {flatten:true}`, route by sessionId → session registry
- [ ] 3.3 Bounded broadcast event streams per session with lag signaling; typed `events::<E>()` filter
- [ ] 3.4 `CdpError` taxonomy; per-command timeout; disconnect fails all in-flight commands (test)
- [ ] 3.5 Typed protocol subset: Browser.getVersion, Target.{createBrowserContext, disposeBrowserContext, createTarget, attachToTarget, closeTarget}, Page.{enable, navigate, setLifecycleEventsEnabled, captureScreenshot}, Runtime.{enable, evaluate}; `execute_raw` escape hatch; tolerant deserialization
- [ ] 3.6 Navigation-complete semantics: race lifecycleEvent(load) / loadEventFired with timeout

## 4. End-to-end operations

- [ ] 4.1 High-level helper: connect → create context → create page → navigate(url) → evaluate(expr) → screenshot → teardown
- [ ] 4.2 Integration test against installed browser: example.com title assertion + non-empty screenshot bytes (skip-if-no-browser marker for CI)

## 5. Doctor command (doctor-cli spec)

- [ ] 5.1 `aib doctor`: run full cycle per discovered browser; per-step pass/fail; one browser's failure doesn't abort others; non-zero exit on any failure
- [ ] 5.2 Latency: ≥100 × `Runtime.evaluate("1+1")`, report p50/p95, warn at p50 ≥ 5 ms
- [ ] 5.3 `--json` mode emitting the full machine-readable report
- [ ] 5.4 Run on this machine against Chrome and Edge; record results in the change (exit-criteria evidence)

## 6. Wrap-up

- [ ] 6.1 Update README status section; document `aib doctor` usage
- [ ] 6.2 `openspec validate phase-0-cdp-spike` clean; archive change after implementation is verified
