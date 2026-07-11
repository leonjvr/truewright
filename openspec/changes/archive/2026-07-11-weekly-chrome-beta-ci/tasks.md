# Tasks: weekly-chrome-beta-ci

## 1. `crates/cdp/src/launch.rs`
- [x] 1.1 `discover_browsers()` checks `AIB_CHROME_PATH` first; when set and the file exists, returns a single `DiscoveredBrowser { kind: Chrome, path, is_headless_shell: false }` without touching registry/well-known-path search
- [x] 1.2 `AIB_CHROME_PATH` set but not an existing file → typed error immediately, no fallback
- [x] 1.3 (found during implementation, not in the original scope) `resolve_headless_browser()` also checks `AIB_CHROME_PATH` first, ahead of the managed `chrome-headless-shell` cache/download -- otherwise every headless launch (the test suite's default) would silently ignore the override entirely

## 2. Docker
- [x] 2.1 `docker/Dockerfile.chrome-beta` (new): adds Google's apt repo + signing key, installs `google-chrome-beta`, keeps the rest of the toolchain identical to `docker/Dockerfile`
- [x] 2.2 `docker/run-tests-chrome-beta.sh` (new): builds that image, runs `cargo test --workspace && aib doctor --json` inside it with `AIB_CHROME_PATH=/usr/bin/google-chrome-beta`

## 3. GitHub Actions
- [x] 3.1 `.github/workflows/chrome-beta.yml` (new): `schedule` (weekly cron) + `workflow_dispatch` triggers, runs `docker/run-tests-chrome-beta.sh` on `ubuntu-latest`
- [x] 3.2 Syntax-validated locally with a YAML parser (`python3 -c "import yaml; yaml.safe_load(...)"`) since it cannot be observed running on GitHub (no remote configured for this repo)

## 4. Verification
- [x] 4.1 Unit test: `AIB_CHROME_PATH` pointed at the real discovered Chrome path on this machine → discovery returns exactly that path
- [x] 4.2 Unit test: `AIB_CHROME_PATH` pointed at a nonexistent path → typed error, not silent fallback
- [x] 4.3 Live-build `docker/Dockerfile.chrome-beta` and run `docker/run-tests-chrome-beta.sh` for real -- confirmed `google-chrome-beta` (151.0.7922.19-1) installs, `aib doctor --json` reports `"path": "/usr/bin/google-chrome-beta"` with `"ok": true`, and the full `cargo test --workspace` suite passes against it (zero failures)
- [x] 4.4 `cargo test --workspace` on host (existing suite unaffected -- a real env-var test race between two AIB_CHROME_PATH tests was caught here and fixed by merging them into one test function) and `bash docker/run-tests.sh` (existing Stable/Chromium image unaffected)
- [x] 4.5 State the GitHub Actions execution limitation plainly in the proposal, design, and PROPOSAL.md roadmap entry -- not glossed over

## 5. Wrap-up
- [x] 5.1 Update README (env var + new CI job, with the "not yet observed running" caveat)
- [x] 5.2 Update PROPOSAL.md's Phase 5 roadmap
- [x] 5.3 `openspec archive weekly-chrome-beta-ci -y`, fix any "Purpose: TBD" placeholder in the synced spec (there shouldn't be one -- this is a MODIFIED requirement on an existing spec)
- [x] 5.4 Three commits: Propose, Implement, Sync-specs-and-archive
