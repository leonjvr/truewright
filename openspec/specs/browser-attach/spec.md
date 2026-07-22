# browser-attach

## Purpose

Locate an installed Chromium browser (Chrome/Edge) without downloading anything, launch it against an isolated profile, and tear it down cleanly — the attach layer underneath everything else in truewright.
## Requirements
### Requirement: Discover installed Chromium browsers
The system SHALL discover installed Chrome and Edge on Windows by reading the registry `App Paths` keys (`chrome.exe`, `msedge.exe`), and SHALL discover installed Chrome/Chromium on Linux by checking well-known binary paths (no registry equivalent exists on Linux). On both platforms the system SHALL fall back to well-known installation paths when a platform-specific lookup (registry, on Windows) is absent or unavailable. Discovery MUST NOT download or install any browser. Before registry/well-known-path discovery runs, the system SHALL check the `TRUEWRIGHT_CHROME_PATH` environment variable: when set, it forces discovery to return exactly that binary (kind `Chrome`) instead of searching, and the system MUST return a typed error immediately if the path does not point at an existing file, rather than falling back to normal discovery.

#### Scenario: Both browsers installed (Windows)
- **WHEN** discovery runs on a Windows machine with Chrome and Edge installed
- **THEN** both executables are returned with their resolved absolute paths and browser kind (chrome | edge)

#### Scenario: Chromium installed (Linux)
- **WHEN** discovery runs on a Linux machine with `/usr/bin/google-chrome` or `/usr/bin/chromium` present
- **THEN** the executable is returned with its resolved absolute path and browser kind (chrome)

#### Scenario: No browser installed
- **WHEN** discovery runs on a machine with no supported browser installed
- **THEN** the system returns a typed `NoBrowserFound` error listing the locations that were checked

#### Scenario: TRUEWRIGHT_CHROME_PATH overrides discovery
- **WHEN** `TRUEWRIGHT_CHROME_PATH` is set to the path of an existing browser binary
- **THEN** discovery returns exactly that binary and does not search the registry or well-known paths

#### Scenario: TRUEWRIGHT_CHROME_PATH points at a missing file
- **WHEN** `TRUEWRIGHT_CHROME_PATH` is set but no file exists at that path
- **THEN** discovery returns a typed error immediately, without falling back to normal discovery

### Requirement: Launch with an isolated profile
The system SHALL launch the browser with a dedicated `--user-data-dir` under an OS-appropriate per-user data directory: `%LOCALAPPDATA%\truewright\profiles\<name>` on Windows, `$XDG_DATA_HOME/truewright/profiles/<name>` (falling back to `~/.local/share/truewright/profiles/<name>`) on Linux. The CDP transport is chosen by platform: on Unix the launch SHALL use `--remote-debugging-pipe` (CDP over inherited fds 3/4, no TCP port), on Windows `--remote-debugging-port=0` (OS-assigned loopback port). A `TRUEWRIGHT_CDP_TRANSPORT=websocket` override SHALL force the TCP/WebSocket path on Unix as an escape hatch. The system MUST NOT attach to or launch against the user's live default profile. On Linux, the system SHALL additionally pass `--no-sandbox` when running as root, inside a container, or under CI (detected via the `container`/`CI` environment variables and the `/.dockerenv` / `/run/.containerenv` marker files) — required for headless Chromium where the sandbox cannot initialize — never on Windows. The auto-applied `--no-sandbox` MUST be de-duplicated against one the caller supplied explicitly.

### Requirement: Extra Chrome launch flags
The system SHALL append caller-supplied raw Chrome/Edge command-line flags to every launched session, from three cumulative sources: the config `[browser].extra_args` list, the repeatable `--chrome-arg` CLI option (on `mcp`, `doctor`, and `agent`), and the `TRUEWRIGHT_CHROME_ARGS` environment variable (whitespace-separated). All three MUST apply together; where two flags conflict, Chrome's own last-flag-wins behavior governs. The `TRUEWRIGHT_CHROME_ARGS` variable SHALL be read at the launch layer so it reaches every entry point uniformly, including the test suite. A `--no-sandbox` CLI shortcut SHALL be available on `mcp`, `doctor`, and `agent` as a convenience equivalent to `--chrome-arg=--no-sandbox`.

#### Scenario: Flags from all three sources merge
- **WHEN** `[browser].extra_args = ["--kiosk"]` is configured, `--chrome-arg=--window-size=1440,900` is passed, and `TRUEWRIGHT_CHROME_ARGS="--window-position=0,0"` is set
- **THEN** the launched browser receives all three flags in addition to the built-in ones

#### Scenario: --no-sandbox shortcut forces the flag when detection misses
- **WHEN** `truewright mcp --no-sandbox` is run in an environment where container/CI/root auto-detection does not fire (e.g. an unprivileged LXC)
- **THEN** the launched browser still receives `--no-sandbox`, and it is not duplicated if also supplied via config/env/`--chrome-arg`

