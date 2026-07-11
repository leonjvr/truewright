# Tasks: same-origin-iframes

## 1. `assets/walker.js`
- [ ] 1.1 Dedicated `<iframe>` branch in `walk()`: attempt `contentDocument` access; if accessible and loaded, recurse into `.body` under a wrapping `iframe` role node
- [ ] 1.2 Fallback leaf for cross-origin (`contentDocument` is `null`) and not-yet-loaded (`contentDocument.body` is `null`) cases, with a descriptive `name` (including `src` when cross-origin)
- [ ] 1.3 Depth limit continues to apply across frame recursion (already inherited via the existing `depth` parameter)

## 2. `assets/resolve.js`
- [ ] 2.1 After the existing visibility/`scrollIntoView` logic, walk `window.frameElement` up from the resolved element's own window, scrolling each ancestor iframe into view too
- [ ] 2.2 Accumulate each ancestor iframe's own `getBoundingClientRect()` offset into the final `x`/`y`, so the reported coordinates are top-level-page-relative

## 3. Verification
- [ ] 3.1 New fixture(s): a same-origin case via `<iframe srcdoc="...">` with an interactive element inside; a cross-origin case via `<iframe src="data:text/html,...">`
- [ ] 3.2 Integration test: snapshot shows the same-origin iframe's interactive content with a usable ref, and shows the cross-origin iframe as an explicit not-inspectable leaf
- [ ] 3.3 Integration test: `browser_click`/`browser_type` on a ref resolved from inside the same-origin iframe actually registers on that element (real click, not just coordinate-math asserted in isolation) -- live-verify the coordinate accumulation actually works, per design.md's stated risk, before trusting the JS math alone
- [ ] 3.4 Confirm every pre-existing snapshot/action test still passes unmodified (no iframe present -> code path never triggered, byte-for-byte same output)
- [ ] 3.5 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container

## 4. Wrap-up
- [ ] 4.1 Update README noting same-origin iframe support and the cross-origin boundary behavior
- [ ] 4.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 4.3 `openspec archive same-origin-iframes -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 4.4 Three commits: Propose, Implement, Sync-specs-and-archive
