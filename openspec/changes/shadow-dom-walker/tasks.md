# Tasks: shadow-dom-walker

## 1. `assets/walker.js`
- [ ] 1.1 In `walk()`, when `el.shadowRoot` is non-null, source children from `shadowRoot.children` instead of `el.children`
- [ ] 1.2 When walking a `<slot>` element, use `slot.assignedElements()` if non-empty, else fall back to the slot's own `children`
- [ ] 1.3 No wrapping marker node for shadow content (deliberate asymmetry vs. the `iframe` role wrapper -- see design.md)

## 2. `assets/resolve.js`
- [ ] 2.1 No change expected -- confirm via the live test in section 3 rather than assuming

## 3. Verification
- [ ] 3.1 New fixture: a custom element (`customElements.define`) with an open shadow root containing a directly-rendered interactive element AND a default `<slot>` receiving projected light-DOM content
- [ ] 3.2 Integration test: snapshot shows both the shadow tree's own content and the slotted/projected content, each with a usable ref
- [ ] 3.3 Integration test: `browser_click` on a ref resolved from inside the shadow root actually registers (real click, not just tree-structure assertions) -- live-verify `resolve.js` genuinely needs no changes rather than assuming from the design doc alone
- [ ] 3.4 Confirm every pre-existing snapshot/action test still passes unmodified (no shadow root present -> code path never triggered)
- [ ] 3.5 `cargo test --workspace` on host and `bash docker/run-tests.sh` in the container

## 4. Wrap-up
- [ ] 4.1 Update README noting shadow-DOM support and the closed-shadow-root limitation
- [ ] 4.2 Update PROPOSAL.md's Phase 5 roadmap
- [ ] 4.3 `openspec archive shadow-dom-walker -y`, fix any "Purpose: TBD" placeholder in the synced spec
- [ ] 4.4 Three commits: Propose, Implement, Sync-specs-and-archive
