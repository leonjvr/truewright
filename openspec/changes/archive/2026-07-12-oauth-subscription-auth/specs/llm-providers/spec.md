## MODIFIED Requirements

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

## ADDED Requirements

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
