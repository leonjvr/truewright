# Tasks: Action Trace

## 1. Engine

- [ ] 1.1 `crates/engine/src/console.rs`: `TraceEntry::Action { text, timestamp_ms }`
- [ ] 1.2 `ConsoleCapture::start` accepts (and populates) a shared `action_trace_sink` cell; `stop()` clears it before persisting
- [ ] 1.3 `Session` holds `action_trace_sink: Arc<Mutex<Option<Arc<Mutex<Vec<TraceEntry>>>>>>`; a private helper appends an action entry if a sink is set (no-op otherwise)
- [ ] 1.4 `navigate`/`click_with`/`type_text_with`/`press` call the helper with a one-line summary

## 2. Verification

- [ ] 2.1 Host: full suite green
- [ ] 2.2 Integration test: start a trace, navigate + click + type, stop the trace, assert the JSONL contains action entries for each in the correct chronological position relative to any console output those actions provoked
- [ ] 2.3 Integration test: actions performed with no trace active don't error and (implicitly) produce no trace file
- [ ] 2.4 Container: `bash docker/run-tests.sh` green

## 3. Wrap-up

- [ ] 3.1 README documents that `browser_console_start` also captures actions, with an example trace line
- [ ] 3.2 `openspec validate action-trace` clean; sync specs; archive
