## Why

The first slice of Phase 5 (Hardening). Real-world apps an agent needs to test frequently open a new top-level browsing context as a side effect of interacting with the current page -- third-party OAuth login ("Sign in with Google"), an external payment processor, a `target="_blank"` link, a `window.open()` call. Today `Session` is hard-coded to exactly one `cdp::ops::Page` for its entire lifetime (Phase 1 design.md Decision #1: "one browser, one context, one page"), and nothing in `crates/cdp` subscribes to target-creation events at all -- a popup that opens is completely invisible to the engine, and there's no way for an agent to even discover it exists, let alone drive it. That blocks a whole class of real test flows ("click login, complete OAuth in the popup, come back") that this project exists to support.

## What Changes

- New CDP layer: `Target.setAutoAttach` (command) plus `Target.attachedToTarget`/`Target.detachedFromTarget` (events) in `crates/cdp/src/protocol/target.rs`, wired through the existing generic `CdpEvent`/`EventStream` machinery already used for `Page`/`Network`/`Fetch` events -- no new architecture, just new protocol types.
- `Browser`/`BrowserContext` enables auto-attach for its context so new top-level targets created as a side effect of page interaction (popups, new tabs) are attached automatically rather than requiring an explicit `new_page` call.
- `engine::Session` moves from a single `page: cdp::ops::Page` field to a small registry: every attached page tracked by target ID, with one "active" page that every existing action method (`click`/`type_text`/`snapshot`/`navigate`/etc.) operates against -- unchanged behavior for the common single-page case.
- New MCP tools: `browser_list_pages()` (target ID, URL, title, which one is active) and `browser_switch_page(target_id)` (changes which page subsequent actions target). No auto-switching to a newly-opened popup -- an agent must explicitly notice (via `browser_list_pages`) and switch, matching this project's consistent bias toward explicit, observable state over magic that could silently redirect an agent's next action somewhere it didn't expect.

**Explicitly out of scope (deferred), and why this is a genuinely reduced first slice:**
- **Cross-origin OOPIF (out-of-process iframe) attach.** A cross-origin `<iframe>` is technically a separate CDP target too, but reaching into it requires the walker/`resolve.js` to traverse across attached child targets, not just switch which top-level page is "active" -- a materially bigger structural change to snapshot/resolve than this slice's page-level switching. This slice covers top-level popups/new tabs only, the far more common real-world case (OAuth login flows); true OOPIF support is a real follow-up, not silently dropped.
- **Automatic cleanup of stale/abandoned popups.** A popup the agent never switches to or closes just sits there attached; no automatic GC beyond existing session teardown killing the whole browser.
- **Any change to `true_input`'s window targeting.** `true_input` already resolves its own OS window per-page via the launched process's PID and CDP's own window bounds; unaffected by this change since it doesn't depend on which page is "active" the way CDP-dispatched actions do.

## Capabilities

### New Capabilities
- `popup-auto-attach`: the engine notices and attaches to new top-level targets (popups, new tabs) created as a side effect of page interaction, and exposes them to an agent as explicitly listable/switchable pages.

## Impact

- `crates/cdp/src/protocol/target.rs`: new `SetAutoAttach` command, `AttachedToTarget`/`DetachedFromTarget` events.
- `crates/cdp/src/ops.rs`: `BrowserContext`/`Page` gain the plumbing to enable auto-attach and expose newly-attached targets as an event stream.
- `crates/engine/src/session.rs`: `Session`'s single `page` field becomes a registry keyed by target ID plus an active-page pointer; every existing action method resolves against the active page. This is a real structural change to a field that's been "exactly one page" since Phase 1, but the external single-page behavior is unchanged when no popup ever opens.
- `crates/mcp/src/lib.rs`: two new tools, `browser_list_pages`/`browser_switch_page`.
- New test fixture with a `target="_blank"` link and/or `window.open()` call, for verification.
- No real-OS side effects (unlike `true-user-input`) -- verification runs headless in the existing test harness and in the Docker container like every other phase before Phase 4.
