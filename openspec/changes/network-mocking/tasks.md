# Tasks: Network Mocking (Record/Replay)

## 1. CDP protocol

- [ ] 1.1 `crates/cdp/src/protocol/network.rs`: `Network.enable`, `ResponseReceived` event, `LoadingFinished` event, `Network.getResponseBody`/`GetResponseBodyResponse`
- [ ] 1.2 `crates/cdp/src/protocol/fetch.rs`: `Fetch.enable`/`EnableParams` (request pattern), `RequestPaused` event, `Fetch.fulfillRequest`/`FulfillRequestParams`, `Fetch.failRequest`/`FailRequestParams`
- [ ] 1.3 `cdp::ops::Page`: `enable_network_capture()`, `get_response_body(request_id) -> (body_base64, base64_encoded)`, `enable_request_interception()`, `fulfill_request(request_id, status, headers, body_base64)`, `fail_request(request_id, reason)`

## 2. Cassette format + recording

- [ ] 2.1 `crates/engine/src/network/cassette.rs`: `Cassette` (serde, `Vec<CassetteEntry>`), `CassetteEntry { method, url, status, headers, body_base64 }`
- [ ] 2.2 `crates/engine/src/network/recording.rs`: `NetworkRecording` (mirrors `recording.rs`'s collector-task shape) — subscribes to `ResponseReceived`/`LoadingFinished`, fetches each body via `getResponseBody`, buffers entries; `stop()` persists the cassette under `<data-dir>/aib/network/<name>.json`

## 3. Replay

- [ ] 3.1 `crates/engine/src/network/replay.rs`: `NetworkReplay` — loads a cassette, groups entries into a `HashMap<(method, url), VecDeque<CassetteEntry>>`; subscribes to `RequestPaused`, pops the next matching entry and fulfills, or fails the request if no entry matches
- [ ] 3.2 `stop()` disables interception cleanly (no requests left hanging)

## 4. Engine/MCP integration

- [ ] 4.1 `Session::network_record_start(name) -> Result<NetworkRecording>`, `Session::network_replay_start(name) -> Result<NetworkReplay>`
- [ ] 4.2 MCP: `browser_network_record_start(name)`, `browser_network_record_stop()`, `browser_network_replay_start(name)`, `browser_network_replay_stop()`; "already in progress" guards mirroring `browser_record_start`/`browser_train_start`

## 5. Test infrastructure

- [ ] 5.1 `crates/engine/tests/support/http_server.rs`: minimal hand-rolled HTTP/1.1 server on `tokio::net::TcpListener`, test-only, serving canned JSON responses keyed by path
- [ ] 5.2 `crates/engine/tests/fixtures/network.html`: fetches from the local test server and renders the response into the DOM (so the walker/snapshot can observe what was returned)

## 6. Verification

- [ ] 6.1 Host: full suite green
- [ ] 6.2 Integration test: record a session against the real local test server, stop recording, then replay the same flow with the test server shut down, asserting the rendered DOM content is identical — proves the "no live-network dependency" guarantee, not just that the code compiles
- [ ] 6.3 Integration test: an unmatched request during replay fails as a network error, not a silent passthrough
- [ ] 6.4 Container: `bash docker/run-tests.sh` green

## 7. Wrap-up

- [ ] 7.1 README documents the four `browser_network_*` tools and the cassette format/location
- [ ] 7.2 `openspec validate network-mocking` clean; sync specs; archive
