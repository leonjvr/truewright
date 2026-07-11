# Tasks: shadow-dom-walker

## 1. `assets/walker.js`
- [x] 1.1 In `walk()`, when `el.shadowRoot` is non-null, source children from `shadowRoot.children` instead of `el.children`
- [x] 1.2 When walking a `<slot>` element, use `slot.assignedElements()` if non-empty, else fall back to the slot's own `children`
- [x] 1.3 No wrapping marker node for shadow content (deliberate asymmetry vs. the `iframe` role wrapper -- see design.md)

## 2. `assets/resolve.js`
- [x] 2.1 No change needed -- confirmed via the live test in section 3, not just assumed

## 3. Verification
- [x] 3.1 New fixture `shadow_dom.html`: a custom element (`customElements.define`) with an open shadow root containing a directly-rendered button AND a default `<slot>` receiving a projected `<h2>` heading
- [x] 3.2 Integration test: snapshot shows both the shadow tree's own content and the slotted/projected content, each with a usable ref
- [x] 3.3 Integration test: `browser_click` on a ref resolved from inside the shadow root actually registers (asserts the shadow tree's own click handler fired) -- live-verified on the first attempt, `resolve.js` genuinely needed no changes
- [x] 3.4 Confirmed every pre-existing snapshot/action test still passes unmodified (no shadow root present -> code path never triggered)
- [x] 3.5 `cargo test --workspace` on host (fully green) and `bash docker/run-tests.sh` in the container (green on re-run after one instance of the project's known pre-existing `network_flow.rs` resource-contention flake)

## 4. Wrap-up
- [x] 4.1 Update README noting shadow-DOM support and the closed-shadow-root limitation
- [x] 4.2 Update PROPOSAL.md's Phase 5 roadmap
- [x] 4.3 `openspec archive shadow-dom-walker -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [x] 4.4 Three commits: Propose, Implement, Sync-specs-and-archive
