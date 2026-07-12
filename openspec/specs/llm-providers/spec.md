# llm-providers Specification

## Purpose
Provider-agnostic LLM access for `aib`'s own agent harness: a single OpenAI-compatible chat-completions client any provider (DeepSeek, MiniMax, GLM, Grok, OpenAI, ...) can be configured against by name, plus the role-based config that resolves a client with credentials attached. This is the foundation `agent-harness`'s driving loop and `mcp-task-delegation`'s MCP tools are built on -- it doesn't drive anything by itself yet.
## Requirements
### Requirement: Provider-agnostic chat-completions client
The system SHALL provide a client that sends chat-completion requests (system/user/assistant/tool messages, text and image content, function-calling tool definitions) to any OpenAI-compatible `/chat/completions` endpoint and parses its response into provider-neutral types, without any provider-specific code outside that one client module.

#### Scenario: A text-only conversation round-trips correctly
- **WHEN** a chat request containing only text messages is sent
- **THEN** the request serializes each message's content as a plain string (not a content-parts array), and the response's assistant text is parsed back correctly

#### Scenario: An image-bearing message is sent as a data URI
- **WHEN** a chat request includes a message with an image part
- **THEN** that message's content serializes as a content-parts array containing a `data:` URI image_url part, not a plain string

#### Scenario: Tool calls and tool results round-trip correctly
- **WHEN** a chat request includes an assistant message with tool calls and a following tool-result message
- **THEN** both serialize with the OpenAI function-calling wire shape (`type: "function"`, `function.name`/`function.arguments`), and a response containing tool calls parses back into typed `ToolCall`s with their raw argument JSON preserved unparsed

#### Scenario: Transient server errors are retried, non-transient ones are not
- **WHEN** the provider responds with HTTP 429 or a 5xx status
- **THEN** the request is retried up to twice more with backoff before failing
- **WHEN** the provider responds with a non-transient 4xx status (e.g. 400)
- **THEN** the request fails immediately with a typed error, with no retry

### Requirement: Config file loading with a safe default
The system SHALL load LLM provider/role configuration from a TOML file resolved in order (explicit path, `AIB_CONFIG` env var, project-local `./aib.toml`, per-user data-dir default), and SHALL treat a missing file at the resolved location as a valid, empty configuration rather than an error.

#### Scenario: No config file exists anywhere in the resolution chain
- **WHEN** no config file is found at any location in the resolution order
- **THEN** loading succeeds with an empty configuration, and any browser-only functionality that doesn't need an LLM role continues to work unaffected

#### Scenario: An explicit path overrides every other source
- **WHEN** an explicit config path is given
- **THEN** that file is loaded regardless of `AIB_CONFIG`, `./aib.toml`, or the per-user default's presence

### Requirement: Role resolution with clear, specific errors
The system SHALL resolve a named role (e.g. `"driver"`, `"vision"`) to a ready-to-use client by looking up the role, then its provider, then its credential (a literal `api_key`, an `api_key_env` environment variable, or an OAuth flow), and SHALL fail with a specific, actionable error identifying exactly which lookup failed rather than a generic failure.

#### Scenario: An undefined role is requested
- **WHEN** a role name not present under `[roles]` in the loaded config is resolved
- **THEN** resolution fails with an error naming that specific role as unknown

#### Scenario: A role references a provider that isn't configured
- **WHEN** a role's `provider` value has no matching `[providers.<name>]` entry
- **THEN** resolution fails with an error naming both the role and the missing provider

#### Scenario: A provider has no usable credential
- **WHEN** a provider config has none of `api_key`, `api_key_env`, or `oauth_flow` set
- **THEN** resolution fails with an error naming the provider and explaining how to configure a credential

#### Scenario: An OAuth-authenticated role with no completed login fails clearly
- **WHEN** a role resolves to a provider configured with `oauth_flow` but no login has been completed for that flow
- **THEN** the first request against it fails with an error directing the user to `aib auth login <flow>`, rather than an opaque authentication failure from the provider itself

