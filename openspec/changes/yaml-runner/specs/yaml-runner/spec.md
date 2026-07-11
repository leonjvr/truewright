## ADDED Requirements

### Requirement: Declarative YAML step execution
The engine SHALL support executing a YAML document describing an ordered list of steps (`navigate`, `click`, `type`, `press`, `wait_for`, `assert`) against the current session, in order, using the same underlying action methods a live MCP tool call would use.

#### Scenario: A valid script runs successfully
- **WHEN** a YAML script's steps all succeed against the current page
- **THEN** the run reports success and how many steps ran

#### Scenario: A failing step stops the run
- **WHEN** a YAML script's step fails (e.g. an `assert` step whose condition doesn't hold)
- **THEN** the run stops at that step and reports which step failed and why, without executing subsequent steps

### Requirement: Trace export to a runnable YAML script
The engine SHALL support converting a saved console/action trace's action entries into a YAML script in the same step format, usable directly as `browser_run_yaml` input.

#### Scenario: A captured trace exports to a runnable script
- **WHEN** a trace containing action entries (navigate, click, type) is exported
- **THEN** the resulting YAML script's steps correspond to those actions in their original order

#### Scenario: Observability entries are not exported as steps
- **WHEN** a trace contains console or exception entries alongside action entries
- **THEN** the exported script contains steps only for the action entries, not the console/exception ones
