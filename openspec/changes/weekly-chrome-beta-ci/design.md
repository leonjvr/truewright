## Context

`discover_browsers()` today only ever finds Chrome/Edge via the Windows registry `App Paths` key or a fixed list of well-known install paths (`crates/cdp/src/launch.rs`). There is no way to force a specific binary. That's fine for normal use (find whatever's installed), but it's exactly wrong for "run the suite against Chrome Beta specifically" -- Beta installs to a different path than Stable on both platforms, and a container could plausibly have both Stable/Chromium and Beta present at once, so path-based auto-discovery would need channel-disambiguation logic this change doesn't otherwise need.

## Decision 1: An explicit env-var override, not a Beta-aware `BrowserKind` variant

Two ways to get `aib` running against Beta specifically:
1. Teach discovery about a `Beta` channel (new `BrowserKind` variant or a channel field), with its own well-known paths per platform.
2. Add a single escape hatch: `AIB_CHROME_PATH` forces an exact binary, full stop.

Went with (2). A channel-aware `BrowserKind` is a real feature with real design surface (does `resolve_headless_browser` need channel awareness too? does the CLI need a `--channel` flag? what about Edge Beta/Dev?) that nothing in this change's actual goal -- "run the existing suite against a specific Chrome binary in CI" -- needs. `AIB_CHROME_PATH` is channel-agnostic on purpose: it works identically for Beta, Dev, Canary, or any other binary someone wants to test against later, without this change having to enumerate and special-case each one. If a real product need for a `--channel` flag / multi-channel discovery shows up later, it can be layered on top; this override doesn't foreclose it.

Failure mode: if `AIB_CHROME_PATH` is set but doesn't point at a real file, discovery returns an error immediately rather than silently falling back to auto-discovery. Silent fallback would defeat the entire point in CI -- a typo'd path would make the "Beta" job quietly test Stable/Chromium instead, and nobody would notice until an actual Beta-only regression shipped undetected.

## Decision 2: A separate Dockerfile, not a build-arg on the existing one

`docker/Dockerfile` installs Debian's `chromium` package via `apt-get`. Google Chrome Beta is a different package from a different (Google's own) apt repository, not a version/channel argument to the same install step -- getting it requires adding Google's signing key and repo first. Parameterizing the existing Dockerfile with an `ARG CHANNEL=stable` that branches between "install Debian chromium" and "add Google's repo + install google-chrome-beta" is possible but makes the common path (every regular `run-tests.sh` invocation) pay for a conditional it never takes. A second, small Dockerfile that only exists for this one job is simpler to read and doesn't touch the existing, working image at all.

## Decision 3: What "verified" means here, honestly

Two different things are being shipped, with two different verification stories:
- The `AIB_CHROME_PATH` override and the Chrome-Beta Docker image: fully live-verified. The override is exercised by pointing it at the already-discovered installed Chrome and confirming discovery returns exactly that path (proves the override plumbing works without needing Beta installed for the unit test itself); the Docker image is actually built, actually installs `google-chrome-beta` from Google's repo, and the full `cargo test --workspace` suite actually runs inside it against that real Beta binary.
- The GitHub Actions YAML: cannot be live-verified, because this repo has no git remote yet. It's hand-written to valid, conventional GitHub Actions syntax and checked with a local YAML parser (`python3 -c "import yaml; yaml.safe_load(open(...))"`) to at least rule out syntax errors, but "does GitHub's scheduler actually trigger it, does the runner actually have Docker, does the job actually go green" is unverified and stays unverified until this repo has somewhere to push it.

This distinction is called out explicitly in `proposal.md` and will be repeated in `PROPOSAL.md`'s roadmap entry for this change, rather than letting "shipped" quietly imply "observed running," which it doesn't.

## Bugs found during implementation

**`resolve_headless_browser` ignored the override entirely (found before shipping, via reading the call graph, not via a failing test).** The original plan only patched `discover_browsers()`. But `Session::launch`'s default path calls `resolve_headless_browser(BrowserPreference::Auto)`, which tries the cached/downloaded `chrome-headless-shell` *before* ever calling `discover_browsers()` -- and headless is the default for this project's entire test suite, which is exactly what the Chrome-Beta Docker job runs. Setting `AIB_CHROME_PATH` would have silently done nothing for nearly every real invocation. Fixed by checking the override first in `resolve_headless_browser` too, ahead of the shell.

**A genuine test race, caught by `cargo test --workspace`, not by the isolated `cargo test -p cdp launch::` runs used while iterating.** Two separate test functions both called `std::env::set_var("AIB_CHROME_PATH", ...)` / `remove_var` -- one covering `discover_browsers()`, one covering `resolve_headless_browser()`. Rust's default test harness runs `#[test]`/`#[tokio::test]` functions concurrently on separate threads within the same process; `std::env::set_var` is process-global, unsynchronized mutable state. Running only the `cdp` crate's tests in isolation happened not to trigger the interleaving, but the full-workspace run did: one test's `remove_var` fired while the other test's `resolve_headless_browser` call was still in flight, so it silently fell through to the *actual* managed chrome-headless-shell instead of the override, and the assertion failed comparing the wrong two paths. Fixed by merging both into one test function, so nothing else touches the var while it's in play -- not by adding a `#[serial]`-style dependency, since a single project-owned env var didn't justify a new crate. Confirmed fixed by rerunning `cargo test --workspace` twice more in a row with no failures.
