# Design: Network Mocking (Record/Replay)

## Context

First slice of PROPOSAL.md's Phase 3 ("Determinism"), chosen over the other candidate slices (init scripts/virtual clock/seeded randomness; console capture/JSONL traces) because it's the single biggest lever against test flakiness — an agent's test run should not depend on a live backend being up and behaving identically call to call. The other Phase 3 primitives (init scripts, clock, randomness) and tooling (traces, `browser_assert`, YAML runner) are deliberately deferred to follow-up changes; this one stands alone and is independently valuable.

## Goals / Non-Goals

**Goals:** record real network request/response pairs to a named, persisted cassette; replay a session entirely from a cassette with zero live-network dependency; an unmatched request during replay fails loudly rather than silently reaching the real network (which would quietly break the "no live dependency" guarantee); verified against a real local HTTP server, not just mocked-in-test.

**Non-Goals:** request-body-aware matching (v1 keys on `(method, URL)` only, queued in recorded order per key — see Decision #2); WebSocket interception; a general one-off request-mutation API outside record/replay; the other Phase 3 primitives (init scripts, virtual clock, seeded randomness, traces, assertions, YAML runner) — separate changes, some of which may consume this one's cassette format later.

## Decisions

1. **`Network` domain (passive) for recording, `Fetch` domain (active interception) for replay — different domains for different jobs.** `Network.enable` + `responseReceived`/`loadingFinished` + `getResponseBody` observes traffic without touching it, which is all recording needs and avoids `Fetch`'s per-request pause-and-resume overhead when nothing needs to be substituted. `Fetch.enable` pauses every matching request until this engine explicitly resolves it (`fulfillRequest`/`failRequest`/`continueRequest`), which is exactly what replay needs (substitute a response) and would be unnecessary machinery during recording.
2. **Cassette matching key is `(method, URL)`, served FIFO per key — not body-aware.** Simplest matching rule that still handles the common "the same endpoint polled/paginated multiple times, different content each time" pattern correctly: each `(method, URL)` has its own queue of recorded responses in original chronological order, and each replayed call to that key pops the next one. Body-aware matching (e.g. hashing POST payloads) is real but adds real complexity for marginal v1 benefit — same "no sophistication beyond what's obviously needed" stance the human-motion typing-cadence model took for bigram-awareness.
3. **An unmatched request during replay fails loudly (`Fetch.failRequest`), never falls through to the live network.** The entire point of replay is "no live-backend dependency" — a silent passthrough on a cassette miss would make that guarantee false exactly when it matters (an incomplete recording), and the failure would show up as mysterious flakiness far from its cause instead of an immediate, obvious "this request wasn't recorded."
4. **Response bodies are stored as base64 uniformly**, regardless of content type — `Network.getResponseBody` already returns either raw or base64-encoded text depending on content, so normalizing to base64 for storage (and decoding back to raw bytes for `Fetch.fulfillRequest`, which accepts base64 directly) avoids needing to detect/handle text vs. binary specially.
5. **One JSON file per cassette** (`<data-dir>/aib/network/<name>.json`, an array of entries), not one-file-per-request like screencast frames — network responses are typically far fewer and smaller than video frames captured over the same window, so a single array is simpler to manage and doesn't need `recording.rs`'s manifest/asset-directory split.
6. **`Session::network_record_start`/`network_replay_start` mirror `Recording`/`Training`'s ownership shape exactly**: return an owned value to the caller; `.stop()` on that value finalizes and is where the actual persistence/teardown happens; the MCP layer holds `Arc<Mutex<Option<...>>>` guards with the same "already in progress" check `browser_record_start`/`browser_train_start` already use. Record and replay are mutually exclusive with each other (can't record while replaying or vice versa on the same session) but independent of screencast recording/motion training, which use different CDP domains and don't interfere.
7. **A minimal, hand-rolled, test-only HTTP server** (`tokio::net::TcpListener`, manual HTTP/1.1 request-line/header parsing, canned JSON responses) backs the integration test, rather than a new HTTP-server crate dependency — consistent with this codebase's established preference for hand-rolled protocol code (the entire CDP/WebSocket layer is hand-rolled) and keeps this test-only surface out of the release binary's dependency graph entirely.

## Risks / Trade-offs

- [`(method, URL)`-only matching means two structurally-different requests to the same endpoint (e.g. different query-driven behavior your app doesn't actually send as query params) can't be told apart] → acceptable v1 limit, documented; a real need for finer matching is exactly the signal to revisit Decision #2 in a follow-up.
- [FIFO-per-key replay means recording order matters — replaying actions in a different order than they were recorded could serve semantically wrong responses to the right endpoint] → inherent to any order-based mocking strategy; the mitigation is "record the same flow you intend to replay," which is the normal usage pattern (record once, replay that same scripted flow repeatedly for determinism), not "record anything, replay anything."
- [The hand-rolled test HTTP server is unvalidated general-purpose HTTP; only needs to serve canned JSON to a `fetch()` call from a fixture page] → deliberately minimal, test-only, not exposed as product surface.

## Migration Plan

Purely additive — four new MCP tools, no existing tool's behavior changes.

## Open Questions

None blocking.
