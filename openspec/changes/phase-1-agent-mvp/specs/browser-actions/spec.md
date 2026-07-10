# browser-actions

## ADDED Requirements

### Requirement: Bounded-poll actionability before acting
Before clicking or typing into a ref, the engine SHALL repeatedly resolve the ref (bounded interval, default 100ms, default 5s deadline) until the element is visible (non-zero rendered size, not `display:none`/`visibility:hidden`) and its bounding box is stable across two consecutive resolutions. If the deadline elapses first, the engine MUST return a structured timeout error naming the last known state.

_Scope note: this is a bounded-poll approximation, not the fully event-driven MutationObserver-based gate described in PROPOSAL.md §5 (Phase 5 hardening upgrades this)._

#### Scenario: Element becomes actionable within the deadline
- **WHEN** a button is initially hidden and becomes visible 300ms after the action is requested
- **THEN** the click succeeds once the button is visible and stable, without waiting the full deadline

#### Scenario: Element never becomes actionable
- **WHEN** a ref's element stays hidden for the entire deadline
- **THEN** the engine returns a structured timeout error indicating the element was never visible

### Requirement: Click by ref
The engine SHALL click a ref by resolving its element to center-point viewport coordinates and dispatching a CDP `Input.dispatchMouseEvent` press/release pair at that point.

#### Scenario: Click a button
- **WHEN** `click(ref)` targets a visible, enabled button
- **THEN** the button's click handler fires (observable via a resulting navigation, DOM change, or console log in tests)

### Requirement: Type by ref
The engine SHALL type into a ref by first clicking it to establish focus, then inserting the given text via CDP `Input.insertText`. An optional `submit` flag, when set, SHALL additionally dispatch an Enter key press after the text is inserted.

#### Scenario: Type into a text field
- **WHEN** `type(ref, "hello@example.com")` targets a text input
- **THEN** the input's value becomes `"hello@example.com"`

#### Scenario: Type and submit
- **WHEN** `type(ref, "query", submit: true)` targets a search box inside a form
- **THEN** the text is inserted and an Enter key event is dispatched immediately after

### Requirement: Key press
The engine SHALL support dispatching a small named set of keys (`Enter`, `Tab`, `Escape`, `ArrowDown`, `ArrowUp`, `Backspace`) as CDP `Input.dispatchKeyEvent` keyDown/keyUp pairs, independent of any ref.

#### Scenario: Press Enter
- **WHEN** `press("Enter")` is called while a form field has focus
- **THEN** a keydown and keyup event for Enter are dispatched to the focused element

### Requirement: Wait for text
The engine SHALL support waiting for a substring to appear in the rendered snapshot text, polling at a bounded interval (default 250ms) up to a caller-supplied timeout, returning as soon as the condition is met or a structured timeout error otherwise.

#### Scenario: Text appears before timeout
- **WHEN** `wait_for("Order confirmed", timeout_ms: 5000)` is called and the text appears after 1200ms
- **THEN** the call returns successfully once the text is present, without waiting the full timeout

#### Scenario: Text never appears
- **WHEN** `wait_for("Order confirmed", timeout_ms: 500)` is called and the text never appears
- **THEN** the engine returns a structured timeout error after ~500ms

### Requirement: On-demand screenshot
The engine SHALL capture a screenshot of the current page only when explicitly requested, returning image bytes; screenshots MUST NOT be taken automatically as part of any other operation.

#### Scenario: Explicit screenshot request
- **WHEN** `screenshot()` is called
- **THEN** non-empty image bytes for the current page state are returned
