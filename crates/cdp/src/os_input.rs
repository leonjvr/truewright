//! Real Windows OS-level input dispatch via `SendInput` (true-user-input
//! spec) -- an alternative delivery mechanism for the exact same
//! `mouse_path`/`typing_timeline` timelines already used for CDP dispatch.
//! Windows-only: there is no OS input surface to target headless or on
//! other platforms; the `engine` crate rejects `true_input` before this
//! module is ever reached in that case.

#![cfg(windows)]

use crate::error::{CdpError, Result};
use std::sync::Once;
use std::thread;
use std::time::Duration;
use windows::core::BOOL;
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, RECT, TRUE};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MOVE, MOUSEINPUT, VIRTUAL_KEY,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetSystemMetrics, GetWindowRect,
    GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
    SM_CXSCREEN, SM_CYSCREEN,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};

static DPI_AWARENESS_INIT: Once = Once::new();

/// Declares this process per-monitor-DPI-aware, once. Without this, an
/// unmanifested Rust process defaults to DPI-unaware, and `GetSystemMetrics`/
/// `GetWindowRect` report *scaled* (logical) display metrics instead of
/// physical ones -- silently misplacing every `SendInput` coordinate by the
/// display's scale factor, since our own viewport-to-screen translation
/// already converts to physical pixels via `devicePixelRatio` to match a
/// DPI-aware browser like Chrome. Confirmed via live testing on a
/// 125%-scaled display: clicks landed well off-target until this was added.
fn ensure_dpi_aware() {
    DPI_AWARENESS_INIT.call_once(|| unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    });
}

/// A page's own reported window geometry (true-user-input spec:
/// "Viewport-to-screen coordinate translation"). Read from the page itself
/// via `window.screenX`/`screenY`/`outerWidth`/`outerHeight`/`innerWidth`/
/// `innerHeight`/`devicePixelRatio` -- Chrome composites its own toolbar/
/// tab UI into one native window, so the viewport's on-screen origin can't
/// be derived from `GetClientRect` alone (design.md Decision #2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowGeometry {
    pub screen_x: f64,
    pub screen_y: f64,
    pub outer_width: f64,
    pub outer_height: f64,
    pub inner_width: f64,
    pub inner_height: f64,
    pub device_pixel_ratio: f64,
}

/// Translates a CDP viewport coordinate (CSS px) to an OS screen coordinate
/// (physical px). Assumes side window chrome is split evenly left/right and
/// all top chrome (toolbar/tabs/bookmarks bar) sits above the viewport --
/// true for Chrome's normal window layout (design.md Decision #2).
pub fn viewport_to_screen(geom: WindowGeometry, viewport_x: f64, viewport_y: f64) -> (f64, f64) {
    let side_chrome = (geom.outer_width - geom.inner_width).max(0.0);
    let top_chrome = (geom.outer_height - geom.inner_height).max(0.0);
    let viewport_origin_x = geom.screen_x + side_chrome / 2.0;
    let viewport_origin_y = geom.screen_y + top_chrome;
    let screen_x = (viewport_origin_x + viewport_x) * geom.device_pixel_ratio;
    let screen_y = (viewport_origin_y + viewport_y) * geom.device_pixel_ratio;
    (screen_x, screen_y)
}

/// One point in a real-cursor mouse path, already in OS screen coordinates
/// (translated from a viewport coordinate by the caller via
/// `viewport_to_screen`).
#[derive(Debug, Clone, Copy)]
pub struct ScreenPoint {
    pub at_ms: f64,
    pub x: f64,
    pub y: f64,
}

/// One key event in a real-keyboard typing timeline.
#[derive(Debug, Clone, Copy)]
pub struct TimedChar {
    pub at_ms: f64,
    pub ch: char,
}

/// A hint for disambiguating between a browser process's several OS
/// windows, from CDP's own `Browser.getWindowForTarget` bounds -- every
/// headed session has at least two (the initial-launch window plus the
/// isolated context's actual window; see design.md addendum), so PID
/// filtering alone doesn't identify the one actually hosting the page this
/// engine is driving.
#[derive(Debug, Clone, Copy)]
pub struct WindowHint {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

fn rect_distance(hwnd: HWND, hint: WindowHint) -> i64 {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rect);
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    (rect.left as i64 - hint.left as i64).abs()
        + (rect.top as i64 - hint.top as i64).abs()
        + (width as i64 - hint.width as i64).abs()
        + (height as i64 - hint.height as i64).abs()
}

