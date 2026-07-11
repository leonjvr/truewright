# Design: Browser Assert

## Context

First of Phase 3b-v's two remaining pieces (the YAML runner is the other, and will consume this as one of its step kinds). Distinct from `browser_wait_for`, which already exists but solves a different problem (poll until true or time out) — `browser_assert` is "check right now, fail clearly if false," the shape an actual test assertion needs.

## Goals / Non-Goals

**Goals:** an immediate (non-polling) text-presence/absence check against the current snapshot; a genuine tool-level failure signal (not a protocol error, not a silently-ignorable text response) an agent can branch on; integrates with the existing action trace.

**Non-Goals:** assertion kinds beyond text presence/absence (element state, attributes, counts) — v1 covers the dominant case; a way to assert without taking a fresh snapshot first (staleness would defeat the point of an assertion).

## Decisions

1. **A tool-level error result (`CallToolResult::error`), not a protocol-level `McpError`, on assertion failure.** rmcp distinguishes the two deliberately: a protocol error (`Err(ErrorData)`) is for "the tool call itself was malformed or the server malfunctioned" and typically renders opaquely to the calling agent; `CallToolResult::error` is for "the tool ran correctly and the logical result is a failure," with the content the agent actually sees. An assertion failing is squarely the second case — it's not a bug in the call, it's the check doing its job.
2. **Immediate, not polling — this is not `browser_wait_for` with a different name.** `browser_wait_for` already exists for "eventually true"; `browser_assert` checks the CURRENT snapshot once, synchronously. A caller who wants "wait, then assert" composes the two existing tools rather than `browser_assert` growing a timeout parameter that would just re-implement `browser_wait_for`.
3. **Logged into the active trace as an action entry, pass or fail, reusing `action-trace`'s existing mechanism.** An assertion is itself an "agent did something" event worth seeing in context alongside clicks/types/console output — no new tracing infrastructure needed, `Session::log_action` already exists for exactly this.
4. **Text presence/absence only, in v1.** Matches this project's consistent "no more sophistication than obviously needed" stance (human-motion's typing cadence, network-mocking's method+URL matching) — richer assertions (element `checked`/`disabled` state, attribute values) are a natural additive follow-up once real usage shows which ones are actually needed, not designed speculatively now.

## Risks / Trade-offs

- [Text-presence assertions are coarse — can't distinguish "the right element has this text" from "this text appears anywhere on the page"] → acceptable for v1, consistent with how `browser_wait_for` already works (same substring-match granularity); a ref-scoped assertion is a reasonable richer-assertion-kind candidate for a follow-up.

## Migration Plan

Purely additive — one new MCP tool, no existing tool's behavior changes.

## Open Questions

None blocking.
