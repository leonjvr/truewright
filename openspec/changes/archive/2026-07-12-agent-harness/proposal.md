## Why

The first two changes built a provider-agnostic LLM client and subscription auth -- neither drives anything. This is the third change and the actual core deliverable of the four-part agent-harness effort: `aib` gains its own tool-calling loop, so a user-configured LLM role can drive `aib`'s browser session autonomously to complete a natural-language task, without any outer MCP host in the loop at all. `aib agent "task"` is the first user-facing way to actually use everything `llm-providers`/`oauth-subscription-auth` built.

## What Changes

- **New `crates/agent`**: `Harness` (the step loop), a small tool surface (`crates/agent/src/tools.rs`) executed directly against `engine::Session` -- not through the private MCP `#[tool]` wrappers, which return MCP content blocks and aren't meant to be called from Rust code -- context pruning, capability-flag-routed vision, and Markdown skills.
- **Tool surface**: `navigate`/`snapshot`/`click`/`type`/`press`/`wait_for`/`assert`/`screenshot`/`list_pages`/`switch_page`/`run_yaml`, plus two harness-only termination tools (`task_complete`/`task_failed`). Deliberately smaller than the full MCP surface -- human-motion params, `true_input`, recording, training, network cassettes, virtual clock, init scripts, and console traces stay MCP-only; an autonomous driver doesn't need them and each one widens the blast radius for something running without a human confirming every action.
- **Step loop**: build messages -> call the driver -> execute every tool call in that turn sequentially -> repeat. Recoverable engine errors (stale ref, wait timeout, failed assertion, unknown page/key) come back as tool-result **text**, not a harness-level failure -- the model sees the error and can adapt, which is the actual point of a loop instead of a fixed script. Two consecutive turns with no tool call and no termination call end the run as stuck, not silently forever.
- **Context pruning**: every snapshot-shaped tool result older than the most recent `max_retained_snapshots` (config, default 2) gets rewritten to a short placeholder before the next driver call -- snapshots are the single biggest thing that bloats context on a weak-context driver, and this is checked before every turn, not just occasionally.
- **Vision routing, governed by the role's own capability flag**: a `screenshot` result goes inline (as an image on a follow-up `user` message -- OpenAI-compat's `tool` role doesn't reliably accept image parts) when `driver.vision == true`; otherwise it's sent to the configured `vision` role for interpretation and the **text** comes back as the tool result. No vision role configured and a non-vision driver gets a clear tool-result error explaining how to fix it, not a confusing failure.
- **Skills**: plain Markdown files (optional front-matter stripped, not yet parsed for anything), resolved project-local-first (`./.aib/skills/`) then per-user (`<data-dir>/aib/skills/`) then any configured extra directories -- an unresolvable skill name errors clearly rather than silently running without it.
- **`aib agent "task"` CLI**: one-shot, live step progress (human-readable or `--json` for scripting), `--skill`/`--driver`/`--vision`/`--max-steps` overrides, a fixed `aib-agent` profile by default (deterministic, matching stdio-MCP's own posture). `--driver`/`--vision` accept `<provider>/<model>` directly, bypassing `[roles.*]` entirely -- which needed a small `llm-providers` extension (`Config::resolve_provider_model`, refactored out of `resolve_role`'s existing provider-lookup logic) so a one-off run doesn't need a `[roles.driver]` table just to point at an already-configured provider with a different model.

**Explicitly out of scope (deferred), and why:**
- **MCP integration.** `browser_run_task` and screenshot-interpret mode are `mcp-task-delegation` (change 4) -- this change only needs `Harness`/`SharedSession` to exist and work, not be wired into the MCP server yet.
- **Interactive/chat mode.** One-shot autonomous only, per the plan's confirmed scope; `--interactive` is not built.
- **Streaming token output.** Progress is per-step/per-tool-call (`AgentEvent`), not per-token -- consistent with `llm-providers`' own "non-streaming per turn" decision.

## Capabilities

### New Capabilities
- `agent-harness`: the tool-calling loop, tool surface, context pruning, vision routing, skills, and the `aib agent` CLI.

### Modified Capabilities
- `llm-providers`: `Config::resolve_provider_model` (ad-hoc provider/model resolution bypassing `[roles.*]`), extracted from `resolve_role`'s existing logic.

## Impact

- `crates/agent` (new): `harness.rs`, `tools.rs`, `prompt.rs`, `skills.rs`, `vision.rs`, `types.rs`, `error.rs`, `lib.rs`.
- `crates/llm/src/config.rs`: `resolve_provider_model`, `resolve_client_for_provider` (shared helper both `resolve_role` and the new method use); new `LlmError::UnknownProviderDirect`.
- `src/agent_cmd.rs` (new), `src/main.rs`: `aib agent` subcommand.
- No changes to `crates/cdp`, `crates/engine`, or `crates/mcp`.
