# Tasks: oauth-subscription-auth

## 1. PKCE + flow registry
- [x] 1.1 `auth/pkce.rs`: RFC 7636 verifier (32 random bytes, base64url) + S256 challenge, random `state`
- [x] 1.2 `auth/flows.rs`: `OAuthFlowSpec` (data, not code); `CHATGPT` entry verified against `openai/codex`'s own source (auth/token URLs, client id, scope, extra authorize params, redirect port 1455/fallback 1457, form-vs-JSON exchange asymmetry)
- [x] 1.3 `auth/jwt.rs`: unverified (by design -- our own freshly-issued token) JWT payload decode, for account-id/expiry claim extraction

## 2. Local callback + login orchestration
- [x] 2.1 `auth/callback.rs`: hand-rolled one-shot `TcpListener` server, `bind`/`accept_one` split so the caller can bind before printing the authorize URL (no race against a fast redirect); denied-consent (`error=`) and missing-code/state handled as clear failures, not hangs
- [x] 2.2 `auth/login.rs`: `login`/`login_with_flow` (bind -> print+open authorize URL -> await callback -> validate state -> exchange), best-effort cross-platform browser open (Windows/macOS/xdg-open) with the URL always printed regardless
- [x] 2.3 `exchange_code_with_flow` (form-encoded) / `refresh_with_flow` (JSON-encoded) as independently-callable, flow-spec-parameterized primitives -- not just the string-id-based public API, so they're testable against a local stub and usable with a future config-overridden flow

## 3. Token store + CredentialSource::OAuth
- [x] 3.1 `auth/store.rs`: `TokenStore`/`StoredTokens`, one JSON file per flow, `chmod 600` on Unix, `list()` for `aib auth status`
- [x] 3.2 `CredentialSource::OAuth` variant + `bearer()` (transparent refresh within 60s of expiry) + `account_id()`
- [x] 3.3 **Bug found and fixed**: storage keyed by `provider` name instead of `flow_id`, silently broken whenever they differ -- fixed to key consistently by `flow_id`; regression test added with deliberately mismatched names (see design.md)

## 4. ChatGPT Responses-API client
- [x] 4.1 `client_responses.rs`: `input`-array request building (system/user as content-part items, assistant tool-calls as one `function_call` item each, tool results as `function_call_output`), flat (non-function-wrapped) tool defs
- [x] 4.2 SSE aggregation into one `ChatResponse` (terminal-event detection by shape, not by tracking `event:` names), `ChatGPT-Account-ID` header, aib's own `originator` (not Codex's, see design.md Decision 4)
- [x] 4.3 `config.rs`: `ProviderConfig.oauth_flow`, `Config.token_store`, `ProviderKind::OpenAiResponses` now fully resolves (was a placeholder error in llm-providers)

## 5. CLI
- [x] 5.1 `aib auth login <flow>` / `status` / `logout <flow>`

## 6. Verification
- [x] 6.1 Unit tests: PKCE bounds/determinism/uniqueness, JWT payload decode (including malformed-input safety), authorize-URL construction, urlencode correctness
- [x] 6.2 Real local HTTP tests (`tests/oauth_flow.rs`): form-encoded exchange + account-id/expiry extraction from a real JWT, JSON-encoded refresh, non-success token response fails clearly, a real callback listener accepting a real HTTP GET (success + denied-consent paths), token store persistence
- [x] 6.3 Real local SSE tests (`tests/responses_client_flow.rs`): text reply and tool-call reply aggregated from a genuine `text/event-stream` byte stream (not a pre-parsed value), `ChatGPT-Account-ID` header confirmed present on a real outgoing request
- [x] 6.4 Regression test for the provider-name-vs-flow-id storage bug (deliberately mismatched names)
- [x] 6.5 Manual CLI smoke test: `aib auth status`/`logout` against a real pre-seeded token file in a real temp data dir, confirming end-to-end output formatting
- [x] 6.6 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container, both green
- [x] 6.7 `cargo clippy --workspace --all-targets` clean
- [ ] 6.8 **Deferred, user-assisted**: a real `aib auth login chatgpt` against the user's actual ChatGPT subscription, and one real completion through `ResponsesClient` against the real `chatgpt.com/backend-api/codex` backend -- not run without the user's direct involvement (see design.md's testing note)

## 7. Wrap-up
- [x] 7.1 Update README (OAuth login, config `oauth_flow`, `aib auth` commands, the unofficial/unverified-backend caveat)
- [x] 7.2 Update PROPOSAL.md's roadmap
- [ ] 7.3 `openspec archive oauth-subscription-auth -y`
- [ ] 7.4 Three commits: Propose, Implement, Sync-specs-and-archive
