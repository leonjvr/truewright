# Tasks: mcp-streamable-http

## 1. Dependencies
- [x] 1.1 `rmcp` (on the `aib` binary crate's dependency entry) gains `transport-streamable-http-server`
- [x] 1.2 Add `axum` (0.8), `tokio-util` (`CancellationToken`), and `tokio`'s `signal` feature (`Ctrl+C`) to the `aib` binary crate

## 2. `crates/mcp/src/lib.rs`
- [x] 2.1 `AibTools` gains a `profile_name: String` field; `new`/`with_browser_pref` keep passing the literal `"aib-mcp"` (stdio behavior unchanged); `ensure_session` uses `self.profile_name` instead of the hard-coded literal
- [x] 2.2 `AibTools::with_profile_name` -- a caller-supplied profile name, used by the HTTP factory closure to generate a unique one per session

## 3. `src/mcp.rs` (moved into a new `src/lib.rs` module so integration tests can call it in-process)
- [x] 3.1 `run()` (stdio) unchanged; new `run_http()` for the `--http` path
- [x] 3.2 `router()`: `StreamableHttpService::new(factory, LocalSessionManager::default(), config)` where `factory` builds a fresh `AibTools::with_profile_name` with a randomly-suffixed profile name each call
- [x] 3.3 Bearer-token middleware (`axum::middleware::from_fn_with_state`) checking `Authorization: Bearer <token>`, `401` on mismatch/missing
- [x] 3.4 Token resolution: `--token` arg > generate random (printed once to stderr along with the listen address)
- [x] 3.5 `TcpListener::bind(("127.0.0.1", port))`; `axum::serve(..).with_graceful_shutdown(..)` wired to a `CancellationToken` cancelled on `tokio::signal::ctrl_c()`

## 4. `src/main.rs`
- [x] 4.1 `Command::Mcp` gains `--http` (bool), `--port <u16>` (default `8787`), `--token <String>` (optional) args

## 5. Verification
- [x] 5.1 `reqwest` (pinned to 0.13 to match `rmcp`'s own internal `reqwest` dependency -- a 0.12/0.13 mismatch otherwise creates two incompatible `reqwest::Client` types and a confusing trait-bound error), `rmcp` client + `transport-streamable-http-client-reqwest` features, and `anyhow` as dev-dependencies
- [x] 5.2 Integration tests (`tests/mcp_http_flow.rs`): no `Authorization` header -> `401`; wrong bearer token -> `401`; correct token completes the real MCP `initialize` handshake and `list_all_tools` reports `browser_navigate` among the real tool set
- [x] 5.3 Integration test: two concurrent sessions each successfully call `browser_navigate` against their own independently-launched browser with no profile-directory collision (skips without a real installed browser, matching Phase 0's convention)
- [x] 5.4 `cargo test --workspace` on host (repeated runs, all green) and `bash docker/run-tests.sh` in the container, both green; also manually verified against the compiled `aib.exe mcp --http` binary with `curl` (401/401/200 with the real tool list in the response)

## 6. Wrap-up
- [x] 6.1 Update README with `aib mcp --http` usage (flags, the printed token, how to point an MCP client at it)
- [x] 6.2 Update PROPOSAL.md's Phase 5 roadmap
- [x] 6.3 `openspec archive mcp-streamable-http -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [x] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
