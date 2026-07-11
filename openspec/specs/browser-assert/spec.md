# browser-assert Specification

## Purpose
The explicit pass/fail check an actual test assertion needs: check the current page snapshot right now, succeed silently if it holds, and fail as a genuine tool-call failure an agent can branch on if it doesn't — distinct from `browser_wait_for`'s poll-until-true-or-timeout shape, which solves a different problem.
## Requirements
### Requirement: Immediate text-presence assertion
The engine SHALL support checking, immediately and without polling, whether a substring is present (or absent) in the current page snapshot, succeeding silently when the check holds and failing with a clear expected-vs-actual message when it does not.

#### Scenario: A passing assertion succeeds
- **WHEN** the current snapshot contains the asserted text and presence is expected
- **THEN** the assertion succeeds with no error

#### Scenario: A failing assertion fails clearly
- **WHEN** the current snapshot does not contain the asserted text and presence is expected
- **THEN** the assertion fails with a message identifying what was expected and that it was not found

#### Scenario: An absence assertion succeeds when the text is not present
- **WHEN** absence is expected and the current snapshot does not contain the asserted text
- **THEN** the assertion succeeds with no error

### Requirement: Assertion failure is a tool-level failure, not a protocol error
A failed assertion MUST surface to the calling agent as a genuine tool-call failure result distinct from a malformed-request or server-fault protocol error.

#### Scenario: A failed assertion is a tool error result
- **WHEN** an assertion fails
- **THEN** the MCP tool call returns an error result the agent can see and branch on, not an opaque protocol-level error

