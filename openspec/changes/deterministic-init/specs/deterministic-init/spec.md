## ADDED Requirements

### Requirement: Init scripts run before a page's own scripts
The engine SHALL support registering JavaScript that runs before any of a page's own scripts, on every subsequent navigation in the session, distinct from the existing after-load (`Runtime.evaluate`-based) injection.

#### Scenario: An init script's effect is visible to the page's own first-run code
- **WHEN** an init script sets a value before navigation, and the target page's own inline script (run at parse time, before any agent action) reads that value
- **THEN** the page's own script observes the value the init script set

#### Scenario: Init scripts apply to every subsequent navigation
- **WHEN** an init script is registered and the session navigates twice in a row
- **THEN** the init script's effect is present after both navigations, not just the first

### Requirement: Seeded, reproducible Math.random
The engine SHALL support overriding `Math.random` with a deterministic pseudo-random generator seeded from a caller-supplied value, such that the same seed produces the same sequence of values across separate navigations.

#### Scenario: Same seed reproduces the same sequence
- **WHEN** randomness is seeded with the same value across two separate navigations, and the page calls `Math.random()` the same number of times on each
- **THEN** both navigations observe identical sequences of values

#### Scenario: Different seeds produce different sequences
- **WHEN** randomness is seeded with two different values across two separate navigations
- **THEN** the observed `Math.random()` sequences differ
