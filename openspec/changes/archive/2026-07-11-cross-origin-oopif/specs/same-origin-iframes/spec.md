## MODIFIED Requirements

### Requirement: Cross-origin iframes are surfaced explicitly, not silently dropped
The engine SHALL render a cross-origin iframe backed by an attachable Out-Of-Process IFrame (OOPIF) CDP target by including its real content, wrapped under an `iframe` node, the same as same-origin content -- and SHALL render any other cross-origin (or not-yet-loaded) iframe as an explicit leaf node describing that it cannot be inspected, rather than omitting it from the snapshot.

#### Scenario: A cross-origin iframe backed by an OOPIF shows its real content
- **WHEN** `browser_snapshot` is called on a page containing a cross-origin `<iframe>` that Chrome has isolated into its own OOPIF target
- **THEN** the rendered snapshot includes that iframe's actual interactive content (with namespaced refs) nested under an `iframe` entry, not a "not inspectable" placeholder

#### Scenario: A cross-origin iframe is shown as a boundary, not silence
- **WHEN** `browser_snapshot` is called on a page containing a cross-origin `<iframe>` that has no correlated, attached OOPIF target (e.g. an opaque-origin `data:` URL, which Chrome never promotes to its own process)
- **THEN** the rendered snapshot includes a node for it indicating it isn't inspectable, distinguishable from both same-origin content and OOPIF content

### Requirement: Correct click/type coordinates for elements inside a same-origin iframe
The engine SHALL translate a ref resolved from inside a same-origin iframe, or inside an OOPIF-backed cross-origin iframe, into top-level-page viewport coordinates, so `browser_click`/`browser_type` land on the correct on-screen location regardless of which process actually renders that pixel.

#### Scenario: Clicking a ref inside a same-origin iframe hits the right element
- **WHEN** `browser_click` is called with a ref that resolved from inside a same-origin iframe
- **THEN** the click registers on that element, not on whatever happens to occupy the same untranslated coordinates in the top-level page

#### Scenario: Clicking a ref inside an OOPIF-backed cross-origin iframe hits the right element
- **WHEN** `browser_click` is called with a namespaced ref (e.g. `f1:e3`) that resolved from inside an OOPIF's own attached session
- **THEN** the local coordinates resolved inside the OOPIF's session are combined with the OOPIF's own on-screen position in the top-level page, and the click registers on the correct element inside the cross-origin frame
