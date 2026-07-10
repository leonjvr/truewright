# browser-attach

## Purpose

Locate an installed Chromium browser (Chrome/Edge) without downloading anything, launch it against an isolated profile, and tear it down cleanly — the attach layer underneath everything else in ai-browser.

## Requirements

### Requirement: Discover installed Chromium browsers
The system SHALL discover installed Chrome and Edge on Windows by reading the registry `App Paths` keys (`chrome.exe`, `msedge.exe`), and SHALL discover installed Chrome/Chromium on Linux by checking well-known binary paths (no registry equivalent exists on Linux). On both platforms the system SHALL fall back to well-known installation paths when a platform-specific lookup (registry, on Windows) is absent or unavailable. Discovery MUST NOT download or install any browser.

#### Scenario: Both browsers installed (Windows)
- **WHEN** discovery runs on a Windows machine with Chrome and Edge installed
- **THEN** both executables are returned with their resolved absolute paths and browser kind (chrome | edge)

#### Scenario: Chromium installed (Linux)
- **WHEN** discovery runs on a Linux machine with `/usr/bin/google-chrome` or `/usr/bin/chromium` present
- **THEN** the executable is returned with its resolved absolute path and browser kind (chrome)

#### Scenario: No browser installed
- **WHEN** discovery runs on a machine with no supported browser installed
- **THEN** the system returns a typed `NoBrowserFound` error listing the locations that were checked

### Requirement: Launch with an isolated profile
The system SHALL launch the browser with `--remote-debugging-port=0` (OS-assigned port) and a dedicated `--user-data-dir` under an OS-appropriate per-user data directory: `%LOCALAPPDATA%\aib\profiles\<name>` on Windows, `$XDG_DATA_HOME/aib/profiles/<name>` (falling back to `~/.local/share/aib/profiles/<name>`) on Linux. The system MUST NOT attach to or launch against the user's live default profile. On Linux, the system SHALL additionally pass `--no-sandbox` only when running as root (required for headless Chromium in an unprivileged container init) — never on Windows, and never on Linux when not running as root.

#### Scenario: First launch creates profile (Windows)
- **WHEN** the browser is launched on Windows with profile name `default` and no prior profile directory exists
- **THEN** the directory `%LOCALAPPDATA%\aib\profiles\default` is created and the browser starts using it

#### Scenario: First launch creates profile (Linux)
- **WHEN** the browser is launched on Linux with profile name `default` and no prior profile directory exists
- **THEN** the directory `$XDG_DATA_HOME/aib/profiles/default` (or `~/.local/share/aib/profiles/default`) is created and the browser starts using it

#### Scenario: Debugging endpoint resolution
- **WHEN** the browser process starts
- **THEN** the system reads the DevTools WebSocket URL (from the `DevToolsActivePort` file or stderr banner) and connects within 10 seconds or returns a typed `AttachTimeout` error

### Requirement: Clean teardown
The system SHALL close browser contexts it created and, when it launched the browser process itself, SHALL terminate that process on shutdown. Teardown MUST leave no orphaned browser processes from launched instances. On Unix, the launched browser SHALL run in its own session/process group (`setsid`) so that teardown can terminate the whole group — not just the root process — ensuring zygote-forked renderer/GPU/utility children do not survive as orphans when there is no init/reaper (e.g. inside a bare container).

#### Scenario: Teardown after successful run
- **WHEN** the client disconnects after a run against a browser it launched
- **THEN** created contexts are disposed via `Target.disposeBrowserContext` and the browser process exits

#### Scenario: Attach to externally started browser
- **WHEN** the system attached to a browser it did not launch
- **THEN** teardown disposes only the contexts it created and leaves the browser process running

#### Scenario: No orphaned children in a bare container
- **WHEN** a launched browser is terminated on Unix with no init/reaper present
- **THEN** its renderer/GPU/utility child processes are also terminated, not left running as orphans

### Requirement: Reduced-footprint headless launch flags
When launching headless, the system SHALL pass memory/CPU-reduction flags in addition to the base flag set: `--disable-dev-shm-usage`, `--disable-software-rasterizer`, `--disable-extensions`, `--mute-audio`, and `--disable-gpu`. Headed launches MUST NOT receive `--disable-gpu`.

#### Scenario: Headless launch carries reduction flags
- **WHEN** a browser is launched headless
- **THEN** the spawned process's command line includes the reduction flags listed above

#### Scenario: Headed launch keeps GPU
- **WHEN** a browser is launched headed
- **THEN** `--disable-gpu` is not passed

### Requirement: Managed chrome-headless-shell for headless runs
For headless launches, the system SHALL prefer a managed `chrome-headless-shell` binary: resolve the latest stable version for the current platform from the Chrome for Testing known-good-versions endpoint, download and extract it into a per-user cache directory (`<data-dir>/aib/browsers/<version>/`), and reuse an already-cached shell without any network access. If resolution, download, or extraction fails, the system MUST fall back to the installed browser with a logged warning rather than failing the launch. Headed launches SHALL always use the installed browser. Callers MUST be able to force the installed browser for headless runs too (opt-out).

#### Scenario: First headless run downloads and uses the shell
- **WHEN** a headless launch occurs with no cached shell and network available
- **THEN** the shell is downloaded once, cached under `<data-dir>/aib/browsers/<version>/`, and the launched process is the shell binary

#### Scenario: Subsequent runs use the cache offline
- **WHEN** a headless launch occurs with a previously cached shell
- **THEN** the shell launches from cache with no network requests

#### Scenario: Download failure falls back to installed browser
- **WHEN** a headless launch occurs with no cached shell and the download fails
- **THEN** the launch proceeds with the installed browser and a warning is logged

#### Scenario: Opt-out forces installed browser
- **WHEN** a headless launch is requested with the installed-browser opt-out
- **THEN** no shell resolution or download is attempted and the installed browser is used
