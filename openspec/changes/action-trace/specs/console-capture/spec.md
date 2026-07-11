## ADDED Requirements

### Requirement: Action entries interleaved into the active trace
When a console/exception trace is active, `Session`'s own action methods (navigate, click, type, press) SHALL append a one-line summary entry into that same trace, chronologically interleaved with console and exception entries by real wall-clock time.

#### Scenario: An action is recorded while a trace is active
- **WHEN** a trace is active and the agent performs a click
- **THEN** the saved trace contains an action entry summarizing that click

#### Scenario: Actions are not recorded when no trace is active
- **WHEN** no trace is active and the agent performs actions
- **THEN** those actions incur no trace-related work and nothing is recorded

#### Scenario: Action and console entries interleave by real time
- **WHEN** a trace is active and an action provokes a console message
- **THEN** the saved trace's action entry and the resulting console entry appear in the order they actually happened
