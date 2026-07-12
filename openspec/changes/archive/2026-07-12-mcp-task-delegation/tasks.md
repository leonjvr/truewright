# Tasks: mcp-task-delegation

## 1. `llm`/`agent` Clone plumbing
- [x] 1.1 `#[derive(Clone)]` on `Client`, `CompatClient`, `ResponsesClient`, `RoleClient` (`crates/llm`)
- [x] 1.2 `#[derive(Clone)]` on `Harness` (`crates/agent`)

## 2. `crates/mcp` wiring
- [x] 2.1 New dependency on `agent` in `crates/mcp/Cargo.toml`
- [x] 2.2 `AgentConfig { harness: Arc<agent::Harness>, skill_dirs: Vec<PathBuf> }`; `AibTools` gains `agent: Option<AgentConfig>` + `.with_agent(...)` builder (existing constructors/call sites unchanged)
- [x] 2.3 `RunTaskRequest { task, guidance?, skills?, max_steps? }` + `browser_run_task` tool: shares the session via `SharedSession::from_arc`, resolves skills, builds a per-call `Harness` clone with the MCP-specific default `max_steps` (25) unless overridden, returns a transcript + PASS/FAIL as success, a transcript + reason as a tool-level error on failure/no-driver-configured
- [x] 2.4 `browser_screenshot` gains optional `interpret`/`guidance` params, routing through `Harness::interpret_image` when set; default behavior (no args) unchanged
- [x] 2.5 `browser_record_stop` gains the same optional params for its preview frame
- [x] 2.6 `instructions` string documents `browser_run_task` and the interpret params

## 3. CLI + startup wiring
- [x] 3.1 `aib mcp` gains `--config <path>`
- [x] 3.2 `src/mcp.rs`: `run`/`router`/`run_http` thread an `agent: Option<mcp_server::AgentConfig>` through to each `AibTools`
- [x] 3.3 `src/main.rs`: loads config once, resolves `[roles.driver]`/`[roles.vision]`, builds the optional `AgentConfig` -- a parse failure warns to stderr; no `[roles.driver]` configured stays silent (`agent: None`)

## 4. Verification
- [x] 4.1 New `tests/support/llm_stub.rs` at the workspace-binary level (mirrors `crates/agent/tests/support/llm_stub.rs`, private to its own crate's tests)
- [x] 4.2 Existing `tests/mcp_http_flow.rs` call sites updated for the new `agent` parameter (`None`, preserving today's behavior)
- [x] 4.3 New test: a full `browser_run_task` pass against a real fixture page and a real local stub, over a real streamable-HTTP MCP client
- [x] 4.4 New test: `browser_run_task` with no driver configured fails with a clear invalid-params error
- [x] 4.5 New test: `browser_screenshot(interpret: true)` returns text from a real second vision stub, not an image block
- [x] 4.6 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container, both green
- [x] 4.7 `cargo clippy --workspace --all-targets` clean
- [x] 4.8 Manual end-to-end smoke test: real compiled `aib mcp --http` binary, a real separate-process Python LLM stub, and a real headless Chrome session, driven by raw `curl` MCP requests -- confirmed `browser_run_task` returns the expected transcript + PASS outcome

## 5. Wrap-up
- [x] 5.1 Update README
- [x] 5.2 Update PROPOSAL.md's roadmap
- [x] 5.3 `openspec archive mcp-task-delegation -y`
- [x] 5.4 Three commits: Propose, Implement, Sync-specs-and-archive
