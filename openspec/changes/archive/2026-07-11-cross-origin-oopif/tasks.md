# Tasks: cross-origin-oopif

## 1. CDP protocol layer
- [x] 1.1 New `crates/cdp/src/protocol/dom.rs`: `DOM.enable`, `DOM.getFrameOwner({frameId}) -> {backendNodeId}`, `DOM.resolveNode({backendNodeId}) -> {object: RemoteObject}`
- [x] 1.2 `crates/cdp/src/protocol/runtime.rs`: `Runtime.callFunctionOn` command; `RemoteObject` gains `object_id: Option<String>`
- [x] 1.3 ~~`Page.getFrameTree` command~~ -- added, then removed again (see 4.3 note): live testing showed it doesn't report cross-process children at all, so the planned use for it never worked and the dead code was deleted rather than left behind
- [x] 1.4 `crates/cdp/src/launch.rs`: `launch_with_flags(browser, profile_name, headless, extra_args: &[&str])`, with `launch()` delegating to it with `&[]` (needed so a test can force `--site-per-process` without adding product-facing surface)

## 2. `crates/cdp/src/ops.rs`
- [x] 2.1 ~~`Page::frame_tree()`~~ -- added then removed, same reason as 1.3
- [x] 2.2 `Page::ref_for_frame_owner(frame_id)`: `DOM.enable` (idempotent) → `DOM.getFrameOwner` → `DOM.resolveNode` → `Runtime.callFunctionOn` invoking the same get-or-create-ref logic as `refFor` in `walker.js`; returns `Ok(None)` (not an error) if the frame is gone/not found rather than failing the whole snapshot

## 3. `crates/engine/assets/walker.js`
- [x] 3.1 `walkIframe` calls `refFor(el)` for every iframe element (same-origin and cross-origin alike), so the correlation step in Rust can find the matching tree node by ref

## 4. `crates/engine/src/session.rs`
- [x] 4.1 New OOPIF registry (`Mutex<HashMap<String /* frame_id */, OopifEntry>>`), populated/pruned in the existing `refresh_attached_pages` poll (widening its target-type filter, not adding a second consumer of the same discovery event stream)
- [x] 4.2 Frame-tag assignment (`f1`, `f2`, ... first-seen order, stable per `Session`), stored alongside the registry
- [x] 4.3 `snapshot()`: after the top-level walk, correlate each registered OOPIF via `ref_for_frame_owner` (this call succeeding, on this page's own session, IS the scope check -- `Page.getFrameTree` pre-filtering was tried first and, per live testing, doesn't work at all for cross-process children, so it was dropped rather than kept as dead weight), find the matching tree node by ref, run `walker.js` again in the OOPIF's own session, namespace its refs (`f<N>:e<M>`), splice as that node's children
- [x] 4.4 `resolve_ref`: recognize the `f<N>:` prefix, evaluate `resolve.js` in the OOPIF's own session for the local point, add the correlated top-level ref's own on-screen position (center minus half width/height) to translate to top-level viewport coordinates

## 5. Test infrastructure
- [x] 5.1 `crates/engine/tests/support/http_server.rs`: `TestServer::url_on(host, path)` plus `start_with(build_routes: impl FnOnce(u16) -> HashMap<...>)` (the top page's own body needs to reference this same server's port in its iframe `src`, which is only known after the listener binds)
- [x] 5.2 A fixture serving a cross-*site* iframe: top page on one `.localhost` subdomain, iframe `src` on a different one, with real interactive content inside the iframe (`oopif_top.html` / `oopif_inner.html`)
- [x] 5.3 New test `crates/engine/tests/cross_origin_oopif_flow.rs`, launched with `--site-per-process` via the new `launch_with_flags`, that asserts the snapshot shows the iframe's real content (namespaced refs) and that clicking a namespaced ref actually fires the OOPIF content's own click handler

## 6. Verification
- [x] 6.1 Live-verify against the real fixture: content visible in snapshot, click lands correctly (confirmed by the handler actually firing, not just that the CDP call didn't error) -- passed 4x in a row on host (stability check) plus directly re-run in the Docker container
- [x] 6.2 Confirm the existing `data:`-URL cross-origin fixture (`iframe_snapshot_flow.rs`) still renders the "not inspectable" leaf -- passed on both host and Docker runs, no regression
- [x] 6.3 `cargo test --workspace` on host (green, modulo two pre-existing, independently-confirmed resource-contention flakes -- `network_flow.rs`'s replay test and `true_input_flow.rs`'s live-SendInput test, both reproduced then passed clean in isolation, neither touching OOPIF code) and `bash docker/run-tests.sh` in the container (green, `aib doctor` reports `"ok": true`)
- [x] 6.4 `cargo clippy --workspace --all-targets` clean

## 7. Wrap-up
- [x] 7.1 Update README (OOPIF support, ref namespacing, what's still deferred)
- [x] 7.2 Update PROPOSAL.md's Phase 5 roadmap
- [x] 7.3 `openspec archive cross-origin-oopif -y`
- [x] 7.4 Three commits: Propose, Implement, Sync-specs-and-archive
