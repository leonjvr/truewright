# Design: Screencast Capture

## Context

Follows `browser-efficiency`. The user explicitly wants short motion videos in addition to screenshots so an agent can verify moving parts (animations, transitions, drag interactions) actually behaved correctly, not just that a static frame looks right.

## Goals / Non-Goals

**Goals:** real frame capture via the standard CDP mechanism; a guaranteed, dependency-free output format (GIF); a hard duration cap so a forgotten recording can't grow unbounded; MCP tools that return a quick visual preview without dumping the whole clip's tokens.

**Non-Goals:** WebM/MP4 via `ffmpeg` (no verified environment has it — see Decision #4); audio capture (CDP screencast is video-only); frame-rate tuning beyond CDP's own throttling; multiple concurrent recordings per session.

## Decisions

1. **JPEG format, capped resolution (800×600 default), quality 60 default.** Keeps individual frame payloads and the eventual GIF small — consistent with the project's resource-efficiency focus. Configurable via `browser_record_start`'s `quality` parameter; resolution cap is fixed for v1 (not exposed as a parameter — one less thing for an agent to get wrong).
2. **Background collector task, not a poll loop.** Mirrors `navigate_and_wait`'s use of `Session::events::<E>()` (existing bounded-broadcast infrastructure from Phase 0) — a spawned task consumes `page::ScreencastFrame` events, acks each one immediately (CDP stalls the stream otherwise), and pushes into a shared `Arc<Mutex<Vec<Frame>>>`. `Recording::stop()` signals the task via a `oneshot` channel and joins it.
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
