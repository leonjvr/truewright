# Design: Phase 0 — CDP Spike

## Context

Greenfield repo. The full architecture is defined in /PROPOSAL.md; this design covers only the Phase 0 slice: workspace setup, browser discovery/launch, a minimal CDP client, and `aib doctor`. Everything later (daemon, MCP, snapshots, motion) layers on the interfaces built here, so the priority is getting the connection/session/error model right, not feature breadth.

Constraints: Windows 11 dev machine; installed Chrome/Edge only (no downloads); Rust stable; single static binary.

## Goals / Non-Goals

**Goals**
- Prove attach → context → navigate → evaluate → screenshot works against installed Chrome and Edge with p50 command round-trip < 5 ms.
- Establish the CDP client's concurrency model (demux task, pending map, broadcast events) that the daemon will reuse unchanged.
- Typed-plus-raw protocol approach validated with the smallest useful domain subset.

**Non-Goals**
- No daemon, no MCP, no snapshots, no input dispatch, no `BrowserProtocol` trait (extracted in Phase 1 once a second consumer exists).
- No macOS/Linux discovery (trait-shaped seams only where free).
- No pipe transport (`--remote-debugging-pipe`) — WebSocket only; pipe is a later optimization behind the `Transport` trait.

## Decisions

1. **Hand-rolled CDP client over `tokio-tungstenite`; no chromiumoxide.**
   Alternatives: chromiumoxide (unmaintained, full-protocol codegen → long compiles, churn surface), rust-headless-chrome (sync, downloads browsers by default). We need ~15 commands in Phase 0; hand-written `serde` structs are smaller than the dependency. We copy proven ideas (flatten sessions, pending-map demux) without the code.

2. **Single reader task + `oneshot` pending map + per-session bounded `broadcast` channels.**
   One task owns the WebSocket read half; it locks nothing on the hot path (`dashmap` for the pending/session registries). Responses complete `oneshot` senders; events fan out via `tokio::sync::broadcast` (capacity 1024, lag = dropped-oldest + `Lagged` signal). Alternative — per-command mpsc actors — adds hops without benefit at this scale.

3. **Port discovery via `DevToolsActivePort` file, not stderr parsing.**
   Launch with `--remote-debugging-port=0`; poll `<user-data-dir>/DevToolsActivePort` (exists-and-two-lines) with a 10 s deadline. Stderr parsing breaks under localized builds and redirected pipes.

4. **Lifecycle waiting via `Page.setLifecycleEventsEnabled` + `lifecycleEvent(load)` race with `Page.loadEventFired`.**
   Deterministic navigation completion without polling; the same mechanism extends to `networkAlmostIdle` in Phase 1.

5. **Error taxonomy fixed now**: `CdpError { Protocol{code,message}, Timeout, Disconnected, LaunchFailed, AttachTimeout, NoBrowserFound }` via `thiserror`. Every later layer maps from this enum; changing it later is expensive.

6. **Latency benchmark inside `doctor`, not a separate bench harness.**
   The exit criterion (p50 < 5 ms over ≥100 `Runtime.evaluate("1+1")` calls) is measured where users can rerun it (`aib doctor`), keeping the claim honest on every machine.

## Risks / Trade-offs

- [Chrome/Edge flags or `DevToolsActivePort` behavior differ between channels] → doctor runs against every discovered browser; CI job later runs stable+beta weekly.
- [WebSocket-only transport caps throughput vs pipe] → irrelevant at Phase 0 command rates; `Transport` trait keeps the door open.
- [p50 < 5 ms may fail on loaded machines] → doctor reports p95 too and warns rather than hard-fails on latency; the criterion is evaluated on an idle machine.
- [Hand-written structs drift from protocol] → tolerant deserialization (no `deny_unknown_fields`); only ~15 commands to maintain.

## Migration Plan

Greenfield — nothing to migrate. Rollback = revert commits. The Phase 0 → Phase 1 seam: `cdp` crate's public API (`Connection::connect`, `Session::execute<T>`, `Session::events<E>()`, `execute_raw`) is the contract the `engine` crate will consume.

## Open Questions

- Edge-specific launch quirks (first-run dialogs, `msedge.exe` telemetry flags) — resolve empirically during implementation; add flags to a per-browser-kind launch profile.
- Whether `Target.createTarget` with `about:blank` then `Page.navigate`, or direct URL in `createTarget`, gives cleaner lifecycle signals — decide in implementation, spec only requires navigation-complete semantics.
