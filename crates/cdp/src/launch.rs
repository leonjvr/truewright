//! Browser discovery, launch, and teardown — see specs/browser-attach.
//!
//! Discovery never downloads a browser; it only locates an already-installed
//! Chrome/Edge. Launch always uses a dedicated profile directory, never the
//! user's live default profile.

use crate::error::{CdpError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::{Child, Command as ProcessCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Chrome,
    Edge,
}

impl BrowserKind {
    pub fn exe_name(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "chrome.exe",
            BrowserKind::Edge => "msedge.exe",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Chrome",
            BrowserKind::Edge => "Edge",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredBrowser {
    pub kind: BrowserKind,
    pub path: PathBuf,
    /// True when this is a managed `chrome-headless-shell` binary rather
    /// than an installed full browser. The shell is headless-only, so
    /// launch skips the `--headless=new` flag for it.
    pub is_headless_shell: bool,
}

/// Which browser binary headless sessions should use (browser-attach spec:
/// "Managed chrome-headless-shell for headless runs").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserPreference {
    /// Prefer a cached/downloadable `chrome-headless-shell` for headless
    /// runs; fall back to the installed browser. Headed runs always use
    /// the installed browser.
    #[default]
    Auto,
    /// Always use the installed browser; never resolve or download a shell.
    Installed,
}

/// Discover installed Chrome/Edge (browser-attach spec: "Discover installed
/// Chromium browsers"). Registry `App Paths` first, well-known paths as
/// fallback.
// CdpError is kept as one flat enum per design.md Decision #5 (every layer
// maps errors from this taxonomy); boxing it would ripple through call sites
// for marginal benefit at this size, so the large-error lint is accepted here.
#[allow(clippy::result_large_err)]
pub fn discover_browsers() -> Result<Vec<DiscoveredBrowser>> {
    if let Some(path) = std::env::var_os("TRUEWRIGHT_CHROME_PATH") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(vec![DiscoveredBrowser {
                kind: BrowserKind::Chrome,
                path,
                is_headless_shell: false,
            }])
        } else {
            Err(CdpError::NoBrowserFound {
                checked: format!("TRUEWRIGHT_CHROME_PATH={}", path.display()),
            })
        };
    }

    let mut found = Vec::new();
    let mut checked = Vec::new();

    for kind in [BrowserKind::Chrome, BrowserKind::Edge] {
        if let Some(path) = find_browser(kind, &mut checked) {
            found.push(DiscoveredBrowser {
                kind,
                path,
                is_headless_shell: false,
            });
        }
    }

    if found.is_empty() {
        return Err(CdpError::NoBrowserFound {
            checked: checked.join(", "),
        });
    }
    Ok(found)
}

/// Picks the binary for a headless session (browser-attach spec: "Managed
/// chrome-headless-shell for headless runs"): `TRUEWRIGHT_CHROME_PATH` override (if
/// set) → cached shell → downloaded shell → installed browser, unless the
/// caller opted out with [`BrowserPreference::Installed`]. Failures on the
/// shell path degrade to the installed browser with a warning, never a hard
/// error. The override takes priority over the managed shell too -- forcing
/// a specific binary (e.g. Chrome Beta in CI) would otherwise be silently
/// ignored by every headless launch, which is the common case for the test
/// suite this override exists to redirect.
#[allow(clippy::result_large_err)]
pub async fn resolve_headless_browser(pref: BrowserPreference) -> Result<DiscoveredBrowser> {
    if std::env::var_os("TRUEWRIGHT_CHROME_PATH").is_some() {
        return discover_browsers().map(|mut found| found.remove(0));
    }
    if pref == BrowserPreference::Auto {
        if let Some(shell) = crate::download::cached_shell() {
            return Ok(shell);
        }
        match crate::download::ensure_shell().await {
            Ok(shell) => return Ok(shell),
            Err(e) => {
                tracing::warn!(error = %e, "chrome-headless-shell unavailable; falling back to installed browser");
            }
        }
    }
    let mut found = discover_browsers()?;
    Ok(found.remove(0))
}

