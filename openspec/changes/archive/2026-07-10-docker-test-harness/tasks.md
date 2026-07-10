# Tasks: Docker Test Harness

## 1. Cross-platform discovery in `cdp::launch`

- [x] 1.1 Add Linux well-known paths to `well_known_paths` (`/usr/bin/google-chrome`, `/usr/bin/google-chrome-stable`, `/usr/bin/chromium`, `/usr/bin/chromium-browser`); Windows branch unchanged
- [x] 1.2 `profile_base_dir() -> Result<PathBuf>`: `%LOCALAPPDATA%` on Windows, `$XDG_DATA_HOME` then `~/.local/share` on Linux; replace the hardcoded `LOCALAPPDATA` read in `launch()`
- [x] 1.3 `--no-sandbox` flag added only when `cfg!(target_os = "linux")` and running as root (`unsafe { libc::geteuid() == 0 }` or a `sudo`-free equivalent via `/proc/self/status` Uid line to avoid a new dependency)
- [x] 1.4 Unit tests for `profile_base_dir` and the Linux branch of `well_known_paths` (parallel to the existing Windows tests)

## 2. Docker image and runner

- [x] 2.1 `docker/Dockerfile`: Debian slim, Rust toolchain, `chromium` + `fonts-liberation` + Chromium's runtime deps
- [x] 2.2 `docker/run-tests.sh`: builds the image, runs `cargo test --workspace` and `aib doctor --json` inside it, repo mounted read-write, container `target/` in a named volume
- [x] 2.3 `.dockerignore` excluding `target/`, `.git/`

## 3. Verification

- [x] 3.1 `docker/run-tests.sh` run once; record the full test-suite pass/fail and `aib doctor --json` output as evidence in this change
- [x] 3.2 Confirm Windows behavior unchanged: `cargo test --workspace` and `aib doctor` still pass on the host after the cross-platform refactor

## 4. Wrap-up

- [x] 4.1 README: add a "Testing in Docker" section pointing at `docker/run-tests.sh`
- [x] 4.2 `openspec validate docker-test-harness` clean; archive after verified
