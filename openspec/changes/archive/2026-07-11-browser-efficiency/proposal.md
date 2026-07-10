## Why

The external-research review (`.research/REVIEW.md`, 2026-07-11) confirmed that ai-browser's driver-side footprint is already won (measured 8 MB vs Playwright's 120 MB), but the browser binary itself — identical either way today — is the dominant cost of every session. Two browser-side levers are available with no architectural change: memory-reduction launch flags, and `chrome-headless-shell`, the stripped headless-only binary that skips the paint pipeline and GPU compositing while still supporting screenshots. The user explicitly decided (2026-07-11) to relax the zero-download principle: headless runs should auto-download and cache headless-shell for maximum out-of-the-box savings, with installed Chrome as the fallback and headed runs unchanged.

## What Changes

- Headless launches add memory/CPU-reduction flags: `--disable-dev-shm-usage`, `--disable-software-rasterizer`, `--disable-extensions`, `--mute-audio`, `--disable-gpu`.
- New managed-browser download: resolve the latest stable `chrome-headless-shell` for the current platform (win64/linux64) from the Chrome for Testing "last-known-good-versions-with-downloads" endpoint, download and unzip into a per-user cache directory, and reuse the cache on subsequent runs.
- Headless launches prefer the cached/downloadable headless-shell; if resolution or download fails (e.g. offline), they fall back to the installed browser with a logged warning. Headed launches always use the installed browser. A `--browser installed` CLI opt-out forces the installed browser even for headless runs.
- `aib doctor` reports full browser process-tree RSS per browser (not just the driver), so the headless-shell saving is measured rather than asserted.

**Out of scope:** `--remote-debugging-pipe` transport (flagged in the research as a further optimization; the `Transport` trait seam keeps it possible later), macOS download targets, and version pinning/rollback of cached shells beyond "cache per version directory".

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `browser-attach`: headless launches gain the reduced-footprint flag set and the managed headless-shell resolution/download/fallback behavior.
- `doctor-cli`: the report gains per-browser process-tree memory measurement.

## Impact

- `crates/cdp/src/launch.rs`: flag additions; launch path selects shell vs installed.
- New `crates/cdp/src/download.rs`: Chrome for Testing resolution, download, unzip, cache.
- New deps: `ureq` (small blocking HTTP client, called via `spawn_blocking`), `zip` (archive extraction).
- `src/doctor.rs` + `src/main.rs`: tree-RSS measurement, `--browser` flag plumbing.
- Network access at runtime (first headless run downloads ~100 MB once per shell version); everything cached under the aib data dir afterwards.
