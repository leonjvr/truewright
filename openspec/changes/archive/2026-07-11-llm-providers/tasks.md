# Tasks: llm-providers

## 1. Crate scaffold
- [x] 1.1 New `crates/llm` workspace member; `Cargo.toml` deps (`reqwest` +`rustls` feature locally, `toml`, existing workspace deps)
- [x] 1.2 `types.rs`: wire-neutral `Role`/`Part`/`Message`/`ToolCall`/`ToolDef`/`ChatRequest`/`ChatResponse`/`FinishReason`/`Usage`, no provider-specific serde shape
- [x] 1.3 `error.rs`: `LlmError` (config read/parse, unknown role/provider, missing/no credential, not-yet-implemented, HTTP transport/status/parse)

## 2. Config + credentials
- [x] 2.1 `auth.rs`: `CredentialSource::Static(String)` behind one async `bearer()` method (structured so change 2 adds `OAuth` without touching call sites)
- [x] 2.2 `config.rs`: `ProviderKind` (openai-compat implemented, openai-responses parses but errors clearly on resolve), `ProviderConfig`/`RoleConfig`/`AgentSettings`/`SkillsConfig`, `Config::load` (4-way precedence, missing file = empty config), `Config::resolve_role`

## 3. OpenAI-compatible client
- [x] 3.1 `client_compat.rs`: private `Wire*` request/response structs, text-vs-content-parts serialization, function-calling tool defs/calls, response parsing including usage
- [x] 3.2 `CompatClient::complete`: real HTTP POST, bearer auth, per-provider extra headers, 429/5xx retry with backoff (3 attempts), non-retryable errors surface immediately
- [x] 3.3 `client.rs`: `Client` enum (`Compat` implemented; `Responses` variant deferred to change 2), `RoleClient { client, model, vision }`

## 4. CLI
- [x] 4.1 `aib llm ping <role>` subcommand: resolves the role, sends one trivial completion, prints model/latency/reply; `--config` override

## 5. Verification
- [x] 5.1 Unit tests (`config.rs`): full-config parse + role resolution, missing-file-is-empty, unknown role/provider, no-credential, not-yet-implemented, env-var precedence + credential resolution (single test function per the project's own env-var-race lesson)
- [x] 5.2 Unit tests (`client_compat.rs`): text-only serialization, image-part data-URI serialization, tool-call/tool-result wire shape, text response parsing, tool-call response parsing
- [x] 5.3 Real local HTTP integration tests (`tests/compat_client_flow.rs` + new `tests/support/stub_server.rs`): real socket round trip with Authorization header assertion, 5xx-then-success retry, non-retryable 4xx surfaces immediately without retry
- [x] 5.4 Live-provider smoke test (`tests/live_smoke.rs`), gated on `DEEPSEEK_API_KEY` -- confirmed to skip cleanly with no key present in this environment
- [x] 5.5 Manual end-to-end CLI smoke test: `aib llm ping driver` against a real local Python HTTP stub via `AIB_CONFIG`, confirming the full CLI -> config -> role resolution -> real HTTP call -> output path works, not just the library in isolation
- [x] 5.6 `cargo test --workspace` on host (green) and `bash docker/run-tests.sh` in the container (green)
- [x] 5.7 `cargo clippy --workspace --all-targets` clean

## 6. Wrap-up
- [x] 6.1 Update README (new crate, config file, `aib llm ping`)
- [x] 6.2 Update PROPOSAL.md's roadmap
- [x] 6.3 `openspec archive llm-providers -y`
- [x] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
