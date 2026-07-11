# Tasks: YAML Runner

## 1. Dependency

- [x] 1.1 Add `serde_yaml` to workspace `Cargo.toml` and `crates/engine/Cargo.toml`

## 2. Runner

- [x] 2.1 `crates/engine/src/yaml_runner.rs`: `Step` enum (`Navigate(String)`, `Click(String)`, `Type { r#ref, text, submit }`, `Press(String)`, `WaitFor { text, timeout_ms }`, `Assert { text, present }`), externally-tagged via `#[serde(rename_all = "snake_case")]`
- [x] 2.2 `run(session: &Session, source: &str) -> Result<RunSummary>`: parses YAML, executes each step via the corresponding `Session` method in order, fail-fast on the first error
- [x] 2.3 `Session::run_yaml(source: &str) -> Result<RunSummary>` (thin wrapper)
- [x] 2.4 `export(entries: &[TraceEntry]) -> String`: filters to `Action` entries, maps each back to a `Step`, serializes as YAML

## 3. MCP integration

- [x] 3.1 `browser_run_yaml(source)` tool
- [x] 3.2 `browser_export_yaml(name)` tool: loads the named trace's JSONL, exports to YAML text

## 4. Verification

- [x] 4.1 Host: full suite green
- [x] 4.2 Integration test: a hand-written YAML script (navigate + type + click + assert) against the form fixture runs successfully end to end
- [x] 4.3 Integration test: a script with a deliberately-failing assert step stops there and reports it, without running subsequent steps
- [x] 4.4 Integration test: a real captured trace (navigate + type + click) exports to YAML, and running that exported YAML reproduces the same end state
- [x] 4.5 Container: `bash docker/run-tests.sh` green

## 5. Wrap-up

- [x] 5.1 README documents the YAML step format, `browser_run_yaml`, and `browser_export_yaml`
- [x] 5.2 `openspec validate yaml-runner` clean; sync specs; archive
