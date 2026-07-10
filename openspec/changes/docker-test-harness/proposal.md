## Why

Phase 1 testing ran against the developer's real Chrome install on the host machine. Two real incidents came out of that: a test that panicked before reaching cleanup orphaned a headless Chrome process holding its profile directory locked (fixed with a `Drop` safety net, but the orphan itself had to be hunted down among 70+ of the developer's own legitimate Chrome windows — the auto-mode safety classifier correctly refused a broad `Stop-Process` sweep because the PIDs couldn't be distinguished from the user's real browsing session). Every future phase (2–5) will iterate on click/type/input-timing code and needs to launch and kill browsers repeatedly; doing that against the host's real Chrome is both riskier (harder to safely clean up orphans) and noisier (test runs are commingled with the developer's actual browser). A disposable Docker container isolates all of that: kill the container, everything it touched is gone, with zero risk to the host.

`cdp::launch` currently only discovers browsers on Windows (registry `App Paths` + `%LOCALAPPDATA%`-rooted well-known paths and profile directory) — non-goals explicitly scoped Phase 0 to Windows-only. Running in a Linux container requires extending discovery and the profile-directory base path to Linux, which is a small, well-contained addition to the existing `browser-attach` capability.

## What Changes

- `browser-attach` capability extended: Linux well-known-path discovery (`/usr/bin/google-chrome`, `/usr/bin/chromium`, etc., no registry lookup) and a cross-platform profile-directory base (`%LOCALAPPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` on Linux) instead of a Windows-only hardcoded path.
- New `docker/Dockerfile`: a Debian-slim image with the Rust toolchain, Chromium, and the fonts/deps Chromium needs headless, used to build and run `aib` and its test suite in an isolated container.
- New `docker/run-tests.sh` (or equivalent Compose/Make target): one command to build the image and run `cargo test --workspace` plus `aib doctor` inside it, mounting the repo read-write for a normal edit/test loop.
- No change to Windows behavior — registry discovery and `%LOCALAPPDATA%` profiles are untouched; this is additive.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `browser-attach`: discovery gains a Linux well-known-path table; the isolated-profile requirement's base directory becomes OS-aware instead of Windows-only.

## Impact

- `crates/cdp/src/launch.rs`: `well_known_paths` gains a Linux branch; profile base-dir resolution becomes a small `fn profile_base_dir() -> Result<PathBuf>` cross-platform helper.
- New `docker/` directory at repo root: `Dockerfile`, a test-runner script, `.dockerignore`.
- Going forward, Phase 2+ development and testing happens inside this container by default; the host is used only when something genuinely needs the real Windows environment (e.g. eventual Phase 4 true-user `SendInput` testing, which is Windows-only by nature and can't run in a Linux container).
