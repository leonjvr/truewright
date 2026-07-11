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
The system SHALL resolve a named role (e.g. `"driver"`, `"vision"`) to a ready-to-use client by looking up the role, then its provider, then its credential, and SHALL fail with a specific, actionable error identifying exactly which lookup failed rather than a generic failure.

#### Scenario: An undefined role is requested
- **WHEN** a role name not present under `[roles]` in the loaded config is resolved
- **THEN** resolution fails with an error naming that specific role as unknown

#### Scenario: A role references a provider that isn't configured
- **WHEN** a role's `provider` value has no matching `[providers.<name>]` entry
- **THEN** resolution fails with an error naming both the role and the missing provider

#### Scenario: A provider has no usable credential
- **WHEN** a provider config has neither `api_key` nor a set `api_key_env` environment variable
- **THEN** resolution fails with an error naming the provider and explaining how to configure a credential

### Requirement: CLI connectivity probe
The system SHALL provide an `aib llm ping <role>` command that resolves the given role and sends one real completion request, printing the resolved model, round-trip latency, and the reply text on success, or a clear error on failure.

#### Scenario: Pinging a correctly configured role succeeds
- **WHEN** `aib llm ping <role>` is run against a role whose provider is reachable and correctly credentialed
- **THEN** it prints the model name, a latency measurement, and the provider's reply text, and exits successfully

#### Scenario: Pinging a misconfigured role fails clearly
- **WHEN** `aib llm ping <role>` is run against an unknown role, a role with an unconfigured provider, or a provider with no credential
- **THEN** it prints the specific resolution error and exits with a failure code

