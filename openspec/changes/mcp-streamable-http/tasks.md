# Tasks: mcp-streamable-http

## 1. Dependencies
- [ ] 1.1 `crates/mcp/Cargo.toml`: add `transport-streamable-http-server` to `rmcp`'s feature list
- [ ] 1.2 Add `axum` (0.8) as a new workspace dependency, used by `src/mcp.rs` (the `aib` binary crate, not `crates/mcp` itself -- keeps `crates/mcp` transport-agnostic)

## 2. `crates/mcp/src/lib.rs`
- [ ] 2.1 `AibTools` gains a `profile_name: String` field; `new`/`with_browser_pref` keep passing the literal `"aib-mcp"` (stdio behavior unchanged); `ensure_session` uses `self.profile_name` instead of the hard-coded literal
- [ ] 2.2 A way to construct an `AibTools` with a caller-supplied profile name (for the HTTP factory closure to generate a unique one per session)

## 3. `src/mcp.rs`
- [ ] 3.1 `run()` branches: `--http` false (default) keeps the existing `stdio()` path unchanged; `--http` true builds the HTTP path below
- [ ] 3.2 HTTP path: `StreamableHttpService::new(factory, LocalSessionManager::default(), config)` where `factory` builds a fresh `AibTools` with a randomly-suffixed profile name each call
- [ ] 3.3 Bearer-token middleware (`axum::middleware::from_fn`) checking `Authorization: Bearer <token>`, `401` on mismatch/missing
- [ ] 3.4 Token resolution: `--token` arg > generate random (print once to stderr along with the listen address)
- [ ] 3.5 Bind `TcpListener::bind(("127.0.0.1", port))`; `axum::serve(..).with_graceful_shutdown(..)` wired to a `CancellationToken` cancelled on `tokio::signal::ctrl_c()`

## 4. `src/main.rs`
- [ ] 4.1 `Command::Mcp` gains `--http` (bool), `--port <u16>` (default `8787`), `--token <String>` (optional) args

## 5. Verification
- [ ] 5.1 Add `reqwest` as a dev-dependency (or to the `aib` binary crate's dev-deps) for a real HTTP-client integration test
- [ ] 5.2 Integration test: start the HTTP server bound to an OS-assigned port (`127.0.0.1:0`, mirroring `rmcp`'s own test pattern) in-process; a request with no `Authorization` header gets `401`; a request with a wrong token gets `401`; a request with the correct token completing the MCP initialize handshake and a real tool call (e.g. `browser_navigate`) succeeds
- [ ] 5.3 Integration test: two concurrent sessions against the same server each get a distinct profile directory (assert on the generated names, or that both browsers launch without error)
- [ ] 5.4 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container

## 6. Wrap-up
- [ ] 6.1 Update README with `aib mcp --http` usage (flags, the printed token, how to point an MCP client at it)
- [ ] 6.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 6.3 `openspec archive mcp-streamable-http -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
