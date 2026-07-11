# Design: Shadow-DOM-Aware Walker

## Context

The second of two "walker sees into places it currently can't" changes in this Phase 5 pass, following `same-origin-iframes`. Directly motivated by PROPOSAL.md's own stated top risk ("reimplementing Playwright's actionability edge cases... test against a corpus of gnarly real pages"). Shadow DOM turns out to be structurally simpler than iframes: no new document, no new viewport, no coordinate translation -- purely a tree-traversal gap.

## Goals / Non-Goals

**Goals:** the walker recurses into open shadow roots; `<slot>`-projected light-DOM content appears in the right place in the tree; refs/clicks work on shadow-tree elements with zero additional plumbing.

**Non-Goals:** closed shadow roots (undetectable from script, not a scoping choice); deeply nested/recursive slot-forwarding chains.

## Decisions

1. **When `el.shadowRoot` is non-null, walk `shadowRoot.children` instead of `el.children`.** Mutually exclusive, not additive -- light-DOM children of a shadow host are either projected via a matching `<slot>` (and so already reachable through the shadow tree's own slot walking, decision 2) or not rendered at all (no matching slot). Walking both would show duplicate or irrelevant content depending on the component; walking only the shadow tree matches what's actually rendered.
2. **`<slot>` elements walk `slot.assignedElements()` instead of their own children.** A slot's own children in markup are fallback content (rendered only when nothing is projected into it) -- the common case is content IS projected, and `assignedElements()` returns exactly that (the real light-DOM nodes now appearing at this position in the rendered tree). Falling back to the slot's own (usually empty) children when nothing is assigned is naturally correct too, since `assignedElements()` returns an empty array in that case and normal fallback-content children remain reachable... actually: when nothing is assigned, `<slot>` renders ITS OWN children (the fallback content) -- so the walk needs to fall back to `el.children` when `assignedElements()` is empty, not just show nothing. Implemented as: use `assignedElements()` if non-empty, else fall back to `el.children`.
3. **No wrapping marker node for shadow content, unlike iframes' `iframe`-role wrapper.** A deliberate asymmetry: an iframe boundary has real functional consequences an agent benefits from knowing about (a separate document; a cross-origin one might be invisible). An *open* shadow root has none once walked -- the content is simply there, seamlessly, matching what a sighted user actually sees. Adding a marker node here would be noise, not signal, working against this project's "token-efficient" design goal.
4. **Closed shadow roots get no special leaf/message, unlike cross-origin iframes.** `contentDocument === null` unambiguously means "cross-origin" for iframes (a real signal worth surfacing as an explicit boundary). `shadowRoot === null` means EITHER "no shadow root" OR "closed shadow root" -- genuinely indistinguishable from script. There is nothing honest to say beyond "walked as a normal element, found nothing interesting inside" (or whatever its light-DOM fallback children happen to produce), so no special-casing is added; this is the honest ceiling, not an oversight.
5. **`resolve.js` is unchanged.** Confirmed via research before scoping: `getBoundingClientRect()` on a shadow-tree element already returns coordinates relative to the same top-level (or enclosing-iframe) viewport as any light-DOM element, since shadow DOM affects tree structure/encapsulation only, never layout or rendering geometry. The iframe work's coordinate-accumulation logic exists specifically because iframes introduce a *new document with its own viewport origin*; shadow roots don't.

## Risks / Trade-offs

- [Slot-projection fallback logic (`assignedElements()` non-empty vs. empty) is a real behavioral branch worth getting right, not obviously so from reading the spec alone] → live-tested against a real fixture exercising both the "content is projected" and default-slot-with-fallback cases before this is considered done.
- [Deeply nested slot-forwarding (a slotted element that itself contains further slots re-projecting further) isn't specially handled] → the same generic recursive `walk()` call already handles arbitrary nesting depth for any tree shape; this isn't a distinct code path, so it likely works by construction, but isn't a corpus finding this v1 specifically chased down.

## Migration Plan

Purely additive -- pages with no shadow DOM are byte-for-byte unaffected (no shadow roots to hit the new code path at all).

## Open Questions

None blocking.
