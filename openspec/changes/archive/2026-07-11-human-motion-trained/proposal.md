## Why

The synthetic `human-motion` capability (`careful`/`average`/`fast` personas) produces plausible-looking motion from hand-picked constants, but some applications under test specifically try to detect whether a *human* or a *bot* is driving them, with heuristics tuned against exactly this kind of hand-authored synthetic timing. The user's explicit ask: an optional training mode where a real human performs an action once, the system learns their timing/motion *variability*, and replay uses a *freshly varied* approximation of it — never the same twice — rather than either a fixed synthetic persona or a literal, byte-identical recording (which would itself be a detectable, mechanically-repeated tell). If replay is requested for a name that was never trained, the system must say so clearly rather than silently substituting a synthetic default.

## What Changes

- New training mode: `browser_train_start(name)` begins capturing genuinely trusted DOM input events (`mousemove`, `mousedown`, `mouseup`, `keydown`, `keyup`, with high-resolution timestamps) from the page the human is actually interacting with; `browser_train_stop()` ends capture and fits a `Persona` from the recorded samples (mouse: duration/distance pairs for a Fitts's-law fit, positional deviation from the ideal chord for jitter, frequency of overshoot-then-correct moves; typing: inter-keystroke interval mean/std). The fitted persona is a `Persona` value — same struct the synthetic presets already use — so it's a drop-in replacement anywhere a `Persona` is consumed.
- Trained profiles persist by name under `<data-dir>/aib/profiles/human/<name>.json` and survive across sessions.
- `browser_click`/`browser_type` gain a `trained_profile: Option<String>` parameter, orthogonal to the existing `persona` (synthetic-preset) parameter. When set, the engine loads the named profile and replays it with **fresh** per-call sampling from the fitted distributions (mirroring exactly how the synthetic personas already work: a `Persona`'s parameters feed the same `mouse_path`/`typing_timeline` synthesis functions, just with learned numbers instead of hand-picked ones) — so consecutive replays are visibly different from each other while remaining statistically consistent with the original demonstration.
- Requesting `trained_profile` with a name that has no saved profile is a typed error ("training required for profile %s") — never a silent fallback to a synthetic persona.

**Explicitly out of scope:** replaying the literal recorded sequence verbatim (defeats the "not always the same" requirement, and is itself a bot-detection tell); cross-application profile transfer or profile editing UI; capturing anything at the OS level (stays within CDP-observable, page-scoped trusted events, matching this backend's existing scope — see PROPOSAL.md §6's synthetic-human vs true-user-mode split).

## Capabilities

### Modified Capabilities
- `human-motion`: adds training capture, persona-fitting-from-samples, and trained-profile replay (with fresh per-call variability) as new requirements on the existing capability; the untrained-profile-name rejection extends the existing "Persona presets" typed-error precedent to the trained-profile namespace.

## Impact

- `crates/engine/src/motion/`: new `train.rs` (sample capture types, Fitts's-law/jitter/cadence fitting from raw samples) and `profile_store.rs` (JSON persistence under the profiles dir, reusing `cdp::launch::profile_base_dir()`'s pattern).
- `crates/cdp/src/protocol/runtime.rs` (or wherever binding support lives), `crates/cdp/src/ops.rs`: `Runtime.addBinding` + `Runtime.bindingCalled` event plumbing so an injected recorder script can report trusted DOM events back to Rust — new CDP surface, not currently used anywhere in this codebase.
- `crates/engine/src/session.rs`: `Session::train_start(name)` / `Session::train_stop()`; `click_with`/`type_text_with` accept an optional trained-profile lookup alongside the existing `HumanLike` (synthetic) path.
- `crates/mcp/src/lib.rs`: `browser_train_start`/`browser_train_stop` tools; `trained_profile` param on `browser_click`/`browser_type`.
- New fixture and a headed verification where the user is asked to physically perform a short click+type sequence during training, then watches two separate trained replays and confirms they're visibly not identical.
