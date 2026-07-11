## Why

CDP is Chrome's own internal debugging protocol, not a stable public API -- fields get renamed, events get restructured, and behavior shifts between channels before it ever reaches Stable. Running the suite only against whatever Chrome happens to be installed on the dev/CI machine means protocol drift is discovered the same week it ships to users, not weeks earlier while it's still sitting in Beta. `PROPOSAL.md` has called for "weekly CI against stable + beta channels" since Phase 5 was scoped; this change ships the beta half (the repo already runs the full suite against whatever's installed locally, which today is effectively Stable/Chromium).

**Known limitation, stated up front:** this repository has no git remote and no existing `.github/workflows/` directory (confirmed via `git remote -v` and directory listing). A GitHub Actions workflow file can be written correctly, but its actual execution on GitHub's runners cannot be observed from here -- there is nowhere to push it and watch it run. What *can* be verified live, and will be: the underlying mechanism the workflow depends on (an explicit browser-path override, a Chrome-Beta-installing Docker image, and a real `cargo test --workspace` run against that image) all run for real, on this machine, via Docker, exactly as the workflow would invoke them.

## What Changes

- `crates/cdp/src/launch.rs`: `discover_browsers()` checks a new `AIB_CHROME_PATH` env var first. When set, it forces that exact binary (kind `Chrome`) instead of doing registry/well-known-path discovery -- and if the path doesn't point at a real file, discovery fails loudly with a clear error rather than silently falling back to whatever else is installed. This is the one piece of product code this change needs: a way to point `aib` at a specific Chrome binary (e.g. Beta) regardless of where it's installed, for exactly this CI use case.
- `docker/Dockerfile.chrome-beta` (new): installs `google-chrome-beta` from Google's own apt repository (not Debian's `chromium` package, which only tracks Stable-equivalent builds) alongside the existing toolchain/fonts setup.
- `docker/run-tests-chrome-beta.sh` (new): builds that image and runs `cargo test --workspace` plus `aib doctor --json` inside it with `AIB_CHROME_PATH=/usr/bin/google-chrome-beta` set, mirroring `run-tests.sh`'s existing structure.
- `.github/workflows/chrome-beta.yml` (new): runs `run-tests-chrome-beta.sh` on a weekly cron schedule plus `workflow_dispatch` for on-demand runs. A failing run fails the workflow and (GitHub's default behavior) notifies watchers by email -- no custom notification wiring added.

**Explicitly out of scope (deferred), and why:**
- **Actually observing the workflow run on GitHub.** No remote exists for this repo yet; this is a real constraint, not a corner being cut. If/when a remote is added, the first scheduled or manually-dispatched run is the true end-to-end verification.
- **A parallel `chrome-stable` scheduled workflow.** `docker/run-tests.sh` already runs on every local/manual invocation against Debian's `chromium` package; this change adds the missing Beta leg, not a full CI matrix rebuild. `PROPOSAL.md`'s broader "stable+beta CI" note also covers *general* CI (running on every push), which is separate, larger scope than the Beta-drift-detection job this change ships.
- **Automatic issue filing on failure / Slack notifications / etc.** GitHub Actions' built-in failure notification is the mechanism; nothing beyond it is added.
- **Edge Beta.** Chrome Beta is the channel `PROPOSAL.md` calls out; Edge discovery/launch is untouched by this change.

## Capabilities

### Modified Capabilities
- `browser-attach`: discovery gains an explicit-path override (`AIB_CHROME_PATH`) that bypasses registry/well-known-path search.

## Impact

- `crates/cdp/src/launch.rs`: `AIB_CHROME_PATH` override in `discover_browsers()`.
- `docker/Dockerfile.chrome-beta` (new), `docker/run-tests-chrome-beta.sh` (new).
- `.github/workflows/chrome-beta.yml` (new) -- written correctly, syntax-validated locally, but not observable running (no git remote).
- No CDP protocol changes to production request/response handling; no real-OS side effects.
