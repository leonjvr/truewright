# Tasks: Human Motion (Synthetic)

## 1. Motion module

- [x] 1.1 `crates/engine/src/motion/mod.rs`: `Persona` struct (fitts_a, fitts_b, jitter_px, overshoot_p, dwell_ms mean/std, key_delay_ms mean/std); `careful`/`average`/`fast` presets
- [x] 1.2 `crates/engine/src/motion/path.rs`: `mouse_path(from, to, target_size, persona, rng) -> Vec<TimedPoint>` — Bezier w/ Fitts's-law duration, overshoot+correction, jitter
- [x] 1.3 `crates/engine/src/motion/typing.rs`: `typing_timeline(text, persona, rng) -> Vec<TimedKey>` — log-normal per-character delay
- [x] 1.4 `rand` + `rand_pcg` deps added to `crates/engine/Cargo.toml`
- [x] 1.5 Unit tests: same seed → identical timeline (golden-style equality check); `careful` duration > `fast` duration for the same distance; unknown persona name rejected

## 2. CDP dispatch

- [x] 2.1 `cdp::ops::Page::move_mouse_to(x, y)`: single `Input.dispatchMouseEvent` type=`mouseMoved`
- [x] 2.2 `cdp::ops::Page::dispatch_char(key, code, text)`: keyDown/keyUp pair carrying the actual character (extends the existing named-key dispatch to arbitrary characters)

## 3. Engine/MCP integration

- [x] 3.1 `Session::click` gains `human_like: Option<(Persona, Option<u64>)>` (or equivalent); when set, resolves actionability as today then walks the computed path via `move_mouse_to` with real pacing before press/release
- [x] 3.2 `Session::type_text` gains the same option; when set, dispatches per-character via `dispatch_char` with real pacing instead of `insert_text`
- [x] 3.3 `browser_click`/`browser_type` MCP tools gain `human_like: bool`, `persona: Option<String>` params; unknown persona name surfaces as a tool error; result text reports the seed used

## 4. Verification

- [x] 4.1 Host: full suite green (existing instant-mode tests unaffected)
- [x] 4.2 Manual headed demo: human-like click + type against the form fixture, watched live — cursor visibly curves/moves before clicking, text appears character-by-character with visible pauses; record the transcript as evidence
- [x] 4.3 Container: `bash docker/run-tests.sh` green

## 5. Wrap-up

- [x] 5.1 README documents `human_like`/`persona` params and the seed-reproducibility guarantee
- [x] 5.2 `openspec validate human-motion-synthetic` clean; sync specs; archive
