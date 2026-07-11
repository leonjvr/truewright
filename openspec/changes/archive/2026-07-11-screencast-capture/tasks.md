# Tasks: Screencast Capture

## 1. CDP layer

- [x] 1.1 `page.rs`: `StartScreencast`/`StartScreencastParams` (format=jpeg, quality, maxWidth, maxHeight), `StopScreencast`, `ScreencastFrameAck`/`ScreencastFrameAckParams`
- [x] 1.2 `page.rs`: `ScreencastFrame` event (`data` base64, `metadata.timestamp`, frame ack id — named distinctly from CDP session id to avoid confusion)
- [x] 1.3 `ops.rs`: `Page` gains `start_screencast`, `stop_screencast`, `ack_screencast_frame`, `events::<E>()` delegate; derive `Clone`

## 2. Engine recording

- [x] 2.1 `crates/engine/src/recording.rs`: `RecordingOptions` (max_duration, quality), `Frame` (bytes, timestamp), `RecordingOutput` (dir, frame_count, duration, gif_path, preview_frame)
- [x] 2.2 Background collector: spawned task consuming `ScreencastFrame` events, acking each, pushing into a shared buffer; stops on explicit signal or max-duration deadline
- [x] 2.3 `Session::start_recording(options) -> Result<Recording>`; `Recording::stop(self) -> Result<RecordingOutput>` writes JPEGs + manifest.json, assembles GIF via `image` crate
- [x] 2.4 `image` dependency added to `crates/engine/Cargo.toml`

## 3. MCP tools

- [x] 3.1 `browser_record_start(max_duration_ms?, quality?)`: errors if a recording is already active
- [x] 3.2 `browser_record_stop()`: returns artifact dir, frame count, duration, and a mid-clip preview frame as image content

## 4. Fixture + tests

- [x] 4.1 `crates/engine/tests/fixtures/animated.html`: a visibly moving/counting element (CSS animation or JS-driven)
- [x] 4.2 Integration test: start recording, wait, stop, assert frame count > 1, manifest well-formed, GIF non-empty — skip-if-no-browser marker
- [x] 4.3 Unit test: GIF assembly / manifest writing against synthetic in-memory frames (no browser needed)

## 5. Verification

- [x] 5.1 Host: full suite green; manual headed run — record the animated fixture, open the resulting GIF, confirm real motion is visible
- [x] 5.2 Container: `bash docker/run-tests.sh` green
- [x] 5.3 README documents the new tools and the GIF-only (no WebM) scope

## 6. Wrap-up

- [x] 6.1 `openspec validate screencast-capture` clean; sync specs; archive
