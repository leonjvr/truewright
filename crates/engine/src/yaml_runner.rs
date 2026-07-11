//! Declarative YAML step execution and trace export (yaml-runner spec).
//!
//! `Step` has a hand-written `Deserialize`/`Serialize` rather than a
//! derived one: serde_yaml represents a newtype enum variant (a
//! single-field tuple variant, e.g. `Navigate(String)`) as a YAML *tag*
//! (`!navigate "url"`) by default, not the `{navigate: "url"}` single-key
//! map this format's whole point is to read like (GitHub Actions/Ansible
//! step-list style) -- found by the first integration test run actually
//! parsing hand-written YAML, not assumed. See design.md's addendum.

use crate::console::TraceEntry;
use crate::error::{EngineError, Result};
use crate::session::Session;
use serde::de::Error as DeError;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::time::Duration;

const DEFAULT_WAIT_FOR_MS: u64 = 5000;

#[derive(Debug, Clone)]
pub enum Step {
    Navigate(String),
    Click(String),
    Type {
        r#ref: String,
        text: String,
        submit: bool,
    },
    Press(String),
    WaitFor {
        text: String,
        timeout_ms: Option<u64>,
    },
    Assert {
        text: String,
        present: bool,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct TypePayload {
    r#ref: String,
    text: String,
    #[serde(default)]
    submit: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct WaitForPayload {
    text: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssertPayload {
    text: String,
    #[serde(default = "default_true")]
    present: bool,
}

fn default_true() -> bool {
    true
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: HashMap<String, serde_yaml::Value> = Deserialize::deserialize(deserializer)?;
        if map.len() != 1 {
            return Err(DeError::custom(
                "each step must have exactly one key (e.g. `navigate: \"...\"` or `click: e6`)",
            ));
        }
        let (key, value) = map.into_iter().next().unwrap();
        match key.as_str() {
            "navigate" => Ok(Step::Navigate(
                serde_yaml::from_value(value).map_err(DeError::custom)?,
            )),
            "click" => Ok(Step::Click(
                serde_yaml::from_value(value).map_err(DeError::custom)?,
            )),
            "type" => {
                let p: TypePayload = serde_yaml::from_value(value).map_err(DeError::custom)?;
                Ok(Step::Type {
                    r#ref: p.r#ref,
                    text: p.text,
                    submit: p.submit,
                })
            }
            "press" => Ok(Step::Press(
                serde_yaml::from_value(value).map_err(DeError::custom)?,
            )),
            "wait_for" => {
                let p: WaitForPayload = serde_yaml::from_value(value).map_err(DeError::custom)?;
                Ok(Step::WaitFor {
                    text: p.text,
                    timeout_ms: p.timeout_ms,
                })
            }
            "assert" => {
                let p: AssertPayload = serde_yaml::from_value(value).map_err(DeError::custom)?;
                Ok(Step::Assert {
                    text: p.text,
                    present: p.present,
                })
            }
            other => Err(DeError::custom(format!(
                "unknown step kind {other:?} (expected one of: navigate, click, type, press, wait_for, assert)"
            ))),
        }
    }
}

impl Serialize for Step {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Step::Navigate(url) => map.serialize_entry("navigate", url)?,
            Step::Click(r#ref) => map.serialize_entry("click", r#ref)?,
            Step::Type { r#ref, text, submit } => map.serialize_entry(
                "type",
                &TypePayload {
                    r#ref: r#ref.clone(),
                    text: text.clone(),
                    submit: *submit,
                },
            )?,
            Step::Press(key) => map.serialize_entry("press", key)?,
            Step::WaitFor { text, timeout_ms } => map.serialize_entry(
                "wait_for",
                &WaitForPayload {
                    text: text.clone(),
                    timeout_ms: *timeout_ms,
                },
            )?,
            Step::Assert { text, present } => map.serialize_entry(
                "assert",
                &AssertPayload {
                    text: text.clone(),
                    present: *present,
                },
            )?,
        }
        map.end()
    }
}

fn step_kind(step: &Step) -> &'static str {
    match step {
        Step::Navigate(_) => "navigate",
        Step::Click(_) => "click",
        Step::Type { .. } => "type",
        Step::Press(_) => "press",
        Step::WaitFor { .. } => "wait_for",
        Step::Assert { .. } => "assert",
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Script {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    steps: Vec<Step>,
}

#[derive(Debug)]
pub struct RunSummary {
    pub steps_run: usize,
    pub total_steps: usize,
}

async fn run_step(session: &Session, step: &Step) -> Result<()> {
    match step {
        Step::Navigate(url) => {
            session.navigate(url).await?;
        }
        Step::Click(r#ref) => session.click(r#ref).await?,
        Step::Type { r#ref, text, submit } => session.type_text(r#ref, text, *submit).await?,
        Step::Press(key) => session.press(key).await?,
        Step::WaitFor { text, timeout_ms } => {
            session
                .wait_for(text, Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_WAIT_FOR_MS)))
                .await?;
        }
        Step::Assert { text, present } => session.assert_text(text, *present).await?,
    }
    Ok(())
}

/// Parses and executes a YAML script's steps in order, using the same
/// `Session` methods a live MCP call would use (yaml-runner spec:
/// "Declarative YAML step execution"). Stops at the first failing step.
// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
pub async fn run(session: &Session, source: &str) -> Result<RunSummary> {
    let script: Script = serde_yaml::from_str(source)
        .map_err(|e| EngineError::YamlRunner(format!("failed to parse YAML script: {e}")))?;

    let total_steps = script.steps.len();
    for (i, step) in script.steps.iter().enumerate() {
        if let Err(e) = run_step(session, step).await {
            return Err(EngineError::YamlStepFailed {
                step_number: i + 1,
                total_steps,
                step_kind: step_kind(step).to_string(),
                message: e.to_string(),
            });
        }
    }

    Ok(RunSummary {
        steps_run: total_steps,
        total_steps,
    })
}

/// Strips a leading Rust-`Debug`-quoted string (`"..."`, with `\"`/`\\`
/// escapes) from the front of `s`, returning the unescaped content and
/// whatever's left after the closing quote.
fn take_debug_quoted(s: &str) -> Option<(String, &str)> {
    let rest = s.strip_prefix('"')?;
    let bytes = rest.as_bytes();
    let mut i = 0;
    let mut escaped = false;
    let mut out = String::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if escaped {
            match c {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                other => out.push(other),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some((out, &rest[i + 1..]));
        } else {
            out.push(c);
        }
        i += 1;
    }
    None
}

/// Reconstructs a `Step` from one `action-trace`/`browser-assert` entry's
/// existing text summary (design.md Decision #4: reuses the exact same
/// summary format those changes already produce, rather than inventing a
/// second serialization for the same data). `wait_for` is not currently
/// logged as an action (out of `action-trace`'s original scope), so it
/// can't round-trip through export -- an accepted, documented gap, not a
/// bug.
fn action_to_step(text: &str) -> Option<Step> {
    if let Some(rest) = text.strip_prefix("navigate ") {
        return Some(Step::Navigate(rest.to_string()));
    }
    if let Some(rest) = text.strip_prefix("click ") {
        return Some(Step::Click(rest.to_string()));
    }
    if let Some(rest) = text.strip_prefix("type ") {
        let mut parts = rest.splitn(2, ' ');
        let r#ref = parts.next()?.to_string();
        let (text, _) = take_debug_quoted(parts.next()?)?;
        return Some(Step::Type {
            r#ref,
            text,
            submit: false,
        });
    }
    if let Some(rest) = text.strip_prefix("press ") {
        return Some(Step::Press(rest.to_string()));
    }
    if let Some(rest) = text.strip_prefix("assert text=") {
        let (text, remainder) = take_debug_quoted(rest)?;
        let present = remainder.trim_start().strip_prefix("present=")?.starts_with("true");
        return Some(Step::Assert { text, present });
    }
    None
}

/// Converts a captured trace's `Action` entries into a runnable YAML
/// script, skipping `Console`/`Exception` entries -- they're observability,
/// not replayable steps (yaml-runner spec: "Trace export to a runnable
/// YAML script").
#[allow(clippy::result_large_err)]
pub fn export(entries: &[TraceEntry]) -> Result<String> {
    let steps: Vec<Step> = entries
        .iter()
        .filter_map(|e| match e {
            TraceEntry::Action { text, .. } => action_to_step(text),
            _ => None,
        })
        .collect();

    let script = Script { name: None, steps };
    serde_yaml::to_string(&script)
        .map_err(|e| EngineError::YamlRunner(format!("failed to serialize YAML: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_to_step_round_trips_each_kind() {
        assert!(matches!(
            action_to_step("navigate https://example.com"),
            Some(Step::Navigate(url)) if url == "https://example.com"
        ));
        assert!(matches!(
            action_to_step("click e6"),
            Some(Step::Click(r)) if r == "e6"
        ));
        assert!(matches!(
            action_to_step(r#"type e6 "hello@example.com""#),
            Some(Step::Type { r#ref, text, submit: false }) if r#ref == "e6" && text == "hello@example.com"
        ));
        assert!(matches!(
            action_to_step("press Enter"),
            Some(Step::Press(k)) if k == "Enter"
        ));
        assert!(matches!(
            action_to_step(r#"assert text="Sign up" present=true => pass"#),
            Some(Step::Assert { text, present: true }) if text == "Sign up"
        ));
        assert!(matches!(
            action_to_step(r#"assert text="nope" present=false => fail"#),
            Some(Step::Assert { text, present: false }) if text == "nope"
        ));
    }

    #[test]
    fn action_to_step_handles_escaped_quotes_in_typed_text() {
        let step = action_to_step(r#"type e6 "say \"hi\"""#).expect("should parse");
        match step {
            Step::Type { text, .. } => assert_eq!(text, "say \"hi\""),
            other => panic!("expected Type, got {other:?}"),
        }
    }

    #[test]
    fn export_skips_console_and_exception_entries() {
        let entries = vec![
            TraceEntry::Action {
                text: "navigate https://example.com".to_string(),
                timestamp_ms: 1.0,
            },
            TraceEntry::Console {
                level: "log".to_string(),
                text: "hello".to_string(),
                timestamp_ms: 2.0,
            },
            TraceEntry::Exception {
                text: "boom".to_string(),
                timestamp_ms: 3.0,
            },
            TraceEntry::Action {
                text: "click e6".to_string(),
                timestamp_ms: 4.0,
            },
        ];
        let yaml = export(&entries).expect("export succeeds");
        let script: Script = serde_yaml::from_str(&yaml).expect("re-parses");
        assert_eq!(script.steps.len(), 2);
        assert!(matches!(&script.steps[0], Step::Navigate(u) if u == "https://example.com"));
        assert!(matches!(&script.steps[1], Step::Click(r) if r == "e6"));
    }
}