/// Locates the visible, titled top-level window owned by `pid` that best
/// matches `hint`'s on-screen bounds (true-user-input spec: "window
/// discovery by PID" -- CDP's `Browser` domain exposes no native HWND, so
/// this still enumerates by PID, but disambiguates multiple matches by
/// bounds rather than erroring, since a process legitimately owning more
/// than one top-level window turned out to be the common case, not the
/// exception -- see design.md addendum).
#[allow(clippy::result_large_err)]
fn find_browser_window(pid: u32, hint: WindowHint) -> Result<HWND> {
    struct SearchState {
        pid: u32,
        found: Vec<HWND>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut SearchState);
        let mut window_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if window_pid == state.pid
            && IsWindowVisible(hwnd).as_bool()
            && GetWindowTextLengthW(hwnd) > 0
        {
            state.found.push(hwnd);
        }
        TRUE
    }

    let mut state = SearchState {
        pid,
        found: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut state as *mut SearchState as isize),
        );
    }

    state
        .found
        .into_iter()
        .min_by_key(|&hwnd| rect_distance(hwnd, hint))
        .ok_or_else(|| CdpError::Other(format!("no visible browser window found for pid {pid}")))
}

// CdpError is kept as one flat enum per design.md Decision #5 (every layer
// maps errors from this taxonomy); boxing it would ripple through call sites
// for marginal benefit at this size, so the large-error lint is accepted here
// and at every other `Result<_, CdpError>`-returning fn in this module.
#[allow(clippy::result_large_err)]
fn send_inputs(inputs: &[INPUT]) -> Result<()> {
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        let err = unsafe { GetLastError() };
        return Err(CdpError::Other(format!(
            "SendInput dispatched {sent} of {} events (GetLastError: {:?})",
            inputs.len(),
            err
        )));
    }
    Ok(())
}

fn normalize(coord: f64, extent: i32) -> i32 {
    ((coord / extent.max(1) as f64) * 65535.0).round() as i32
}

fn mouse_input(
    dx: i32,
    dy: i32,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn move_input(screen_x: f64, screen_y: f64) -> INPUT {
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    mouse_input(
        normalize(screen_x, width),
        normalize(screen_y, height),
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
    )
}

fn char_input(unit: u16, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// A named key, dispatched by real virtual-key code rather than
/// `KEYEVENTF_UNICODE` -- unlike arbitrary text, named keys (Enter, Tab,
/// arrows) need a real `WM_KEYDOWN` with the matching virtual-key code for
/// page `keydown` listeners keyed on `event.key`/`event.code` to see them
/// correctly; Unicode-mode synthesis reports a synthetic `VK_PACKET` key
/// instead.
fn vk_input(vk: u16, key_up: bool) -> INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS;
    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Brings `pid`'s browser window to the OS foreground -- required before
/// dispatch since `SendInput` delivers to whatever window currently has
/// input focus, unlike CDP dispatch (design.md Decision #5). A bare
/// `SetForegroundWindow` call from a background process (like this one)
/// silently fails under Windows' foreground-lock restriction -- it only
/// grants foreground-switching rights to the process that generated the
/// most recent input, which a headless automation caller never has.
/// Temporarily attaching this thread's input queue to the current
/// foreground window's is the standard, unprivileged way around that
/// restriction (confirmed necessary via live testing: without it, dispatch
/// silently landed on whatever window the user's own session had focused).
#[allow(clippy::result_large_err)]
fn activate(pid: u32, hint: WindowHint) -> Result<HWND> {
    ensure_dpi_aware();
    let hwnd = find_browser_window(pid, hint)?;
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground != hwnd {
            let current_thread = GetCurrentThreadId();
            let mut foreground_pid = 0u32;
            let foreground_thread = GetWindowThreadProcessId(foreground, Some(&mut foreground_pid));
            let attached = foreground_thread != 0
                && foreground_thread != current_thread
                && AttachThreadInput(current_thread, foreground_thread, true).as_bool();

            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);

            if attached {
                let _ = AttachThreadInput(current_thread, foreground_thread, false);
            }
        }
    }
    Ok(hwnd)
}

/// Dispatches a real mouse click at a single screen point: activates the
/// window, moves the OS cursor there, then presses and releases the left
/// button. Runs synchronously (all Win32 calls are fast, local, and
/// non-blocking) -- callers on an async runtime should run this via
/// `spawn_blocking` so the HWND value never needs to cross an `.await`
/// (windows-rs's `HWND` is not `Send`).
#[allow(clippy::result_large_err)]
pub fn click_at(pid: u32, hint: WindowHint, screen_x: f64, screen_y: f64) -> Result<()> {
    activate(pid, hint)?;
    send_inputs(&[move_input(screen_x, screen_y)])?;
    send_inputs(&[mouse_input(0, 0, MOUSEEVENTF_LEFTDOWN)])?;
    send_inputs(&[mouse_input(0, 0, MOUSEEVENTF_LEFTUP)])?;
    Ok(())
}

