# Design: HTML Trace Viewer

## Context

`aib`'s originally-envisioned CLI surface named `trace` as a first-class subcommand alongside `mcp`/`doctor`; this is the first change to actually build it. The existing JSONL trace format (console/exception/action entries, from `console-capture`/`action-trace`) is accurate but not meant for humans to read directly.

## Goals / Non-Goals

**Goals:** render an already-saved trace as one clean, self-contained HTML file; capture screenshots into the trace when one is active, since that's the one thing a text-only trace can never show; expose this both as a CLI command and an MCP tool.

**Non-Goals:** network/snapshot entry types (see proposal.md); a live/streaming viewer; screenshot-directory cleanup.

## Decisions

1. **Screenshots are the only new entry kind.** Every other candidate (network, DOM snapshots) either duplicates an existing, better-suited mechanism (network-mocking's cassettes) or wouldn't render usefully as HTML (a full accessibility tree dump is still just text, no better read as HTML than as the existing indented-text `render()` output). A screenshot answers a question nothing else in the trace can.
2. **Screenshots are saved to disk and referenced by path in the JSONL, not inlined as base64 in the trace file itself.** Keeps the JSONL trace lightweight and greppable (its whole point); the HTML *viewer* is a better place to pay the base64-inflation cost, and only when actually rendering, not on every capture.
3. **The HTML viewer embeds screenshots as base64 data URIs, not `file://` references to the saved PNGs.** A rendered HTML file should be self-contained and portable -- movable, emailable, openable from any location -- without a sibling screenshots directory needing to travel with it. This mirrors this project's own Artifact-publishing conventions (self-contained, no external file dependencies) even though this HTML is written to local disk, not published anywhere.
4. **`ActionTraceSink`'s inner `Option` gains the trace's `name` (`ActiveTrace { name, entries }`), not just the bare entry buffer.** `screenshot()` needs to know *which* trace's screenshot directory to save into -- the name was previously only known inside `ConsoleCapture` itself, never threaded through to the sink `Session` actually holds.
5. **Screenshot save/log failures are swallowed, never surfacing as a `screenshot()` error.** Matches the project's established posture for the whole action-trace mechanism: tracing is a side benefit of calling actions normally, and a tracing hiccup (disk full, permissions) should never turn an otherwise-successful screenshot into a failed tool call.
6. **Entries are explicitly sorted by `timestamp_ms` at render time, not trusted to already be in chronological order.** Console/exception entries are pushed by an async collector task reacting to CDP events; action/screenshot entries are pushed synchronously by whichever `Session` method call triggered them. Both write into the same shared `Vec` under a lock, so push order reflects real-time lock-acquisition order -- almost always chronological, but not something to rely on without an explicit sort given two genuinely concurrent writers.
7. **The MCP tool returns the output file path as text, not the rendered HTML content.** Consistent with `browser_record_stop` (path + one frame, not the whole clip) and `browser_screenshot` (one image) -- this project is consistently token-conscious about not dumping large blobs into an agent's context when a path/reference serves the same purpose.

## Risks / Trade-offs

- [Base64-embedding screenshots can make the rendered HTML large for a trace with many screenshots] → acceptable for v1; a trace with dozens of full-page screenshots is an unusual case, and the alternative (external file references) trades portability for a marginal size win that matters less than "the file just works wherever you open it."

## Migration Plan

Purely additive. Existing traces without any screenshot entries render exactly as before (just console/exception/action rows); `ActionTraceSink`'s shape change is entirely internal, no public API before this existed to break.

## Open Questions

None blocking.
