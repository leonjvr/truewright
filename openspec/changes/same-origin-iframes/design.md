# Design: Same-Origin Iframe Snapshot/Interaction

## Context

The walker/resolve mechanism (page-snapshot spec) was built without any iframe awareness. Popup-auto-attach's design.md flagged full OOPIF (cross-origin iframe) support as "a materially bigger structural change to snapshot/resolve" and deferred it; this change scopes down to exactly the part of that problem that's tractable without CDP protocol changes -- same-origin iframes, which are directly reachable via `contentDocument` from the parent page's own `Runtime.evaluate` context, no separate target/session needed.

## Goals / Non-Goals

**Goals:** the walker recurses into same-origin iframe content; refs from inside a same-origin iframe resolve to correct top-level-page screen coordinates for click/type; a cross-origin iframe is surfaced as an explicit "can't see inside this" leaf, never silently dropped.

**Non-Goals:** cross-origin/OOPIF inspection (a real CDP target-attach + frame-correlation problem, not attempted here); iframe-visibility-chain occlusion checks beyond what the existing (already basic) visibility check already does.

## Decisions

1. **`walker.js` recurses into `el.contentDocument.body` when accessible, exactly like any other container, wrapped under a new `iframe` role node.** `contentDocument` returns `null` (not a thrown exception, in modern Chrome) when the iframe is cross-origin, so a plain accessibility check (`if (!doc) { ...leaf... }`) is sufficient -- no try/catch needed for the common case, though one is used defensively since embedding it in a `walk()` call this deep in a recursive tree, one unexpected throw shouldn't take down the whole snapshot.
2. **Cross-origin and not-yet-loaded iframes render as a leaf with a descriptive `name`, not silence.** An agent seeing nothing where an iframe visually exists is a worse failure mode than an agent seeing "iframe (cross-origin, not inspectable): `<src>`" -- the latter is at least legible and explains itself, matching this project's general bias toward explicit, honest gaps over silent ones (e.g. `browser_switch_page`'s error on an unknown page id, `true_input`'s typed rejection on headless).
3. **No ref-registry change.** Refs live in `window.__aib.refs`/`elRefs` on the *top* window (`walker.js`/`resolve.js` always execute there via `Runtime.evaluate`, never inside a frame's own context) -- a `Map`/`WeakMap` entry's key is a live object reference, and V8 doesn't restrict cross-realm object identity for Map/WeakMap keys. An element from inside a same-origin iframe is just an object like any other as far as the registry is concerned.
4. **`resolve.js` accumulates ancestor-frame offsets by walking `window.frameElement` up to the top, adding each ancestor `<iframe>`'s own `getBoundingClientRect()` (in *its own* parent's coordinate space) to the running total.** `getBoundingClientRect()` on an element inside an iframe is relative to that iframe's own viewport, not the top page's -- but CDP's `Input.dispatchMouseEvent` (which every click ultimately goes through, per `Page::click_at`) operates in top-level-page viewport coordinates. Without this accumulation, every click on an iframe-nested element would land at the wrong place on screen (verified as a real, not theoretical, risk -- see the Addendum once live-tested).
5. **Every ancestor iframe is scrolled into view too, not just the target element within its own frame.** `el.scrollIntoView()` only guarantees the element is visible *within its own document's viewport* -- if the iframe itself is scrolled out of view in the parent page, the element could still be unreachable. Walking the same `frameElement` chain a second time (after the initial `scrollIntoView`, before the final rect read) closes that gap.
6. **Test fixtures use `srcdoc` (same-origin) and `data:` URLs (cross-origin), not two separate files.** `srcdoc` content inherits the parent document's origin exactly -- guaranteed same-origin, zero network/file dependency. A `data:` URL gets a fresh opaque origin every time -- guaranteed cross-origin, equally self-contained. Both avoid the messier question of whether two different `file://` paths count as same-origin in Chrome (they don't reliably -- `file://` origins are themselves a source of inconsistent browser behavior not worth depending on for a deterministic test).

## Risks / Trade-offs

- [Coordinate-accumulation math for nested frames is the kind of thing this project has gotten wrong on the first attempt more than once this session already (true-user-input's DPI/foreground bugs)] → will be live-tested against a real fixture with an actual click before this is considered done, any correction documented in an addendum, not assumed correct from the JS alone.
- [`data:` URL cross-origin iframe test is a synthetic case -- real cross-origin iframes (ads, embedded third-party widgets) may behave slightly differently in edge cases] → acceptable for v1; the fallback path (render a leaf, don't crash) is the same regardless of *why* `contentDocument` is null.

## Migration Plan

Purely additive -- existing snapshots of pages with no iframes are byte-for-byte unaffected (no iframe elements to hit the new code path at all).

## Open Questions

None blocking.
