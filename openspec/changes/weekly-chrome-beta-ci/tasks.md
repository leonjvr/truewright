# Tasks: weekly-chrome-beta-ci

## 1. `crates/cdp/src/launch.rs`
- [ ] 1.1 `discover_browsers()` checks `AIB_CHROME_PATH` first; when set and the file exists, returns a single `DiscoveredBrowser { kind: Chrome, path, is_headless_shell: false }` without touching registry/well-known-path search
- [ ] 1.2 `AIB_CHROME_PATH` set but not an existing file → typed error immediately, no fallback

## 2. Docker
- [ ] 2.1 `docker/Dockerfile.chrome-beta` (new): adds Google's apt repo + signing key, installs `google-chrome-beta`, keeps the rest of the toolchain identical to `docker/Dockerfile`
- [ ] 2.2 `docker/run-tests-chrome-beta.sh` (new): builds that image, runs `cargo test --workspace && aib doctor --json` inside it with `AIB_CHROME_PATH=/usr/bin/google-chrome-beta`

## 3. GitHub Actions
- [ ] 3.1 `.github/workflows/chrome-beta.yml` (new): `schedule` (weekly cron) + `workflow_dispatch` triggers, runs `docker/run-tests-chrome-beta.sh` on `ubuntu-latest`
- [ ] 3.2 Syntax-validated locally with a YAML parser (`python3 -c "import yaml; yaml.safe_load(...)"`) since it cannot be observed running on GitHub (no remote configured for this repo)

## 4. Verification
- [ ] 4.1 Unit test: `AIB_CHROME_PATH` pointed at the real discovered Chrome path on this machine → discovery returns exactly that path
- [ ] 4.2 Unit test: `AIB_CHROME_PATH` pointed at a nonexistent path → typed error, not silent fallback
- [ ] 4.3 Live-build `docker/Dockerfile.chrome-beta` and run `docker/run-tests-chrome-beta.sh` for real -- confirm `google-chrome-beta` installs, `aib doctor` reports it, and `cargo test --workspace` passes against it
- [ ] 4.4 `cargo test --workspace` on host (existing suite unaffected) and `bash docker/run-tests.sh` (existing Stable/Chromium image unaffected)
- [ ] 4.5 State the GitHub Actions execution limitation plainly in the proposal, design, and PROPOSAL.md roadmap entry -- not glossed over

## 5. Wrap-up
- [ ] 5.1 Update README (env var + new CI job, with the "not yet observed running" caveat)
- [ ] 5.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 5.3 `openspec archive weekly-chrome-beta-ci -y`, fix any "Purpose: TBD" placeholder in the synced spec (there shouldn't be one -- this is a MODIFIED requirement on an existing spec)
- [ ] 5.4 Three commits: Propose, Implement, Sync-specs-and-archive
