## MODIFIED Requirements

### Requirement: Discover installed Chromium browsers
The system SHALL discover installed Chrome and Edge on Windows by reading the registry `App Paths` keys (`chrome.exe`, `msedge.exe`), and SHALL discover installed Chrome/Chromium on Linux by checking well-known binary paths (no registry equivalent exists on Linux). On both platforms the system SHALL fall back to well-known installation paths when a platform-specific lookup (registry, on Windows) is absent or unavailable. Discovery MUST NOT download or install any browser. Before registry/well-known-path discovery runs, the system SHALL check the `AIB_CHROME_PATH` environment variable: when set, it forces discovery to return exactly that binary (kind `Chrome`) instead of searching, and the system MUST return a typed error immediately if the path does not point at an existing file, rather than falling back to normal discovery.

#### Scenario: Both browsers installed (Windows)
- **WHEN** discovery runs on a Windows machine with Chrome and Edge installed
- **THEN** both executables are returned with their resolved absolute paths and browser kind (chrome | edge)

#### Scenario: Chromium installed (Linux)
- **WHEN** discovery runs on a Linux machine with `/usr/bin/google-chrome` or `/usr/bin/chromium` present
- **THEN** the executable is returned with its resolved absolute path and browser kind (chrome)

#### Scenario: No browser installed
- **WHEN** discovery runs on a machine with no supported browser installed
- **THEN** the system returns a typed `NoBrowserFound` error listing the locations that were checked

#### Scenario: AIB_CHROME_PATH overrides discovery
- **WHEN** `AIB_CHROME_PATH` is set to the path of an existing browser binary
- **THEN** discovery returns exactly that binary and does not search the registry or well-known paths

#### Scenario: AIB_CHROME_PATH points at a missing file
- **WHEN** `AIB_CHROME_PATH` is set but no file exists at that path
- **THEN** discovery returns a typed error immediately, without falling back to normal discovery

### Requirement: Managed chrome-headless-shell for headless runs
For headless launches, the system SHALL prefer a managed `chrome-headless-shell` binary: resolve the latest stable version for the current platform from the Chrome for Testing known-good-versions endpoint, download and extract it into a per-user cache directory (`<data-dir>/aib/browsers/<version>/`), and reuse an already-cached shell without any network access. If resolution, download, or extraction fails, the system MUST fall back to the installed browser with a logged warning rather than failing the launch. Headed launches SHALL always use the installed browser. Callers MUST be able to force the installed browser for headless runs too (opt-out). When `AIB_CHROME_PATH` is set, it SHALL take priority over the managed shell as well as over normal discovery -- a headless launch with the override set MUST use exactly that binary, not the cached/downloaded shell.

#### Scenario: First headless run downloads and uses the shell
- **WHEN** a headless launch occurs with no cached shell and network available
- **THEN** the shell is downloaded once, cached under `<data-dir>/aib/browsers/<version>/`, and the launched process is the shell binary

#### Scenario: Subsequent runs use the cache offline
- **WHEN** a headless launch occurs with a previously cached shell
- **THEN** the shell launches from cache with no network requests

#### Scenario: Download failure falls back to installed browser
- **WHEN** a headless launch occurs with no cached shell and the download fails
- **THEN** the launch proceeds with the installed browser and a warning is logged

#### Scenario: AIB_CHROME_PATH overrides the managed shell too
- **WHEN** `AIB_CHROME_PATH` is set and a headless launch occurs, regardless of whether a cached shell exists
- **THEN** the launch uses the `AIB_CHROME_PATH` binary, not the managed shell

#### Scenario: Opt-out forces installed browser
- **WHEN** a headless launch is requested with the installed-browser opt-out
- **THEN** no shell resolution or download is attempted and the installed browser is used
