//! The step loop (agent-harness spec: "Step loop", "Context pruning",
//! "Vision routing", "Termination").

use crate::error::{AgentError, Result};
use crate::prompt::system_prompt;
use crate::skills::Skill;
use crate::tools::{execute_tool, tool_defs, ToolOutcome};
use crate::types::{AgentEvent, SharedSession, TaskOutcome};
use crate::vision;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use llm::{ChatRequest, Message, Part, RoleClient};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;

/// Two consecutive turns with no tool call and no termination call end the
/// run as stuck -- one nudge is given a chance to work before giving up.
const MAX_CONSECUTIVE_NUDGES: u32 = 2;

#[derive(Clone)]
pub struct Harness {
    pub driver: RoleClient,
    pub vision: Option<RoleClient>,
    pub max_steps: u32,
    /// Bounds each individual driver `complete()` call -- not the whole
    /// step (driver call + tool execution). Tool-execution-side hangs are
    /// bounded by `wait_for`'s own `timeout_ms` and the overall
    /// `task_timeout` instead; nesting a second timeout around tool
    /// execution too would risk racing against `wait_for`'s own timeout
    /// for no real benefit, since `task_timeout` already catches a
    /// runaway task.
    pub step_timeout: Duration,
    pub task_timeout: Duration,
    pub max_retained_snapshots: u32,
}

impl Harness {
    /// Runs `task` to completion (or failure/timeout/step-budget
    /// exhaustion). `events`, if given, receives one `AgentEvent` per
    /// step/tool-call/outcome -- the CLI renders these live; MCP
    /// accumulates them into a transcript.
    pub async fn run_task(
        &self,
        session: &SharedSession,
        task: &str,
        skills: &[Skill],
        guidance: Option<&str>,
        events: Option<Sender<AgentEvent>>,
    ) -> Result<TaskOutcome> {
        let start = Instant::now();
        let system = system_prompt(task, skills, guidance);
        let mut messages = vec![Message::system(system), Message::user(task)];
        let tools = tool_defs();

        let mut step: u32 = 0;
        let mut consecutive_nudges: u32 = 0;
        // Indices into `messages` of tool-result entries whose text is a
        // full page snapshot -- pruned to the most recent
        // `max_retained_snapshots` before each driver call, since
        // snapshots are the single biggest thing bloating context on a
        // weak-context driver.
        let mut snapshot_message_indices: Vec<usize> = Vec::new();

        loop {
            if start.elapsed() > self.task_timeout {
                return Err(AgentError::TaskTimeout);
            }
            step += 1;
            if step > self.max_steps {
                return Err(AgentError::MaxStepsExceeded(self.max_steps));
            }
            emit(
                &events,
                AgentEvent::Step {
                    n: step,
                    max: self.max_steps,
                },
            )
            .await;

            prune_snapshots(
                &mut messages,
                &snapshot_message_indices,
                self.max_retained_snapshots,
            );

            let req = ChatRequest {
                model: String::new(),
                messages: messages.clone(),
                tools: tools.clone(),
            };
            let resp = tokio::time::timeout(self.step_timeout, self.driver.complete(req))
                .await
                .map_err(|_| AgentError::TaskTimeout)??;

            if resp.message.tool_calls.is_empty() {
                consecutive_nudges += 1;
                messages.push(resp.message);
                if consecutive_nudges >= MAX_CONSECUTIVE_NUDGES {
                    return Err(AgentError::NoProgress);
                }
                messages.push(Message::user(
                    "Respond with a tool call, or call task_complete/task_failed to end the task.",
                ));
                continue;
            }
            consecutive_nudges = 0;

            let tool_calls = resp.message.tool_calls.clone();
            messages.push(resp.message);

            for tc in &tool_calls {
                emit(
                    &events,
                    AgentEvent::ToolCall {
                        name: tc.name.clone(),
                        args_summary: truncate_chars(&tc.arguments, 200),
                    },
                )
                .await;

                if tc.name == "task_complete" {
                    let summary = extract_field(&tc.arguments, "summary").unwrap_or_default();
                    emit(
                        &events,
                        AgentEvent::Done {
                            passed: true,
                            summary: summary.clone(),
                        },
                    )
                    .await;
                    return Ok(TaskOutcome {
                        passed: true,
                        summary,
                        steps_used: step,
                    });
                }
                if tc.name == "task_failed" {
                    let reason = extract_field(&tc.arguments, "reason").unwrap_or_default();
                    emit(
                        &events,
                        AgentEvent::Done {
                            passed: false,
                            summary: reason.clone(),
                        },
                    )
                    .await;
                    return Ok(TaskOutcome {
                        passed: false,
                        summary: reason,
                        steps_used: step,
                    });
                }

                match execute_tool(session, &tc.name, &tc.arguments).await? {
                    ToolOutcome::Text(text) => {
                        let ok =
                            !text.starts_with("error:") && !text.starts_with("assertion failed");
                        emit(
                            &events,
                            AgentEvent::ToolResult {
                                name: tc.name.clone(),
                                ok,
                                summary: truncate_chars(&text, 300),
                            },
                        )
                        .await;
                        let idx = messages.len();
                        let is_snapshot_like =
                            matches!(tc.name.as_str(), "navigate" | "snapshot" | "wait_for");
                        messages.push(Message::tool_result(tc.id.clone(), text));
                        if is_snapshot_like {
                            snapshot_message_indices.push(idx);
                        }
                    }
                    ToolOutcome::Screenshot { bytes, guidance } => {
                        self.handle_screenshot(&mut messages, tc, bytes, guidance, &events)
                            .await?;
                    }
                }
            }
        }
    }

