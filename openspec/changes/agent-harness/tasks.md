# Tasks: agent-harness

## 1. Crate scaffold + types
- [x] 1.1 New `crates/agent` workspace member (deps: engine, llm, tokio, serde, serde_json, thiserror, tracing, base64; dev-dep cdp for tests)
- [x] 1.2 `types.rs`: `SharedSession` (exact shape `AibTools` already stores its session in), `AgentEvent`, `TaskOutcome`
- [x] 1.3 `error.rs`: `AgentError` (LLM passthrough, no session, step/task budget exceeded, no progress, unknown tool, no vision role, unknown skill, io)

## 2. Tool surface + executor
- [x] 2.1 `tools.rs`: `tool_defs()` -- navigate/snapshot/click/type/press/wait_for/assert/screenshot/list_pages/switch_page/run_yaml + task_complete/task_failed
- [x] 2.2 `execute_tool`: dispatches directly against `engine::Session`; recoverable engine errors and malformed argument JSON become `Ok(ToolOutcome::Text("error: ..."))`, not `Err`; only a missing session or a genuinely unknown tool name is `Err(AgentError)`

## 3. Prompt + skills + vision
- [x] 3.1 `prompt.rs`: system prompt adapted from the MCP server's own instructions string, restricted to this tool subset, plus the termination contract, skills, and caller guidance in that order
- [x] 3.2 `skills.rs`: Markdown skill resolution (project-local -> per-user -> extra dirs, first match wins), front-matter stripped, unresolvable name errors clearly
- [x] 3.3 `vision.rs`: `interpret_screenshot` against a configured vision role with a default or caller-supplied guidance prompt

## 4. Step loop
- [x] 4.1 `harness.rs`: `Harness::run_task` -- build/send/append messages, execute tool calls sequentially, step/task timeouts, max-steps, no-progress-after-nudge termination
- [x] 4.2 Context pruning: snapshot-shaped tool results older than `max_retained_snapshots` elided before each driver call
- [x] 4.3 Vision routing keyed on `RoleClient.vision`: inline image (user-role message, not tool-role) for a vision-capable driver; routed to the vision role, text result, otherwise; clear error with neither

## 5. `llm-providers` extension (ad-hoc provider/model resolution)
- [x] 5.1 `crates/llm/src/config.rs`: `resolve_client_for_provider` shared helper (role-name-aware error messages), `resolve_role`/new `resolve_provider_model` both thin callers of it
- [x] 5.2 New `LlmError::UnknownProviderDirect`
- [x] 5.3 Unit test: direct provider/model resolution works with no `[roles]` table at all; unknown provider errors clearly

## 6. CLI
- [x] 6.1 `src/agent_cmd.rs` (new): resolves config/roles/skills, launches the browser, runs the task, renders progress (human-readable or `--json`), maps outcome to exit code
- [x] 6.2 `aib agent <task>` subcommand: `--skill` (repeatable), `--driver`/`--vision` (`<provider>/<model>` override), `--max-steps`, `--headed`, `--browser`, `--profile` (fixed `aib-agent` default), `--config`, `--json`

## 7. Verification
- [x] 7.1 New `crates/agent/tests/support/llm_stub.rs`: hand-rolled OpenAI-compatible `/chat/completions` stub (scripted FIFO responses, request recording) -- new code, can't reuse `crates/engine`'s or `crates/llm`'s own private test-only stub servers
- [x] 7.2 `tests/agent_loop.rs`: full navigate/type/click/assert/task_complete sequence against a real Chrome session and a real local stub (5 tests: full pass, stale-ref error feedback and recovery, context pruning across distinctly-marked pages, no-progress nudge-then-fail, max-steps budget)
- [x] 7.3 `tests/vision_routing.rs`: non-vision driver routes to a real second vision stub and receives text (not an image); vision-capable driver receives the raw image inline; no vision role configured fails clearly (3 tests)
- [x] 7.4 Manual end-to-end CLI smoke test: `aib agent` against a real separate-process Python HTTP stub and real headless Chrome, both human-readable and `--json` output modes, confirmed exit code 0 on pass
- [x] 7.5 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container, both green
- [x] 7.6 `cargo clippy --workspace --all-targets` clean

## 8. Wrap-up
- [ ] 8.1 Update README (`aib agent`, tool subset, vision routing, skills)
- [ ] 8.2 Update PROPOSAL.md's roadmap
- [ ] 8.3 `openspec archive agent-harness -y`
- [ ] 8.4 Three commits: Propose, Implement, Sync-specs-and-archive
