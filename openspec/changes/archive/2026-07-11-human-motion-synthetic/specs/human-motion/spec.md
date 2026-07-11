# human-motion

## ADDED Requirements

### Requirement: Seeded, reproducible timelines
The engine SHALL compute human-like input timelines (mouse paths, typing cadence) from a seeded pseudo-random generator, such that the same seed and the same action inputs (start/end points, text) produce the same timeline. The seed used SHALL be reported back to the caller.

#### Scenario: Same seed reproduces the same timeline
- **WHEN** a human-like click is dispatched twice with the same seed, start position, and target
- **THEN** both dispatches compute identical sequences of mouse positions and timings

#### Scenario: Seed is reported
- **WHEN** a human-like action is requested without an explicit seed
- **THEN** the engine generates one and includes it in the result, so the run can be reproduced later

### Requirement: Curved, timed mouse movement to a target
When human-like mode is requested for a click, the engine SHALL move the mouse from its last known position to the target along a curved (not straight-line, not instantaneous) path, dispatched as a sequence of `Input.dispatchMouseEvent` (`mouseMoved`) events over time, with total duration derived from a Fitts's-law relationship between distance and target size, before dispatching the press/release.

#### Scenario: Path is not a single jump
- **WHEN** a human-like click targets an element far from the current mouse position
- **THEN** more than one `mouseMoved` event is dispatched between the start and end points, spaced over a non-zero duration

#### Scenario: Instant mode is unaffected
- **WHEN** a click is requested without human-like mode
- **THEN** behavior is unchanged from today: a single press/release at the target's center, no movement sequence

### Requirement: Per-character typing cadence
When human-like mode is requested for typing, the engine SHALL dispatch each character as a separate `Input.dispatchKeyEvent` down/up pair with an inter-character delay drawn from the active persona's distribution, rather than inserting the whole string in one `Input.insertText` call.

#### Scenario: Characters arrive with variable timing
- **WHEN** human-like typing dispatches a multi-character string
- **THEN** consecutive characters are separated by non-uniform delays (not a fixed interval), and the target field's value matches the typed string once complete

### Requirement: Persona presets
The engine SHALL provide at least three built-in personas (`careful`, `average`, `fast`) with distinct timing/jitter parameters, selectable by name; an unrecognized persona name MUST be rejected with a typed error rather than silently substituting a default.

#### Scenario: Selecting a persona changes timing
- **WHEN** the same click is dispatched human-like with `careful` versus `fast`
- **THEN** the `careful` run's total movement duration is longer

#### Scenario: Unknown persona is rejected
- **WHEN** human-like mode is requested with a persona name that isn't one of the built-ins
- **THEN** the engine returns a typed error identifying the invalid name
