# shadow-dom-walker Specification

## Purpose
Lets an agent see and interact with content inside open shadow roots (web components, design-system libraries, framework-agnostic widgets) instead of a snapshot showing an empty leaf where real, interactive content renders.
## Requirements
### Requirement: Open shadow root content is visible in snapshots
The engine SHALL include an open shadow root's content in the page snapshot, spliced seamlessly into the tree at the shadow host's position, rather than showing an empty leaf.

#### Scenario: A web component's shadow content appears in the snapshot
- **WHEN** `browser_snapshot` is called on a page containing a custom element with an open shadow root containing interactive content
- **THEN** the rendered snapshot includes that content (with refs), not an empty leaf where the custom element is

### Requirement: Slotted content is shown in its projected position
The engine SHALL walk a `<slot>` element's actually-projected (assigned) light-DOM content when present, falling back to the slot's own markup children only when nothing is projected.

#### Scenario: Default-slot-projected light DOM appears under the shadow tree
- **WHEN** a custom element passes light-DOM children into a shadow tree via a default `<slot>`
- **THEN** the snapshot shows that projected content nested under the shadow tree's structure, not omitted and not duplicated elsewhere

#### Scenario: Unfilled slot falls back to its own fallback content
- **WHEN** a `<slot>` has no assigned light-DOM content
- **THEN** the snapshot shows the slot's own fallback markup, if any

### Requirement: Refs and clicks work on shadow-tree elements without special handling
The engine SHALL resolve refs for elements inside an open shadow root to correct on-screen coordinates using the same mechanism as any other element, with no shadow-specific coordinate logic.

#### Scenario: Clicking a ref inside a shadow root hits the right element
- **WHEN** `browser_click` is called with a ref that resolved from inside an open shadow root
- **THEN** the click registers on that element