    async fn handle_screenshot(
        &self,
        messages: &mut Vec<Message>,
        tc: &llm::ToolCall,
        bytes: Vec<u8>,
        guidance: Option<String>,
        events: &Option<Sender<AgentEvent>>,
    ) -> Result<()> {
        if self.driver.vision {
            // The OpenAI-compat `tool` role doesn't reliably accept image
            // parts across providers -- the image goes on a follow-up
            // `user` message instead, a standard workaround.
            messages.push(Message::tool_result(
                tc.id.clone(),
                "screenshot follows".to_string(),
            ));
            let b64 = STANDARD.encode(&bytes);
            messages.push(Message::user_with_image(
                "Screenshot:",
                Part::ImagePngB64(b64),
            ));
            emit(
                events,
                AgentEvent::ToolResult {
                    name: tc.name.clone(),
                    ok: true,
                    summary: "screenshot (inline)".to_string(),
                },
            )
            .await;
            return Ok(());
        }

        match &self.vision {
            Some(vision_role) => {
                let interpretation =
                    vision::interpret_screenshot(vision_role, &bytes, guidance.as_deref()).await?;
                emit(
                    events,
                    AgentEvent::Vision {
                        chars: interpretation.chars().count(),
                    },
                )
                .await;
                emit(
                    events,
                    AgentEvent::ToolResult {
                        name: tc.name.clone(),
                        ok: true,
                        summary: truncate_chars(&interpretation, 300),
                    },
                )
                .await;
                messages.push(Message::tool_result(tc.id.clone(), interpretation));
            }
            None => {
                let msg = "error: no vision role configured for this driver, which has no vision \
                           of its own. Rely on snapshot() instead, or ask the user to add \
                           [roles.vision] to their config."
                    .to_string();
                emit(
                    events,
                    AgentEvent::ToolResult {
                        name: tc.name.clone(),
                        ok: false,
                        summary: msg.clone(),
                    },
                )
                .await;
                messages.push(Message::tool_result(tc.id.clone(), msg));
            }
        }
        Ok(())
    }

    /// Interprets a single image outside of a task run -- MCP's
    /// `browser_screenshot(interpret: true)` (mcp-task-delegation) calls
    /// this directly.
    pub async fn interpret_image(&self, bytes: &[u8], guidance: Option<&str>) -> Result<String> {
        let Some(vision_role) = &self.vision else {
            return Err(AgentError::NoVisionRole);
        };
        vision::interpret_screenshot(vision_role, bytes, guidance).await
    }
}

async fn emit(events: &Option<Sender<AgentEvent>>, event: AgentEvent) {
    if let Some(tx) = events {
        let _ = tx.send(event).await;
    }
}

/// Truncates on a `char` boundary (not a byte index, which could split a
/// multi-byte UTF-8 sequence and panic) for progress-log summaries.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}... ({} chars total)", s.chars().count())
}

fn extract_field(raw_args: &str, field: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw_args)
        .ok()?
        .get(field)?
        .as_str()
        .map(str::to_string)
}

/// Rewrites every snapshot-shaped tool result older than the most recent
/// `keep` to a short placeholder, in place.
fn prune_snapshots(messages: &mut [Message], snapshot_indices: &[usize], keep: u32) {
    let keep = keep as usize;
    if snapshot_indices.len() <= keep {
        return;
    }
    let elide_count = snapshot_indices.len() - keep;
    for &idx in &snapshot_indices[..elide_count] {
        if let Some(msg) = messages.get_mut(idx) {
            msg.content = vec![Part::Text(
                "[snapshot elided -- call snapshot() again if needed]".to_string(),
            )];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_chars_is_utf8_safe_and_leaves_short_strings_alone() {
        assert_eq!(truncate_chars("short", 10), "short");
        let long = "a".repeat(500);
        let truncated = truncate_chars(&long, 10);
        assert!(truncated.starts_with("aaaaaaaaaa"));
        assert!(truncated.contains("500 chars total"));
        // A multi-byte-character string truncated mid-way must not panic.
        let emoji = "😀".repeat(50);
        let _ = truncate_chars(&emoji, 10);
    }

    #[test]
    fn prune_snapshots_elides_all_but_the_most_recent() {
        let mut messages = vec![
            Message::tool_result("c1", "snapshot 1"),
            Message::tool_result("c2", "snapshot 2"),
            Message::tool_result("c3", "snapshot 3"),
        ];
        prune_snapshots(&mut messages, &[0, 1, 2], 1);
        assert!(messages[0].text().contains("elided"));
        assert!(messages[1].text().contains("elided"));
        assert_eq!(messages[2].text(), "snapshot 3");
    }

    #[test]
    fn prune_snapshots_is_a_no_op_when_under_the_limit() {
        let mut messages = vec![Message::tool_result("c1", "snapshot 1")];
        prune_snapshots(&mut messages, &[0], 2);
        assert_eq!(messages[0].text(), "snapshot 1");
    }

    #[test]
    fn extract_field_reads_a_string_field_and_handles_malformed_json() {
        assert_eq!(
            extract_field(r#"{"summary":"done"}"#, "summary"),
            Some("done".to_string())
        );
        assert_eq!(extract_field("not json", "summary"), None);
        assert_eq!(extract_field(r#"{"other":"x"}"#, "summary"), None);
    }
}
