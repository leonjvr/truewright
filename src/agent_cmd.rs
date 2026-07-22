//! `truewright agent "task"` CLI wiring (agent-harness spec): resolves config,
//! roles, and skills, launches a browser, and runs the task through
//! `agent::Harness`, rendering live progress.

use agent::{AgentEvent, Harness, SharedSession};
use std::path::PathBuf;
use std::time::Duration;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    task: &str,
    skills: &[String],
    driver_override: Option<&str>,
    vision_override: Option<&str>,
    max_steps_override: Option<u32>,
    headed: bool,
    browser: cdp::launch::BrowserPreference,
    extra_chrome_args: Vec<String>,
    profile: &str,
    config_path: Option<PathBuf>,
    json: bool,
) -> std::process::ExitCode {
    let truewright_data_dir = match crate::resolve_truewright_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::from(2);
        }
    };

    let config = match llm::Config::load(&truewright_data_dir, config_path.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {e}");
            return std::process::ExitCode::from(2);
        }
    };

    let driver = match resolve_role_or_override(&config, "driver", driver_override, false) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("failed to resolve driver role: {e}");
            eprintln!(
                "configure [roles.driver] in your config, or pass --driver <provider>/<model>."
            );
            return std::process::ExitCode::from(2);
        }
    };

    let vision = if vision_override.is_some() || config.has_role("vision") {
        match resolve_role_or_override(&config, "vision", vision_override, true) {
            Ok(rc) => Some(rc),
            Err(e) => {
                eprintln!("failed to resolve vision role: {e}");
                return std::process::ExitCode::from(2);
            }
        }
    } else {
        None
    };

    let skill_dirs = agent::default_skill_dirs(&truewright_data_dir, &config.skills.dirs);
    let resolved_skills = match agent::resolve_skills(skills, &skill_dirs) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to resolve skills: {e}");
            return std::process::ExitCode::from(2);
        }
    };

    let session = match engine::Session::launch_with_args(
        profile,
        !headed,
        browser,
        &extra_chrome_args,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to launch browser: {e}");
            return std::process::ExitCode::from(2);
        }
    };
    let shared = SharedSession::new(session);

    let harness = Harness {
        driver,
        vision,
        max_steps: max_steps_override.unwrap_or(config.agent.max_steps),
        step_timeout: Duration::from_secs(config.agent.step_timeout_secs),
        task_timeout: Duration::from_secs(config.agent.task_timeout_secs),
        max_retained_snapshots: config.agent.max_retained_snapshots,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let render_task = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            render_event(&event, json);
        }
    });

    let outcome = harness
        .run_task(&shared, task, &resolved_skills, None, Some(tx))
        .await;
    let _ = render_task.await;

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        if let Err(e) = s.close().await {
            eprintln!("warning: failed to close browser session cleanly: {e}");
        }
    }
    drop(guard);

    match outcome {
        Ok(result) => {
            println!(
                "{} ({} steps): {}",
                if result.passed { "PASS" } else { "FAIL" },
                result.steps_used,
                result.summary
            );
            if result.passed {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("agent run failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// `--driver`/`--vision` accept `<provider>/<model>`, bypassing
/// `[roles.*]` entirely; otherwise falls back to the named role from
/// config. `default_vision` is the capability assumed for an ad-hoc
/// override with no way to state it on the command line -- `true` for
/// `--vision` (the whole point of overriding it is to point at a
/// vision-capable model), `false` for `--driver` (the common case).
#[allow(clippy::result_large_err)]
fn resolve_role_or_override(
    config: &llm::Config,
    role_name: &str,
    override_value: Option<&str>,
    default_vision: bool,
) -> llm::Result<llm::RoleClient> {
    match override_value {
        Some(spec) => {
            let (provider, model) = spec.split_once('/').ok_or_else(|| {
                llm::LlmError::UnknownProviderDirect(format!(
                    "{spec:?} (expected \"<provider>/<model>\")"
                ))
            })?;
            config.resolve_provider_model(provider, model, default_vision)
        }
        None => config.resolve_role(role_name),
    }
}

fn render_event(event: &AgentEvent, json: bool) {
    if json {
        let value = match event {
            AgentEvent::Step { n, max } => serde_json::json!({"type": "step", "n": n, "max": max}),
            AgentEvent::ToolCall { name, args_summary } => {
                serde_json::json!({"type": "tool_call", "name": name, "args": args_summary})
            }
            AgentEvent::ToolResult { name, ok, summary } => {
                serde_json::json!({"type": "tool_result", "name": name, "ok": ok, "summary": summary})
            }
            AgentEvent::Vision { chars } => serde_json::json!({"type": "vision", "chars": chars}),
            AgentEvent::Done { passed, summary } => {
                serde_json::json!({"type": "done", "passed": passed, "summary": summary})
            }
        };
        println!("{value}");
        return;
    }

    match event {
        AgentEvent::Step { n, max } => println!("step {n}/{max}"),
        AgentEvent::ToolCall { name, args_summary } => println!("  -> {name} {args_summary}"),
        AgentEvent::ToolResult { name, ok, summary } => {
            println!("  {} {name}: {summary}", if *ok { "ok" } else { "error" });
        }
        AgentEvent::Vision { chars } => println!("  (vision interpretation: {chars} chars)"),
        AgentEvent::Done { passed, summary } => {
            println!("{} {summary}", if *passed { "done" } else { "failed" });
        }
    }
}
