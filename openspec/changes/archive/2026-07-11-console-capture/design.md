# Design: Console Capture

## Context

First of Phase 3b-iii's remaining tooling pieces. Unlike `network-mocking`'s `Fetch`/`Network` domains or `deterministic-init`/`virtual-clock`'s init-script mechanism, this needs no new CDP domain enable at all — `Runtime` is already enabled on every page (Phase 1's walker/resolve injection depends on it), so this change is purely "subscribe to two more event types on a domain that's already flowing."

## Goals / Non-Goals

**Goals:** capture every `console.*` call and uncaught exception between an explicit start/stop window, in chronological order, as a named JSONL trace; verified against a real fixture that logs at multiple levels and throws, not just unit-tested parsing.

**Non-Goals:** a unified action trace merging in navigate/click/type calls (own follow-up — see proposal.md); level filtering (v1 captures everything unfiltered); pretty-printing complex object arguments beyond a reasonable string summary (matches `console.log`'s own DevTools behavior of showing a summary, not a full recursive dump).

## Decisions

1. **Two event types, one trace format.** `Runtime.consoleAPICalled` (regular `console.*` calls) and `Runtime.exceptionThrown` (uncaught exceptions) are structurally different CDP events but both represent "something the page wants a developer to know about" — normalized into the same JSONL entry shape (`{"type": "console"|"exception", ...}`) so a reader doesn't need to understand two different schemas to get the full picture of what happened.
2. **Console arguments are rendered as a best-effort string, not preserved as structured values.** Each `console.*` call's arguments arrive as CDP `RemoteObject`s (a primitive's `.value`, or a `.description` for objects/functions). Rendering each to a string (preferring `.value`, falling back to `.description`) and joining with spaces mirrors what a developer actually reads in a console, and avoids needing to handle arbitrarily-nested remote-object structures.
3. **Capture reuses the collector-task-plus-persist shape from `NetworkRecording`/`Training` exactly** — `ConsoleCapture::start` subscribes to both event streams and buffers entries; `stop()` persists to `<data-dir>/aib/traces/<name>.jsonl` and returns a summary. No new architectural pattern.
4. **JSONL, not one big JSON array.** Consistent with how tools like this normally emit logs (streamable, appendable, one line per event) and easy to `tail -f`/`grep` even though this v1 writes it all at once on `stop()` rather than incrementally.

## Risks / Trade-offs

- [String-rendering console arguments loses structure a consumer might want (e.g. a logged object's actual fields)] → acceptable for v1's "see what happened" goal; a future change could add a structured-value variant if a real need shows up.
- [No level filtering means noisy pages produce large traces] → acceptable; filtering is a straightforward follow-up if it turns out to matter, and default-capture-everything is safer than default-drop for a debugging tool.

## Migration Plan

Purely additive — two new MCP tools, no existing tool's behavior changes.

## Open Questions

None blocking.
