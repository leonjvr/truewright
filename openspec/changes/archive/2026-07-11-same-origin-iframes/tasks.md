# Tasks: same-origin-iframes

## 1. `assets/walker.js`
- [x] 1.1 Dedicated `<iframe>` branch in `walk()`: attempt `contentDocument` access; if accessible and loaded, recurse into `.body` under a wrapping `iframe` role node
- [x] 1.2 Fallback leaf for cross-origin (`contentDocument` is `null`) and not-yet-loaded (`contentDocument.body` is `null`) cases, with a descriptive `name` (including `src` when cross-origin)
- [x] 1.3 Depth limit continues to apply across frame recursion (already inherited via the existing `depth` parameter)

## 2. `assets/resolve.js`
- [x] 2.1 After the existing visibility/`scrollIntoView` logic, walk `window.frameElement` up from the resolved element's own window, scrolling each ancestor iframe into view too
- [x] 2.2 Accumulate each ancestor iframe's own `getBoundingClientRect()` offset into the final `x`/`y`, so the reported coordinates are top-level-page-relative

## 3. Verification
- [x] 3.1 New fixture `iframes.html`: a same-origin case via `<iframe srcdoc="...">` with an interactive button inside; a cross-origin case via `<iframe src="data:text/html,...">`
- [x] 3.2 Integration test: snapshot shows the same-origin iframe's interactive content with a usable ref, and shows the cross-origin iframe as an explicit not-inspectable leaf
- [x] 3.3 Integration test: `browser_click` on a ref resolved from inside the same-origin iframe actually registers on that element (asserts the iframe's own click handler fired, not just that the CDP call didn't error) -- live-verified the coordinate accumulation actually works on the first attempt, no correction needed
- [x] 3.4 Confirmed every pre-existing snapshot/action test still passes unmodified (no iframe present -> code path never triggered)
- [x] 3.5 `cargo test --workspace` on host (repeated runs, all green modulo the project's known pre-existing `true_input_flow` resource-contention flake) and `bash docker/run-tests.sh` in the container, both green

## 4. Wrap-up
- [x] 4.1 Update README noting same-origin iframe support and the cross-origin boundary behavior
- [x] 4.2 Update PROPOSAL.md's Phase 5 roadmap
- [x] 4.3 `openspec archive same-origin-iframes -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [x] 4.4 Three commits: Propose, Implement, Sync-specs-and-archive
