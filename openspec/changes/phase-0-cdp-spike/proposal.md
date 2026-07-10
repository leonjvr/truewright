# Proposal: Phase 0 — CDP Spike

## Why

The ai-browser engine (see /PROPOSAL.md) depends on one load-bearing bet: that a hand-rolled, minimal CDP client in Rust can attach to the user's *installed* Chrome/Edge and drive it with lower overhead than Playwright's stack. Phase 0 validates that bet before any higher layer (snapshots, motion, MCP) is built on it.

## What Changes

- New Cargo workspace with the first two crates: `crates/cdp` (minimal CDP client) and the `aib` CLI binary.
- Browser discovery: locate installed Chrome and Edge on Windows (registry `App Paths`, well-known-path fallback) and launch with `--remote-debugging-port` and a dedicated `--user-data-dir` (never the live profile).
- CDP client over `tokio-tungstenite`: command/response correlation, flatten-session event demux, typed structs for the Phase 0 command subset, raw JSON escape hatch.
- Core operations end-to-end: create browser context, create page target, navigate with lifecycle wait, `Runtime.evaluate`, capture screenshot, clean teardown.
- `aib doctor` command: runs the full attach→navigate→evaluate→screenshot cycle against every discovered browser and reports pass/fail plus command round-trip latency.

## Capabilities

### New Capabilities

- `browser-attach`: discovering installed Chromium browsers, launching/attaching with an isolated profile, and lifecycle/teardown guarantees.
- `cdp-client`: the minimal CDP protocol client — transport, command execution (typed + raw), session routing, event subscription, and error semantics.
- `doctor-cli`: the `aib doctor` self-check command and its reporting contract.

### Modified Capabilities

_None — greenfield._

## Impact

- New code only: `Cargo.toml` (workspace), `crates/cdp/`, `src/main.rs` (CLI).
- Dependencies introduced: `tokio`, `tokio-tungstenite`, `serde`/`serde_json`, `futures`, `thiserror`, `tracing`, `clap`.
- Requires Chrome and/or Edge installed on the machine (no browser downloads — by design).
- Exit criteria (from PROPOSAL.md roadmap): `aib doctor` passes on both Chrome and Edge; command round-trip p50 < 5 ms.
- Everything in later phases (engine, motion, MCP) builds on the interfaces established here; `BrowserProtocol` trait extraction is deferred to Phase 1 to avoid premature abstraction.
