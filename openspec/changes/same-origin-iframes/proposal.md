## Why

The walker has zero iframe awareness today -- confirmed by reading `assets/walker.js`: an `<iframe>` element is not in `INTERACTIVE_TAGS`/`STRUCTURAL_TAGS`, and `el.children` never crosses into `contentDocument` (a separate document tree, not a light-DOM child), so even a **same-origin** iframe's contents are invisible to a snapshot -- not just cross-origin ones. Real-world apps embed same-origin iframes constantly (rich-text editors, embedded widgets, payment forms, app-internal micro-frontends); an agent testing such an app today gets a snapshot with a same-origin iframe rendered as nothing at all, and no way to click/type anything inside it. This is a real, common gap, and -- unlike full cross-origin OOPIF support -- closing it for the same-origin case turns out to need no CDP protocol changes at all, just walker/resolve JS changes, since same-origin iframe content is directly reachable via `contentDocument` from the parent's own execution context.

## What Changes

- `assets/walker.js`: recursing into an `<iframe>` element's `contentDocument` when accessible (same-origin), wrapping its walked subtree under an `iframe` role node so the snapshot's tree structure reflects the frame boundary. When `contentDocument` isn't accessible (cross-origin) or hasn't loaded yet, a leaf node says so explicitly (with the `src`, when cross-origin) rather than silently producing nothing -- an agent should never wonder whether an iframe was missed by mistake versus genuinely inspectable.
- `assets/resolve.js`: accumulates each ancestor `<iframe>`'s own `getBoundingClientRect()` offset (walking up via `window.frameElement`) so a ref resolved from inside a same-origin iframe reports coordinates in the *top-level page's* viewport space -- the space CDP's `Input.dispatchMouseEvent` already expects. Also scrolls every ancestor iframe into view, not just the target element within its own frame, so a nested/off-screen frame doesn't leave the target unreachable.
- The ref registry (`window.__aib.refs`/`elRefs`, a `Map`/`WeakMap` on the top window) needs no change -- object references work across document/realm boundaries in V8 regardless of which document a DOM node belongs to, and the walker/resolve scripts always execute in the top window's own realm.
- **No Rust-side struct changes.** `WalkerNode::role` is a plain `String` (not an enum) and unknown JSON fields are already ignored by `serde` -- the new `iframe` role and its descriptive `name` text for the not-inspectable case flow straight through the existing rendering code untouched.

**Explicitly out of scope (deferred), and why this is a meaningfully reduced slice, not the full "OOPIF" item on Phase 5's list:**
- **Cross-origin iframes remain uninspectable.** A cross-origin iframe becomes a real separate CDP target (out-of-process, "OOPIF") requiring its own `Target.attachToTarget`, its own `Runtime.evaluate` scope, and -- the genuinely hard part -- correlating that target back to *which* `<iframe>` element in the parent tree it belongs to (needs `Page.frameAttached`/`frameNavigated` parentId plumbing not in `crates/cdp/src/protocol/page.rs` today) so the snapshot can splice it into the right place rather than just listing a disconnected second tree. This slice makes that boundary *visible and explicit* in the snapshot (an agent can see an iframe is there and that it can't be inspected) without attempting to solve it.
- **Deeply nested cross-realm ref collisions.** Not a concern for v1's scope (same-origin only, single registry) but noted for whoever eventually tackles cross-origin: a cross-target ref scheme will need its own namespacing story this slice doesn't have to solve.

## Capabilities

### New Capabilities
- `same-origin-iframes`: the walker sees into same-origin iframes and an agent can click/type on elements inside them, with cross-origin iframes surfaced as an explicit, honest boundary rather than silently invisible.

## Impact

- `crates/engine/assets/walker.js`: iframe recursion + cross-origin/not-yet-loaded fallback leaf.
- `crates/engine/assets/resolve.js`: cross-frame coordinate accumulation + ancestor-frame scroll-into-view.
- New test fixtures: a same-origin case via `<iframe srcdoc="...">` (guaranteed same-origin, no separate file or network dependency) and a cross-origin case via `<iframe src="data:...">` (a `data:` URL gets a unique opaque origin, guaranteed cross-origin, equally self-contained).
- No CDP protocol changes, no real-OS side effects -- verification runs headless like every phase before Phase 4, no live-testing check-in needed, though live testing is still how the coordinate math will actually be confirmed correct (this project's coordinate-translation track record this session says trust nothing until it's actually clicked something).
