# agent-harness Specification

## Purpose
Run a configured LLM driver role as `truewright`'s own autonomous agent, driving a browser session through a tool-calling loop with recoverable-failure feedback, context pruning, capability-flag-routed vision, and attachable skills, without depending on an external MCP host to supply the model.
## Requirements
### Requirement: Autonomous tool-calling loop over a browser session
The system SHALL run a configured LLM driver role in a loop that calls browser tools against a session, feeding each tool's result back to the model, until the model signals completion or failure, or a step/time budget is exhausted.

#### Scenario: A scripted sequence of actions completes the task
- **WHEN** the driver produces navigate, then interaction, then verification tool calls, then calls the completion tool
- **THEN** each tool call is executed against the real session, its result is fed back to the model, and the run ends with a passing outcome reporting the number of steps used

#### Scenario: The step budget is enforced
- **WHEN** the driver never calls the completion or failure tool before the configured maximum step count is reached
- **THEN** the run ends with a clear step-budget-exceeded error rather than continuing indefinitely

#### Scenario: Two consecutive non-progressing turns end the run
- **WHEN** the driver responds with no tool call and no termination call twice in a row (after one nudge)
- **THEN** the run ends with a clear no-progress error rather than continuing to prompt indefinitely

### Requirement: Recoverable tool failures are fed back to the model, not treated as fatal
The system SHALL represent a recoverable per-action failure (a stale reference, a timed-out wait, a failed assertion, an unrecognized page or key) as the tool's result text, and SHALL continue the loop, rather than aborting the run.

#### Scenario: A stale ref error is fed back and the model recovers
- **WHEN** a tool call references an element ref that no longer resolves
- **THEN** the next request to the model includes that error as the tool's result, and the run continues to a subsequent step rather than failing immediately

### Requirement: Context pruning keeps older page snapshots from growing unbounded
The system SHALL retain only the most recent configured number of full page-snapshot tool results in the model's context, replacing older ones with a short placeholder before each request.

#### Scenario: Older snapshots are elided once the retained limit is exceeded
- **WHEN** more page snapshots have accumulated in the conversation than the configured retention limit
- **THEN** the oldest excess snapshots are replaced with a placeholder in the next request, while the most recent ones and every non-snapshot tool result remain intact

### Requirement: Screenshot routing follows the driver role's vision capability
The system SHALL send a screenshot inline as image content when the driver role is vision-capable, and SHALL instead route it to a configured vision role for text interpretation when the driver role is not vision-capable, returning that interpretation as the tool result.

#### Scenario: A non-vision driver receives a text interpretation
- **WHEN** the driver role has no vision capability and a screenshot tool call is made
- **THEN** the screenshot is sent to the configured vision role together with any guidance, and the vision role's text interpretation -- not the image itself -- is returned as the tool result

#### Scenario: A vision-capable driver receives the image directly
- **WHEN** the driver role has vision capability and a screenshot tool call is made
- **THEN** the raw image is included in the next message sent to the driver, without being routed through a separate vision role

#### Scenario: No vision role configured for a non-vision driver fails clearly
- **WHEN** the driver role has no vision capability and no vision role is configured, and a screenshot tool call is made
- **THEN** the tool result explains that no vision role is configured and how to add one, rather than failing opaquely

### Requirement: Skills attach reusable guidance to a task
The system SHALL resolve named skills from Markdown files (searched project-local, then per-user, then any configured extra directories, in that order) and include their content in the model's system prompt, and SHALL fail clearly when a named skill cannot be found in any location.

#### Scenario: A resolvable skill is included in the prompt
- **WHEN** a task is run with an attached skill name that exists in a search directory
- **THEN** that skill's content appears in the system prompt sent to the driver

#### Scenario: An unresolvable skill name fails the run before it starts
- **WHEN** a task is run with an attached skill name that doesn't exist in any search directory
- **THEN** the run fails clearly identifying the missing skill, rather than silently proceeding without it

### Requirement: CLI to run a task autonomously
The system SHALL provide a `truewright agent <task>` command that runs the task to completion using the configured (or overridden) driver role, printing live step progress, and exiting with a status code reflecting the outcome.

#### Scenario: A completed task exits successfully
- **WHEN** `truewright agent <task>` runs to a passing outcome
- **THEN** it prints the final summary and exits with a success code

#### Scenario: A failed task exits with a failure code
- **WHEN** `truewright agent <task>` runs to a failing outcome, or hits a step/time budget, or a no-progress condition
- **THEN** it prints the reason and exits with a non-zero failure code

#### Scenario: Driver/vision can be overridden without editing config
- **WHEN** `truewright agent` is run with a `<provider>/<model>` override for the driver or vision role
- **THEN** that provider and model are used directly for the run, without requiring a matching `[roles.*]` entry in the config file

