## Why

The walker has zero shadow-DOM awareness, confirmed by reading `assets/walker.js`: `walk()` only ever iterates `el.children` (light DOM), and a shadow root's content is a separate tree not reachable that way -- so any web component with an attached shadow root is walked as an empty leaf (or pruned entirely) today, regardless of how much real, interactive content it renders. Modern real-world apps use web components constantly (design-system libraries, framework-agnostic widgets), and this is exactly the risk PROPOSAL.md itself called out up front: "reimplementing Playwright's actionability edge cases is the highest risk... test against a corpus of gnarly real pages, ship a documented known unsupported list." Unlike the iframe work, this one turns out to need **no coordinate-translation changes at all** -- shadow DOM affects tree structure, not layout or viewport geometry, so `resolve.js` is untouched.

## What Changes

- `assets/walker.js`: when an element has an **open** shadow root (`el.shadowRoot` is non-null), walk its `shadowRoot.children` instead of its light-DOM `children` -- open shadow roots are directly readable from any script in the same document, no CDP/execution-context changes needed.
- `<slot>` elements are walked via `slot.assignedElements()` (the light-DOM nodes actually projected into that slot) instead of the slot's own (usually empty, fallback-only) children -- without this, the extremely common `<my-card><h2>Title</h2></my-card>`-style default-slot pattern would show empty custom-element content, missing most real-world usage.
- Shadow content is spliced into the tree **seamlessly, with no wrapping marker node** -- unlike the iframe work's deliberate `iframe`-role wrapper. This is an intentional asymmetry: an iframe boundary has real functional consequences worth surfacing (a separate document, possible cross-origin invisibility); an *open* shadow root, once walked, has none -- it's pure implementation detail an agent generally doesn't need to know about to interact with what's rendered.
- **Closed shadow roots are not distinguishably surfaced.** `element.shadowRoot` returns `null` identically for "no shadow root" and "closed shadow root" -- there is no script-observable way to tell them apart, unlike cross-origin iframes (`contentDocument === null` is unambiguous). A closed-shadow custom element simply falls through to walking its light-DOM children as before (usually nothing useful, sometimes stale fallback content) -- this is the honest ceiling of what's possible, not a gap left uncovered by choice.

**Explicitly out of scope (deferred), and why:**
- **Closed shadow roots.** Not a scoping choice -- genuinely undetectable from script, a hard platform boundary, not a "v1 reduction" the way cross-origin iframes were.
- **Nested slot forwarding edge cases** (a slot's assigned content itself containing further `<slot>` re-projection, common in complex component library internals) -- `assignedElements()` handles the single-level common case; deeply recursive slot-forwarding chains are a real but rarer pattern left for a future corpus finding, not designed around speculatively.

## Capabilities

### New Capabilities
- `shadow-dom-walker`: the walker sees into open shadow roots (including slotted/projected content), so an agent can snapshot and interact with web-component-based UI instead of seeing empty leaves where real content renders.

## Impact

- `crates/engine/assets/walker.js`: shadow-root child-source switch + slot projection.
- `crates/engine/assets/resolve.js`: **no changes** -- confirmed coordinates for shadow-tree elements are already relative to the same viewport as any other element, since shadow DOM introduces no new document/browsing context.
- No Rust struct changes (`WalkerNode` is already schema-agnostic to new tag/role values, same as the iframe work).
- New test fixture: a custom element with an open shadow root using a default `<slot>`, verifying both directly-rendered shadow content and slotted (projected) light-DOM content show up correctly, with working refs.
- No CDP protocol changes, no real-OS side effects -- headless verification like the rest of Phase 3/5, though live-tested regardless per this project's now-consistent practice of not trusting coordinate/DOM-traversal JS until it's actually clicked something for real.
