## Why

`aib` today is a browser-automation *tool* other people's LLM agents drive over MCP. This is the first of four changes that let `aib` also drive itself: a user configures any LLM provider (Grok, GLM, DeepSeek, OpenAI, MiniMax, ...) and model, and `aib` runs the tool-calling loop internally against its own browser session. Almost every one of those providers speaks the same OpenAI-compatible `/chat/completions` wire shape with function calling, so the actual new capability this change ships is a provider-agnostic client for that shape, plus the config format that lets a user point it at any of them by name -- everything downstream (change 3's agent loop, change 4's MCP delegation) builds on this without touching provider-specific code again. Subscription-based OAuth auth (change 2) and the actual driving loop (change 3) are explicitly out of scope here -- this change is the client + config foundation alone, independently useful via `aib llm ping <role>` and independently testable.

## What Changes

- **New `crates/llm` crate**: wire-neutral chat types (`Message`/`Part`/`ToolCall`/`ToolDef`/`ChatRequest`/`ChatResponse`) that carry no provider-specific serde shape -- conversion to a given provider's wire format lives entirely in that provider's own client module, so the Responses-API shape change 2 adds never touches these.
- **`CompatClient`**: the OpenAI-compatible `/chat/completions` client every provider in this change's scope speaks -- DeepSeek, MiniMax, GLM, Grok, and OpenAI itself via a plain API key. Text and image (`data:` URI) content parts, function-calling tool defs and tool-call/tool-result messages, HTTP 429/5xx retried with exponential backoff (3 attempts), everything else a typed `LlmError`.
- **Config**: `<data-dir>/aib/config.toml` (resolution order: `--config` flag > `AIB_CONFIG` env var > `./aib.toml` project-local > the per-user data dir default -- a missing file at the end of that chain is a *valid empty config*, not an error, since `aib`'s existing browser tools must keep working with zero LLM setup). `[providers.<name>]` (kind, base_url, api_key/api_key_env, extra headers) and `[roles.<name>]` (provider, model, vision flag) sections; `[agent]` settings (max_steps, timeouts, context-pruning depth) parsed now even though nothing consumes them until change 3, so the config schema doesn't grow a breaking change later.
- **Credential resolution**: `CredentialSource` -- literal `api_key` or `api_key_env` (read once at role-resolution time) in this change; structured so change 2 adds an `OAuth` variant without changing any call site (`CompatClient` only ever calls `.bearer()`).
- **`aib llm ping <role>`**: resolves a configured role and sends one trivial completion, printing the model, round-trip latency, and the reply -- the live-verification hook for this change, and a genuinely useful connectivity check on its own.
- **`ProviderKind::OpenAiResponses`** is accepted by config parsing (so the schema is forward-compatible) but resolving a role that uses it returns a clear "not implemented yet" error -- change 2 implements it, this change doesn't pretend to.

**Explicitly out of scope (deferred), and why:**
- **OAuth / subscription auth.** Real scope on its own (PKCE, callback server, token store, refresh) -- change 2.
- **The agent loop itself.** This change proves the client works; nothing here calls it in a loop yet -- change 3.
- **MCP surface.** No `browser_run_task`, no screenshot interpretation -- change 4, once the loop exists to delegate to.
- **Streaming.** `complete()` is non-streaming per turn -- progress users actually care about is per-tool-call (change 3's concern), not per-token; the one place a provider's backend is SSE-only (change 2's ChatGPT-subscription backend) hides that behind the same non-streaming interface, not exposed here.

## Capabilities

### New Capabilities
- `llm-providers`: provider-agnostic chat-completions client, config loading, role resolution, and a CLI connectivity probe.

## Impact

- `Cargo.toml`: new workspace member `crates/llm`; new deps `toml`, `sha2`-adjacent nothing yet (that's change 2); `reqwest` promoted from a dev-only dependency to a real runtime one (with the `rustls` feature -- the workspace's existing `reqwest` entry has no TLS backend at all).
- `src/main.rs`: new `aib llm ping <role>` subcommand.
- No changes to `crates/cdp`, `crates/engine`, or `crates/mcp` -- this change is additive and self-contained.
