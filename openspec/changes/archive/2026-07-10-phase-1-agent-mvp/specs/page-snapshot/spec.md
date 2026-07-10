# page-snapshot

## ADDED Requirements

### Requirement: Injected accessibility-style walker
The engine SHALL produce a page snapshot by evaluating an injected JavaScript walker against the page's main world, computing for each interactive or structural element: a role (from ARIA `role` or tag-based mapping), an accessible name (`aria-label`, `aria-labelledby`, associated `<label>`, `alt`, or trimmed text content), and visibility. Non-interactive, non-structural leaf elements with no rendered text MUST be omitted.

#### Scenario: Snapshot of a simple form
- **WHEN** `snapshot()` is called on a page containing a labeled text input and a submit button
- **THEN** the returned tree includes a `textbox` node with the input's accessible name and a `button` node with the button's name, each carrying a ref

#### Scenario: Non-interactive noise is omitted
- **WHEN** `snapshot()` is called on a page containing decorative `<div>` wrappers with no text and no interactive descendants
- **THEN** those wrappers do not appear as nodes in the returned tree

### Requirement: Stable refs scoped to the page's lifetime
The walker SHALL assign each interactive/structural element a stable `eN` ref the first time it is encountered, and SHALL return the same ref for the same element on subsequent snapshots within the same page (no navigation in between). Refs MUST reset when the page navigates to a new document.

#### Scenario: Same element, same ref across snapshots
- **WHEN** `snapshot()` is called twice in a row on an unchanged page
- **THEN** a given element receives the same ref both times

#### Scenario: Refs reset on navigation
- **WHEN** the page navigates to a new URL and `snapshot()` is called again
- **THEN** refs are assigned fresh (no collision with, or reuse of, refs from the previous document is guaranteed by the browser's page reload of the JS global)

### Requirement: Text rendering for LLM consumption
The engine SHALL render the walker's tree as indented text, one node per line, showing role, quoted accessible name (if any), value (for inputs, if non-empty), `checked`/`disabled`/`hidden` state flags (if applicable), and the `[ref]` marker for actionable/structural nodes.

#### Scenario: Rendered line format
- **WHEN** a `textbox` node with name "Card number" and value "4242" is rendered at depth 1
- **THEN** the output line is `  - textbox "Card number" value="4242" [e6]` (indentation and exact fields per the current renderer)

### Requirement: Ref resolution and staleness
The engine SHALL resolve a ref to a live DOM element via the walker's ref map at the time of use (not from a cached position). If the ref no longer maps to an attached element, the engine MUST return a typed stale-ref error rather than acting on stale coordinates.

#### Scenario: Acting on a valid ref
- **WHEN** an action targets a ref that is still present in the DOM
- **THEN** the engine resolves current bounding-box coordinates for that element before acting

#### Scenario: Acting on a stale ref
- **WHEN** an action targets a ref for an element that has since been removed from the DOM
- **THEN** the engine returns a typed `StaleRef` error identifying the ref
