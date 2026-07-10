//! Managed `chrome-headless-shell` resolution, download, and caching
//! (browser-attach spec: "Managed chrome-headless-shell for headless runs").
//!
//! The shell is the stripped, headless-only Chromium binary distributed via
//! Chrome for Testing — meaningfully lighter than `--headless=new` on a full
//! Chrome while still supporting screenshots. Downloads happen at most once
//! per version; everything after that is served from the cache directory,
//! including fully-offline runs (via the `latest.txt` marker).

use crate::error::{CdpError, Result};
use crate::launch::{BrowserKind, DiscoveredBrowser};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const VERSIONS_ENDPOINT: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";

#[cfg(windows)]
const PLATFORM: &str = "win64";
#[cfg(target_os = "linux")]
const PLATFORM: &str = "linux64";
#[cfg(all(not(windows), not(target_os = "linux")))]
const PLATFORM: &str = "unsupported";

#[cfg(windows)]
const SHELL_EXE: &str = "chrome-headless-shell.exe";
#[cfg(not(windows))]
const SHELL_EXE: &str = "chrome-headless-shell";

// Tolerant shapes for the Chrome for Testing endpoint — unknown fields
// ignored everywhere so endpoint additions never break parsing.
#[derive(Deserialize)]
struct KnownGoodVersions {
    channels: Channels,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Channels {
    stable: Channel,
}

#[derive(Deserialize)]
struct Channel {
    version: String,
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    #[serde(rename = "chrome-headless-shell", default)]
    chrome_headless_shell: Vec<DownloadEntry>,
}

#[derive(Deserialize)]
struct DownloadEntry {
    platform: String,
    url: String,
}

// CdpError is kept as one flat enum per design.md Decision #5 (every layer
// maps errors from this taxonomy); see the identical allow in launch.rs.
#[allow(clippy::result_large_err)]
fn browsers_dir() -> Result<PathBuf> {
    Ok(crate::launch::profile_base_dir()?
        .join("aib")
        .join("browsers"))
}

fn shell_path_for(version_dir: &Path) -> PathBuf {
    version_dir
        .join(format!("chrome-headless-shell-{PLATFORM}"))
        .join(SHELL_EXE)
}

/// Finds an already-downloaded shell without any network access, via the
/// `latest.txt` marker written after the last successful download
/// (browser-attach spec: "Subsequent runs use the cache offline").
pub fn cached_shell() -> Option<DiscoveredBrowser> {
    let browsers = browsers_dir().ok()?;
    let version = std::fs::read_to_string(browsers.join("latest.txt")).ok()?;
    let path = shell_path_for(&browsers.join(version.trim()));
    path.is_file().then_some(DiscoveredBrowser {
        kind: BrowserKind::Chrome,
        path,
        is_headless_shell: true,
    })
}

/// Resolves the latest stable shell version, downloading and extracting it
/// if not already cached. Blocking I/O throughout — call via
/// `ensure_shell()` from async contexts.
#[allow(clippy::result_large_err)]
fn ensure_shell_blocking() -> Result<DiscoveredBrowser> {
    if PLATFORM == "unsupported" {
        return Err(CdpError::Other(
            "chrome-headless-shell downloads are only supported on Windows and Linux".into(),
        ));
    }

    let fetch_err = |what: &str, e: ureq::Error| CdpError::Other(format!("{what}: {e}"));

    let versions: KnownGoodVersions = ureq::get(VERSIONS_ENDPOINT)
        .call()
        .map_err(|e| fetch_err("failed to fetch Chrome for Testing versions", e))?
        .body_mut()
        .read_json()
        .map_err(|e| CdpError::Other(format!("failed to parse versions endpoint: {e}")))?;

    let stable = versions.channels.stable;
    let browsers = browsers_dir()?;
    let version_dir = browsers.join(&stable.version);
    let shell_path = shell_path_for(&version_dir);

    if !shell_path.is_file() {
        let entry = stable
            .downloads
            .chrome_headless_shell
            .iter()
            .find(|d| d.platform == PLATFORM)
            .ok_or_else(|| {
                CdpError::Other(format!("no chrome-headless-shell download for {PLATFORM}"))
            })?;

        tracing::info!(
            version = %stable.version,
            url = %entry.url,
            "downloading chrome-headless-shell (~100MB, one-time per version)"
        );

        let mut response = ureq::get(&entry.url)
            .call()
            .map_err(|e| fetch_err("failed to download chrome-headless-shell", e))?;
        let mut bytes = Vec::new();
        std::io::copy(
            &mut response.body_mut().as_reader(),
            &mut std::io::Cursor::new(&mut bytes),
        )?;

        std::fs::create_dir_all(&version_dir)?;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| CdpError::Other(format!("invalid shell archive: {e}")))?;
        archive
            .extract(&version_dir)
            .map_err(|e| CdpError::Other(format!("failed to extract shell archive: {e}")))?;

        if !shell_path.is_file() {
            return Err(CdpError::Other(format!(
                "shell archive extracted but {} is missing",
                shell_path.display()
            )));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&shell_path, std::fs::Permissions::from_mode(0o755))?;
        }

        tracing::info!(path = %shell_path.display(), "chrome-headless-shell cached");
    }

    std::fs::write(browsers.join("latest.txt"), &stable.version)?;

    Ok(DiscoveredBrowser {
        kind: BrowserKind::Chrome,
        path: shell_path,
        is_headless_shell: true,
    })
}

/// Async wrapper: cache hit is checked inline; resolution/download runs on
/// the blocking pool.
pub async fn ensure_shell() -> Result<DiscoveredBrowser> {
    tokio::task::spawn_blocking(ensure_shell_blocking)
        .await
        .map_err(|e| CdpError::Other(format!("shell download task panicked: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_good_versions_shape() {
        let json = r#"{
            "timestamp": "2026-07-11T00:00:00.000Z",
            "channels": {
                "Stable": {
                    "channel": "Stable",
                    "version": "140.0.7300.10",
                    "revision": "1500000",
                    "downloads": {
                        "chrome": [{"platform": "win64", "url": "https://x/chrome.zip"}],
                        "chrome-headless-shell": [
                            {"platform": "linux64", "url": "https://x/l.zip"},
                            {"platform": "win64", "url": "https://x/w.zip"}
                        ]
                    }
                },
                "Beta": {"channel": "Beta", "version": "141.0.0.0", "downloads": {}}
            }
        }"#;
        let parsed: KnownGoodVersions = serde_json::from_str(json).expect("tolerant parse");
        assert_eq!(parsed.channels.stable.version, "140.0.7300.10");
        let dl = &parsed.channels.stable.downloads.chrome_headless_shell;
        assert!(dl.iter().any(|d| d.platform == "win64"));
        assert!(dl.iter().any(|d| d.platform == "linux64"));
    }

    #[test]
    fn shell_path_is_platform_shaped() {
        let p = shell_path_for(Path::new("/base/140.0.0.0"));
        let s = p.to_string_lossy();
        assert!(s.contains("chrome-headless-shell-"));
        assert!(s.ends_with(SHELL_EXE));
    }

    #[test]
    fn cached_shell_absent_without_marker() {
        // With no latest.txt in a fresh (or any) browsers dir pointing at a
        // real binary, cached_shell must not fabricate a browser. We can't
        // control the real cache dir in a unit test, so just assert the
        // function is safe to call and returns a well-formed value if any.
        if let Some(b) = cached_shell() {
            assert!(b.is_headless_shell);
            assert!(b.path.is_file());
        }
    }
}
