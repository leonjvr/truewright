## MODIFIED Requirements

### Requirement: Role resolution with clear, specific errors
The system SHALL resolve a named role (e.g. `"driver"`, `"vision"`) to a ready-to-use client by looking up the role, then its provider, then its credential (a literal `api_key`, an `api_key_env` environment variable, or an OAuth flow), and SHALL fail with a specific, actionable error identifying exactly which lookup failed rather than a generic failure. The system SHALL also support resolving a client directly from a configured provider name and an explicit model string, bypassing the `[roles.*]` table entirely.

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

#### Scenario: A provider is resolved directly by name, without a matching role
- **WHEN** a configured provider name and an explicit model string are resolved directly, with no `[roles.*]` entry referencing that provider
- **THEN** resolution succeeds using that provider's configured credential, exactly as it would if reached through a role

#### Scenario: Direct resolution of an unconfigured provider fails clearly
- **WHEN** a provider name with no matching `[providers.<name>]` entry is resolved directly
- **THEN** resolution fails with an error naming that provider, without referencing a role that was never involved
