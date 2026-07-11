# Tasks: popup-auto-attach

## 1. CDP protocol layer (`crates/cdp/src/protocol/target.rs`)
- [x] 1.1 `Target.setDiscoverTargets` command (`discover` param) -- replaces the originally planned `Target.setAutoAttach`, which live testing showed conflicts with this project's own explicit-attach flow (see design.md addendum)
- [x] 1.2 `Target.targetCreated` event (`targetInfo { targetId, type, url, title, browserContextId }`)
- [x] 1.3 `Target.targetDestroyed` event (`targetId`)
- [x] 1.4 `Target.getTargetInfo` command, for `list_pages` to report a fresh URL/title per attached page

## 2. `crates/cdp/src/ops.rs`
- [x] 2.1 `Browser::new_context` calls `Target.setDiscoverTargets` on the browser-level session right after subscribing to `targetCreated`/`targetDestroyed`
- [x] 2.2 `BrowserContext` exposes non-blocking-drain event streams for `Target.targetCreated`/`Target.targetDestroyed`, subscribed once at context creation
- [x] 2.3 `BrowserContext::attach_existing_target(target_id)`: the shared explicit `attachToTarget` + enable-domains logic, used by both `new_page` (for a target it just created) and `Session`'s discovery-driven popup attach (for a target it merely noticed)

## 3. `engine::Session` (`crates/engine/src/session.rs`)
- [x] 3.1 Replace the single `page: cdp::ops::Page` field with `pages: Mutex<HashMap<String, cdp::ops::Page>>`, `active_target_id: Mutex<String>`, `primary_target_id: String`
- [x] 3.2 `active_page()` helper resolving the current active `Page`; convert every existing method off the bare `page` field access
- [x] 3.3 `refresh_attached_pages()`: non-blocking drain of pending `targetCreated`/`targetDestroyed` events (filtered to this session's own `browserContextId` and `target_type == "page"`), updating the registry; detaching the active page falls back to `primary_target_id` via a shared `forget_page` helper
- [x] 3.4 `list_pages() -> Result<Vec<PageInfo>>` (calls `refresh_attached_pages` first, then `Target.getTargetInfo` per known page; self-heals via `forget_page` if a page vanishes mid-query -- see design.md addendum bugs #2/#3)
- [x] 3.5 `switch_page(page_id: &str) -> Result<()>`, erroring clearly (typed `EngineError::UnknownPage`) if `page_id` isn't a currently-known page

## 4. MCP integration (`crates/mcp/src/lib.rs`)
- [x] 4.1 `browser_list_pages()` tool
- [x] 4.2 `browser_switch_page(page_id)` tool
- [x] 4.3 Wire `EngineError::UnknownPage` through `map_engine_err`

## 5. Verification
- [x] 5.1 New fixtures `popup_opener.html` (button that calls `window.open()`) and `popup_target.html` (a page that closes itself via `window.close()` when its own button is clicked)
- [x] 5.2 Integration test: click the button, confirm the new page is attached and listed but not active, switch to it, confirm subsequent snapshot/action targets it, click its own close-button, confirm fallback to the primary page; plus a second test for `switch_page` on an unknown id
- [x] 5.3 Confirmed every pre-existing integration test still passes unmodified after the `self.page` → `active_page()` refactor (no behavior change for the single-page case)
- [x] 5.4 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container both green (repeated runs to rule out the project's known resource-contention flake pattern, not a regression)
- [x] 5.5 Corrected three real CDP wiring assumptions that live testing disproved (see design.md addendum): `setAutoAttach` replaced with `setDiscoverTargets`; `list_pages` made self-healing against a transient Chrome-internal target; active-page fallback shared between the event-driven and self-heal paths

## 6. Wrap-up
- [ ] 6.1 Update README with `browser_list_pages`/`browser_switch_page` usage
- [ ] 6.2 Update PROPOSAL.md's Phase 5 row
- [ ] 6.3 `openspec archive popup-auto-attach -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