/// Dispatches a synthesized real-cursor mouse path with real pacing
/// (mirrors `Session::walk_mouse_path`'s CDP timing loop), then clicks at
/// the final point. `path` must be non-empty.
#[allow(clippy::result_large_err)]
pub fn walk_and_click(pid: u32, hint: WindowHint, path: &[ScreenPoint]) -> Result<()> {
    activate(pid, hint)?;
    let mut last_ms = 0.0;
    for point in path {
        let delta = (point.at_ms - last_ms).max(0.0);
        if delta > 0.0 {
            thread::sleep(Duration::from_secs_f64(delta / 1000.0));
        }
        send_inputs(&[move_input(point.x, point.y)])?;
        last_ms = point.at_ms;
    }
    send_inputs(&[mouse_input(0, 0, MOUSEEVENTF_LEFTDOWN)])?;
    send_inputs(&[mouse_input(0, 0, MOUSEEVENTF_LEFTUP)])?;
    Ok(())
}

/// Dispatches a synthesized typing timeline as real keyboard events
/// (mirrors `Session::dispatch_typing`'s CDP timing loop). Characters
/// outside the Basic Multilingual Plane are sent as their two encoded
/// UTF-16 surrogate units back to back, undelayed.
#[allow(clippy::result_large_err)]
pub fn dispatch_typing(pid: u32, hint: WindowHint, timeline: &[TimedChar]) -> Result<()> {
    activate(pid, hint)?;
    let mut last_ms = 0.0;
    for key in timeline {
        let delta = (key.at_ms - last_ms).max(0.0);
        if delta > 0.0 {
            thread::sleep(Duration::from_secs_f64(delta / 1000.0));
        }
        let mut buf = [0u16; 2];
        for &unit in key.ch.encode_utf16(&mut buf).iter() {
            send_inputs(&[char_input(unit, false)])?;
            send_inputs(&[char_input(unit, true)])?;
        }
        last_ms = key.at_ms;
    }
    Ok(())
}

/// Dispatches a real key-down/key-up pair for a named key (e.g. Enter),
/// using its real Windows virtual-key code rather than Unicode-mode
/// synthesis (see `vk_input`).
#[allow(clippy::result_large_err)]
pub fn press_key(pid: u32, hint: WindowHint, virtual_key_code: u16) -> Result<()> {
    activate(pid, hint)?;
    send_inputs(&[vk_input(virtual_key_code, false)])?;
    send_inputs(&[vk_input(virtual_key_code, true)])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom(
        screen_x: f64,
        screen_y: f64,
        chrome_w: f64,
        chrome_h: f64,
        dpr: f64,
    ) -> WindowGeometry {
        WindowGeometry {
            screen_x,
            screen_y,
            outer_width: 1000.0 + chrome_w,
            outer_height: 800.0 + chrome_h,
            inner_width: 1000.0,
            inner_height: 800.0,
            device_pixel_ratio: dpr,
        }
    }

    #[test]
    fn no_chrome_no_scale_is_identity_plus_origin() {
        let g = geom(100.0, 50.0, 0.0, 0.0, 1.0);
        let (x, y) = viewport_to_screen(g, 10.0, 20.0);
        assert_eq!((x, y), (110.0, 70.0));
    }

    #[test]
    fn top_chrome_shifts_y_not_x() {
        let g = geom(0.0, 0.0, 0.0, 88.0, 1.0);
        let (x, y) = viewport_to_screen(g, 0.0, 0.0);
        assert_eq!((x, y), (0.0, 88.0));
    }

    #[test]
    fn side_chrome_splits_evenly() {
        let g = geom(0.0, 0.0, 20.0, 0.0, 1.0);
        let (x, _y) = viewport_to_screen(g, 0.0, 0.0);
        assert_eq!(x, 10.0);
    }

    #[test]
    fn device_pixel_ratio_scales_everything() {
        let g = geom(100.0, 50.0, 0.0, 88.0, 2.0);
        let (x, y) = viewport_to_screen(g, 10.0, 20.0);
        assert_eq!((x, y), (220.0, (50.0 + 88.0 + 20.0) * 2.0));
    }

    #[test]
    fn normalize_maps_extent_to_full_range() {
        assert_eq!(normalize(0.0, 1920), 0);
        assert_eq!(normalize(1920.0, 1920), 65535);
        assert_eq!(normalize(960.0, 1920), 32768);
    }
}
