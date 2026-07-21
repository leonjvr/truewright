//! The tool surface exposed to the driver model, and the executor that
//! dispatches a model's tool call directly against `engine::Session`
//! (agent-harness spec: "Tool surface"). Deliberately a small subset of
//! what the MCP server exposes -- human-motion params, true_input,
//! recording, training, network cassettes, virtual clock, init scripts,
//! and console traces stay MCP-only; an autonomous driver doesn't need
//! them and they widen the blast radius for something running without a
//! human in the loop for every action.

use crate::error::{AgentError, Result};
use crate::types::SharedSession;
use llm::ToolDef;
use serde_json::json;
use std::time::Duration;

const DEFAULT_WAIT_TIMEOUT_MS: u64 = 5000;

/// The tool definitions sent to the driver model. `task_complete`/
/// `task_failed` are harness-only -- they never touch `engine::Session`,
/// they end the loop (`execute_tool` never sees them; the loop checks for
/// them before dispatching).
pub fn tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "navigate".to_string(),
            description: "Navigates to a URL and returns the page's accessibility snapshot.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        },
        ToolDef {
            name: "snapshot".to_string(),
            description: "Returns the current page's accessibility snapshot as ref-annotated text (e.g. a button shown as `[e6]`). Actions do not auto-return a snapshot; call this again after an action that may have changed the page.".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "click".to_string(),
            description: "Clicks the element identified by ref (e.g. \"e6\", taken from a snapshot).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "ref": { "type": "string" } },
                "required": ["ref"]
            }),
        },
        ToolDef {
            name: "right_click".to_string(),
            description: "Right-clicks (secondary-clicks) the element identified by ref, firing a native contextmenu event -- use this to open a page's own right-click / context menu.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "ref": { "type": "string" } },
                "required": ["ref"]
            }),
        },
        ToolDef {
            name: "type".to_string(),
            description: "Clicks the element identified by ref to focus it, then types text. Set submit true to press Enter afterward.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "ref": { "type": "string" },
                    "text": { "type": "string" },
                    "submit": { "type": "boolean" }
                },
                "required": ["ref", "text"]
            }),
        },
        ToolDef {
            name: "press".to_string(),
            description: "Presses a single named key on whatever currently has focus (e.g. \"Enter\", \"Tab\", \"Escape\").".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "key": { "type": "string" } },
                "required": ["key"]
            }),
        },
        ToolDef {
            name: "wait_for".to_string(),
            description: "Polls the page until the given text appears (or times out), then returns the snapshot. Use this instead of guessing a fixed delay.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "timeout_ms": { "type": "integer", "description": "Defaults to 5000." }
                },
                "required": ["text"]
            }),
        },
        ToolDef {
            name: "assert".to_string(),
            description: "Immediately checks whether text is present (or absent, if present=false) in the current snapshot -- no polling, unlike wait_for. Fails as a real error if the assertion doesn't hold, which you should treat as a test failure.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "present": { "type": "boolean", "description": "Defaults to true." }
                },
                "required": ["text"]
            }),
        },
        ToolDef {
            name: "screenshot".to_string(),
            description: "Takes a screenshot of the current page. If you can't see images, this is interpreted by a vision model instead and you receive a text description.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guidance": { "type": "string", "description": "Optional: what to look for, used when the screenshot is vision-interpreted." }
                }
            }),
        },
        ToolDef {
            name: "list_pages".to_string(),
            description: "Lists every currently attached page (tabs/popups opened as a side effect of an action) and which one is active.".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "switch_page".to_string(),
            description: "Switches which page subsequent actions target.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "page_id": { "type": "string" } },
                "required": ["page_id"]
            }),
        },
        ToolDef {
            name: "run_yaml".to_string(),
            description: "Runs a declarative YAML script (navigate/click/type/press/wait_for/assert steps) against the current session, stopping at the first failing step.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "source": { "type": "string" } },
                "required": ["source"]
            }),
        },
        ToolDef {
            name: "task_complete".to_string(),
            description: "Call this when the task has been completed successfully. This ends the run -- always call it (or task_failed) rather than just stopping.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "summary": { "type": "string" } },
                "required": ["summary"]
            }),
        },
        ToolDef {
            name: "task_failed".to_string(),
            description: "Call this when the task cannot be completed. This ends the run.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "reason": { "type": "string" } },
                "required": ["reason"]
            }),
        },
    ]
}

/// What executing one tool call produced. `Screenshot` is carried
/// separately from `Text` because the harness's vision-routing decision
/// (inline image vs. route to the vision role) happens one layer up, in
/// `harness.rs` -- the executor's job is just "run the action," not
/// "decide how to represent the result to the model."
pub enum ToolOutcome {
    Text(String),
    Screenshot {
        bytes: Vec<u8>,
        guidance: Option<String>,
    },
}

