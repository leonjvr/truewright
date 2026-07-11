# Tasks: Virtual Clock

## 1. Injected clock

- [x] 1.1 `crates/engine/assets/virtual_clock.js`: `FakeDate` wrapper (`Function.prototype.bind.apply` forwarding, shared `.prototype`), `Date.now`/`performance.now` overrides, `setTimeout`/`clearTimeout`/`setInterval`/`clearInterval`/`requestAnimationFrame`/`cancelAnimationFrame` overrides backed by a virtual-time queue, `window.__aibAdvanceClock(ms)` (find-earliest-fire-repeat loop)

## 2. Engine/MCP integration

- [x] 2.1 `Session::set_clock(time_ms: u64) -> Result<()>`: builds the clock-install script with the given start time, registers via `add_init_script`
- [x] 2.2 `Session::advance_clock(ms: u64) -> Result<()>`: evaluates `window.__aibAdvanceClock(ms)` via `Runtime.evaluate` (active call, not init-time)
- [x] 2.3 MCP: `browser_set_clock(time_ms)`, `browser_advance_clock(ms)` tools

## 3. Verification

- [x] 3.1 Host: full suite green
- [x] 3.2 Integration test: a `setTimeout`-scheduled DOM change is absent before advancing past its delay, present after
- [x] 3.3 Integration test: a `setTimeout` chain scheduled within the same advance (callback schedules another callback still due within the window) all fire in one `browser_advance_clock` call
- [x] 3.4 Integration test: `Date.now()` reflects `browser_set_clock`'s value and does not change without an explicit advance
- [x] 3.5 Container: `bash docker/run-tests.sh` green

## 4. Wrap-up

- [x] 4.1 README documents `browser_set_clock`/`browser_advance_clock` and the "frozen unless advanced" model
- [x] 4.2 `openspec validate virtual-clock` clean; sync specs; archive
