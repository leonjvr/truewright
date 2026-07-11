# browser-recording

## Purpose

Short video capture of the page via CDP's screencast mechanism, so an agent can verify moving parts (animations, transitions, drag interactions) actually behaved correctly — not just that one static screenshot looks right.

## Requirements

### Requirement: Screencast-based frame capture
The engine SHALL start capturing frames via CDP `Page.startScreencast` (JPEG format) when a recording is started, collecting each `Page.screencastFrame` event's image bytes and timestamp, and acknowledging each frame via `Page.screencastFrameAck` so the stream continues. Capture SHALL stop when explicitly stopped, or automatically once a maximum duration elapses (default 30 seconds), whichever comes first.

#### Scenario: Frames are collected while recording
- **WHEN** a recording is started and the page changes visibly (e.g. a CSS animation) for at least one second
- **THEN** stopping the recording yields more than one captured frame

#### Scenario: Automatic stop after the maximum duration
- **WHEN** a recording is started and never explicitly stopped
- **THEN** frame capture halts on its own once the maximum duration elapses, rather than growing unbounded

### Requirement: Recording artifacts
Stopping a recording SHALL write the captured JPEG frames and a JSON manifest (one entry per frame with its timestamp) to a per-recording directory, and SHALL assemble an animated GIF from the frame sequence using per-frame delays derived from the real capture timestamps.

#### Scenario: Artifacts exist after stopping
- **WHEN** a recording with at least two frames is stopped
- **THEN** the recording directory contains the individual JPEG frames, a manifest listing their timestamps, and a non-empty animated GIF

### Requirement: MCP recording tools
The MCP server SHALL expose `browser_record_start(max_duration_ms?, quality?)` and `browser_record_stop()`. `browser_record_start` MUST fail with a typed error if a recording is already in progress for the session. `browser_record_stop` SHALL return the artifact directory path, frame count, capture duration, and one representative frame from partway through the recording as inline image content — not the full frame sequence.

#### Scenario: Starting a second recording fails
- **WHEN** `browser_record_start` is called while a recording is already active
- **THEN** the tool call returns an error rather than silently starting a second capture

#### Scenario: Stop returns a preview frame
- **WHEN** `browser_record_stop` completes successfully
- **THEN** the response includes one image content block from partway through the captured sequence, alongside the artifact path and frame count
