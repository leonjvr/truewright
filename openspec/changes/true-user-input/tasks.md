# Tasks: true-user-input

## 1. Dependency setup
- [ ] 1.1 Add `windows` as an explicit workspace dependency (pin to the version already resolved in `Cargo.lock`, v0.61.x) with features `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_WindowsAndMessaging`, `Win32_Foundation`, `Win32_Graphics_Gdi` (for monitor metrics)
- [ ] 1.2 Confirm the crate compiles Windows-only (`#[cfg(windows)]` gating at the module level, not scattered per-function)

## 2. OS input module (`crates/cdp/src/os_input.rs`)
- [ ] 2.1 Window discovery: `EnumWindows` filtered by `GetWindowThreadProcessId(hwnd) == target_pid`, visible (`IsWindowVisible`), non-empty title (`GetWindowTextLengthW`); return an error if zero or ambiguous multiple matches
- [ ] 2.2 Foreground activation via `SetForegroundWindow`
- [ ] 2.3 Viewport-to-screen coordinate translation: evaluate `window.screenX/screenY/outerWidth/outerHeight/innerWidth/innerHeight/devicePixelRatio` via the existing `Runtime.evaluate` path, compute viewport screen origin, scale by `devicePixelRatio`
- [ ] 2.4 Mouse dispatch: `SendInput` with `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE` (normalized 0..65535 against primary display metrics) for movement, `MOUSEEVENTF_LEFTDOWN`/`LEFTUP` for clicks
- [ ] 2.5 Keyboard dispatch: `SendInput` with `KEYBDINPUT`/`KEYEVENTF_UNICODE` for arbitrary Unicode character down/up pairs
- [ ] 2.6 Unit tests for coordinate-translation math (pure function, no real window needed)

## 3. Engine/MCP integration
- [ ] 3.1 `Session::click_with`/`type_text_with`: accept a `true_input: bool`, route to `os_input` dispatch instead of CDP dispatch when set, reusing the existing `mouse_path`/`typing_timeline` synthesis unchanged
- [ ] 3.2 Reject cleanly (typed `EngineError` variant) when `true_input: true` and the session is headless, or the host is not Windows
- [ ] 3.3 `crates/mcp/src/lib.rs`: `true_input: bool` parameter on `browser_click`/`browser_type`; wire the new error variant through `map_engine_err`

## 4. Verification
- [ ] 4.1 Unit tests for window-matching and coordinate math pass without a real window
- [ ] 4.2 **Check in with the user immediately before running any live test** -- explain exactly what will happen (real cursor movement, real keystrokes, which window) and get explicit confirmation, same as the `human-motion-trained` physical-human demo pattern
- [ ] 4.3 Live integration test (host only, real Windows desktop session; not runnable in the Docker container, which has no real display/input focus -- note this explicitly, mirroring how `human-motion-trained` scoped its own container limitation): launch headed, click a known element via `true_input: true`, verify the click registered on the page
- [ ] 4.4 Confirm `docker/run-tests.sh` still passes (unaffected -- Windows-only code compiled out on Linux) and `cargo test --workspace` passes on host

## 5. Wrap-up
- [ ] 5.1 Update README with the `true_input` parameter, its Windows-only/headed-only caveats, and the real-OS-side-effect warning
- [ ] 5.2 Update PROPOSAL.md's Phase 4 roadmap row with verified exit-criteria evidence
- [ ] 5.3 `openspec archive true-user-input -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 5.4 Three commits: Propose, Implement, Sync-specs-and-archive
