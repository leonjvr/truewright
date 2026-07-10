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
}

/// Discover installed Chrome/Edge (browser-attach spec: "Discover installed
/// Chromium browsers"). Registry `App Paths` first, well-known paths as
/// fallback.
// CdpError is kept as one flat enum per design.md Decision #5 (every layer
// maps errors from this taxonomy); boxing it would ripple through call sites
// for marginal benefit at this size, so the large-error lint is accepted here.
#[allow(clippy::result_large_err)]
pub fn discover_browsers() -> Result<Vec<DiscoveredBrowser>> {
    let mut found = Vec::new();
    let mut checked = Vec::new();

    for kind in [BrowserKind::Chrome, BrowserKind::Edge] {
        if let Some(path) = find_browser(kind, &mut checked) {
            found.push(DiscoveredBrowser { kind, path });
        }
    }

    if found.is_empty() {
        return Err(CdpError::NoBrowserFound {
            checked: checked.join(", "),
        });
    }
    Ok(found)
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
fn profile_base_dir() -> Result<PathBuf> {
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
/// lacks; `--no-sandbox` is the standard workaround, but only applied when
/// actually running as root on Linux (never on Windows, never as a normal
/// Linux user).
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

pub struct LaunchedBrowser {
    pub kind: BrowserKind,
    pub ws_url: String,
    pub user_data_dir: PathBuf,
    child: Option<Child>,
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
    let user_data_dir = profile_base_dir()?
        .join("aib")
        .join("profiles")
        .join(profile_name);
    std::fs::create_dir_all(&user_data_dir)?;

    let devtools_port_file = user_data_dir.join("DevToolsActivePort");
    let _ = std::fs::remove_file(&devtools_port_file); // stale from a prior run

    let mut cmd = ProcessCommand::new(&browser.path);
    cmd.arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--remote-allow-origins=*")
        .arg("--disable-background-networking")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());
    #[cfg(target_os = "linux")]
    if running_as_root() {
        cmd.arg("--no-sandbox");
    }
    if headless {
        cmd.arg("--headless=new");
    }

    let child = cmd
        .spawn()
        .map_err(|e| CdpError::LaunchFailed(format!("{}: {}", browser.path.display(), e)))?;

    let ws_url = wait_for_devtools_endpoint(&devtools_port_file, Duration::from_secs(10)).await?;

    Ok(LaunchedBrowser {
        kind: browser.kind,
        ws_url,
        user_data_dir,
        child: Some(child),
    })
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
    /// process running.
    pub fn attach_existing(kind: BrowserKind, ws_url: String, user_data_dir: PathBuf) -> Self {
        Self {
            kind,
            ws_url,
            user_data_dir,
            child: None,
        }
    }

    pub fn we_launched(&self) -> bool {
        self.child.is_some()
    }

    /// Terminates the browser process if we launched it; a no-op otherwise
    /// (browser-attach spec: "Clean teardown").
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
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
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let dir = std::env::temp_dir().join(format!("aib-test-{}-{}", std::process::id(), 1));
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
        let dir = std::env::temp_dir().join(format!("aib-test-{}-{}", std::process::id(), 2));
        let missing = dir.join("DevToolsActivePort");

        let result = wait_for_devtools_endpoint(&missing, Duration::from_millis(150)).await;
        assert!(matches!(result, Err(CdpError::AttachTimeout)));
    }
}