fn find_browser(kind: BrowserKind, checked: &mut Vec<String>) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(path) = registry_lookup(kind, checked) {
            return Some(path);
        }
    }

    for candidate in well_known_paths(kind) {
        checked.push(candidate.display().to_string());
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(windows)]
fn registry_lookup(kind: BrowserKind, checked: &mut Vec<String>) -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let subkey = format!(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{}",
        kind.exe_name()
    );

    for (hive_name, hive) in [("HKLM", HKEY_LOCAL_MACHINE), ("HKCU", HKEY_CURRENT_USER)] {
        checked.push(format!("registry:{}\\{}", hive_name, subkey));
        let root = RegKey::predef(hive);
        if let Ok(key) = root.open_subkey(&subkey) {
            if let Ok(path) = key.get_value::<String, _>("") {
                let path = PathBuf::from(path);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn well_known_paths(kind: BrowserKind) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
        let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
        let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);

        match kind {
            BrowserKind::Chrome => {
                for base in [program_files, program_files_x86, local_app_data]
                    .into_iter()
                    .flatten()
                {
                    paths.push(base.join(r"Google\Chrome\Application\chrome.exe"));
                }
            }
            BrowserKind::Edge => {
                for base in [program_files_x86, program_files, local_app_data]
                    .into_iter()
                    .flatten()
                {
                    paths.push(base.join(r"Microsoft\Edge\Application\msedge.exe"));
                }
            }
        }
    }

    // No registry-equivalent lookup exists on Linux, so well-known paths are
    // the only discovery mechanism there (browser-attach spec: "Discover
    // installed Chromium browsers" — Chromium/Linux scenario). Edge isn't
    // available on a stock Linux install, so no paths are added for it.
    #[cfg(target_os = "linux")]
    {
        if kind == BrowserKind::Chrome {
            for candidate in [
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
            ] {
                paths.push(PathBuf::from(candidate));
            }
        }
    }

    paths
}

/// Per-user data directory root: `%LOCALAPPDATA%` on Windows,
/// `$XDG_DATA_HOME` (falling back to `~/.local/share`) on Linux
/// (browser-attach spec: "Launch with an isolated profile").
#[allow(clippy::result_large_err)]
pub fn profile_base_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| CdpError::LaunchFailed("LOCALAPPDATA is not set".into()))
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg));
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            CdpError::LaunchFailed("neither XDG_DATA_HOME nor HOME is set".into())
        })?;
        Ok(home.join(".local").join("share"))
    }
}

/// Chromium's sandbox needs privileges a bare container init typically
/// lacks; `--no-sandbox` is the standard workaround, applied automatically
/// on Linux when running as root, inside a container, or under CI (never on
/// Windows). Callers can also force it explicitly (CLI `--no-sandbox`,
/// `--chrome-arg=--no-sandbox`, or `TRUEWRIGHT_CHROME_ARGS`).
#[cfg(target_os = "linux")]
fn running_as_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("Uid:").map(|rest| {
                    rest.split_whitespace()
                        .next()
                        .and_then(|uid| uid.parse::<u32>().ok())
                        == Some(0)
                })
            })
        })
        .unwrap_or(false)
}

/// Best-effort detection of a container or CI environment, where Chromium's
/// setuid/namespace sandbox usually can't initialize (the unprivileged
/// LXC/CI case `--no-sandbox` exists for). `container` is set by
/// systemd-nspawn/LXC/podman, `CI` by essentially every CI system, and the
/// two marker files by Docker/Podman respectively. None of these is
/// authoritative, so this only *adds* `--no-sandbox` (a safe no-op when the
/// sandbox would have worked); it never removes it.
#[cfg(target_os = "linux")]
fn in_container_or_ci() -> bool {
    std::env::var_os("container").is_some()
        || std::env::var_os("CI").is_some()
        || Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
}