/// Executes one tool call against the shared session. Recoverable
/// per-action failures (a stale ref, a wait timeout, a failed assertion,
/// an unknown page/key, malformed argument JSON) come back as
/// `Ok(ToolOutcome::Text("error: ..."))`, not `Err` -- the model can see
/// the error and adapt, which is the whole point of an agent loop over a
/// one-shot script. `Err(AgentError)` is reserved for harness-level
/// problems: the session is gone, or the model called a tool name that
/// isn't in `tool_defs()` at all (which a well-behaved provider shouldn't
/// do, since it was given that exact list, but isn't impossible).
pub async fn execute_tool(
    session: &SharedSession,
    name: &str,
    raw_args: &str,
) -> Result<ToolOutcome> {
    let args: serde_json::Value = match serde_json::from_str(raw_args) {
        Ok(v) => v,
        Err(e) => {
            return Ok(ToolOutcome::Text(format!(
                "error: invalid arguments JSON: {e}"
            )))
        }
    };

    let guard = session.0.lock().await;
    let Some(s) = guard.as_ref() else {
        return Err(AgentError::NoSession);
    };

    let outcome = match name {
        "navigate" => {
            let Some(url) = args["url"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"url\"".to_string(),
                ));
            };
            text_or_error(s.navigate(url).await)
        }
        "snapshot" => text_or_error(s.snapshot().await),
        "click" => {
            let Some(r#ref) = args["ref"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"ref\"".to_string(),
                ));
            };
            match s.click(r#ref).await {
                Ok(()) => ToolOutcome::Text("ok".to_string()),
                Err(e) => ToolOutcome::Text(format!(
                    "error: {e}. Take a fresh snapshot if the ref may be stale."
                )),
            }
        }
        "right_click" => {
            let Some(r#ref) = args["ref"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"ref\"".to_string(),
                ));
            };
            match s.right_click(r#ref).await {
                Ok(()) => ToolOutcome::Text("ok".to_string()),
                Err(e) => ToolOutcome::Text(format!(
                    "error: {e}. Take a fresh snapshot if the ref may be stale."
                )),
            }
        }
        "type" => {
            let Some(r#ref) = args["ref"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"ref\"".to_string(),
                ));
            };
            let Some(text) = args["text"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"text\"".to_string(),
                ));
            };
            let submit = args["submit"].as_bool().unwrap_or(false);
            match s.type_text(r#ref, text, submit).await {
                Ok(()) => ToolOutcome::Text("ok".to_string()),
                Err(e) => ToolOutcome::Text(format!(
                    "error: {e}. Take a fresh snapshot if the ref may be stale."
                )),
            }
        }
        "press" => {
            let Some(key) = args["key"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"key\"".to_string(),
                ));
            };
            match s.press(key).await {
                Ok(()) => ToolOutcome::Text("ok".to_string()),
                Err(e) => ToolOutcome::Text(format!("error: {e}")),
            }
        }
        "wait_for" => {
            let Some(text) = args["text"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"text\"".to_string(),
                ));
            };
            let timeout_ms = args["timeout_ms"]
                .as_u64()
                .unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
            text_or_error(s.wait_for(text, Duration::from_millis(timeout_ms)).await)
        }
        "assert" => {
            let Some(text) = args["text"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"text\"".to_string(),
                ));
            };
            let present = args["present"].as_bool().unwrap_or(true);
            match s.assert_text(text, present).await {
                Ok(()) => ToolOutcome::Text("assertion passed".to_string()),
                Err(e) => ToolOutcome::Text(format!("assertion failed: {e}")),
            }
        }
        "screenshot" => {
            let guidance = args["guidance"].as_str().map(str::to_string);
            match s.screenshot().await {
                Ok(bytes) => ToolOutcome::Screenshot { bytes, guidance },
                Err(e) => ToolOutcome::Text(format!("error: {e}")),
            }
        }
        "list_pages" => match s.list_pages().await {
            Ok(pages) => {
                let text = pages
                    .iter()
                    .map(|p| {
                        format!(
                            "{} {} {:?}{}",
                            p.page_id,
                            p.url,
                            p.title,
                            if p.active { " (active)" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ToolOutcome::Text(if text.is_empty() {
                    "no pages".to_string()
                } else {
                    text
                })
            }
            Err(e) => ToolOutcome::Text(format!("error: {e}")),
        },
        "switch_page" => {
            let Some(page_id) = args["page_id"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"page_id\"".to_string(),
                ));
            };
            match s.switch_page(page_id).await {
                Ok(()) => ToolOutcome::Text("ok".to_string()),
                Err(e) => ToolOutcome::Text(format!("error: {e}")),
            }
        }
        "run_yaml" => {
            let Some(source) = args["source"].as_str() else {
                return Ok(ToolOutcome::Text(
                    "error: missing required argument \"source\"".to_string(),
                ));
            };
            match s.run_yaml(source).await {
                Ok(summary) => ToolOutcome::Text(format!("{summary:?}")),
                Err(e) => ToolOutcome::Text(format!("error: {e}")),
            }
        }
        other => return Err(AgentError::UnknownTool(other.to_string())),
    };

    Ok(outcome)
}

fn text_or_error(result: engine::Result<String>) -> ToolOutcome {
    match result {
        Ok(text) => ToolOutcome::Text(text),
        Err(e) => ToolOutcome::Text(format!("error: {e}")),
    }
}
