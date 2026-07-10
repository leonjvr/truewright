# cdp-client

## ADDED Requirements

### Requirement: Command execution with response correlation
The client SHALL send CDP commands over a single WebSocket connection with monotonically increasing message ids and SHALL resolve each pending command via its correlated response. Commands MUST support a per-command timeout (default 30 s) and resolve to a typed error on protocol error, timeout, or disconnect.

#### Scenario: Successful command round-trip
- **WHEN** `Browser.getVersion` is executed against an attached browser
- **THEN** the typed response is returned and the pending-command map no longer contains the message id

#### Scenario: Protocol error surfaces as typed error
- **WHEN** a command is rejected by the browser (e.g., invalid params)
- **THEN** the client returns `CdpError::Protocol { code, message }` rather than panicking or hanging

#### Scenario: Disconnect fails in-flight commands
- **WHEN** the WebSocket closes while commands are pending
- **THEN** every pending command resolves with `CdpError::Disconnected`

### Requirement: Flatten-session routing
The client SHALL attach to targets with `Target.attachToTarget { flatten: true }` and SHALL route incoming messages by `sessionId`: responses to their pending command, events to that session's subscribers. Messages without a `sessionId` route to the browser-level session.

#### Scenario: Two sessions receive only their own events
- **WHEN** two page targets are attached and both navigate
- **THEN** each session's subscribers receive only lifecycle events carrying that session's id

### Requirement: Typed commands with raw escape hatch
The client SHALL provide hand-written typed request/response structs for the Phase 0 subset (Browser, Target, Page, Runtime domains) and SHALL provide `execute_raw(method, params) -> Value` for any other method. Typed deserialization MUST tolerate unknown fields (no `deny_unknown_fields`) so browser updates do not break parsing.

#### Scenario: Raw escape hatch
- **WHEN** `execute_raw("SystemInfo.getInfo", {})` is called
- **THEN** the raw JSON result is returned without requiring a typed struct

#### Scenario: Unknown fields tolerated
- **WHEN** the browser returns a response containing fields added in a newer protocol version
- **THEN** typed deserialization succeeds, ignoring the unknown fields

### Requirement: Bounded event subscription
The client SHALL expose per-session event streams backed by bounded broadcast channels. When a subscriber lags beyond the channel capacity, the client MUST drop that subscriber's oldest events and signal lag; a slow consumer MUST NOT cause unbounded memory growth in the daemon.

#### Scenario: Slow subscriber lags without OOM
- **WHEN** a subscriber stops polling while events continue to arrive
- **THEN** the channel drops oldest events for that subscriber and surfaces a lag notification when polling resumes

### Requirement: Core page operations
The client SHALL support, end-to-end: `Target.createBrowserContext`, `Target.createTarget` (page in that context), `Page.navigate` awaiting the `load` lifecycle event, `Runtime.evaluate` returning the remote value, and `Page.captureScreenshot` returning image bytes.

#### Scenario: Navigate and evaluate
- **WHEN** a page in a fresh context navigates to `https://example.com` and evaluates `document.title`
- **THEN** the evaluate result is the string `"Example Domain"`

#### Scenario: Screenshot capture
- **WHEN** `Page.captureScreenshot` is called on a loaded page
- **THEN** valid PNG or JPEG bytes of non-zero length are returned