/// Whether launch should append `--no-sandbox` on its own (Linux only).
#[cfg(target_os = "linux")]
fn sandbox_should_be_disabled() -> bool {
    running_as_root() || in_container_or_ci()
}

/// Extra Chrome flags from the `TRUEWRIGHT_CHROME_ARGS` environment
/// variable, whitespace-separated (e.g.
/// `"--no-sandbox --window-size=1440,900 --kiosk"`). Read at the launch
/// layer so it applies to every entry point — `mcp`, `doctor`, `agent`, and
/// the test suite — without each having to thread it through. Chrome's own
/// flags never contain spaces in a single token, so whitespace splitting is
/// sufficient and matches how the same flags are passed on a real command
/// line.
fn chrome_args_from_env() -> Vec<String> {
    std::env::var("TRUEWRIGHT_CHROME_ARGS")
        .ok()
        .map(|raw| split_chrome_args(&raw))
        .unwrap_or_default()
}

/// Whitespace-splits a raw `TRUEWRIGHT_CHROME_ARGS` value into individual
/// flags, dropping empty tokens from runs of spaces. Pulled out from
/// [`chrome_args_from_env`] so the parsing is unit-testable without mutating
/// process-global environment state.
fn split_chrome_args(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(str::to_string).collect()
}

/// Chrome's automation-oriented default flags, mirroring Playwright's set for
/// headless runs. These cut background service processes and resident memory —
/// the biggest wins being `--disable-features=…` (kills background service
/// processes), `--disable-component-extensions-with-background-pages`,
/// `--disable-breakpad` (no crash-handler process), `--disable-sync`,
/// `--metrics-recording-only`, and `--no-service-autorun` — taking the Chrome
/// process tree from ~15 down to ~10. Applied before caller flags, so any one
/// can be overridden or removed via `--chrome-arg` / `[browser].extra_args` /
/// `TRUEWRIGHT_CHROME_ARGS` (Chrome's last-flag-wins rule).
const HEADLESS_AUTOMATION_FLAGS: &[&str] = &[
    "--allow-pre-commit-input",
    "--disable-back-forward-cache",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-component-update",
    "--disable-default-apps",
    "--disable-field-trial-config",
    "--disable-hang-monitor",
    "--disable-infobars",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-search-engine-choice-screen",
    "--disable-sync",
    "--force-color-profile=srgb",
    "--hide-scrollbars",
    "--metrics-recording-only",
    "--no-service-autorun",
    "--no-startup-window",
    "--password-store=basic",
    "--use-mock-keychain",
    "--disable-features=AvoidUnnecessaryBeforeUnloadCheckSync,DestroyProfileOnBrowserClose,\
     DialMediaRouteProvider,GlobalMediaControls,HttpsUpgrades,MediaRouter,PaintHolding,\
     Translate,OptimizationHints,ThirdPartyStoragePartitioning",
];

pub struct LaunchedBrowser {
    pub kind: BrowserKind,
    endpoint: Endpoint,
    pub user_data_dir: PathBuf,
    child: Option<Child>,
}

/// How to reach the launched browser's CDP endpoint. Unix launches default to
/// a `--remote-debugging-pipe` transport (no TCP port to allocate or scan, and
/// no `DevToolsActivePort` file to poll — removing a class of attach-timeout
/// flakiness in sandboxed/container environments); Windows launches, and any
/// attach-to-external, use the TCP DevTools WebSocket.
enum Endpoint {
    WebSocket(String),
    /// Parent-side CDP pipe transport, taken exactly once when the connection
    /// is built. Behind a `Mutex<Option<…>>` because [`LaunchedBrowser`] is
    /// held by shared reference at connect time (see
    /// `Browser::connect_launched`) yet the transport must be moved out.
    #[cfg(unix)]
    Pipe(std::sync::Mutex<Option<crate::transport::PipeTransport>>),
}

