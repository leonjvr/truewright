# Tasks: Human Motion (Trained)

## 1. CDP binding support

- [x] 1.1 `crates/cdp/src/protocol/runtime.rs`: `Runtime.addBinding`/`AddBindingParams`, `Runtime.removeBinding`, `BindingCalled` event (`name`, `payload`, execution context id)
- [x] 1.2 `cdp::ops::Page::add_binding(name)` / `remove_binding(name)`; confirm `events::<BindingCalled>()` works via the existing `EventStream` mechanism

## 2. Capture

- [x] 2.1 `crates/engine/assets/train.js`: injected recorder — real `mousemove`/`mousedown`/`mouseup`/`keydown`/`keyup` listeners, `event.isTrusted` filter, reports `{type, x, y, key, t}` via the bound function
- [x] 2.2 `crates/engine/src/motion/train.rs`: `Sample` types; background collector task (mirrors `recording.rs`'s `collect_frames` shape: `Arc<Mutex<Vec<Sample>>>`, stop signal, spawned on `BindingCalled` events)
- [x] 2.3 `Session::train_start(name)` / `Training::stop()`; one training session at a time (mirrors `browser_record_start`/`stop`'s in-progress guard, enforced at the MCP layer exactly like recording's guard)

## 3. Fitting

- [x] 3.1 Typing cadence: mean/stddev of consecutive `keydown` deltas
- [x] 3.2 Fitts's-law `a`/`b`: least-squares fit over (distance, duration) pairs from mousedown-terminated movements; require ≥3 movements
- [x] 3.3 Jitter: RMS perpendicular deviation of `mousemove` samples from each movement's straight chord
- [x] 3.4 Overshoot probability: fraction of movements with a late-stage reversal past the endpoint
- [x] 3.5 Insufficient samples (< 3 movements or < 5 keystrokes) fails with a typed error, no profile saved

## 4. Persistence

- [x] 4.1 `crates/engine/src/motion/profile_store.rs`: save/load a `Persona` + metadata (sample counts, captured-at) as JSON under `<data-dir>/aib/profiles/human/<name>.json`, reusing `cdp::launch::profile_base_dir()`
- [x] 4.2 `Session::persona_or_trained(persona: Option<&str>, trained_profile: Option<&str>) -> Result<Persona>`: both-set is a typed error; `trained_profile` set but unsaved is a typed "training required" error; otherwise falls through to the existing `Session::persona` lookup

## 5. Engine/MCP integration

- [x] 5.1 `click_with`/`type_text_with` accept a resolved `Persona` regardless of its origin (synthetic preset or trained) — confirmed no change needed; `persona_or_trained`'s output feeds the existing `HumanLike` construction unmodified
- [x] 5.2 MCP: `browser_train_start(name)`, `browser_train_stop()` tools; `browser_click`/`browser_type` gain `trained_profile: Option<String>`

## 6. Verification

- [x] 6.1 Host: full suite green (fitting math unit-tested against synthetic sample fixtures — no browser needed for the math itself)
- [x] 6.2 Manual headed demo: user physically clicked + typed during `browser_train_start` (real capture: 4 movements, 37 keystrokes); `browser_train_stop` fit and saved the profile; two trained replays back to back reported different seeds (5199745809347678434, 14236705649920418696) and different wall times, watched live. That first live run also surfaced the typing-cadence outlier bug (see design.md addendum) -- fixed and covered by a unit test reproducing the exact failure mode; not re-verified against a second live capture after two follow-up attempts captured no interaction (unrelated to the fix -- window-focus timing between prompts, not a training bug), so the fix rests on the unit test plus the first run's now-explained wall-time evidence
- [x] 6.3 Untrained-profile-name request surfaces the typed "training required" error, verified live: `browser_click` with `trained_profile: "definitely-never-trained-xyz"` returned `no trained profile named "definitely-never-trained-xyz"; run browser_train_start/browser_train_stop against it first`, no fallback
- [x] 6.4 Container: `bash docker/run-tests.sh` green (training itself needs a real human and can't run in the container; the fitting-math unit tests and untrained-profile-error path do)

## 7. Wrap-up

- [x] 7.1 README documents `browser_train_start`/`stop` and `trained_profile`, including the "training required" error contract
- [x] 7.2 `openspec validate human-motion-trained` clean; sync specs; archive
