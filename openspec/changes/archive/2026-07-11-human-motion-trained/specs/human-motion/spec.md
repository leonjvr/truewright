## ADDED Requirements

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
