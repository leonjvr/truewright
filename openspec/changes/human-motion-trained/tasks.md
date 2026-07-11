# Tasks: Human Motion (Trained)

## 1. CDP binding support

- [ ] 1.1 `crates/cdp/src/protocol/runtime.rs`: `Runtime.addBinding`/`AddBindingParams`, `Runtime.removeBinding`, `BindingCalled` event (`name`, `payload`, execution context id)
- [ ] 1.2 `cdp::ops::Page::add_binding(name)` / `remove_binding(name)`; confirm `events::<BindingCalled>()` works via the existing `EventStream` mechanism

## 2. Capture

- [ ] 2.1 `crates/engine/assets/train.js`: injected recorder — real `mousemove`/`mousedown`/`mouseup`/`keydown`/`keyup` listeners, `event.isTrusted` filter, reports `{type, x, y, key, t}` via the bound function
- [ ] 2.2 `crates/engine/src/motion/train.rs`: `Sample` types; background collector task (mirrors `recording.rs`'s `collect_frames` shape: `Arc<Mutex<Vec<Sample>>>`, stop signal, spawned on `BindingCalled` events)
- [ ] 2.3 `Session::train_start(name)` / `Session::train_stop()`; one training session at a time (mirrors `browser_record_start`/`stop`'s in-progress guard)

## 3. Fitting

- [ ] 3.1 Typing cadence: mean/stddev of consecutive `keydown` deltas
- [ ] 3.2 Fitts's-law `a`/`b`: least-squares fit over (distance, duration) pairs from mousedown-terminated movements; require ≥3 movements
- [ ] 3.3 Jitter: RMS perpendicular deviation of `mousemove` samples from each movement's straight chord
- [ ] 3.4 Overshoot probability: fraction of movements with a late-stage reversal past the endpoint
- [ ] 3.5 Insufficient samples (< 3 movements or < 5 keystrokes) fails with a typed error, no profile saved

## 4. Persistence

- [ ] 4.1 `crates/engine/src/motion/profile_store.rs`: save/load a `Persona` + metadata (sample counts, captured-at) as JSON under `<data-dir>/aib/profiles/human/<name>.json`, reusing `cdp::launch::profile_base_dir()`
- [ ] 4.2 `Session::persona_or_trained(persona: Option<String>, trained_profile: Option<String>) -> Result<Persona>`: both-set is a typed error; `trained_profile` set but unsaved is a typed "training required" error; otherwise falls through to the existing `Session::persona` lookup

## 5. Engine/MCP integration

- [ ] 5.1 `click_with`/`type_text_with` accept a resolved `Persona` regardless of its origin (synthetic preset or trained) — no change needed if task 4.2's resolver feeds the existing `HumanLike` construction
- [ ] 5.2 MCP: `browser_train_start(name)`, `browser_train_stop()` tools; `browser_click`/`browser_type` gain `trained_profile: Option<String>`

## 6. Verification

- [ ] 6.1 Host: full suite green (fitting math unit-tested against synthetic sample fixtures — no browser needed for the math itself)
- [ ] 6.2 Manual headed demo: user is prompted to physically perform a click + type sequence during `browser_train_start`; after `browser_train_stop`, two separate trained replays are dispatched back to back and visibly differ (cursor path/timing), watched live
- [ ] 6.3 Untrained-profile-name request surfaces the typed "training required" error, verified live (not just unit-tested)
- [ ] 6.4 Container: `bash docker/run-tests.sh` green (training itself needs a real human and can't run in the container; the fitting-math unit tests and untrained-profile-error path do)

## 7. Wrap-up

- [ ] 7.1 README documents `browser_train_start`/`stop` and `trained_profile`, including the "training required" error contract
- [ ] 7.2 `openspec validate human-motion-trained` clean; sync specs; archive
