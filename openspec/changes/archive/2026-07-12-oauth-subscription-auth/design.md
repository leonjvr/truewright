## Context

`llm-providers`' `CredentialSource` was deliberately designed (its own design.md, Decision 2) as a closed-but-growable enum behind one async `bearer()` method, specifically so an OAuth variant could be added without touching `CompatClient`/`ResponsesClient`'s call sites. This change is that addition, plus the ChatGPT-specific backend it exists to reach.

## Decision 1: Verify against OpenAI's own source, not third-party writeups

Every constant this change hardcodes (`auth.openai.com/oauth/authorize`, client id `app_EMoamEEZ73f0CkXaXp7hrann`, the exact scope string, the `chatgpt.com/backend-api/codex` backend URL, the `ChatGPT-Account-ID` header name) was confirmed by reading `openai/codex`'s own Rust source (`codex-rs/login/`, `codex-rs/model-provider*/`) directly, not from blog posts describing the flow secondhand. Two details a secondhand description would likely have gotten wrong or omitted entirely, caught this way:
- The initial code exchange is **form-encoded**; the refresh call to the *same* endpoint is **JSON-encoded**. Genuinely asymmetric, not a typo in Codex's own code -- confirmed by reading both call sites.
- The token endpoint's response has no `expires_in` field at all. Codex's own response struct doesn't even declare one. Expiry comes from decoding the `exp` claim inside the `id_token` JWT instead.

This is still unofficial surface. `OAuthFlowSpec` being a plain data struct (not hardcoded control flow) is the mitigation: if OpenAI changes an endpoint or a header name, that's a one-struct-literal edit, not a redesign, and a future `[oauth.chatgpt]` config-override table (not built in this change, but not foreclosed by this shape either) would let a user patch drift themselves without a new `aib` release.

## Decision 2: `provider` name vs. `flow_id` are NOT interchangeable as a storage key -- a real bug, found before shipping

Original implementation: `CredentialSource::OAuth { provider, flow_id, store }`, with `bearer()`/`account_id()` calling `store.load(provider)`. This looked reasonable and every test up to that point passed, because every test happened to name its provider identically to its flow id (`"chatgpt"` both places) -- the natural, common case. But `login_with_flow`/`refresh_with_flow` save tokens keyed by `flow.id`, not by whatever name a *config* gives the provider. A config like:
```toml
[providers.my-work-chatgpt]
kind = "openai-responses"
oauth_flow = "chatgpt"
```
would silently never find its own stored login -- `store.load("my-work-chatgpt")` looking for a file that was actually saved as `chatgpt.json`. Caught by deliberately writing one test (`stored_tokens_are_found_when_provider_name_differs_from_flow_id`) that used different names for the two, specifically because every other test's coincidental name-matching made this invisible. Fixed by keying storage consistently by `flow_id` everywhere; `provider` survives only in error messages (`"run aib auth login <provider>"`), where showing the user's own configured name is what's actually useful.

This is recorded here as a general lesson for this project's own test-writing habit, not just a one-off fix: **a test that reuses the same value in two conceptually-distinct fields can hide a bug that only manifests when a caller legitimately gives them different values.** The `cross-origin-oopif` change's env-var-race lesson (this session's own memory) was about test *concurrency*; this one is about test *data shape* -- both are "the test technically passed but wasn't actually exercising the real invariant."

## Decision 3: A dedicated `ResponsesClient`, sharing nothing but the neutral types with `CompatClient`

The Responses API is not a variant of chat completions -- `input` (not `messages`) as a flat array where an assistant tool call and a text reply are represented as *different item shapes* (`function_call` vs. `message`), tool definitions with no `function` wrapper, and (for this specific backend) an SSE-only response. `client_responses.rs` owns this wire shape entirely privately, converting to/from the same `Message`/`Part`/`ToolCall`/`ChatResponse` types `client_compat.rs` uses -- so nothing above the `Client` enum (the agent loop, eventually) needs to know or care which shape a given role's provider actually speaks.

SSE aggregation deliberately doesn't try to reconstruct anything from incremental `response.output_text.delta` events -- it just watches for the terminal `response.completed` event (identified structurally, by successfully parsing a `{"response": {"output": [...]}}` envelope, not by tracking the SSE `event:` line name) and uses whichever one arrived last before the stream closes. This is correct because delta events are redundant with the final completed event's full text, and the public interface here is non-streaming anyway (`llm-providers` design.md, Decision "Non-streaming per turn confirmed as the right v1 call") -- there is no reason to do the extra bookkeeping partial-delta reconstruction would need.

## Decision 4: `originator: aib_agent_harness`, not `codex_cli_rs`

Codex CLI sends `originator: codex_cli_rs` on every request against this backend. Sending that same value from `aib` would make requests indistinguishable from the real Codex CLI to OpenAI's own backend -- which is a form of misrepresenting what client is actually making the call, not just a technical detail. `aib` sends its own identifier instead. This is a real, acknowledged risk (noted in `proposal.md`'s scope and in `llm-providers`' own risk list): an unrecognized originator value could plausibly be rejected or rate-limited differently by a backend that only expects a small known set of client identifiers. It's the honest choice regardless, and if it turns out to be rejected in practice, that surfaces immediately and clearly (an HTTP error, not silent misbehavior) the first time a real ChatGPT-subscription request is attempted -- which per Decision 5 below hasn't happened yet in this environment.

## Decision 5: What "verified" honestly means for this change

Every HTTP-shaped piece of this change runs against a **real local server or socket** in the test suite -- not hand-constructed `serde_json::Value`s fed straight to a parser:
- `exchange_code_with_flow`/`refresh_with_flow` against a real local HTTP stub, asserting the real `Content-Type` each uses (form vs. JSON) and that account-id/expiry extraction from a real (self-signed, unverified-signature) JWT works.
- The local callback listener (`bind_callback`/`accept_callback`) against a real `TcpListener` and a real HTTP GET simulating the browser's redirect, including the denied-consent (`error=...`) path.
- `ResponsesClient` against a real local server emitting an actual `text/event-stream` byte stream (not a single pre-parsed JSON value), for both a text reply and a tool-call reply, plus a real-request assertion that the `ChatGPT-Account-ID` header is actually present on the wire.
- The full CLI (`aib auth status`/`logout`) against a real pre-seeded token file in a real temp data directory, confirming output formatting end-to-end.

**What is not verified, and why that's stated plainly rather than glossed over:** the real `auth.openai.com` authorize/token endpoints, and the real `chatgpt.com/backend-api/codex` backend, have not been called in this environment. Doing so requires a real browser-based login against an actual ChatGPT subscription -- a human has to click through OpenAI's own consent screen. This is flagged as a manual, user-assisted verification step, consistent with this session's standing practice of checking in before any action with a real, external, human-involving side effect -- not run without the user's direct involvement and awareness.
