## Why

The user's stated goal for ai-browser is a cost-effective Playwright-type tool agents use to test applications, and explicitly asked for short motion videos of moving parts in addition to screenshots — an agent watching a single static screenshot can't tell whether an animation, a loading spinner, a drag interaction, or a transition actually behaved correctly. `.research/High Performance Browser Automation.md` (and `.research/REVIEW.md`'s verification of it) confirmed CDP's `Page.startScreencast` is the standard, well-established mechanism for this: bidirectional frame streaming, works headless, no external dependency required to capture frames. This is the second of the two browser-side phases from the re-prioritized roadmap (after `browser-efficiency`), ahead of the human-motion engine.

## What Changes

- CDP layer: `Page.startScreencast`/`Page.screencastFrame`/`Page.screencastFrameAck`/`Page.stopScreencast` typed commands and event.
- Engine: `Session::start_recording(options) -> Recording` spawns a background frame collector (rides the same bounded event-stream infrastructure as `navigate_and_wait`'s lifecycle events); `Recording::stop()` halts capture, writes the JPEG frame sequence plus a JSON manifest (per-frame timestamps) to a recordings directory, and assembles an animated GIF via the pure-Rust `image` crate. A hard maximum duration (default 30s) self-terminates a forgotten recording.
- MCP tools: `browser_record_start(max_duration_ms?, quality?)` and `browser_record_stop()` — the latter returns the artifact paths, frame count, duration, and one representative mid-clip frame as inline MCP image content (not the whole clip — token-conscious).
- New animated test fixture (CSS/JS-driven moving element) so recording is verified against something that actually changes over time, not a static page.

**Explicitly out of scope:** WebM/MP4 encoding via `ffmpeg` (optional enhancement noted in design.md, not implemented — neither the Windows host nor the Docker test image has `ffmpeg` available, and shipping an unverified code path would violate this project's practice of only claiming what's actually been tested). The animated GIF is the only guaranteed output format for this change.

## Capabilities

### New Capabilities
- `browser-recording`: screencast-based video capture — start/stop, frame collection with a duration cap, GIF assembly, and the MCP tool surface.

### Modified Capabilities
_None — additive to `cdp-client`'s existing typed-command mechanism and `mcp-server`'s tool set without changing their requirements._

## Impact

- `crates/cdp/src/protocol/page.rs`: new commands/event.
- `crates/cdp/src/ops.rs`: `Page` gains `start_screencast`/`stop_screencast`/`ack_screencast_frame`/`events` and becomes `Clone` (needed so the background collector task can hold its own handle independent of the caller's borrow).
- New `crates/engine/src/recording.rs`; `crates/engine/src/session.rs` gains `start_recording`.
- New dependency: `image` (pure Rust, JPEG decode + GIF encode), engine crate only.
- `crates/mcp/src/lib.rs`: two new tools, one new `Arc<Mutex<Option<engine::Recording>>>` field.
- New `crates/engine/tests/fixtures/animated.html` and an integration test.
