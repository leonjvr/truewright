# Tasks: Browser Efficiency

## 1. Launch flags

- [ ] 1.1 Add reduction flags to headless launches in `cdp::launch::launch` (`--disable-dev-shm-usage`, `--disable-software-rasterizer`, `--disable-extensions`, `--mute-audio`, `--disable-gpu`); headed launches unchanged

## 2. Managed headless-shell

- [ ] 2.1 `crates/cdp/src/download.rs`: resolve Stable `chrome-headless-shell` (win64/linux64) from the Chrome for Testing endpoint; tolerant JSON parsing
- [ ] 2.2 Download + unzip into `<data-dir>/aib/browsers/<version>/`; `latest.txt` marker for offline cache lookup; `ureq` + `zip` deps, run via `spawn_blocking`
- [ ] 2.3 `resolve_headless_browser(allow_download)` selection: cached shell → download → installed-browser fallback with logged warning; shell launch skips `--headless=new`
- [ ] 2.4 `engine::Session::launch` gains `BrowserPreference` (Auto | Installed); `aib mcp --browser installed`, `aib doctor --browser installed` plumbing
- [ ] 2.5 Unit tests: endpoint JSON parsing (fixture), cache-path resolution, fallback-on-failure path

## 3. Doctor tree memory

- [ ] 3.1 `tree_rss_mb` per browser in doctor via `sysinfo` (bin-only dep): sum RSS of browser root + descendants while page is loaded
- [ ] 3.2 Doctor runs the managed shell by default (headless) alongside/instead of installed browsers per the new selection; `--browser installed` restores old behavior

## 4. Verification

- [ ] 4.1 Host: full suite green; `aib doctor --json` shows tree_rss for shell and (with `--browser installed`) for installed Chrome; record both as evidence in this change
- [ ] 4.2 Container: `bash docker/run-tests.sh` green (Linux download path exercised or cleanly skipped/fallback)
- [ ] 4.3 README + PROPOSAL flag/download behavior documented

## 5. Wrap-up

- [ ] 5.1 `openspec validate browser-efficiency` clean; sync specs; archive
