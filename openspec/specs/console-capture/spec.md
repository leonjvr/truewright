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