/// Launch with an isolated profile (browser-attach spec: "Launch with an
/// isolated profile"). `profile_name` selects the directory under an
/// OS-appropriate per-user data directory (see `profile_base_dir`) — never
/// the live default profile.
pub async fn launch(
    browser: &DiscoveredBrowser,
    profile_name: &str,
    headless: bool,
) -> Result<LaunchedBrowser> {
    launch_with_flags(browser, profile_name, headless, &[]).await
}

/// Same as [`launch`], with additional raw command-line flags appended.
/// Exists for tests that need a specific Chrome behavior (e.g.
/// `--site-per-process` to force genuine OOPIF creation for a cross-origin
/// fixture, cross-origin-oopif spec) without adding a flag to the product's
/// own CLI/MCP surface for a test-only concern.
pub async fn launch_with_flags(
    browser: &DiscoveredBrowser,
    profile_name: &str,
    headless: bool,
    extra_args: &[&str],
) -> Result<LaunchedBrowser> {
    let user_data_dir = profile_base_dir()?
        .join("truewright")
        .join("profiles")
        .join(profile_name);
    std::fs::create_dir_all(&user_data_dir)?;

    // Unix defaults to a `--remote-debugging-pipe` transport; Windows (no pipe
    // implementation) uses the TCP DevTools port. The transport flag itself is
    // added below, once, so the two paths share the rest of the command line.
    let use_pipe = use_cdp_pipe();

    let mut cmd = ProcessCommand::new(&browser.path);
    cmd.arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-background-networking")
        // Without these, Chrome throttles compositor-frame-dependent work
        // (including the ack for Input.dispatch*Event) to ~once per 5s for
        // any window that isn't OS-focused/visible — which every launch
        // here is, since nothing calls Target.activateTarget. Confirmed via
        // per-call timing instrumentation during human-motion's headed demo:
        // every dispatchMouseEvent took ~5000ms until these were added.
        // Puppeteer/Playwright set the same three flags for this reason.
        .arg("--disable-backgrounding-occluded-windows")
        .arg("--disable-renderer-backgrounding")
        .arg("--disable-background-timer-throttling")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());
    if use_pipe {
        cmd.arg("--remote-debugging-pipe");
    } else {
        // `--remote-allow-origins=*` only matters for the HTTP/WS debugging
        // endpoint (the pipe has no origin check).
        cmd.arg("--remote-debugging-port=0")
            .arg("--remote-allow-origins=*");
    }
    if headless {
        // Reduced-footprint flag set (browser-attach spec: "Reduced-footprint
        // headless launch flags"). Never applied headed — --disable-gpu in
        // particular would degrade a visible window.
        cmd.arg("--disable-dev-shm-usage")
            .arg("--disable-software-rasterizer")
            .arg("--disable-extensions")
            .arg("--mute-audio")
            .arg("--disable-gpu");
        // Playwright's automation defaults (see HEADLESS_AUTOMATION_FLAGS) —
        // the bulk of the process/RSS reduction.
        for flag in HEADLESS_AUTOMATION_FLAGS {
            cmd.arg(flag);
        }
        // chrome-headless-shell is headless-only; the mode flag is for full
        // Chrome binaries. Old `--headless` (not `--headless=new`) is the
        // lightweight automation mode Playwright uses — `--headless=new` runs
        // the full browser and spawns extra processes. `--headless=new` stays
        // opt-in: a caller can pass `--chrome-arg=--headless=new`, which wins
        // under Chrome's last-flag-wins rule since caller flags come after.
        if !browser.is_headless_shell {
            cmd.arg("--headless");
        }
    }

    // Caller-supplied flags (config `[browser].extra_args` + repeated
    // `--chrome-arg`) first, then `TRUEWRIGHT_CHROME_ARGS` from the
    // environment. All are appended after the base/headless flags so a user
    // override wins under Chrome's last-flag-wins rule.
    let mut requested: Vec<String> = extra_args.iter().map(|s| s.to_string()).collect();
    requested.extend(chrome_args_from_env());

    // Auto `--no-sandbox` in a root/container/CI context, unless the caller
    // already asked for it (Chrome tolerates the duplicate, but this keeps
    // the command line clean and the intent unambiguous).
    #[cfg(target_os = "linux")]
    if sandbox_should_be_disabled() && !requested.iter().any(|a| a == "--no-sandbox") {
        cmd.arg("--no-sandbox");
    }

    for arg in &requested {
        cmd.arg(arg);
    }

    // Transport wiring. On Unix a `--remote-debugging-pipe` launch needs the
    // two CDP pipes created and their child ends placed on fds 3/4 (Chrome
    // reads commands from 3, writes to 4); the `setsid` for clean teardown
    // rides in the same `pre_exec`. On Windows (and a forced-WebSocket Unix
    // run) there is no pipe, and `setsid` is a no-op that doesn't apply.
    #[cfg(unix)]
    let pipe_parent_fds = setup_transport(&mut cmd, use_pipe)?;
    #[cfg(not(unix))]
    let _ = use_pipe; // always false off-Unix

    if !use_pipe {
        // Clear any stale port file only on the path that reads one back.
        let _ = std::fs::remove_file(user_data_dir.join("DevToolsActivePort"));
    }

    let child = cmd.spawn().map_err(|e| {
        #[cfg(unix)]
        if let Some(fds) = pipe_parent_fds {
            fds.close_all(); // don't leak the pipe fds if the browser never started
        }
        CdpError::LaunchFailed(format!("{}: {}", browser.path.display(), e))
    })?;

    let endpoint = build_endpoint(
        use_pipe,
        &user_data_dir,
        #[cfg(unix)]
        pipe_parent_fds,
    )
    .await?;

    Ok(LaunchedBrowser {
        kind: browser.kind,
        endpoint,
        user_data_dir,
        child: Some(child),
    })
}

