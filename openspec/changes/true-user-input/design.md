# Design: True User Input (Windows SendInput)

## Context

Phase 4 of PROPOSAL.md's roadmap, and a genuinely different kind of change from everything else built so far: every prior input mode (instant, synthetic, trained) dispatches through CDP, which stays entirely within the browser's own automation surface. This dispatches through the real Windows input pipeline -- the mouse cursor visibly moves for real, and keyboard focus is briefly, really taken. That's the whole point (defeating detection that inspects OS-level signals CDP dispatch can't reach), but it also means this change's blast radius extends outside the browser window for the first time in this project.

## Goals / Non-Goals

**Goals:** real `SendInput`-based mouse/keyboard dispatch, reusing the existing human-motion timeline synthesis unchanged; correct targeting despite Chrome's window layout not being externally introspectable via native APIs alone; safe verification that doesn't surprise the user mid-test.

**Non-Goals:** a dedicated OS-level DPI "calibration probe" (see Decision #2 -- not needed the way originally scoped); multi-monitor targeting beyond the primary display; a serialization queue for concurrent real-input sessions; non-Windows platforms.

## Decisions

1. **Locate the browser's OS window via `EnumWindows` filtered by the launched browser process's PID, not a native handle CDP exposes.** CDP's `Browser.getWindowForTarget` returns its own internal `windowId` and bounds, not a native `HWND` -- there's no CDP-level shortcut here. Filter candidates to visible, top-level windows (`IsWindowVisible`, non-empty title) belonging to the target PID; for a freshly-launched, single-profile, single-page session (this project's existing "no daemon, one page" scope), exactly one such window should exist.
2. **Viewport-to-screen coordinate translation reads `window.screenX`/`screenY`/`outerWidth`/`outerHeight`/`innerWidth`/`innerHeight`/`devicePixelRatio` from the page itself via the existing `Runtime.evaluate` path, not native win32 window-hierarchy introspection.** Modern Chrome renders its own toolbar/tabs/bookmarks-bar UI inside one composited top-level window with no separate native child window for the content viewport, so `GetClientRect` alone can't tell you where the page content starts. The page's own `window.screenX`/`screenY` (outer window's screen position) combined with `outerWidth - innerWidth` (side chrome) and `outerHeight - innerHeight` (top chrome, assumed to hold effectively all of it -- window borders at the bottom are negligible in practice) gives the viewport's screen-space origin reliably, using information CDP already provides access to. This is the same category of technique real browser-automation tooling uses for OS-level dispatch, not a novel hack.
3. **`window.devicePixelRatio` is read and applied directly, upgrading v1's DPI/zoom handling beyond originally planned.** The proposal's initial framing assumed v1 would hard-code 100% scale and defer all calibration; reading `devicePixelRatio` (which already reflects *both* OS display scaling and any browser zoom the user has applied) costs nothing extra since step 2 already evaluates page globals, and makes coordinate translation correct for whatever the actual current DPI/zoom happen to be, not just the default case. What's still deferred is *validating* PROPOSAL.md's originally-stated full matrix (100/150/200% DPI x 80/100/125% zoom) -- the mechanism generalizes, but only today's actual machine configuration gets exercised by this change's own verification.
4. **Keyboard dispatch uses `KEYEVENTF_UNICODE`, not virtual-key-code mapping.** Mirrors the same reasoning `dispatch_char`'s CDP `"char"` event type used (`human-motion-synthetic` design.md Decision #2): arbitrary Unicode text doesn't reliably have virtual-key-code representations, and `SendInput`'s Unicode mode exists specifically so callers don't need one.
5. **The OS window is brought to the foreground (`SetForegroundWindow`) before every real-input action, headed sessions only.** `SendInput` delivers to whatever window currently has OS input focus -- unlike CDP dispatch, which reaches a specific tab regardless of window focus -- so without this, real input could land on an unrelated foreground window instead of the browser. Headless sessions have no real window to focus and are rejected with a clear error rather than silently falling back to CDP dispatch.
6. **Timing is unchanged; only final delivery changes.** `true_input: true` reuses the exact same `mouse_path`/`typing_timeline` synthesis (synthetic or trained) already built -- the *when* of each event is identical either way; only the *how* (CDP `Input.dispatch*Event` vs. real `SendInput`) changes. No new timing model, no new persona concept.

## Risks / Trade-offs

- [Real OS input affects the whole system, not just the browser tab -- a mis-focused window, a background app stealing focus mid-dispatch, or a coordinate-translation error could send clicks/keystrokes somewhere unintended] → mitigated by foreground-activation immediately before dispatch and headed-only scope, but not eliminated; verification is done with the user's explicit, immediate confirmation before any live run, same as `human-motion-trained`'s physical-human demo pattern -- never run unattended against a machine the user isn't actively watching.
- [Coordinate translation via `window.screenX`/`outerWidth` approximates where all "chrome UI height" sits (assumes it's entirely above the viewport, none below) -- true for Chrome's normal window layout but not something the OS guarantees] → acceptable; matches how equivalent real-world tooling handles this, and a visibly-wrong click during verification would surface the assumption's failure immediately and loudly, not silently.
- [Primary-display-only absolute coordinate normalization misfires if the browser window is on a secondary monitor] → explicitly out of scope; documented, not silently wrong -- verification runs with the browser on the primary display.

## Migration Plan

Purely additive -- a new `true_input` parameter, default `false` (existing CDP dispatch unchanged). Rejected clearly (typed error) for headless sessions or non-Windows platforms, never a silent fallback.

## Open Questions

None blocking.
