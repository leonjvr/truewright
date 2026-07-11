# human-motion Specification

## Purpose
Seeded, reproducible synthesis of human-like mouse and keyboard input (curved timed paths, non-uniform typing cadence) as an opt-in alternative to instant dispatch, so `browser_click`/`browser_type` can drive an application the way a real user would rather than teleporting/instant-clicking. Personas are either hand-authored presets (`careful`/`average`/`fast`) or fitted from a real human's captured demonstration via an explicit training mode; replay from either source uses the same synthesis and dispatch machinery.
## Requirements
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

### Requirement: Training capture from real trusted input
The engine SHALL support an explicit training mode that captures `mousemove`, `mousedown`, `mouseup`, `keydown`, and `keyup` events with high-resolution timestamps from a page the user is actively interacting with, started and stopped by name. Events are filtered by `event.isTrusted === true` (excluding a page's own untrusted, JS-dispatched events) AND by an explicit suppression flag that this engine's own click/type/press dispatch sets for the duration of each of its actions while training is active — CDP-dispatched `Input.dispatch*Event` calls are themselves `isTrusted` in Chrome, so `isTrusted` alone cannot distinguish this engine's own synthetic input from a real human's; the suppression flag is the mechanism that actually does.

#### Scenario: Training captures a real interaction
- **WHEN** training is started, a human performs a click and types text on the page, and training is stopped
- **THEN** the engine has recorded timestamped samples of that interaction, not synthetic dispatch events

#### Scenario: Synthetic dispatch is not captured as training data
- **WHEN** training is active and this engine's own click/type/press dispatch (instant or human-like) fires a synthetic input event
- **THEN** that event is not included in the captured samples, because the suppression flag was set for the duration of that dispatch

### Requirement: Persona fitted from captured samples
On `browser_train_stop`, the engine SHALL fit a `Persona` (Fitts's-law constants, jitter, overshoot probability, typing-cadence mean/stddev) from the captured samples and persist it by the training session's name, reusable anywhere a built-in persona is. A capture with too few distinct mouse movements or keystrokes to fit MUST fail with a typed error instead of persisting a degenerate profile.

#### Scenario: Sufficient capture produces a usable profile
- **WHEN** training captures at least 3 distinct mouse movements and 5 keystrokes, then is stopped
- **THEN** a named profile is saved and can be selected for a later human-like action

#### Scenario: Insufficient capture is rejected
- **WHEN** training is stopped after fewer than 3 mouse movements or fewer than 5 keystrokes were captured
- **THEN** the engine returns a typed error and does not save a profile

### Requirement: Trained-profile replay with fresh variability
When a human-like action specifies a trained profile, the engine SHALL replay it through the same seeded timeline synthesis used for built-in personas, so that omitting an explicit seed produces different (but statistically consistent) motion/timing on each call rather than a literal repeat of the captured demonstration.

#### Scenario: Consecutive replays are not identical
- **WHEN** the same human-like click is dispatched twice against a trained profile without an explicit seed
- **THEN** the two dispatches' mouse paths and timings differ, unlike a literal recording-and-replay

#### Scenario: An explicit seed still reproduces exactly
- **WHEN** a human-like action against a trained profile is dispatched with an explicit seed
- **THEN** repeating it with the same seed reproduces the same path/timing, consistent with the untrained-persona seeding guarantee

### Requirement: Untrained profile fails clearly
Requesting a trained profile name with no saved profile MUST be rejected with a typed error identifying that training is required, rather than silently falling back to a synthetic persona or default behavior.

#### Scenario: Replay against a never-trained name
- **WHEN** a human-like action specifies `trained_profile` with a name that has no saved profile
- **THEN** the engine returns a typed error stating that profile has not been trained, and does not fall back to a synthetic persona

#### Scenario: Ambiguous persona selection is rejected
- **WHEN** a human-like action specifies both `persona` and `trained_profile`
- **THEN** the engine returns a typed error rather than silently preferring one