/// Whether to use the `--remote-debugging-pipe` transport. Unix defaults to
/// the pipe; `TRUEWRIGHT_CDP_TRANSPORT=websocket` forces the TCP path back on
/// (an escape hatch if a pipe ever misbehaves). Off Unix there is no pipe
/// implementation, so this is always `false`.
fn use_cdp_pipe() -> bool {
    if !cfg!(unix) {
        return false;
    }
    !std::env::var("TRUEWRIGHT_CDP_TRANSPORT")
        .map(|v| v.eq_ignore_ascii_case("websocket") || v.eq_ignore_ascii_case("ws"))
        .unwrap_or(false)
}

/// Parent-side ends of the two CDP pipes, held between `spawn` and building
/// the transport. `cmd_write` feeds Chrome's fd 3 (commands out); `evt_read`
/// drains Chrome's fd 4 (responses/events in). The two child ends
/// (`cmd_read`, `evt_write`) are only needed by the forked child's `pre_exec`
/// and are closed in the parent immediately after spawn.
#[cfg(unix)]
#[derive(Clone, Copy)]
struct PipeParentFds {
    cmd_write: libc::c_int,
    evt_read: libc::c_int,
    cmd_read: libc::c_int,
    evt_write: libc::c_int,
}

#[cfg(unix)]
impl PipeParentFds {
    fn close_all(&self) {
        unsafe {
            libc::close(self.cmd_write);
            libc::close(self.evt_read);
            libc::close(self.cmd_read);
            libc::close(self.evt_write);
        }
    }
}

