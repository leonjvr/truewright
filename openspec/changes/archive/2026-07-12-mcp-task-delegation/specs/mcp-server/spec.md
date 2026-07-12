## MODIFIED Requirements

### Requirement: Core tool set
The server SHALL expose the following tools: `browser_navigate(url)`, `browser_snapshot()`, `browser_click(ref)`, `browser_type(ref, text, submit?)`, `browser_press(key)`, `browser_wait_for(text, timeout_ms?)`, `browser_screenshot(interpret?, guidance?)`, `browser_close()`. `browser_navigate` and `browser_snapshot` SHALL return the rendered page-snapshot text as their result content. `browser_screenshot`'s optional `interpret` parameter, when true, SHALL return a text interpretation from the configured vision role instead of the raw image.

#### Scenario: Navigate returns a snapshot
- **WHEN** `browser_navigate` is called with a URL
- **THEN** the tool result contains the rendered snapshot text of the loaded page

#### Scenario: Screenshot defaults to returning the raw image
- **WHEN** `browser_screenshot` is called with no arguments
- **THEN** the tool result contains the raw PNG image, exactly as before this capability existed

#### Scenario: Screenshot can be interpreted instead of returned as an image
- **WHEN** `browser_screenshot` is called with `interpret: true` and a vision role is configured
- **THEN** the tool result contains the vision role's text interpretation instead of the image

## ADDED Requirements

### Requirement: Task delegation to the configured agent driver
The server SHALL expose a `browser_run_task(task, guidance?, skills?, max_steps?)` tool that runs the configured driver role's agent loop against the server's own shared browser session, and SHALL fail with a clear, actionable error -- not a crash or a silent no-op -- when no driver role is configured.

#### Scenario: A delegated task runs against the shared session
- **WHEN** `browser_run_task` is called with a driver role configured, and the driver's tool calls complete the task
- **THEN** the tool result reports a passing outcome and a step transcript, and the session's page reflects the actions taken

#### Scenario: A delegated task can be driven further by the outer host afterward
- **WHEN** `browser_run_task` completes (pass or fail)
- **THEN** the same session remains available to every other `browser_*` tool call that follows, unchanged from how the session behaves between any two ordinary tool calls

#### Scenario: No driver configured fails clearly
- **WHEN** `browser_run_task` is called and no `[roles.driver]` is configured
- **THEN** the tool call fails with an error identifying that no driver is configured and how to add one, rather than an opaque failure

#### Scenario: A failed delegated task is a tool-level error
- **WHEN** the driver calls its failure-termination tool, or the run hits its step/time budget
- **THEN** `browser_run_task` returns a tool-level error result carrying the transcript and reason, the same way `browser_assert`/`browser_run_yaml` report a failure

### Requirement: Recorded-video preview can also be interpreted
`browser_record_stop`'s preview frame SHALL accept the same optional `interpret`/`guidance` parameters as `browser_screenshot`, with the same default (raw image) behavior when omitted.

#### Scenario: Recording preview interpreted instead of returned as an image
- **WHEN** `browser_record_stop` is called with `interpret: true` and a vision role is configured, and a preview frame exists
- **THEN** the tool result contains the vision role's text interpretation of that frame instead of the image
