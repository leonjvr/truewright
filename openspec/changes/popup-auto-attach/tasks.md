# Tasks: popup-auto-attach

## 1. CDP protocol layer (`crates/cdp/src/protocol/target.rs`)
- [ ] 1.1 `Target.setAutoAttach` command (`autoAttach`, `waitForDebuggerOnStart`, `flatten` params)
- [ ] 1.2 `Target.attachedToTarget` event (`sessionId`, `targetInfo { targetId, type, url, title, browserContextId }`, `waitingForDebugger`)
- [ ] 1.3 `Target.detachedFromTarget` event (`sessionId`, `targetId`)
- [ ] 1.4 `Target.getTargetInfo` command, for `list_pages` to report a fresh URL/title per attached page

## 2. `crates/cdp/src/ops.rs`
- [ ] 2.1 `BrowserContext::new_page` calls `Target.setAutoAttach` on the newly created page's own session right after enabling `Page`/`Runtime`
- [ ] 2.2 `Page` exposes event streams for `Target.attachedToTarget`/`Target.detachedFromTarget` on its own session (reusing the existing `events::<E>()` machinery)
- [ ] 2.3 A way to construct a `Page` from an already-attached `sessionId` + `targetId` (factor the shared bit of `new_page`'s attach/enable-domains logic out so both the explicit and auto-attach paths use it)

## 3. `engine::Session` (`crates/engine/src/session.rs`)
- [ ] 3.1 Replace the single `page: cdp::ops::Page` field with `pages: Mutex<HashMap<String, cdp::ops::Page>>`, `active_target_id: Mutex<String>`, `primary_target_id: String`
- [ ] 3.2 `active_page()` helper resolving the current active `Page`; convert every existing method off the bare `page` field access
- [ ] 3.3 `refresh_attached_pages()`: non-blocking drain of the pending `attachedToTarget`/`detachedFromTarget` events, updating the registry; detaching the active page falls back to `primary_target_id`
- [ ] 3.4 `list_pages() -> Result<Vec<PageInfo>>` (calls `refresh_attached_pages` first, then `Target.getTargetInfo` per known page)
- [ ] 3.5 `switch_page(page_id: &str) -> Result<()>`, erroring clearly (typed `EngineError`) if `page_id` isn't a currently-known page

## 4. MCP integration (`crates/mcp/src/lib.rs`)
- [ ] 4.1 `browser_list_pages()` tool
- [ ] 4.2 `browser_switch_page(page_id)` tool
- [ ] 4.3 Wire the new `EngineError` variant(s) through `map_engine_err`

## 5. Verification
- [ ] 5.1 New fixture with a `target="_blank"` link (and/or a `window.open()` button) for exercising a real popup open
- [ ] 5.2 Integration test: click the link, confirm the new page is attached and listed but not active, switch to it, confirm subsequent snapshot/action targets it, close it (or navigate it to close), confirm fallback to the primary page
- [ ] 5.3 Confirm every pre-existing integration test still passes unmodified after the `self.page` → `active_page()` refactor (no behavior change for the single-page case)
- [ ] 5.4 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container (no real-OS side effects this time, unlike `true-user-input` -- should run fine in both)
- [ ] 5.5 Correct any CDP session/event-routing assumption that live testing disproves (see design.md Risks), documenting the correction in a design.md addendum per this project's established convention

## 6. Wrap-up
- [ ] 6.1 Update README with `browser_list_pages`/`browser_switch_page` usage
- [ ] 6.2 Update PROPOSAL.md's Phase 5 row (or split it further if more Phase 5 slices follow)
- [ ] 6.3 `openspec archive popup-auto-attach -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 6.4 Three commits: Propose, Implement, Sync-specs-and-archive
