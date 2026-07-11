## Why

`browser_click`/`browser_type` dispatch instantly (a single press+release at the target center, `Input.insertText` for the whole string at once) — the exact "teleporting mouse, instant clicks" gap PROPOSAL.md names as one of Playwright's two core problems, still open in ai-browser itself. The user also wants to test applications that specifically try to detect whether a human or a bot is driving them; that requires curved, timed mouse movement and per-character typing cadence as an available mode, not just a faster synthetic dispatch. This is the original roadmap's Phase 2, now unblocked after the two browser-efficiency/screencast changes.

## What Changes

- New `motion` module in `crates/engine`: seeded (reproducible), persona-parameterized synthetic input timelines — minimum-jerk/Bezier mouse paths sized by Fitts's law, with overshoot+correction and jitter; per-character typing delays from a log-normal distribution. Personas: `careful`, `average` (default), `fast`.
- CDP layer gains real per-step mouse movement (`Input.dispatchMouseEvent` type=`mouseMoved`, dispatched along the path at the timeline's own pacing) and per-keystroke dispatch (`Input.dispatchKeyEvent` down/up pairs with real inter-key delays), alongside the existing instant `click_at`/`insert_text` — both paths coexist, selected per call.
- `Session::click`/`Session::type_text` gain a `human_like: Option<Persona>` parameter (`None` keeps today's instant behavior — no behavior change by default); `browser_click`/`browser_type` MCP tools gain `human_like: bool` and `persona: Option<String>` parameters.
- A `seed: Option<u64>` on the session (or per-call) makes human-like runs reproducible: same seed, same timeline — reported back so a flaky-looking run can be replayed exactly.

**Explicitly out of scope (follow-up change):** learning motion/timing from a real human demonstration and replaying a statistically-varied approximation of it. That's the next change, built on this one's `Persona`/timeline machinery (a trained profile is a persona fitted from real samples instead of hand-authored constants).

## Capabilities

### New Capabilities
- `human-motion`: seeded, persona-parameterized mouse-path and typing-cadence synthesis, wired into click/type as an opt-in mode.

### Modified Capabilities
- `browser-actions`: click/type gain the `human_like`/`persona` option; default (instant) behavior is unchanged, so no existing requirement's behavior actually changes — this is additive from the spec's point of view, listed here only because it touches the same tool signatures documented there.

## Impact

- `crates/engine/src/motion/`: new module (persona presets, path/typing synthesis, seeded RNG).
- `crates/cdp/src/protocol/input.rs`, `crates/cdp/src/ops.rs`: real per-step mouse move + per-keystroke dispatch methods, alongside existing instant methods.
- `crates/engine/src/session.rs`, `crates/mcp/src/lib.rs`: new parameters on existing click/type paths.
- New dependency: `rand` + `rand_pcg` (seeded, reproducible RNG), `engine` crate only.
- New animated/interactive fixture or reuse of `form.html`/`animated.html` for a headed visual verification the user watches directly.