#### Scenario: First launch creates profile (Windows)
- **WHEN** the browser is launched on Windows with profile name `default` and no prior profile directory exists
- **THEN** the directory `%LOCALAPPDATA%\truewright\profiles\default` is created and the browser starts using it

#### Scenario: First launch creates profile (Linux)
- **WHEN** the browser is launched on Linux with profile name `default` and no prior profile directory exists
- **THEN** the directory `$XDG_DATA_HOME/truewright/profiles/default` (or `~/.local/share/truewright/profiles/default`) is created and the browser starts using it

#### Scenario: Debugging endpoint resolution (WebSocket transport)
- **WHEN** the browser process starts on the TCP/WebSocket path (Windows, or a forced-WebSocket Unix run)
- **THEN** the system reads the DevTools WebSocket URL (from the `DevToolsActivePort` file or stderr banner) and connects within 10 seconds or returns a typed `AttachTimeout` error

#### Scenario: Pipe transport needs no endpoint discovery
- **WHEN** the browser process starts on the Unix `--remote-debugging-pipe` path
- **THEN** the system speaks CDP directly over the inherited fds with no port allocation, no `DevToolsActivePort` poll, and therefore no attach-timeout scan

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
When launching headless, the system SHALL pass memory/CPU-reduction flags in addition to the base flag set: `--disable-dev-shm-usage`, `--disable-software-rasterizer`, `--disable-extensions`, `--mute-audio`, and `--disable-gpu`. It SHALL additionally pass Chrome's automation-oriented default flags (matching Playwright's set — e.g. `--disable-features=…` to kill background service processes, `--disable-component-extensions-with-background-pages`, `--disable-breakpad`, `--disable-sync`, `--metrics-recording-only`, `--no-service-autorun`, `--no-startup-window`, `--use-mock-keychain`, `--password-store=basic`, `--force-color-profile=srgb`, `--hide-scrollbars`), which cut the Chrome process tree from ~15 down to ~10. For a full-Chrome headless run (not `chrome-headless-shell`), the system SHALL use the lightweight old `--headless` mode, not `--headless=new`; `--headless=new` remains opt-in via a caller flag override. Every one of these flags MUST remain overridable/removable via `--chrome-arg` / `[browser].extra_args` / `TRUEWRIGHT_CHROME_ARGS` (Chrome's last-flag-wins rule). Headed launches MUST NOT receive `--disable-gpu` or the automation flag set.

#### Scenario: Headless launch carries reduction and automation flags
- **WHEN** a browser is launched headless
- **THEN** the spawned process's command line includes the reduction flags and the automation flag set, and (for full Chrome) old `--headless`

#### Scenario: Caller can override a default flag
- **WHEN** a headless launch is given `--chrome-arg=--headless=new`
- **THEN** the later caller-supplied `--headless=new` wins over the default `--headless`

#### Scenario: Headed launch keeps GPU
- **WHEN** a browser is launched headed
- **THEN** `--disable-gpu` and the automation flag set are not passed

### Requirement: Managed chrome-headless-shell for headless runs
For headless launches, the system SHALL prefer a managed `chrome-headless-shell` binary: resolve the latest stable version for the current platform from the Chrome for Testing known-good-versions endpoint, download and extract it into a per-user cache directory (`<data-dir>/truewright/browsers/<version>/`), and reuse an already-cached shell without any network access. If resolution, download, or extraction fails, the system MUST fall back to the installed browser with a logged warning rather than failing the launch. Headed launches SHALL always use the installed browser. Callers MUST be able to force the installed browser for headless runs too (opt-out). When `TRUEWRIGHT_CHROME_PATH` is set, it SHALL take priority over the managed shell as well as over normal discovery -- a headless launch with the override set MUST use exactly that binary, not the cached/downloaded shell.

#### Scenario: First headless run downloads and uses the shell
- **WHEN** a headless launch occurs with no cached shell and network available
- **THEN** the shell is downloaded once, cached under `<data-dir>/truewright/browsers/<version>/`, and the launched process is the shell binary

#### Scenario: Subsequent runs use the cache offline
- **WHEN** a headless launch occurs with a previously cached shell
- **THEN** the shell launches from cache with no network requests

#### Scenario: Download failure falls back to installed browser
- **WHEN** a headless launch occurs with no cached shell and the download fails
- **THEN** the launch proceeds with the installed browser and a warning is logged

#### Scenario: TRUEWRIGHT_CHROME_PATH overrides the managed shell too
- **WHEN** `TRUEWRIGHT_CHROME_PATH` is set and a headless launch occurs, regardless of whether a cached shell exists
- **THEN** the launch uses the `TRUEWRIGHT_CHROME_PATH` binary, not the managed shell

#### Scenario: Opt-out forces installed browser
- **WHEN** a headless launch is requested with the installed-browser opt-out
- **THEN** no shell resolution or download is attempted and the installed browser is used

