# Tasks: Deterministic Init (Init Scripts + Seeded Randomness)

## 1. CDP protocol

- [x] 1.1 `crates/cdp/src/protocol/page.rs`: `AddScriptToEvaluateOnNewDocument`/`AddScriptToEvaluateOnNewDocumentParams` (`source`), `AddScriptToEvaluateOnNewDocumentResponse` (`identifier`)
- [x] 1.2 `cdp::ops::Page::add_init_script(source) -> Result<String>`

## 2. Engine

- [x] 2.1 `crates/engine/assets/seeded_random.js`: mulberry32 PRNG, overrides `Math.random`, seed value interpolated in by the caller
- [x] 2.2 `Session::add_init_script(source: &str) -> Result<()>`
- [x] 2.3 `Session::seed_randomness(seed: u64) -> Result<()>`: builds the seeded-random script with the given seed and registers it via `add_init_script`

## 3. MCP integration

- [x] 3.1 `browser_add_init_script(source)`, `browser_seed_randomness(seed)` tools

## 4. Verification

- [x] 4.1 Host: full suite green
- [x] 4.2 Integration test: an init script's effect is visible to a fixture page's own first-run inline script (proves before-page-scripts ordering, not just before-agent-action)
- [x] 4.3 Integration test: same seed reproduces an identical `Math.random()` sequence across two separate navigations; different seeds produce different sequences
- [x] 4.4 Container: `bash docker/run-tests.sh` green

## 5. Wrap-up

- [x] 5.1 README documents `browser_add_init_script`/`browser_seed_randomness` and the "register before navigate" call order
- [x] 5.2 `openspec validate deterministic-init` clean; sync specs; archive
