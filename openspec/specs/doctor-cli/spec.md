# doctor-cli

## Purpose

`truewright doctor`: a self-check command that proves the attach→navigate→evaluate→screenshot cycle works against every installed Chromium browser on the machine, with per-step reporting and command round-trip latency.

## Requirements

### Requirement: Full-cycle self-check per browser
`truewright doctor` SHALL, for every discovered browser: launch/attach with an isolated profile, create a browser context and page, navigate to a known page, evaluate JavaScript, capture a screenshot, and tear down. Each step's pass/fail MUST be reported per browser, and failures MUST NOT abort checks for other browsers.

#### Scenario: Healthy machine
- **WHEN** `truewright doctor` runs on a machine with Chrome and Edge installed
- **THEN** it prints a per-browser checklist with all steps passing and exits with code 0

#### Scenario: One browser broken
- **WHEN** Edge fails to launch but Chrome completes all steps
- **THEN** the report shows Edge failed at the launch step with the underlying error, Chrome passes, and the exit code is non-zero

### Requirement: Latency measurement
`truewright doctor` SHALL measure command round-trip latency by issuing at least 100 lightweight commands (e.g., `Runtime.evaluate("1+1")`) and SHALL report p50 and p95 per browser. The Phase 0 exit criterion is p50 < 5 ms.

#### Scenario: Latency report
- **WHEN** the doctor cycle completes against a browser
- **THEN** the output includes p50 and p95 round-trip latency in milliseconds, and flags p50 ≥ 5 ms as a warning

### Requirement: Machine-readable output
`truewright doctor --json` SHALL emit the full report as a single JSON object (browsers, steps, errors, latency percentiles) on stdout, suitable for CI assertions.

#### Scenario: JSON mode
- **WHEN** `truewright doctor --json` runs
- **THEN** stdout parses as valid JSON containing per-browser step results and latency numbers, with human-readable text suppressed

### Requirement: Process-tree memory measurement
`truewright doctor` SHALL measure and report the total resident memory (RSS) of the launched browser's full process tree (the root browser process plus its child renderer/GPU/utility processes) while a page is loaded, per browser checked. The JSON report MUST include this as a numeric field so before/after comparisons between browser binaries are measurable.

#### Scenario: Tree memory in the report
- **WHEN** `truewright doctor --json` completes a cycle against a browser
- **THEN** the browser's report includes a `tree_rss_mb` field with a value greater than zero

#### Scenario: Comparing binaries
- **WHEN** doctor runs against both a managed headless-shell and an installed browser
- **THEN** each browser entry carries its own `tree_rss_mb`, allowing a direct comparison
