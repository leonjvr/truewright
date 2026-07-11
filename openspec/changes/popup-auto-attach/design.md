# Design: Popup Auto-Attach

## Context

The first slice of Phase 5 (Hardening). Every prior phase built strictly on top of Phase 1's "one browser, one context, one page" `Session` (design.md Decision #1 explicitly deferred a daemon/multi-session model, but that decision was about *processes*, not about a single process tracking more than one page -- `Session`'s own API was written, per that same decision, so a future daemon could wrap multiple `Session`s; this change instead teaches one `Session` to track multiple *pages* within itself, which is a smaller, compatible step, not a reversal of Decision #1).

## Goals / Non-Goals

**Goals:** notice a new top-level target (popup/new tab) opened as a side effect of interacting with the current page; let an agent explicitly discover and switch to it; keep every existing single-page behavior and MCP tool signature unchanged when no popup ever opens.

**Non-Goals:** cross-origin OOPIF (iframe) attach (see proposal.md's scope reduction); automatic popup cleanup/GC; auto-switching to a newly-opened popup without the agent asking.

## Decisions

1. **Auto-attach is enabled per-page, not browser-wide.** `Target.setAutoAttach({autoAttach: true, waitForDebuggerOnStart: false, flatten: true})` is called on each page's own CDP session (the same session `Page::navigate`/`evaluate`/etc. already use), immediately after creating it in `BrowserContext::new_page` -- mirroring how Puppeteer implements its `page.on('popup')`. This scopes auto-attach to targets actually related to that page (things it opens), rather than every target browser-wide, so a `Session` never picks up a popup from some unrelated context. `waitForDebuggerOnStart: false` since there's no debugger-pause workflow here; the new target should just start running immediately.
2. **`Target.attachedToTarget` events are consumed lazily, not via a background task.** This project has no persistent event loop -- an MCP tool call is the only time the process does anything (`design.md` Decision #1's "no daemon" reasoning applies here too). Rather than adding one, each `Page` exposes a bounded event stream (the existing `events::<E>()` machinery already used for `Page`/`Network`/`Fetch` events) for `Target.attachedToTarget`/`Target.detachedFromTarget`, and `Session` drains whatever has arrived so far, non-blockingly, at the start of `list_pages()`. An agent's natural workflow -- click something that might open a popup, then call `browser_list_pages` to check -- already provides the "ask again if it's not there yet" retry point; no artificial polling wait is added for v1.
3. **`Session` gains a page registry (`HashMap<target_id, Page>`) plus an `active_target_id`, both behind the same interior-mutability pattern already used for `mouse_pos`/`training_active` (`Mutex`), not `&mut self`.** Every existing action method (`click`/`type_text`/`snapshot`/`navigate`/etc.) resolves against "the active page" through a small `active_page()` helper instead of a bare `self.page` field access -- mechanical to apply, but touches most of `session.rs`. External behavior for the common single-page case (nothing ever calls `browser_switch_page`) is unchanged.
4. **A dedicated `primary_target_id` (the page `Session::launch` originally created) is tracked separately from `active_target_id`.** If the currently-active page detaches (the popup closes itself, e.g. after an OAuth redirect completes), `active_target_id` resets to `primary_target_id` rather than to an arbitrary remaining page -- a predictable fallback mirroring how closing a popup naturally returns a real user's focus to the tab that opened it.
5. **`browser_list_pages`/`browser_switch_page` use `page_id` as the external parameter name, mapped internally to CDP's `targetId`.** Matches this project's existing convention of translating CDP jargon into agent-friendly terms (e.g. `ref` instead of `backendNodeId`).
6. **No proactive staleness check on every action call for a switched-to page that has since closed.** `switch_page` validates the requested `page_id` exists in the registry at switch time (cheap, valuable at the point of intent); if a page closes *after* being switched to and an action is attempted anyway, the resulting CDP-level protocol error surfaces via the existing `EngineError::Cdp` variant rather than a bespoke pre-check on every call -- acceptable for v1, consistent with keeping this slice reduced.

## Risks / Trade-offs

- [The exact CDP session/event-routing behavior of page-scoped `setAutoAttach` + flattened `attachedToTarget` is based on documented/observed CDP behavior (matching Puppeteer's implementation), not yet exercised against this project's own hand-rolled `Connection`/session-demuxing code] → real risk of a live-testing correction, same as `yaml-runner`'s `serde_yaml` surprise and `true-user-input`'s three live-testing bugs -- will be verified against a real popup-opening fixture before this is considered done, and any correction will be documented in a design.md addendum, not silently patched.
- [`session.rs`'s `self.page` → `self.active_page()` conversion touches most of the file] → mechanical, low-risk-per-line, but large diff; mitigated by running the full existing test suite (which exercises every action method) after the refactor, before adding any new popup-specific behavior on top.

## Migration Plan

Purely additive at the MCP surface (two new tools); the `Session` struct's internal field shape changes, but its public method signatures don't, so no caller-visible break for the single-page case.

## Open Questions

None blocking -- CDP session-routing specifics will be confirmed empirically during implementation, per the Risks section above.
