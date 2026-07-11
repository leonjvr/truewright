# Design: Human Motion (Synthetic)

## Context

Original Phase 2 from PROPOSAL.md's roadmap, now unblocked. Directly requested by the user with an explicit ask to visually watch it work (headed demo) and a follow-up requirement (next change) to learn motion from a real human demonstration rather than only procedural personas.

## Goals / Non-Goals

**Goals:** curved, timed mouse movement and per-character typing as an opt-in mode on the existing `click`/`type_text`; seeded reproducibility; a headed demo the user watches directly, matching how Phase 1 and screencast-capture were verified.

**Non-Goals (this change):** learning from a real human (next change); drag/hover-menu-specific choreography beyond what a click/type path already covers; true OS-level input (Phase 4, later, Windows `SendInput`) — this stays CDP-only, matching PROPOSAL.md's synthetic-human backend.

## Decisions

1. **`Persona` is plain data (durations/jitter/curvature parameters), not a trait.** Keeps the next change's "trained persona" story simple: a trained profile is just a `Persona` value fitted from real samples instead of one of the three hand-authored presets — same struct, same consumer code in the path/typing synthesis functions.
2. **`rand` + `rand_pcg` (Pcg64), not `rand_chacha`.** Pcg64 is fast, small, and its determinism-from-seed guarantee is exactly what's needed; cryptographic strength (ChaCha) is irrelevant here and slower for no benefit.
3. **Timeline computed entirely up front, then dispatched with real `tokio::time::sleep` pacing between CDP calls** — mirrors PROPOSAL.md §5's original design (`motion::Timeline` as backend-agnostic data) and the existing `recording.rs` pattern of computing before executing. Keeps the synthesis function pure and independently testable (feed a seed, get a deterministic `Vec<TimedMouseEvent>`, assert on it — no browser needed for that part).
4. **Mouse path model: minimum-jerk-style Bezier with a Fitts's-law duration and a probabilistic overshoot+correction.** `duration_ms = fitts_a + fitts_b * log2(distance / target_size + 1)`; control points perturbed from the straight line by up to `curvature_px`; ~15% of moves (persona-dependent) overshoot the target by a small amount before a short corrective move — this is what makes the path visually read as "a person aimed for it," which is the whole point of the headed demo.
5. **Typing model: per-character delay from a log-normal distribution (mean/stddev from the persona), no bigram-class sophistication in v1.** Bigram-aware timing (same-hand vs alternating-hand delay classes) is a real refinement but adds meaningful complexity for a synthetic-preset v1; a single per-persona distribution already produces visibly non-uniform timing. The next change's *trained* typing profile can and should be bigram-aware if the real samples show it — that's learned from data, not hand-tuned, so the sophistication is worth it there.
6. **Instant path is untouched; human-like is strictly additive.** `Session::click`/`type_text` keep their current signatures' behavior when `human_like` is `None`/`false` — no regression risk to Phase 1's verified behavior, and the existing bounded-poll actionability gate still runs first in both modes (human-like movement happens only once the target is confirmed actionable, same as today).

## Risks / Trade-offs

- [Per-character CDP dispatch is slower than one `Input.insertText` call] → intentional; that's the entire point of human-like mode, and instant mode remains available for the common case.
- [Fitts's-law/Bezier parameters are hand-picked, not empirically validated against real human data] → acceptable for v1's "looks and behaves plausibly human" goal; the next change replaces hand-picked constants with learned ones where it matters (a specific app's bot-detection heuristics).
- [Headed demo depends on the browser actually rendering synthetic `mouseMoved` dispatches as a visible cursor] → confirmed feasible (this is the same mechanism Playwright's `page.mouse.move()` uses and is visibly observable in a headed window); verified directly in this change's verification step, not assumed.

## Migration Plan

Additive. No existing tool behavior changes when the new parameters are omitted.

## Open Questions

None blocking.