/// Registers the `pre_exec` for a launch, and (for the pipe transport) creates
/// the two CDP pipes. Always installs `setsid` so teardown can kill the whole
/// process group; when `use_pipe`, the same `pre_exec` also `dup2`s the child
/// pipe ends onto fds 3/4 and clears their close-on-exec so Chrome inherits
/// them across `exec`.
#[cfg(unix)]
#[allow(clippy::result_large_err)]
fn setup_transport(cmd: &mut ProcessCommand, use_pipe: bool) -> Result<Option<PipeParentFds>> {
    if !use_pipe {
        // WebSocket/TCP path: only the process-group isolation is needed.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        return Ok(None);
    }

    // cmd pipe: parent writes -> child reads (child end goes to fd 3).
    let (cmd_read, cmd_write) = make_cloexec_pipe()?;
    // evt pipe: child writes (fd 4) -> parent reads.
    let (evt_read, evt_write) = make_cloexec_pipe()?;

    // Captured by value (RawFd is Copy) into the child-side closure; the parent
    // keeps its own copies in the returned struct.
    unsafe {
        cmd.pre_exec(move || {
            // Place the child's pipe ends on the fds Chrome expects, then clear
            // FD_CLOEXEC so they survive exec. `dup2` onto a distinct fd
            // already clears CLOEXEC, but the explicit `fcntl` also covers the
            // case where a source fd is already 3 or 4 (dup2 is then a no-op).
            if libc::dup2(cmd_read, 3) < 0 || libc::dup2(evt_write, 4) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            libc::fcntl(3, libc::F_SETFD, 0);
            libc::fcntl(4, libc::F_SETFD, 0);
            libc::setsid();
            Ok(())
        });
    }

    Ok(Some(PipeParentFds {
        cmd_write,
        evt_read,
        cmd_read,
        evt_write,
    }))
}

/// Builds the connection endpoint after spawn: for the pipe transport, closes
/// the child ends in the parent (so a browser exit yields EOF on `evt_read`)
/// and wraps the parent ends in a [`crate::transport::PipeTransport`]; for the
/// WebSocket transport, polls the `DevToolsActivePort` file for the WS URL.
#[allow(clippy::result_large_err)]
async fn build_endpoint(
    use_pipe: bool,
    user_data_dir: &Path,
    #[cfg(unix)] pipe_parent_fds: Option<PipeParentFds>,
) -> Result<Endpoint> {
    if !use_pipe {
        let port_file = user_data_dir.join("DevToolsActivePort");
        let ws_url = wait_for_devtools_endpoint(&port_file, Duration::from_secs(10)).await?;
        return Ok(Endpoint::WebSocket(ws_url));
    }

    #[cfg(unix)]
    {
        let fds = pipe_parent_fds.expect("pipe launch always yields parent fds");
        // The parent has no use for the child ends; closing `evt_write` here is
        // what lets a browser exit surface as EOF on our reader.
        unsafe {
            libc::close(fds.cmd_read);
            libc::close(fds.evt_write);
        }
        let transport =
            crate::transport::PipeTransport::from_parent_fds(fds.cmd_write, fds.evt_read)?;
        Ok(Endpoint::Pipe(std::sync::Mutex::new(Some(transport))))
    }
    #[cfg(not(unix))]
    {
        // Off Unix `use_pipe` is always false, so this arm is unreachable.
        Err(CdpError::LaunchFailed(
            "pipe transport is not supported on this platform".into(),
        ))
    }
}

/// Creates a pipe with both ends close-on-exec, returning `(read, write)`.
/// Linux uses `pipe2(O_CLOEXEC)` (atomic, no fd-leak window); other Unix
/// falls back to `pipe` + `fcntl`.
#[cfg(unix)]
#[allow(clippy::result_large_err)]
fn make_cloexec_pipe() -> Result<(libc::c_int, libc::c_int)> {
    let mut fds = [0 as libc::c_int; 2];

    #[cfg(target_os = "linux")]
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    #[cfg(not(target_os = "linux"))]
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if rc != 0 {
        return Err(CdpError::Io(std::io::Error::last_os_error()));
    }

    #[cfg(not(target_os = "linux"))]
    for &fd in &fds {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) < 0 {
                let e = std::io::Error::last_os_error();
                libc::close(fds[0]);
                libc::close(fds[1]);
                return Err(CdpError::Io(e));
            }
        }
    }

    Ok((fds[0], fds[1]))
}

