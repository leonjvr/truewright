## Why

Every determinism primitive Phase 3 has shipped so far (network mocking, init scripts, seeded randomness, virtual clock, console capture, action trace, assertions) makes a *single* test run reproducible — but there's still no way to *define* a test as a reusable, shareable artifact independent of whatever MCP conversation produced it. This is the last piece of Phase 3: a declarative YAML step format an agent (or a human) can write directly, and a runner that executes it — plus the reverse direction, exporting an already-captured action trace back into a runnable YAML script, so "record once, replay as a checked-in test" becomes possible.

## What Changes

- A YAML step schema covering the action set already built: `navigate`, `click`, `type` (ref/text/submit), `press`, `wait_for` (text/timeout_ms), `assert` (text/present). Each step is a single-key map (`- navigate: "https://..."`, `- type: {ref: e6, text: "..."}`), matching common step-list idioms (GitHub Actions, Ansible).
- `browser_run_yaml(source)`: parses and executes a YAML script's steps in order against the current session, stopping at the first failing step (fail-fast) and reporting which step failed and why.
- `browser_export_yaml(name)`: reads an already-saved console/action trace (from `console-capture`/`action-trace`) and converts its `action` entries into a runnable YAML script, skipping `console`/`exception` entries (observability, not replayable steps) — the "record once, get a script" direction.
- New dependency: `serde_yaml` (workspace-level, `engine` crate only) for YAML parsing/serialization. Noted as no longer actively maintained upstream but still the most widely-used, stable option for straightforward structural (de)serialization — the risk profile that actually matters here (a maintained fork exists if this ever needs revisiting).

**Explicitly out of scope:** `human_like`/`persona`/`trained_profile`/seed parameters on YAML `click`/`type` steps (v1 covers the instant-dispatch action set only, matching every other "no more sophistication than obviously needed" scope call this project has made — a natural additive follow-up); a CLI subcommand for running YAML scripts outside an MCP session (the MCP tool is the primary interface every other capability in this project has shipped through; a standalone `aib run` is a reasonable follow-up if a real need for CI-without-an-agent shows up); step-level retries or conditional branching (a script is a flat, ordered list of steps — YAML programming-language ambitions are explicitly not the goal).

## Capabilities

### New Capabilities
- `yaml-runner`: a declarative YAML step format for the existing browser-action set, a runner that executes it fail-fast, and an exporter that converts a captured action trace into a runnable script.

## Impact

- `Cargo.toml` (workspace), `crates/engine/Cargo.toml`: new `serde_yaml` dependency.
- `crates/engine/src/yaml_runner.rs` (new): `Step` enum (serde-tagged by variant name), `run(session, source) -> Result<RunSummary>`, `export(entries) -> String`.
- `crates/engine/src/session.rs`: `Session::run_yaml(source) -> Result<RunSummary>` (delegates each step to the existing action methods -- no new dispatch logic, just orchestration).
- `crates/mcp/src/lib.rs`: `browser_run_yaml(source)`, `browser_export_yaml(name)` tools.
- New fixture + integration test: a YAML script exercising navigate/type/click/assert against the form fixture runs successfully; a script with a deliberately-wrong assertion stops at that step and reports it; a real captured trace exports to YAML that, when run, reproduces the same end state.
