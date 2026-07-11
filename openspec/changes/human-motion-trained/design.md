# Design: Human Motion (Trained)

## Context

Follow-up to `human-motion-synthetic` (archived), which deliberately kept `Persona` as plain data specifically so a trained profile could be "just another `Persona` value" reusing the same `mouse_path`/`typing_timeline` synthesis and the same `click_with`/`type_text_with` dispatch machinery. This change is almost entirely about *getting* a `Persona` from real samples and persisting it by name — the replay side barely changes.

## Goals / Non-Goals

**Goals:** capture genuinely trusted human input during an explicit training window; fit a `Persona` from it; persist/load by name; replay through the existing synthetic machinery unchanged; reject replay of an untrained name with a clear, typed error; replay is never byte-identical across calls.

**Non-Goals:** literal recording/replay of the exact captured sequence (the whole point is variability, and a literal replay is itself a detectable tell); bigram-aware or multi-session-averaged fitting (v1 fits one profile from one training session, same "no sophistication beyond what the data obviously supports" stance as the synthetic typing model); OS-level capture (stays CDP/page-scoped, matching PROPOSAL.md §6's synthetic-human vs true-user-mode split — a trained *synthetic-backend* persona is still dispatched via CDP, not `SendInput`).

## Decisions

1. **Replay requires zero new mechanism beyond producing a `Persona`.** The existing `HumanLike { persona, seed }` / `click_with` / `type_text_with` path already draws a fresh random seed when the caller omits one — that's already "never the same twice." A trained profile just supplies a fitted `Persona` instead of a hand-picked preset through the exact same pipe. `trained_profile: Option<String>` on `browser_click`/`browser_type` resolves to a `Persona` by loading and deserializing the saved profile, then constructs the same `HumanLike` the synthetic path already builds. If both `persona` and `trained_profile` are supplied, that's a request error (ambiguous) — pick one.
2. **Capture reuses three existing patterns wholesale:** the bounded `EventStream<E: CdpEvent>` subscription mechanism (Phase 0), the background-collector-task-with-`Arc<Mutex<Vec<_>>>`-and-stop-signal shape (`recording.rs`'s `collect_frames`), and `cdp::launch::profile_base_dir()` for the on-disk location. New CDP surface: `Runtime.addBinding` + the `Runtime.bindingCalled` event, not previously used in this codebase — needed so an injected recorder script can report real DOM events back to Rust while training is active.
3. **The recorder only reports `event.isTrusted === true` events.** Defensive hygiene: a training session should never accidentally ingest this engine's own synthetic dispatch (e.g., a concurrent or leftover instant `click_at` call) as if it were human data. `mousemove` is reported at native event rate (browsers already coalesce these); `mousedown`/`mouseup`/`keydown`/`keyup` are reported individually, each with `performance.now()`.
4. **Fitting math, per parameter:**
   - **Typing cadence** (`key_delay_mean_ms`/`key_delay_std_ms`): directly the sample mean/stddev of consecutive `keydown` timestamp deltas. No regression needed — most robust of the fitted parameters.
   - **Fitts's-law constants** (`fitts_a_ms`/`fitts_b_ms`): a training session must contain **at least 3 distinct mouse-down-terminated movements** (a movement = the `mousemove` run between two `mousedown` events, or from capture-start to the first `mousedown`) above a minimum distance, so `duration_ms = a + b·log2(distance/size+1)` has enough (distance, duration) pairs for a 2-parameter least-squares fit. Fewer than 3 movements (or fewer than 5 keystrokes for typing) fails capture with a typed error rather than saving a degenerate/overfit profile.
   - **Jitter** (`jitter_px`): RMS perpendicular deviation of the raw `mousemove` samples from the straight chord between each movement's start and end point. An approximation (the synthetic model adds jitter around a Bezier, not a straight line), acceptable for the same reason the synthetic model's hand-picked constants were acceptable — v1 needs "visibly human," not a rigorously validated biomechanical model.
   - **Overshoot probability** (`overshoot_p`): fraction of captured movements whose path's projection onto the chord direction decreases (a reversal) within the final 20% of the movement, i.e. the cursor visibly passed the endpoint and came back.
5. **Profiles persist as JSON** under `<data-dir>/aib/profiles/human/<name>.json` — a serialized `Persona` plus metadata (sample counts, capture timestamp) for `browser_train_stop`'s response and future debugging, not consumed by replay. Same directory family as `aib/profiles/<name>` (browser user-data) and `aib/recordings/<id>` (screencasts); `profiles/human/` disambiguates from the browser-profile directories already living under `profiles/`.
6. **One training session at a time**, mirroring `browser_record_start`/`stop`'s existing "already in progress" guard — training and recording both hold page-level event-stream state and shouldn't interleave with each other or with themselves.

## Risks / Trade-offs

- [3-movement minimum for a 2-parameter Fitts's-law fit is a small sample; the fitted `a`/`b` may not generalize well] → acceptable for v1's "learn *this* human's rough timing signature" goal, not a claim of statistical rigor; documented as an approximation same as the synthetic model's hand-picked constants.
- [`isTrusted` filtering can be defeated by a sufficiently determined synthetic-event forger] → not a security boundary — training mode's threat model is "don't accidentally pollute training data with our own dispatch," not "resist an adversarial page." Out of scope.
- [Injected recorder script adds a `Runtime.addBinding` global the trained page's own JS could theoretically observe] → training only runs against pages the user explicitly opens for that purpose; no different in kind from the existing walker/resolve scripts already evaluated in the main world (documented main-world limitation carried over from Phase 1).

## Migration Plan

Additive. `trained_profile` is a new, optional parameter; omitting it (today's default) is unaffected. No existing requirement's behavior changes.

## Open Questions

None blocking.