async fn wait_for_devtools_endpoint(port_file: &Path, timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(contents) = tokio::fs::read_to_string(port_file).await {
            let mut lines = contents.lines();
            if let (Some(port), Some(path)) = (lines.next(), lines.next()) {
                let (port, path) = (port.trim(), path.trim());
                if !port.is_empty() && !path.is_empty() {
                    return Ok(format!("ws://127.0.0.1:{port}{path}"));
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(CdpError::AttachTimeout);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

impl LaunchedBrowser {
    /// Wrap a browser this process did not launch (browser-attach spec:
    /// "Attach to externally started browser"). `shutdown` then leaves the
    /// process running. An external attach is always over the TCP DevTools
    /// WebSocket — the pipe transport only exists for browsers we spawn.
    pub fn attach_existing(kind: BrowserKind, ws_url: String, user_data_dir: PathBuf) -> Self {
        Self {
            kind,
            endpoint: Endpoint::WebSocket(ws_url),
            user_data_dir,
            child: None,
        }
    }

    /// The DevTools WebSocket URL, when this launch used the TCP transport
    /// (Windows, or an attach-to-external). `None` for a Unix CDP pipe, whose
    /// connection is built from the inherited pipe fds instead — see
    /// [`Browser::connect_launched`](crate::ops::Browser::connect_launched).
    pub fn ws_url(&self) -> Option<&str> {
        match &self.endpoint {
            Endpoint::WebSocket(url) => Some(url),
            #[cfg(unix)]
            Endpoint::Pipe(_) => None,
        }
    }

    /// Takes the CDP pipe transport out for building the connection. Returns
    /// `None` on a WebSocket endpoint, or if already taken (connect happens
    /// once). `pub(crate)` — only `Browser::connect_launched` calls it.
    #[cfg(unix)]
    pub(crate) fn take_pipe_transport(&self) -> Option<crate::transport::PipeTransport> {
        match &self.endpoint {
            Endpoint::Pipe(slot) => slot.lock().ok().and_then(|mut guard| guard.take()),
            Endpoint::WebSocket(_) => None,
        }
    }

    pub fn we_launched(&self) -> bool {
        self.child.is_some()
    }

    /// OS process id of the launched browser root, when we launched it.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(|c| c.id())
    }

    /// Terminates the browser process if we launched it; a no-op otherwise
    /// (browser-attach spec: "Clean teardown"). On Unix this kills the
    /// whole process group (see `launch`'s `setsid` call), so
    /// zygote-forked renderer/GPU/utility children die too instead of
    /// surviving as orphans.
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                kill_process_group(pid);
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        Ok(())
    }
}

impl Drop for LaunchedBrowser {
    /// Best-effort safety net: if `shutdown` was never called (e.g. the
    /// caller panicked before reaching it), send SIGKILL/TerminateProcess
    /// so a launched browser doesn't outlive this value and hold its
    /// profile directory locked indefinitely. `shutdown` is still the
    /// primary path — it also `.wait()`s to reap the process.
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                kill_process_group(pid);
            }
            let _ = child.start_kill();
        }
    }
}

