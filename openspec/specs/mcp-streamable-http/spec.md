# mcp-streamable-http Specification

## Purpose
Lets an MCP client that connects over HTTP rather than spawning a local subprocess drive the same browser-automation tool surface stdio does -- a standalone `truewright mcp --http` process that any number of MCP-HTTP-capable clients can point at, loopback-only and bearer-token-guarded.
## Requirements
### Requirement: Streamable-HTTP MCP transport
The engine SHALL support serving the MCP tool surface over a loopback HTTP listener (`truewright mcp --http`), in addition to the existing stdio transport, exposing the identical set of tools either way.

#### Scenario: HTTP mode serves the same tools as stdio
- **WHEN** `truewright mcp --http` is running and a client completes the MCP initialize handshake with a valid bearer token
- **THEN** the same tool calls available over stdio (`browser_navigate`, `browser_click`, etc.) succeed over HTTP

#### Scenario: Stdio remains the default
- **WHEN** `truewright mcp` is run without `--http`
- **THEN** it serves over stdio exactly as before, unaffected by this capability existing

### Requirement: Loopback-only binding
The engine SHALL bind the streamable-HTTP listener to `127.0.0.1` only, with no configuration option to bind a different address.

#### Scenario: The server is unreachable from another host
- **WHEN** `truewright mcp --http` is running
- **THEN** it does not accept connections on any non-loopback network interface

### Requirement: Bearer-token authentication
The engine SHALL require a valid bearer token on every streamable-HTTP request, generating and printing a random token at startup when none is explicitly provided.

#### Scenario: A request without the correct token is rejected
- **WHEN** an HTTP request to the MCP endpoint is missing the `Authorization` header, or carries an incorrect bearer token
- **THEN** the server responds `401 Unauthorized` and does not process the request as an MCP message

#### Scenario: A request with the correct token succeeds
- **WHEN** an HTTP request carries `Authorization: Bearer <token>` matching the server's configured (or generated) token
- **THEN** the request is processed normally

### Requirement: Independent per-session browser
The engine SHALL launch an independent browser session (its own isolated profile directory) for each new streamable-HTTP client session, never sharing a browser across concurrent sessions.

#### Scenario: Two concurrent HTTP sessions don't collide
- **WHEN** two MCP clients connect to `truewright mcp --http` around the same time and each triggers its first tool call
- **THEN** each gets its own browser process with its own profile directory, and neither launch fails due to a profile-directory conflict with the other

