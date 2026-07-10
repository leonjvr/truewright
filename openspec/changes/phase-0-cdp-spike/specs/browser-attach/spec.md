# browser-attach

## ADDED Requirements

### Requirement: Discover installed Chromium browsers
The system SHALL discover installed Chrome and Edge on Windows by reading the registry `App Paths` keys (`chrome.exe`, `msedge.exe`) and SHALL fall back to well-known installation paths when registry entries are absent. Discovery MUST NOT download or install any browser.

#### Scenario: Both browsers installed
- **WHEN** discovery runs on a machine with Chrome and Edge installed
- **THEN** both executables are returned with their resolved absolute paths and browser kind (chrome | edge)

#### Scenario: No browser installed
- **WHEN** discovery runs on a machine with neither Chrome nor Edge installed
- **THEN** the system returns a typed `NoBrowserFound` error listing the locations that were checked

### Requirement: Launch with an isolated profile
The system SHALL launch the browser with `--remote-debugging-port=0` (OS-assigned port) and a dedicated `--user-data-dir` under `%LOCALAPPDATA%\aib\profiles\<name>`. The system MUST NOT attach to or launch against the user's live default profile.

#### Scenario: First launch creates profile
- **WHEN** the browser is launched with profile name `default` and no prior profile directory exists
- **THEN** the directory `%LOCALAPPDATA%\aib\profiles\default` is created and the browser starts using it

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
