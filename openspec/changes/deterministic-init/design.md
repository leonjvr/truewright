# Design: Deterministic Init (Init Scripts + Seeded Randomness)

## Context

First slice of Phase 3b (the remainder of Phase 3 after `network-mocking`), chosen because init scripts are the shared delivery mechanism the other Phase 3b primitives (virtual clock, and potentially others) would build on, and seeded randomness is the simplest possible real consumer of it — small enough to implement and verify in one pass, unlike the virtual clock (deferred to its own change; see proposal.md).

## Goals / Non-Goals

**Goals:** register JS that provably runs before a page's own scripts (not just before an agent happens to call `Runtime.evaluate` afterward); a built-in seeded `Math.random` override, reproducible across separate navigations with the same seed.

**Non-Goals:** virtual clock (own change); overriding `crypto.getRandomValues`/`crypto.randomUUID` (out of scope for v1 — the proposal is specifically about `Math.random`, and Web Crypto is a different, security-flavored API most apps don't reach for during ordinary rendering); removing/clearing a registered init script (scripts persist for the session's lifetime, same as the load-once walker/resolve/train scripts already do; no stated need to remove one mid-session).

## Decisions

1. **`Page.addScriptToEvaluateOnNewDocument`, not `Runtime.evaluate`.** Everything injected into a page so far (`walker.js`, `resolve.js`, `train.js`) runs via `Runtime.evaluate` *after* the page has already loaded — fine for reading page state, wrong for influencing a page's own initialization, which has already happened by then. `addScriptToEvaluateOnNewDocument` registers a script that runs before any of the page's own scripts, on every subsequent navigation — new CDP surface for this codebase.
2. **Init scripts accumulate; no removal API in v1.** Each `browser_add_init_script` call adds one more script to the list, all run in registration order before every subsequent navigation. Matches how the walker/resolve/train scripts already work (load once, live for the session) and avoids designing a removal/lifecycle API nothing has asked for yet.
3. **`browser_seed_randomness(seed)` is sugar over `add_init_script`, not a separate mechanism.** It generates a small, deterministic PRNG (mulberry32 — a handful of lines, adequate quality for making test runs reproducible, explicitly not cryptographic) seeded from the given value, overrides `Math.random`, and registers that as an init script through the exact same path a caller-supplied script would use. One delivery mechanism, two producers (the caller, or this convenience).
4. **Scripts must be registered *before* navigating for their effect to apply** — an inherent property of `addScriptToEvaluateOnNewDocument` (it affects subsequent loads, not whatever's currently loaded). Documented as the expected call order (`browser_add_init_script`/`browser_seed_randomness`, then `browser_navigate`), not treated as a limitation to work around.

## Risks / Trade-offs

- [mulberry32 is not cryptographically secure] → deliberate and fine — the goal is deterministic test runs, not security. Documented in the injected script's own comment so it's never mistaken for a security primitive.
- [An init script that throws breaks the page load it was meant to help] → same class of risk any injected script carries (the existing walker/resolve/train scripts have the same property); the caller authored the script, so a bug in it is the caller's to fix, not something this engine can validate ahead of time.

## Migration Plan

Purely additive — two new MCP tools, no existing tool's behavior changes.

## Open Questions

None blocking.
