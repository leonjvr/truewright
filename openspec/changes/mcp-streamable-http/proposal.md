## Why

The original architecture (PROPOSAL.md) always intended two MCP transports: stdio (an agent host spawns `aib mcp` as a subprocess and owns its stdin/stdout -- already built, Phase 1) and streamable HTTP on loopback with a bearer token (a standalone `aib mcp --http` process that any number of MCP-HTTP-capable clients can point at, without needing to spawn or manage the subprocess themselves). Only stdio exists today. This is the next self-contained slice of Phase 5 -- unlike `popup-auto-attach`/`true-user-input`, it's pure Rust HTTP/MCP-server plumbing with no CDP/browser-protocol risk, so it's a good candidate to build without the live-testing uncertainty those changes needed.

## What Changes

- `crates/mcp/Cargo.toml`: enable `rmcp`'s `transport-streamable-http-server` feature.
- New workspace dependency: `axum` (0.8) -- `rmcp`'s streamable-HTTP feature provides a `tower`-compatible `StreamableHttpService`, not a standalone server; the embedding app supplies the actual HTTP listener, which is `axum::serve` in every documented `rmcp` usage.
- `aib mcp` gains `--http` (bool, opt-in; default remains stdio, fully backward compatible), `--port <u16>` (default `8787`), and `--token <value>` (optional). Always binds to `127.0.0.1` only -- there is no flag to bind a non-loopback address; this is a deliberate security decision, not an oversight.
- Bearer-token auth via an `axum` middleware layer: if `--token` isn't given, a random token is generated at startup and printed once (to stderr, alongside the listen address) so the operator can configure their MCP client; every HTTP request must carry `Authorization: Bearer <token>` or receives `401`.
- Each new streamable-HTTP session gets its own fresh `AibTools` instance with its own lazily-launched browser (matching stdio's existing "one process, one browser" model) -- **not** a shared/pooled browser across concurrent HTTP clients. `Session::launch`'s hard-coded `"aib-mcp"` profile-directory name is made unique per HTTP session (a random suffix) to avoid two concurrent sessions colliding on the same Chrome user-data directory, which stdio's single-process model never had to worry about.

**Explicitly out of scope (deferred), and why:**
- **TLS.** Loopback-only + a bearer token is the stated threat model (PROPOSAL.md); adding TLS for a connection that never leaves the machine is complexity without a matching risk it addresses. A reverse proxy in front is the documented escape hatch if someone genuinely needs it.
- **Cross-session browser sharing/pooling.** Each HTTP session's `AibTools` launches its own independent browser, same as every session already does today -- no daemon-style multi-tenant browser reuse. That would be the actual "multi-session daemon" Phase 1 Decision #1 deferred, and this slice doesn't reopen that decision.
- **Rate limiting / connection caps.** A loopback-only server with a secret bearer token has a narrow, already-trusted threat model; not needed for v1.

## Capabilities

### New Capabilities
- `mcp-streamable-http`: `aib mcp --http` serves the same MCP tool surface stdio does, over a loopback HTTP listener guarded by a bearer token, for MCP clients that connect over HTTP instead of spawning a local subprocess.

## Impact

- `crates/mcp/Cargo.toml`: `rmcp` feature `transport-streamable-http-server`; new `axum` dependency.
- `crates/mcp/src/lib.rs`: `AibTools` gains a profile-name field (defaulting to the existing `"aib-mcp"` literal for stdio, uniquified for HTTP sessions) instead of a hard-coded literal inside `ensure_session`.
- `src/mcp.rs`: branches into the existing stdio path or a new HTTP path (axum router + bearer-token middleware + graceful shutdown on Ctrl+C).
- `src/main.rs`: `Command::Mcp` gains `--http`/`--port`/`--token` args.
- New test exercising the HTTP path end-to-end: no token -> 401, wrong token -> 401, correct token -> a real tool call succeeds.
- No real-OS or CDP-protocol side effects -- verification runs headless like every phase before Phase 4, no live-testing check-in needed.
