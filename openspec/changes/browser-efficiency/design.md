# Design: Browser Efficiency

## Context

Follows the research review (`.research/REVIEW.md`): browser binary is the dominant cost; headless-shell + flags are the levers. User decided auto-download over strict zero-download.

## Goals / Non-Goals

**Goals:** reduction flags on headless launches; managed headless-shell (resolve → download → cache → launch) with installed-Chrome fallback; tree-RSS measurement in doctor for evidence.

**Non-Goals:** pipe transport (later; `Transport` trait seam preserved), macOS targets, proxy configuration for the downloader, checksum verification beyond HTTPS (Chrome for Testing serves over TLS from Google's CDN; good enough for a dev tool now — noted as future hardening).

## Decisions

1. **`ureq` + `zip`, called inside `tokio::task::spawn_blocking`.** A full `reqwest` stack (hyper/rustls/tower) is overkill for one GET-JSON + one GET-zip per shell version. `ureq` is small, synchronous, rustls-based; blocking is fine off the async runtime's worker threads via `spawn_blocking`.
2. **Resolution endpoint:** `https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json`, channel `Stable`, binary `chrome-headless-shell`, platform `win64` or `linux64`. Cache layout: `<profile_base_dir>/aib/browsers/<version>/chrome-headless-shell-<platform>/chrome-headless-shell(.exe)`. A `latest.txt` marker in `browsers/` records the last resolved version so fully-offline runs can find the cache without hitting the endpoint.
3. **Launch selection is a new `BrowserChoice` in `launch`**, not a new `BrowserKind` variant: `resolve_headless_browser(allow_download) -> DiscoveredBrowser` returns either the shell (kind stays `Chrome`, since CDP-wise it is Chromium) or the installed browser. Keeps `BrowserKind`'s serialization and doctor labels stable; the doctor report distinguishes binaries by `path`.
4. **Shell needs no `--headless` flag family changes:** `chrome-headless-shell` is headless-only; passing `--headless=new` to it is harmless but unnecessary — launch skips it for the shell. All other flags (debug port, user-data-dir, reduction flags) apply identically.
5. **Tree RSS on Windows via `Win32_Process` parent-child walk (PowerShell-free):** doctor uses the `sysinfo` crate? No — avoid a heavyweight dep; implement a minimal walk: Windows `CreateToolhelp32Snapshot` is FFI-heavy. Pragmatic choice: use the `sysinfo` crate (well-maintained, cross-platform, gives per-process memory + parent PIDs) scoped to the `aib` binary only (dev-tool, not the library crates). This keeps `cdp` dependency-light while giving doctor accurate tree numbers on both Windows and Linux.
6. **Engine/MCP default:** `engine::Session::launch` gains a `browser: BrowserPreference` (Auto | Installed) parameter defaulting to Auto (shell for headless); `aib mcp --browser installed` and `aib doctor --browser installed` plumb the opt-out.

## Risks / Trade-offs

- [First headless run needs network + ~100 MB download] → cached per version; clear log line with progress; fallback to installed browser keeps offline machines working.
- [Headless-shell rendering differs slightly from full Chrome (fonts/GPU-dependent paths)] → acceptable for agent testing; documented; `--browser installed` exists for pixel-parity needs.
- [Chrome for Testing endpoint shape could change] → tolerant JSON parsing; failure = fallback, never a hard error.
- [`sysinfo` adds compile weight to the `aib` bin] → bin-only dep, not in `cdp`/`engine`.

## Migration Plan

Additive. Existing behavior reachable via `--browser installed`. No rollback concerns.

## Open Questions

None blocking; checksum verification and proxy support noted as future hardening.

## Addendum: bugs found during implementation

Two real bugs surfaced only by actually running `tree_rss_mb` in the Docker container (not by writing code that compiles) — worth recording since they're exactly the class of thing that stays invisible without genuine verification:

1. **`sysinfo::System::refresh_processes()` defaults to `.with_tasks()`, which on Linux enumerates each thread as a pseudo-process** (threads share the process PID namespace via `/proc/[pid]/task/[tid]`), and every such "process" entry reports the *whole process's* RSS. Summing a tree walk over these multiplied real memory by the live thread count — observed as ~13 GB reported for a ~900 MB real Chromium tree (~14×, confirmed against `ps aux`). Fixed by using `refresh_processes_specifics(..., ProcessRefreshKind::nothing().with_memory().without_tasks())` instead of the convenience wrapper.
2. **Orphaned renderer/GPU/utility processes survive a killed browser inside a container with no real init/reaper** — exactly the "Zombie Processes" failure mode `.research/High Performance Browser Automation.md` named explicitly. `LaunchedBrowser::shutdown`/`Drop` previously only killed the root process; `tokio::process::Child::kill()` doesn't cascade to reparented descendants without a process group. Fixed by `setsid()`-ing the browser at launch (Unix only, via `pre_exec`) so it becomes its own session/process-group leader, then sending `SIGKILL` to the negative PID (the whole group) on teardown.

Neither bug affected Windows (the thread-enumeration issue is Linux-specific; Windows process teardown already worked via `TerminateProcess`/`start_kill` since Windows doesn't have the same reparenting-to-orphan behavior in the same way).
