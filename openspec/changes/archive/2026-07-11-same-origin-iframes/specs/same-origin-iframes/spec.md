## ADDED Requirements

### Requirement: Same-origin iframe content is visible in snapshots
The engine SHALL include a same-origin iframe's content in the page snapshot, wrapped under an `iframe` node reflecting the frame boundary, rather than omitting it.

#### Scenario: A same-origin iframe's interactive elements appear in the snapshot
- **WHEN** `browser_snapshot` is called on a page containing a same-origin `<iframe>` with interactive content inside it
- **THEN** the rendered snapshot includes that content (with refs) nested under an `iframe` entry, not silently absent

### Requirement: Cross-origin iframes are surfaced explicitly, not silently dropped
The engine SHALL render a cross-origin (or not-yet-loaded) iframe as an explicit leaf node describing that it cannot be inspected, rather than omitting it from the snapshot.

#### Scenario: A cross-origin iframe is shown as a boundary, not silence
- **WHEN** `browser_snapshot` is called on a page containing a cross-origin `<iframe>`
- **THEN** the rendered snapshot includes a node for it indicating it isn't inspectable, distinguishable from same-origin iframe content

### Requirement: Correct click/type coordinates for elements inside a same-origin iframe
The engine SHALL translate a ref resolved from inside a same-origin iframe into top-level-page viewport coordinates, so `browser_click`/`browser_type` land on the correct on-screen location.

#### Scenario: Clicking a ref inside a same-origin iframe hits the right element
- **WHEN** `browser_click` is called with a ref that resolved from inside a same-origin iframe
- **THEN** the click registers on that element, not on whatever happens to occupy the same untranslated coordinates in the top-level page
