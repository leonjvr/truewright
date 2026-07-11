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
