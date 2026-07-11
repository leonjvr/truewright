## Why

The core promise of PROPOSAL.md's Phase 3 ("Determinism") is that an agent's test run doesn't depend on a live backend actually being up, correctly seeded, and behaving identically call to call — the single biggest source of test flakiness in real-world browser testing. Today `aib` has no way to intercept network traffic at all: every `browser_navigate`/action hits whatever's actually running behind the page. This change adds the foundational capability: record real network responses once, then replay a test entirely against the recording, with no live-backend dependency.

## What Changes

- New CDP surface: the `Network` domain (passive observation: `Network.responseReceived`, `Network.loadingFinished`, `Network.getResponseBody`) for recording, and the `Fetch` domain (active interception: `Fetch.enable`, `Fetch.requestPaused`, `Fetch.fulfillRequest`, `Fetch.failRequest`) for replay. Neither domain is used anywhere in this codebase today.
- `browser_network_record_start(name)`/`browser_network_record_stop()`: passively captures every request/response pair (method, URL, status, headers, body) during the window between start and stop, and persists them as a named "cassette" under `<data-dir>/aib/network/<name>.json`.
- `browser_network_replay_start(name)`/`browser_network_replay_stop()`: intercepts every request via the `Fetch` domain and fulfills it from the named cassette instead of letting it reach the network. Matching is by `(method, URL)`; requests to the same `(method, URL)` more than once are served the recorded responses in original chronological order (a FIFO queue per key), which naturally handles polling/pagination-style repeated calls without needing body-aware matching. A request with no matching cassette entry fails loudly (a network error, not a silent passthrough to the live network) so an incomplete recording surfaces immediately instead of quietly becoming flaky again.
- A minimal in-process HTTP test server (hand-rolled on `tokio::net::TcpListener`, test-only, no new runtime dependency) and a fixture page that fetches from it, so record/replay has something real to record against without depending on any external service.

**Explicitly out of scope (follow-up changes):** request-body-aware matching (v1 matches by method+URL only, queued in order — documented limitation, same "no sophistication beyond what's obviously needed" stance as human-motion's typing-cadence model); WebSocket interception; a general request-body/response mutation API for one-off mocking outside record/replay; init scripts, virtual clock, and seeded `Math.random` (Phase 3's other determinism primitives — separate changes); JSONL traces, `browser_assert`, and the YAML runner (built on top of this, once it exists).

## Capabilities

### New Capabilities
- `network-mocking`: record real network traffic to a named cassette; replay a session entirely against a cassette with no live-network dependency; unmatched requests during replay fail loudly.

## Impact

- `crates/cdp/src/protocol/network.rs`, `crates/cdp/src/protocol/fetch.rs`: new CDP protocol modules (commands + events), not previously used in this codebase.
- `crates/cdp/src/ops.rs`: `Page` gains `enable_network_capture()`, `get_response_body(request_id)`, `enable_request_interception()`, `fulfill_request(...)`, `fail_request(...)`.
- `crates/engine/src/network/`: new module — `Cassette` (serializable request/response entries), `Recording` (passive capture, mirrors `recording.rs`'s collector-task shape), `Replay` (interception + fulfill-from-cassette).
- `crates/engine/src/session.rs`, `crates/mcp/src/lib.rs`: `network_record_start/stop`, `network_replay_start/stop` on `Session`; four corresponding MCP tools.
- `crates/engine/tests/support/`: a minimal hand-rolled HTTP test server (test-only) and a fixture page (`fetch()`-based) for the record/replay integration test to run against without any external dependency.
