# Design: Screencast Capture

## Context

Follows `browser-efficiency`. The user explicitly wants short motion videos in addition to screenshots so an agent can verify moving parts (animations, transitions, drag interactions) actually behaved correctly, not just that a static frame looks right.

## Goals / Non-Goals

**Goals:** real frame capture via the standard CDP mechanism; a guaranteed, dependency-free output format (GIF); a hard duration cap so a forgotten recording can't grow unbounded; MCP tools that return a quick visual preview without dumping the whole clip's tokens.

**Non-Goals:** WebM/MP4 via `ffmpeg` (no verified environment has it — see Decision #4); audio capture (CDP screencast is video-only); frame-rate tuning beyond CDP's own throttling; multiple concurrent recordings per session.

## Decisions

1. **JPEG format, capped resolution (480×360, revised down from an initial 800×600 — see addendum), quality 60 default, `everyNthFrame: 3`.** Keeps individual frame payloads and the eventual GIF small — consistent with the project's resource-efficiency focus, and (per the addendum below) necessary for GIF assembly to complete in reasonable time. Quality is configurable via `browser_record_start`; resolution/frame-density caps are fixed for v1 (not exposed as parameters — one less thing for an agent to get wrong).
2. **Background collector task, not a poll loop; frame acks are fire-and-forget.** Mirrors `navigate_and_wait`'s use of `Session::events::<E>()` (existing bounded-broadcast infrastructure from Phase 0) — a spawned task consumes `page::ScreencastFrame` events and pushes into a shared `Arc<Mutex<Vec<Frame>>>`. Each frame's `Page.screencastFrameAck` is sent via a detached `tokio::spawn`, not awaited inline — CDP only needs the ack sent to keep streaming, and awaiting its response in the hot loop would block the loop's ability to observe the stop/deadline signals if any single ack round trip were ever slow (see addendum). `Recording::stop()` signals the collector via a `oneshot` channel and joins it.
3. **`ops::Page` becomes `Clone`.** The collector task needs its own handle to issue `Page.screencastFrameAck` calls independent of the caller's borrow of `Session`. `Page`'s fields (`Session`, `String`) are already `Clone`; deriving it is mechanical and doesn't change any existing behavior.
4. **No ffmpeg/WebM in this change.** `ffmpeg` is absent from both the Windows host and the `docker/Dockerfile` test image. Shipping an encoding path with zero real verification would be exactly the kind of unverified claim this project has avoided at every prior phase (Phase 0's `aib doctor` evidence, Phase 1's manual MCP walkthrough, `browser-efficiency`'s measured tree-RSS). If WebM support is wanted later, it's a self-contained follow-up: detect `ffmpeg` on PATH, shell out with the concat demuxer using the manifest's real timestamps, treat failure as non-fatal.
5. **GIF assembly via the `image` crate's `GifEncoder`, decoding each captured JPEG then re-encoding as a GIF frame with a delay computed from consecutive real timestamps** (not a fixed frame rate) — so playback speed reflects what actually happened, including any throttling CDP applied under load.
6. **Single recording per session, enforced by the MCP layer, not the engine.** `engine::Session` stays a thin, stateless-per-call API (matches the existing design — no interior mutability in `Session` itself); `AibTools` holds `Arc<Mutex<Option<engine::Recording>>>` alongside its existing session mutex, the same pattern already used for the browser session itself.

## Risks / Trade-offs

- [GIF color quantization loses fidelity vs the source JPEGs] → acceptable for "see the moving parts" verification; not a pixel-diff testing tool.
- [Screencast frame rate is throttled by CDP itself under load, not directly controllable] → documented; the manifest's real timestamps make the GIF's playback speed still accurate even if choppy.
- [Recording buffers frames in memory until stop] → bounded by the 30s hard cap and the resolution/quality caps; acceptable at this scale, revisit if recordings grow much longer in a later phase.

## Migration Plan

Additive. No existing tool or spec changes.

## Open Questions

None blocking. WebM/ffmpeg support deferred as noted in Decision #4.

## Addendum: what the first working version actually looked like

The initial implementation hung indefinitely on the first real integration test run — worth recording precisely because it only surfaced by actually running the thing against a live browser, not by writing code that compiles.

1. **Awaiting `Page.screencastFrameAck` inline blocked the collector loop.** The first version called `page.ack_screencast_frame(...).await` directly inside the `events.next()` match arm, inside the same `tokio::select!` loop that also watches the stop signal and duration deadline. Once inside that match arm, the ack call is a *sequential* await, not one of the racing branches — so if it were ever slow, the loop couldn't reach the top again to check whether it should stop. Tracing showed the collector successfully processing ~24 frames in under half a second, then stalling completely. Fixed by making the ack fire-and-forget (`tokio::spawn`, not awaited) — the collector loop no longer has any await inside a match arm that isn't racing against stop/deadline.
2. **GIF assembly on the async executor was slow enough to look like a second hang.** Even after fixing (1), `Recording::stop()` still looked stuck — because `write_frames`/`assemble_gif` are synchronous, CPU-bound work (JPEG decode + GIF color quantization) running directly on a tokio async worker thread, and in an unopimized debug build, quantizing dozens of 800×600 frames took tens of seconds. Two fixes: moved the whole write/assemble step into `tokio::task::spawn_blocking` (correct regardless of speed — CPU-bound work shouldn't run on an async worker thread), and reduced the default capture resolution to 480×360 with `everyNthFrame: 3` (CDP streams every repaint by default, ~60fps for an animating page, far denser than a short preview needs). Confirmed with real numbers: the same recording took 38.9s in a debug build and 5.66s in `--release` — both plausible for genuinely CPU-bound work, not a hang, but reducing frame count/size makes the debug-build experience reasonable too, not just the shipped release binary.

Both fixes are real robustness improvements independent of the specific numbers: fire-and-forget acks mean one slow CDP round trip can never wedge a recording, and `spawn_blocking` means image encoding never starves the async runtime other tool calls (or the MCP server's own responsiveness) depend on.
