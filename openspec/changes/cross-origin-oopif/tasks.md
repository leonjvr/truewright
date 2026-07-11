# Tasks: cross-origin-oopif

## 1. CDP protocol layer
- [ ] 1.1 New `crates/cdp/src/protocol/dom.rs`: `DOM.enable`, `DOM.getFrameOwner({frameId}) -> {backendNodeId}`, `DOM.resolveNode({backendNodeId}) -> {object: RemoteObject}`
- [ ] 1.2 `crates/cdp/src/protocol/runtime.rs`: `Runtime.callFunctionOn` command; `RemoteObject` gains `object_id: Option<String>`
- [ ] 1.3 `crates/cdp/src/protocol/page.rs`: `Page.getFrameTree` command + `FrameTreeNode`/`Frame` (id, parentId, url) response types
- [ ] 1.4 `crates/cdp/src/launch.rs`: `launch_with_flags(browser, profile_name, headless, extra_args: &[&str])`, with `launch()` delegating to it with `&[]` (needed so a test can force `--site-per-process` without adding product-facing surface)

## 2. `crates/cdp/src/ops.rs`
- [ ] 2.1 `Page::frame_tree()` wraps `Page.getFrameTree`
- [ ] 2.2 `Page::ref_for_frame_owner(frame_id)`: `DOM.enable` (idempotent) → `DOM.getFrameOwner` → `DOM.resolveNode` → `Runtime.callFunctionOn` invoking the same get-or-create-ref logic as `refFor` in `walker.js`; returns `Ok(None)` (not an error) if the frame is gone/not found rather than failing the whole snapshot

## 3. `crates/engine/assets/walker.js`
- [ ] 3.1 `walkIframe` calls `refFor(el)` for every iframe element (same-origin and cross-origin alike), so the correlation step in Rust can find the matching tree node by ref

## 4. `crates/engine/src/session.rs`
- [ ] 4.1 New OOPIF registry (`Mutex<HashMap<String /* frame_id */, cdp::ops::Page>>`), populated/pruned in the existing `refresh_attached_pages` poll (widening its target-type filter, not adding a second consumer of the same discovery event stream)
- [ ] 4.2 Frame-tag assignment (`f1`, `f2`, ... first-seen order, stable per `Session`), stored alongside the registry
- [ ] 4.3 `snapshot()`: after the top-level walk, call `Page::frame_tree()` to scope which registered OOPIFs belong to the active page, correlate each via `ref_for_frame_owner`, find the matching tree node by ref, run `walker.js` again in the OOPIF's own session, namespace its refs (`f<N>:e<M>`), splice as that node's children
- [ ] 4.4 `resolve_ref`: recognize the `f<N>:` prefix, evaluate `resolve.js` in the OOPIF's own session for the local point, add the correlated top-level ref's own on-screen position (center minus half width/height) to translate to top-level viewport coordinates

## 5. Test infrastructure
- [ ] 5.1 `crates/engine/tests/support/http_server.rs`: `TestServer::url_on(host, path)` (or equivalent) to construct a URL on an arbitrary hostname, not just `127.0.0.1`
- [ ] 5.2 A fixture serving a cross-*site* iframe: top page on one `.localhost` subdomain, iframe `src` on a different one, with real interactive content inside the iframe
- [ ] 5.3 New test `crates/engine/tests/cross_origin_oopif_flow.rs`, launched with `--site-per-process` via the new `launch_with_flags`, that asserts the snapshot shows the iframe's real content (namespaced refs) and that clicking a namespaced ref actually fires the OOPIF content's own click handler

## 6. Verification
- [ ] 6.1 Live-verify against the real fixture: content visible in snapshot, click lands correctly (confirmed by the handler actually firing, not just that the CDP call didn't error)
- [ ] 6.2 Confirm the existing `data:`-URL cross-origin fixture (`iframe_snapshot_flow.rs`) still renders the "not inspectable" leaf -- this change must not regress the case it doesn't cover
- [ ] 6.3 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container, both green
- [ ] 6.4 `cargo clippy --workspace --all-targets` clean

## 7. Wrap-up
- [ ] 7.1 Update README (OOPIF support, ref namespacing, what's still deferred)
- [ ] 7.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 7.3 `openspec archive cross-origin-oopif -y`
- [ ] 7.4 Three commits: Propose, Implement, Sync-specs-and-archive
