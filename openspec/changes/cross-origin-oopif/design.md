## Context

`same-origin-iframes` recurses into `contentDocument` from the parent's own `Runtime.evaluate` context -- that only works because same-origin content lives in the same JS realm reachability graph as the top window. A cross-origin iframe Chrome has isolated into its own process (an OOPIF) is a genuinely separate CDP target: its own `Runtime.evaluate` execution context, its own V8 heap, no shared object identity with the parent at all. Reaching it needs real CDP target/session machinery, not just more JS.

## Decision 1: Discover + explicit attach, not `Target.setAutoAttach`

`popup-auto-attach`'s design.md already settled this question for top-level targets: `Target.setAutoAttach` caused CDP to double-manage the primary page's session (auto-attached by CDP *and* explicitly attached by this client), hanging the next command on that session. The fix was browser-wide, observe-only `Target.setDiscoverTargets` plus this client's own explicit `Target.attachToTarget`.

That mechanism already delivers `Target.targetCreated` events for `iframe`-type OOPIF targets too (browser-wide discovery doesn't filter by type at the CDP level -- `crates/engine/src/session.rs`'s `refresh_attached_pages` does, discarding anything that isn't `type: "page"`). So this change needs zero new discovery mechanism: it widens the existing filter to also explicitly attach `iframe`-type targets, into a new registry that's never eligible to become the "active page." No new hang risk, because nothing about *how* targets are attached changes -- only *which target types* get attached.

(The research pass before this proposal considered whether page-scoped `Target.setAutoAttach`, restricted to just a page's own OOPIF children, might sidestep the original hang -- the primary page's own session would stay singly-managed either way. That's plausible but unvalidated, and there's no need to spend a validation cycle on it when the already-proven discover+explicit-attach path handles OOPIF targets with no code-path changes at all.)

## Decision 2: `DOM.getFrameOwner` correlation, not `Target.TargetInfo.openerId` or URL matching

Three ways to answer "which `<iframe>` element in the parent does this OOPIF target belong to":
1. `TargetInfo.openerId` / `openerFrameId` -- these exist in the real CDP protocol but describe *window-opener* relationships (`window.open()`), not frame-embedding relationships. Wrong field for this problem.
2. Match the walked `<iframe>`'s `src` attribute against the OOPIF's navigated URL. Fragile: relative URLs, redirects, and multiple iframes pointing at the same origin all break simple string matching.
3. `DOM.getFrameOwner({frameId}) → backendNodeId` -- purpose-built for exactly this ("given a frame, which DOM node in its parent owns it"), used internally by Playwright's own frame manager for the same problem.

Went with (3). `backendNodeId` isn't itself usable from JS, so it's chained: `DOM.resolveNode({backendNodeId}) → objectId` (a live handle into the parent's `Runtime` realm), then `Runtime.callFunctionOn({objectId, functionDeclaration: get-or-create-ref})` invokes the *exact same* `refFor` logic `assets/walker.js` uses, against the literal live DOM element. If the walker already visited that `<iframe>` in this snapshot pass, this returns the identical ref it already assigned (same `WeakMap`, same live object) -- no separate ref-allocation scheme to keep in sync, no possibility of drift between "the ref the walker assigned" and "the ref this correlation step computed."

This requires `walkIframe` to assign a ref to *every* iframe element, not just same-origin ones (previously only elements *inside* same-origin content ever got refs; the `<iframe>` tag itself never did, since there was nothing actionable about a boundary node). Backward compatible: nothing previously depended on iframe nodes lacking a `ref` field.

## Decision 3: `Page.getFrameTree` scopes correlation to the current page

Target discovery is browser-context-wide, same as it already is for popup pages -- a session with an active page and an OOPIF child, plus a background popup that also happens to have OOPIF children, would otherwise mix up which OOPIF belongs to which page's tree during snapshot splicing. `Page.getFrameTree` (called on the currently active page's own session) returns the full frame tree *including cross-process children* -- confirmed as real CDP behavior, not something requiring the OOPIF's own session to answer. Snapshot splicing only attempts correlation for OOPIF targets whose `frameId` (== `targetId` for `iframe`-type targets, a documented CDP implementation detail also relied on by Puppeteer/Playwright) appears in that frame tree.

## Decision 4: Ref namespacing -- `f<N>:e<M>`, not a flat global counter

Each attached OOPIF runs its *own* copy of `walker.js`, producing its own local `window.__aib` registry with its own `e1, e2, ...` counter, unrelated to the top page's. Two options:
1. Inject a page-supplied starting counter offset into each OOPIF's walker invocation so ref numbers never collide globally.
2. Prefix every OOPIF-sourced ref with a short per-frame tag (`f1:`, `f2:`, ...), keeping each session's own local counter untouched.

Went with (2). It needs no coordination *before* running the walker (no round-trip to agree on a starting offset), it makes which-session-owns-this-ref directly readable from the ref string itself (useful for the reader as much as the code -- an agent or a human debugging a trace immediately knows `f1:e3` names something inside a specific cross-origin frame), and `resolve_ref` can route purely by string-parsing the prefix without any additional state beyond the already-necessary frame-tag → session lookup table. The frame tag is assigned once per `frameId`, first-seen order, stable for the life of the `Session`.

## Decision 5: Coordinate translation composes two already-existing pieces, unmodified

`resolve.js` unmodified, run inside the OOPIF's own session, already produces the right *local* answer: `window.frameElement` is `null` from inside a cross-origin child by browser design (the same-origin check that makes it `null` is symmetric -- it's not only the parent that's restricted from reaching in, the child is equally restricted from reaching out), so the existing ancestor-walk loop's `while (win.frameElement)` condition is false on the very first check and the loop simply doesn't run, returning a click point relative to the OOPIF's own viewport with no changes needed to the script at all.

That local point then needs the OOPIF's own on-screen position added, in the top page's coordinate space -- which is exactly what resolving the (already-correlated, from Decision 2) top-level ref for the iframe host element already computes, via the same `resolve.js`, on the top-level session, no new script needed there either. `resolve.js` returns a *center* point plus width/height, not a top-left corner, so the offset added is `(center_x - width/2, center_y - height/2)`. This doesn't correct for the iframe element's own border/padding box vs. its content's coordinate origin -- neither does the existing same-origin-iframe translation (Decision in `same-origin-iframes/design.md`), so this isn't a new source of imprecision, just an already-accepted one extended to a second case.

Final dispatch (`Input.dispatchMouseEvent`) is still always issued on the top-level page's own session, never the OOPIF's -- CDP's `Input` domain operates on the browser-compositor input queue for a `page`-type target's viewport; an `iframe`-type OOPIF target has no independent input queue of its own to dispatch into, by design (this matches Puppeteer/Playwright's own behavior: OOPIF-aware click implementations still ultimately drive Input through the top-level page).

## Testing: forcing a genuine OOPIF, not an opaque-origin approximation

`same-origin-iframes`' existing cross-origin fixture uses a `data:` URL -- an opaque origin that Chrome never promotes to its own process/target, so it can't exercise this change at all (it would still correctly fall back to the "not inspectable" leaf, but that's not the code path being added). A real OOPIF needs a real cross-*site* navigation. `crates/engine/tests/support/http_server.rs`'s existing `TestServer` only varies by port on `127.0.0.1` -- same site (registrable domain) under Chrome's site-isolation model, so two `TestServer`s on different ports would *not* reliably force separate processes.

`.localhost` is reserved (RFC 6761) and resolves to loopback everywhere without `/etc/hosts` edits; `a.localhost` and `b.localhost` are different registrable domains (neither is in the Public Suffix List, so each single label + `.localhost` counts as its own site), which is different enough for Chrome's site-isolation model to treat them as separate sites. This is extended into `TestServer` as a second `url_on(host, path)` constructor, and the fixture launches Chrome with `--site-per-process` for this test's session specifically (not project-wide -- it forces isolation for *every* cross-site frame, which is exactly what a deterministic test needs, but isn't something every headless session should pay the overhead of by default, consistent with this project's existing efficiency-conscious posture).
