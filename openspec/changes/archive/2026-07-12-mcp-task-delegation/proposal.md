# Proposal: mcp-task-delegation

## Why

The agent-harness change gave `aib` its own driving loop, reachable from the `aib agent "task"` CLI. It is not yet reachable from the `browser` MCP server -- an *outer* MCP host model (e.g. Claude, or any other agent host that already has `aib` configured as an MCP server) has no way to hand off a whole sub-task to `aib`'s own configured driver, and a host model with no vision of its own has no way to get a text description of a screenshot instead of the raw image. This is the fourth and last of the four changes scoped in the original agent-harness plan.

## What Changes

- New `browser_run_task { task, guidance?, skills?, max_steps? }` MCP tool: runs `agent::Harness::run_task` over the *same* `engine::Session` every other `browser_*` tool on this server already shares, so the outer host and the inner driver work the same live browser, never two. Requires `[roles.driver]` configured in `aib`'s config; fails with a clear invalid-params error, not a crash, when it isn't. Returns a compact step transcript plus the final PASS/FAIL outcome as tool result text; a failed task is a tool-level error result, matching how `browser_assert`/`browser_run_yaml` already report failure.
- `browser_screenshot` gains optional `interpret`/`guidance` params: `interpret: true` routes the captured image through `Harness::interpret_image` (agent-harness's own vision-routing entry point, already written with this exact call site in mind) and returns text instead of the image -- for a host model with no vision of its own. `browser_record_stop`'s preview frame gets the same optional params. Omitting `interpret` keeps today's exact behavior (returns the raw image), so this is purely additive.
- `AibTools` gains an optional `agent: Option<AgentConfig>` (driver/vision roles resolved into a `Harness`, plus skill search directories) -- `None` whenever no `[roles.driver]` is configured, which every browser-only tool must keep working under exactly as it does today.
- `aib mcp` gains a `--config <path>` flag (matching `aib agent`/`aib llm ping`'s own) and loads `llm::Config` once at startup to build that optional agent config; a config file that fails to *parse* is a startup warning to stderr (browser tools still work), while simply having no `[roles.driver]` configured is the ordinary, silent, zero-LLM-setup case -- not a warning.
- `crates/mcp` gains a dependency on `crates/agent`.

## Out of scope

- No new CLI surface -- this change is MCP-only; `aib agent` already exists from the prior change.
- No change to how `browser_run_task`'s inner driver call is itself authenticated/configured -- it reuses `[roles.driver]`/`[roles.vision]` exactly as `aib agent` does, not a separate MCP-specific role.
- No support for running two `browser_run_task` calls concurrently against one session -- like every other `browser_*` tool, callers are expected not to interleave; documented in the tool description, not enforced beyond the existing per-call session mutex (which already serializes, it just doesn't queue-and-explain).

## Capabilities

- Modified: `mcp-server` (new `browser_run_task` tool; `browser_screenshot`/`browser_record_stop` gain optional vision-interpretation params)

## Impact

- `crates/mcp`: new dependency on `agent`; `AibTools` gains an `agent: Option<AgentConfig>` field and a `.with_agent(...)` builder method (existing constructors/call sites unchanged); new `browser_run_task` tool; `browser_screenshot`/`browser_record_stop` request structs gain optional fields.
- `crates/llm`: `Client`, `CompatClient`, `ResponsesClient`, `RoleClient` gain `#[derive(Clone)]` (all fields were already cheaply cloneable -- `reqwest::Client`, `String`, and the already-`Clone` `CredentialSource`) so a shared `Harness` can be cheaply copied with one field overridden (the MCP-specific lower default `max_steps`) without touching `Harness::run_task`'s signature.
- `crates/agent`: `Harness` gains `#[derive(Clone)]` for the same reason.
- `src/mcp.rs`: `run`/`router`/`run_http` gain an `agent: Option<mcp_server::AgentConfig>` parameter, threaded through to each `AibTools` (the streamable-HTTP factory clones it cheaply per session, same as every other `Arc`-backed field already does).
- `src/main.rs`: `Command::Mcp` gains `--config`; loads config and resolves `[roles.driver]`/`[roles.vision]` once at startup into the optional `AgentConfig`.
- Existing `tests/mcp_http_flow.rs` call sites updated for the new parameter (all pass `None`, preserving today's exact behavior); new tests exercise the delegation path with a real local LLM stub.
