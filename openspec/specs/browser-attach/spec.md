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
The system SHALL close browser contexts it created and, when it launched the browser process itself, SHALL terminate that process on shutdown. Teardown MUST leave no orphaned browser processes from launched instances.

#### Scenario: Teardown after successful run
- **WHEN** the client disconnects after a run against a browser it launched
- **THEN** created contexts are disposed via `Target.disposeBrowserContext` and the browser process exits

#### Scenario: Attach to externally started browser
- **WHEN** the system attached to a browser it did not launch
- **THEN** teardown disposes only the contexts it created and leaves the browser process running