/// Sends `SIGKILL` to the whole process group led by `pid` (i.e. `kill(-pid,
/// SIGKILL)`), relying on `launch`'s `setsid()` call having made `pid` both
/// the process id and the process group id.
#[cfg(unix)]
fn kill_process_group(pid: u32) {
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TRUEWRIGHT_CHROME_PATH is process-global env state, and Rust's default test
    // harness runs #[test]/#[tokio::test] functions concurrently on separate
    // threads within the same process -- any second test mutating this same
    // var races this one. All TRUEWRIGHT_CHROME_PATH assertions live in this single
    // test function so there is nothing to race with, rather than relying on
    // test-runner isolation that doesn't actually exist here.
    #[tokio::test]
    async fn truewright_chrome_path_overrides_discovery_and_the_managed_headless_shell() {
        let real_chrome = discover_browsers().ok().map(|found| found[0].path.clone());
        let Some(real_chrome) = real_chrome else {
            eprintln!(
                "skipping truewright_chrome_path_overrides_discovery_and_the_managed_headless_shell: no installed browser found"
            );
            return;
        };

        std::env::set_var("TRUEWRIGHT_CHROME_PATH", &real_chrome);
        let result = discover_browsers();
        let found = result.expect("override should discover successfully");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, real_chrome);
        assert_eq!(found[0].kind, BrowserKind::Chrome);

        let result = resolve_headless_browser(BrowserPreference::Auto).await;
        let resolved = result.expect("override should resolve successfully");
        assert_eq!(
            resolved.path, real_chrome,
            "TRUEWRIGHT_CHROME_PATH must win over the managed chrome-headless-shell path"
        );
        std::env::remove_var("TRUEWRIGHT_CHROME_PATH");

        std::env::set_var(
            "TRUEWRIGHT_CHROME_PATH",
            r"C:\definitely\not\a\real\browser.exe",
        );
        let result = discover_browsers();
        std::env::remove_var("TRUEWRIGHT_CHROME_PATH");
        assert!(
            result.is_err(),
            "a nonexistent TRUEWRIGHT_CHROME_PATH must error, not silently fall back"
        );
    }

    #[test]
    fn split_chrome_args_tokenizes_on_whitespace() {
        assert_eq!(
            split_chrome_args("--no-sandbox --window-size=1440,900 --kiosk"),
            vec![
                "--no-sandbox".to_string(),
                "--window-size=1440,900".to_string(),
                "--kiosk".to_string(),
            ]
        );
        // Runs of spaces / leading-trailing whitespace collapse away, and an
        // empty value yields no flags at all (not one empty-string flag).
        assert_eq!(
            split_chrome_args("  --a    --b  "),
            vec!["--a".to_string(), "--b".to_string()]
        );
        assert!(split_chrome_args("").is_empty());
        assert!(split_chrome_args("   ").is_empty());
    }

    #[test]
    fn well_known_paths_are_kind_specific() {
        let chrome = well_known_paths(BrowserKind::Chrome);
        let edge = well_known_paths(BrowserKind::Edge);
        // "chrom" (not "Chrome") so this holds on both Windows
        // (...\Google\Chrome\...) and Linux (/usr/bin/chromium).
        assert!(chrome
            .iter()
            .all(|p| p.to_string_lossy().to_lowercase().contains("chrom")));
        assert!(edge.iter().all(|p| p.to_string_lossy().contains("Edge")));
    }

    #[test]
    fn profile_base_dir_resolves_to_an_absolute_path() {
        let base = profile_base_dir().expect("resolves on this platform");
        assert!(
            base.is_absolute(),
            "expected an absolute path, got {base:?}"
        );
    }

    #[test]
    fn attach_existing_does_not_own_a_child() {
        let lb = LaunchedBrowser::attach_existing(
            BrowserKind::Chrome,
            "ws://127.0.0.1:1/devtools/browser/x".into(),
            PathBuf::from(r"C:\nowhere"),
        );
        assert!(!lb.we_launched());
    }

    #[tokio::test]
    async fn devtools_endpoint_reads_existing_port_file() {
        let dir =
            std::env::temp_dir().join(format!("truewright-test-{}-{}", std::process::id(), 1));
        std::fs::create_dir_all(&dir).unwrap();
        let port_file = dir.join("DevToolsActivePort");
        std::fs::write(&port_file, "12345\n/devtools/browser/abc-123\n").unwrap();

        let url = wait_for_devtools_endpoint(&port_file, Duration::from_secs(1))
            .await
            .expect("port file already present");
        assert_eq!(url, "ws://127.0.0.1:12345/devtools/browser/abc-123");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn devtools_endpoint_times_out_when_file_never_appears() {
        let dir =
            std::env::temp_dir().join(format!("truewright-test-{}-{}", std::process::id(), 2));
        let missing = dir.join("DevToolsActivePort");

        let result = wait_for_devtools_endpoint(&missing, Duration::from_millis(150)).await;
        assert!(matches!(result, Err(CdpError::AttachTimeout)));
    }
}
