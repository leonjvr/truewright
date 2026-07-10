# mcp-server

## ADDED Requirements

### Requirement: Stdio MCP transport
`aib mcp` SHALL run an MCP server communicating over stdio (stdin/stdout), suitable for direct configuration in an MCP-client agent host. Log output MUST go to stderr, never stdout, so it cannot corrupt the MCP JSON-RPC stream.

#### Scenario: Server starts and speaks MCP over stdio
- **WHEN** `aib mcp` is launched by an MCP client
- **THEN** the client's `initialize` handshake succeeds and the client can list the server's tools

### Requirement: Lazy single session
The server SHALL NOT launch a browser at startup. The first tool call that needs a page (e.g. `browser_navigate`) SHALL lazily create one browser session (launch or attach, one context, one page); subsequent tool calls in the same server process SHALL reuse that session until `browser_close` is called or the server exits.

#### Scenario: No browser before first navigate
- **WHEN** the server has just started and no tool has been called yet
- **THEN** no browser process has been launched

#### Scenario: Session reused across tool calls
- **WHEN** `browser_navigate` is followed by `browser_snapshot` and `browser_click`
- **THEN** all three operate on the same page without relaunching the browser

### Requirement: Core tool set
The server SHALL expose the following tools: `browser_navigate(url)`, `browser_snapshot()`, `browser_click(ref)`, `browser_type(ref, text, submit?)`, `browser_press(key)`, `browser_wait_for(text, timeout_ms?)`, `browser_screenshot()`, `browser_close()`. `browser_navigate` and `browser_snapshot` SHALL return the rendered page-snapshot text as their result content.

#### Scenario: Navigate returns a snapshot
- **WHEN** `browser_navigate` is called with a URL
- **THEN** the tool result contains the rendered snapshot text of the loaded page

### Requirement: Engine errors map to typed tool errors
The server SHALL translate engine errors (stale ref, actionability timeout, navigation timeout, CDP/launch failures) into MCP tool errors carrying a human-readable message, rather than panicking or returning an opaque success.

#### Scenario: Stale ref surfaces as a tool error
- **WHEN** `browser_click` is called with a ref that no longer resolves
- **THEN** the tool call returns an error result describing the stale ref, not a crash or silent no-op
