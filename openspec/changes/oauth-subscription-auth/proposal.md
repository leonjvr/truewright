## Why

`llm-providers` covers key-based auth (a literal `api_key` or `api_key_env`) -- enough for DeepSeek, MiniMax, GLM, Grok, and platform OpenAI keys. It doesn't cover the one real subscription-based case that needs a genuine OAuth flow: OpenAI's ChatGPT Plus/Pro/Team/Enterprise subscription, which authenticates via a PKCE authorization-code flow against `auth.openai.com` and drives completions through a different backend (`chatgpt.com/backend-api/codex`, the Responses API shape) than platform API keys use. This is the second of four sequential changes building `aib`'s own agent harness; MiniMax/GLM "coding plan" subscriptions needed no new work here -- they're key-based, already covered by `llm-providers`.

This is explicitly unofficial, interoperable-client territory: OpenAI hasn't published this as a public integration surface, but it's the same mechanism their own open-source Codex CLI uses, and several other tools (Zed, opencode) already implement equivalent clients. Every constant below (`auth.openai.com` endpoints, the client id, the `chatgpt.com/backend-api/codex` backend, header names) was verified directly against `openai/codex`'s own source at implementation time, not third-party writeups -- and it's stated plainly that this can drift if OpenAI changes it, since nothing here is a documented, versioned public API.

## What Changes

- **PKCE authorization flow** (`crates/llm/src/auth/pkce.rs`, `login.rs`): RFC 7636 verifier/S256 challenge generation, a one-shot local callback listener (`crates/llm/src/auth/callback.rs`, hand-rolled -- not `axum`, since this crate has no other reason to depend on a web framework), best-effort system-browser launch with the authorize URL always printed too (so a headless/SSH/container environment still works via copy-paste).
- **Token exchange and refresh** (`login.rs`): the initial code exchange is form-encoded; refresh is JSON-encoded -- a real, easy-to-miss asymmetry confirmed by reading both call sites in Codex's own source, not a copy-paste mistake here. The account id (`ChatGPT-Account-ID` header, required by the backend) comes from a claim inside the `id_token` JWT, not a separate API call; likewise the token's expiry comes from the JWT's own `exp` claim, since the token endpoint's response here doesn't include a separate `expires_in` field.
- **Token store** (`store.rs`): one JSON file per flow at `<data-dir>/aib/auth/<flow>.json`, `chmod 600` on Unix. `CredentialSource` gains an `OAuth` variant; `bearer()` refreshes transparently within 60 seconds of expiry (or on demand if already expired), so nothing above the credential layer needs to know a token was ever stale.
- **`ResponsesClient`** (`crates/llm/src/client_responses.rs`): a second, genuinely different wire client for the OpenAI Responses API shape (`input` items instead of `messages`, flat tool defs, SSE-only backend aggregated internally into the same non-streaming `ChatResponse` every other client returns). `ProviderKind::OpenAiResponses`, left as an honest "not implemented yet" placeholder in `llm-providers`, is fully wired up now.
- **CLI**: `aib auth login <flow>` / `aib auth status` / `aib auth logout <flow>`.
- **Flow registry is data, not code** (`flows.rs`): `OAuthFlowSpec` is a plain struct; the `chatgpt` entry is the only one today, but a second OAuth provider later is a new struct literal, not new control flow.

**A real design bug found and fixed before shipping** (not after): `CredentialSource::OAuth`'s `bearer()`/`account_id()` originally loaded stored tokens by the config's `provider` name, while `login`/`refresh` save them keyed by the flow's own `id` -- silently broken the moment a user names a provider anything other than exactly the flow id (e.g. `[providers.my-work-account]` with `oauth_flow = "chatgpt"`). Every test written up to that point happened to use matching names, so it went uncaught until a deliberately-mismatched-names test was added specifically to probe this. Fixed by keying storage consistently by `flow_id`; `provider` is now used only for user-facing error text. See design.md.

**Explicitly out of scope (deferred), and why:**
- **The actual driving loop.** This change makes ChatGPT-subscription auth *available*, not *used* -- `crates/agent`'s loop (change 3) is what actually calls `complete()` in a tool-calling cycle.
- **Device-code / headless login.** The local-callback flow needs a browser reachable from the machine running `aib`. A headless/remote-login alternative is a real gap (flagged in llm-providers' design.md risks) but not solved here.
- **Live verification against the user's real ChatGPT account.** Everything HTTP-shaped is verified against real local servers (see Testing below); the actual `auth.openai.com` authorize/token round trip and the real `chatgpt.com/backend-api/codex` backend are not independently confirmed in this environment -- this needs the user's own browser and subscription, and is called out as a manual, user-assisted verification step rather than silently assumed to work.

## Capabilities

### Modified Capabilities
- `llm-providers`: `ProviderKind::OpenAiResponses` goes from "parses, errors on resolve" to fully implemented; `CredentialSource` gains OAuth as a second credential kind alongside the static one.

## Impact

- `crates/llm/src/auth/` (new directory, was a single `auth.rs` file): `callback.rs`, `flows.rs`, `jwt.rs`, `login.rs`, `pkce.rs`, `store.rs`, `mod.rs`.
- `crates/llm/src/client_responses.rs` (new).
- `crates/llm/Cargo.toml`: `sha2` (new, PKCE S256), `reqwest` gains `stream` (SSE) and `form` (code exchange) features, `futures-util`/`base64`/`rand` promoted from transitive to direct deps.
- `src/main.rs`: new `aib auth login/status/logout` subcommands.
- No changes to `crates/cdp`, `crates/engine`, or `crates/mcp`.
