# browser-attach

## ADDED Requirements

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
