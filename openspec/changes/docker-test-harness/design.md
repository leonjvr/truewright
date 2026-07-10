# Design: Docker Test Harness

## Context

Discovered a real operational gap while testing Phase 1: a panicking test left a headless Chrome process orphaned and locking its profile directory, and cleaning it up on the host required distinguishing it from 70+ of the developer's own real Chrome windows — the auto-mode safety classifier correctly blocked a broad kill sweep. Phases 2–4 will iterate heavily on browser-launching code (motion timing, input dispatch, eventually a Windows `SendInput` backend); doing that safely means testing in a disposable environment, not the host's real browser session.

## Goals / Non-Goals

**Goals**
- `aib doctor` and `cargo test --workspace` runnable inside a Linux Docker container, isolated from the host's real browser.
- Minimal, targeted Linux support in `cdp::launch` — just enough to discover and launch Chromium in a container, not full Linux desktop parity.
- Zero behavior change on Windows.

**Non-Goals**
- Full cross-platform parity (macOS, Windows-container, WSL2-specific paths) — out of scope; Linux-in-Docker only, because that's what unblocks safe testing.
- Phase 4's Windows `SendInput` mode obviously cannot run in a Linux container — the container is for Phases 2/3/5's CDP-only work; true-user mode still needs host testing, done deliberately and sparingly.
- CI pipeline wiring (GitHub Actions etc.) — this is a local dev-safety harness, not a CI change.

## Decisions

1. **Debian-slim base image with Chromium from apt, not a Chrome-for-Testing download.** Debian's `chromium` package is headless-capable, doesn't require Google's EULA acceptance flow, and installs cleanly with `apt-get install chromium` plus standard font packages (`fonts-liberation`) for text rendering in screenshots.
2. **`--no-sandbox` only when running as root, and only on Linux.** Chromium's sandbox needs `CAP_SYS_ADMIN` or a setuid helper that a bare container init typically lacks; `--no-sandbox` is the standard workaround for containerized Chromium. Gating it to "root + Linux" keeps it from ever silently applying on the host.
3. **Profile base directory via `dirs`-free manual resolution, not a new dependency.** `%LOCALAPPDATA%` (Windows) / `$XDG_DATA_HOME` or `~/.local/share` (Linux) is a two-branch `#[cfg(windows)]`/else split — doesn't justify pulling in the `dirs` crate for two env-var reads.
4. **Repo mounted read-write into the container, not copied.** `cargo build` artifacts and `target/` persist across runs via a named volume for the container's own `/app/target` (separate from the host's `target/` to avoid cross-platform binary contamination), so the edit/test loop stays fast.

## Risks / Trade-offs

- [Debian's `chromium` version may lag Chrome stable] → acceptable; CDP surface used (Target/Page/Runtime/Input) is stable across recent Chromium versions, and this is a dev-testing tool, not the shipped product's browser.
- [Container Chromium behavior can diverge subtly from host Chrome (e.g. font rendering in screenshots)] → doesn't affect functional tests (navigate/snapshot/click/type/wait_for), only pixel-level screenshot comparisons, which this project doesn't do.
- [Adds a second place profile-dir logic can drift (Windows vs Linux)] → both branches covered by unit tests exercising the resolution logic directly, not just via a live browser launch.

## Migration Plan

Additive, no rollback concerns. `docker/` is opt-in tooling; nothing changes for someone who never runs it.

## Open Questions

None outstanding — scope is deliberately narrow.