### Requirement: CLI connectivity probe
The system SHALL provide an `aib llm ping <role>` command that resolves the given role and sends one real completion request, printing the resolved model, round-trip latency, and the reply text on success, or a clear error on failure.

#### Scenario: Pinging a correctly configured role succeeds
- **WHEN** `aib llm ping <role>` is run against a role whose provider is reachable and correctly credentialed
- **THEN** it prints the model name, a latency measurement, and the provider's reply text, and exits successfully

#### Scenario: Pinging a misconfigured role fails clearly
- **WHEN** `aib llm ping <role>` is run against an unknown role, a role with an unconfigured provider, or a provider with no credential
- **THEN** it prints the specific resolution error and exits with a failure code

### Requirement: PKCE authorization flow for subscription-based providers
The system SHALL support OAuth login for providers that authenticate via a browser-based PKCE authorization-code flow, binding a local callback listener, presenting the authorize URL to the user (printed, and best-effort opened in the system browser), and exchanging the returned code for tokens.

#### Scenario: A completed login persists usable tokens
- **WHEN** the user completes the browser sign-in and the local callback receives a matching `code`/`state`
- **THEN** the exchanged access/refresh tokens are persisted, associated with the flow that was logged into

#### Scenario: A state mismatch is rejected
- **WHEN** the local callback receives a `state` value that doesn't match the one generated for this login attempt
- **THEN** the login fails with a clear error rather than accepting the token exchange

#### Scenario: Denied consent is surfaced as a clear failure
- **WHEN** the user denies consent on the provider's authorization screen
- **THEN** the local callback's `error` parameter is surfaced as a clear login failure, not a silent hang or a generic timeout

### Requirement: Persisted, transparently refreshed OAuth tokens
The system SHALL persist OAuth tokens per flow in the per-user data directory, and SHALL transparently refresh a token that is near or past expiry before using it, without the caller needing to handle refresh explicitly.

#### Scenario: A near-expiry token is refreshed before use
- **WHEN** a stored token is within 60 seconds of its recorded expiry (or already expired) and a request needs it
- **THEN** the token is refreshed first, the refreshed tokens are persisted, and the request proceeds with the fresh token

#### Scenario: A token is found by the flow it belongs to, regardless of the provider's configured name
- **WHEN** a config's `[providers.<name>]` entry references an `oauth_flow` whose stored tokens exist
- **THEN** those tokens are found and used, even when `<name>` is not identical to the flow's own id

### Requirement: ChatGPT-subscription backend client
The system SHALL provide a client for the OpenAI Responses API shape used by ChatGPT-subscription access, translating the same provider-neutral chat types every other client uses into that shape's `input`/tool/SSE conventions, and aggregating its SSE-only response into a single non-streaming result.

#### Scenario: A text reply is aggregated from a real SSE stream
- **WHEN** the backend responds with a `text/event-stream` body whose terminal event contains assistant text
- **THEN** that text is returned as the response's message content

#### Scenario: A tool-call reply is aggregated from a real SSE stream
- **WHEN** the backend responds with a `text/event-stream` body whose terminal event contains one or more function calls
- **THEN** those are returned as typed tool calls with their call id, name, and raw arguments preserved

#### Scenario: The account id is sent on every request
- **WHEN** a request is sent using an OAuth credential with a stored account id
- **THEN** the request carries the account id in the header the backend requires

### Requirement: CLI login/status/logout
The system SHALL provide `aib auth login <flow>`, `aib auth status`, and `aib auth logout <flow>` commands to manage OAuth logins.

#### Scenario: Status lists every stored login with its expiry
- **WHEN** `aib auth status` is run with one or more completed logins stored
- **THEN** it lists each flow, its associated account (if known), and whether its token is still valid or will refresh on next use

#### Scenario: Logout removes stored tokens
- **WHEN** `aib auth logout <flow>` is run for a flow with stored tokens
- **THEN** those tokens are deleted and a subsequent `aib auth status` no longer lists that flow

