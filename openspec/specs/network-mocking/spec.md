# network-mocking Specification

## Purpose
Deterministic browser testing that doesn't depend on a live backend being up and behaving identically call to call. Record real network traffic once to a named cassette, then replay a test entirely against that cassette — a request with no matching recording fails loudly rather than silently reaching the real network, so an incomplete recording is obvious immediately instead of quietly flaky.
## Requirements
### Requirement: Passive network recording to a named cassette
The engine SHALL support recording real network request/response pairs (method, URL, status, headers, body) observed between `browser_network_record_start` and `browser_network_record_stop`, persisting them as a named cassette that survives across sessions.

#### Scenario: A recorded request/response is persisted
- **WHEN** recording is active and the page makes a network request that receives a response, and recording is then stopped
- **THEN** a cassette is saved by name containing that request's method, URL, status, headers, and body

#### Scenario: Recording captures the actual response, not a placeholder
- **WHEN** a recorded request returns a JSON body
- **THEN** the saved cassette entry's body decodes back to that exact JSON content

### Requirement: Replay from a cassette with no live-network dependency
The engine SHALL support intercepting every network request during `browser_network_replay_start`/`browser_network_replay_stop` and fulfilling it from a named cassette's matching entry instead of letting it reach the network, matched by `(method, URL)` and served in original recorded order for repeated requests to the same key.

#### Scenario: A matched request is served from the cassette
- **WHEN** replay is active for a cassette containing a recorded `(GET, /api/x)` entry, and the page requests `GET /api/x`
- **THEN** the page receives the recorded response's status, headers, and body, and no request reaches the real network

#### Scenario: Repeated requests to the same endpoint are served in recorded order
- **WHEN** a cassette contains two recorded responses for `(GET, /api/x)` and, during replay, the page requests `GET /api/x` twice
- **THEN** the first request receives the first recorded response and the second request receives the second recorded response

#### Scenario: Live backend is provably unnecessary during replay
- **WHEN** replay is active for a complete cassette and the real backend the cassette was recorded against is unreachable or shut down
- **THEN** the page's requests are still fulfilled correctly from the cassette

### Requirement: Unmatched replay requests fail loudly
A request during replay with no matching cassette entry MUST fail as a network error rather than silently passing through to the live network.

#### Scenario: A request not present in the cassette fails
- **WHEN** replay is active and the page makes a request with no matching `(method, URL)` entry in the cassette
- **THEN** that request fails with a network error, and no request reaches the real network

