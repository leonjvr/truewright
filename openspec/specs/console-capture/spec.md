# console-capture Specification

## Purpose
The fastest way to find out why a test failed or an app behaved unexpectedly: capture what the page itself logged and threw, not just what the accessibility snapshot or a screenshot shows. A named, chronological JSONL trace of console output and uncaught exceptions, captured between an explicit start and stop.
## Requirements
### Requirement: Console and exception capture to a named JSONL trace
The engine SHALL support capturing every `console.*` call and uncaught exception observed between `browser_console_start` and `browser_console_stop`, persisting them in chronological order as a named JSON Lines (JSONL) trace file.

#### Scenario: A console message is captured
- **WHEN** capture is active and the page calls `console.log("hello")`, and capture is then stopped
- **THEN** the saved trace contains an entry recording that message's level and text

#### Scenario: An uncaught exception is captured
- **WHEN** capture is active and the page throws an uncaught exception, and capture is then stopped
- **THEN** the saved trace contains an entry recording that exception

#### Scenario: Entries are in chronological order
- **WHEN** capture is active and the page logs multiple messages and throws in a specific order
- **THEN** the saved trace's entries appear in that same order

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

