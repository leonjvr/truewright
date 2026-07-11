# html-trace-viewer Specification

## Purpose
Turn a captured console/action/screenshot trace into a single, self-contained HTML file an agent or human can open and read chronologically, without needing the original JSONL and screenshot files alongside it.

## Requirements
### Requirement: Screenshots captured into the active trace
The engine SHALL save the image and log a reference to it in the active trace whenever `browser_screenshot` is called while a trace is active, without failing the screenshot call if tracing fails.

#### Scenario: A screenshot taken during an active trace is captured
- **WHEN** `browser_screenshot` is called while a console/action trace is active
- **THEN** the trace gains a screenshot entry, and the screenshot call itself still succeeds and returns the image as normal

#### Scenario: Screenshot logging never fails the screenshot call
- **WHEN** saving or logging the screenshot into the trace fails for any reason
- **THEN** `browser_screenshot` still returns the captured image successfully

### Requirement: Render a saved trace as a self-contained HTML file
The engine SHALL render a previously-saved trace as a single, self-contained HTML file showing every entry in chronological order, with any captured screenshots embedded inline.

#### Scenario: A trace renders to a readable HTML timeline
- **WHEN** a saved trace containing console, exception, action, and screenshot entries is rendered
- **THEN** the resulting HTML file shows all of them in chronological order, distinguishable by kind, with screenshots visible inline without needing any other file alongside it

#### Scenario: Rendering is available both as a CLI command and an MCP tool
- **WHEN** an already-saved trace is rendered via `aib trace view <name>` or the `browser_render_trace` MCP tool
- **THEN** both produce the same HTML output and report where it was written

