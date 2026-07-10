# doctor-cli

## ADDED Requirements

### Requirement: Process-tree memory measurement
`aib doctor` SHALL measure and report the total resident memory (RSS) of the launched browser's full process tree (the root browser process plus its child renderer/GPU/utility processes) while a page is loaded, per browser checked. The JSON report MUST include this as a numeric field so before/after comparisons between browser binaries are measurable.

#### Scenario: Tree memory in the report
- **WHEN** `aib doctor --json` completes a cycle against a browser
- **THEN** the browser's report includes a `tree_rss_mb` field with a value greater than zero

#### Scenario: Comparing binaries
- **WHEN** doctor runs against both a managed headless-shell and an installed browser
- **THEN** each browser entry carries its own `tree_rss_mb`, allowing a direct comparison
