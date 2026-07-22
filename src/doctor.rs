//! `truewright doctor` — attach→navigate→evaluate→screenshot self-check plus
//! command round-trip latency, against every discovered browser
//! (openspec/changes/phase-0-cdp-spike/specs/doctor-cli/spec.md).

use cdp::launch::{self, DiscoveredBrowser, LaunchedBrowser};
use cdp::ops::{Browser, Page};
use serde::Serialize;
use std::future::Future;
use std::time::{Duration, Instant};

const LATENCY_SAMPLES: usize = 100;
const LATENCY_P50_TARGET_MS: f64 = 5.0;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum StepStatus {
    Passed,
    Failed { error: String },
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
struct StepResult {
    name: String,
    #[serde(flatten)]
    status: StepStatus,
    duration_ms: Option<f64>,
}

impl StepResult {
    fn passed(name: &str, duration_ms: f64) -> Self {
        Self {
            name: name.to_string(),
            status: StepStatus::Passed,
            duration_ms: Some(duration_ms),
        }
    }
    fn failed(name: &str, duration_ms: f64, error: String) -> Self {
        Self {
            name: name.to_string(),
            status: StepStatus::Failed { error },
            duration_ms: Some(duration_ms),
        }
    }
    fn skipped(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: StepStatus::Skipped,
            duration_ms: None,
        }
    }
    fn is_ok(&self) -> bool {
        matches!(self.status, StepStatus::Passed)
    }
}

#[derive(Debug, Serialize)]
struct LatencyReport {
    samples: usize,
    p50_ms: f64,
    p95_ms: f64,
    warning: Option<String>,
}

#[derive(Debug, Serialize)]
struct BrowserReport {
    kind: String,
    path: String,
    headless_shell: bool,
    steps: Vec<StepResult>,
    latency: Option<LatencyReport>,
    /// Resident memory of the browser's full process tree (root + renderer/
    /// GPU/utility children) while the page was loaded (doctor-cli spec:
    /// "Process-tree memory measurement").
    tree_rss_mb: Option<f64>,
    passed: bool,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    ok: bool,
    browsers: Vec<BrowserReport>,
}

pub async fn run(
    json: bool,
    headless: bool,
    installed_only: bool,
    extra_chrome_args: Vec<String>,
) -> std::process::ExitCode {
    let mut discovered = match launch::discover_browsers() {
        Ok(list) => list,
        Err(e) => {
            report_discovery_failure(json, &e);
            return std::process::ExitCode::FAILURE;
        }
    };

    // The managed headless-shell is checked as an additional entry (headless
    // runs only — the shell has no headed mode), so its tree_rss_mb can be
    // compared directly against the installed browsers'.
    //
    // On a cold cache this resolution *downloads* the shell (~100MB), and
    // that download logs only at `info` level — below doctor's default
    // `warn` filter — so a first run otherwise looked like it hung or
    // skipped the shell entirely (this is task item #4). A visible line up
    // front, and a visible reason on fallback/failure, makes the first-run
    // behavior legible without needing `RUST_LOG=info`.
    if headless && !installed_only {
        let cold_cache = cdp::download::cached_shell().is_none();
        if cold_cache && !json {
            eprintln!(
                "truewright doctor: resolving managed chrome-headless-shell \
                 (first run downloads ~100MB, then cached)…"
            );
        }
        match launch::resolve_headless_browser(launch::BrowserPreference::Auto).await {
            Ok(shell) if shell.is_headless_shell => {
                if cold_cache && !json {
                    eprintln!("truewright doctor: chrome-headless-shell ready.");
                }
                discovered.insert(0, shell);
            }
            // Resolution succeeded but fell back to an installed browser
            // (already in the list) — the shell was unavailable for a
            // reason worth surfacing, not silently dropping.
            Ok(_) => {
                if !json {
                    eprintln!(
                        "truewright doctor: managed chrome-headless-shell unavailable; \
                         reporting installed browser(s) only."
                    );
                }
            }
            Err(e) => {
                if !json {
                    eprintln!("truewright doctor: headless-shell resolution failed: {e}");
                }
                tracing::warn!(error = %e, "doctor: headless-shell unavailable");
            }
        }
    }

    let mut browsers = Vec::with_capacity(discovered.len());
    for browser in &discovered {
        browsers.push(check_browser(browser, headless, &extra_chrome_args).await);
    }

    let ok = browsers.iter().all(|b| b.passed);
    let report = DoctorReport { ok, browsers };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report is serializable")
        );
    } else {
        print_text_report(&report);
    }

    if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn report_discovery_failure(json: bool, error: &cdp::CdpError) {
    if json {
        let report = serde_json::json!({ "ok": false, "error": error.to_string(), "browsers": [] });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report is serializable")
        );
    } else {
        eprintln!("truewright doctor: {error}");
    }
}

async fn check_browser(
    discovered: &DiscoveredBrowser,
    headless: bool,
    extra_chrome_args: &[String],
) -> BrowserReport {
    let kind_label = discovered.kind.label();
    let profile_name = if discovered.is_headless_shell {
        "doctor-shell".to_string()
    } else {
        format!("doctor-{}", kind_label.to_lowercase())
    };
    let mut steps = Vec::new();

    let arg_refs: Vec<&str> = extra_chrome_args.iter().map(String::as_str).collect();
    let (launch_result, dur) = time(launch::launch_with_flags(
        discovered,
        &profile_name,
        headless,
        &arg_refs,
    ))
    .await;
    let launched = match launch_result {
        Ok(lb) => {
            steps.push(StepResult::passed("launch", dur));
            lb
        }
        Err(e) => {
            steps.push(StepResult::failed("launch", dur, e.to_string()));
            for name in [
                "connect",
                "create_context",
                "create_page",
                "navigate",
                "evaluate",
                "screenshot",
                "teardown",
            ] {
                steps.push(StepResult::skipped(name));
            }
            return finish(discovered, steps, None, None);
        }
    };

    let ws_url = launched.ws_url.clone();
    let root_pid = launched.pid();
    let (functional_steps, latency, tree_rss_mb) = run_functional_steps(&ws_url, root_pid).await;
    steps.extend(functional_steps);
    shutdown(launched).await;

    finish(discovered, steps, latency, tree_rss_mb)
}

fn finish(
    discovered: &DiscoveredBrowser,
    steps: Vec<StepResult>,
    latency: Option<LatencyReport>,
    tree_rss_mb: Option<f64>,
) -> BrowserReport {
    let passed = steps.iter().all(StepResult::is_ok);
    BrowserReport {
        kind: discovered.kind.label().to_string(),
        path: discovered.path.display().to_string(),
        headless_shell: discovered.is_headless_shell,
        steps,
        latency,
        tree_rss_mb,
        passed,
    }
}

/// Sums resident memory of `root_pid` and all its descendants, in MB.
fn measure_tree_rss_mb(root_pid: u32) -> Option<f64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let mut sys = System::new();
    // `.without_tasks()`: sysinfo's default includes each thread as a
    // separate "process" on Linux (threads share the process PID
    // namespace via /proc/[pid]/task/[tid]), and every such entry reports
    // the *whole process's* RSS — summing them multiplies the real total
    // by the thread count (~14x observed on a real Chromium tree).
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_memory().without_tasks(),
    );

    let root = Pid::from_u32(root_pid);
    let mut children_of: std::collections::HashMap<Pid, Vec<Pid>> =
        std::collections::HashMap::new();
    for (pid, proc_) in sys.processes() {
        if let Some(parent) = proc_.parent() {
            children_of.entry(parent).or_default().push(*pid);
        }
    }

    sys.process(root)?;
    let mut total_bytes: u64 = 0;
    let mut visited: std::collections::HashSet<Pid> = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        if !visited.insert(pid) {
            continue;
        }
        if let Some(proc_) = sys.process(pid) {
            total_bytes += proc_.memory();
        }
        if let Some(kids) = children_of.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    Some(total_bytes as f64 / 1024.0 / 1024.0)
}

async fn shutdown(launched: LaunchedBrowser) {
    if let Err(e) = launched.shutdown().await {
        tracing::warn!(error = %e, "doctor: browser shutdown failed");
    }
}

/// Runs connect→context→page→navigate→evaluate→screenshot→teardown,
/// skipping (not aborting the whole run) whatever comes after the first
/// failure — matching "failures MUST NOT abort checks for other browsers"
/// at the step level too.
async fn run_functional_steps(
    ws_url: &str,
    root_pid: Option<u32>,
) -> (Vec<StepResult>, Option<LatencyReport>, Option<f64>) {
    let mut steps = Vec::new();

    let (connect_result, dur) = time(Browser::connect(ws_url)).await;
    let browser = match connect_result {
        Ok(b) => {
            steps.push(StepResult::passed("connect", dur));
            b
        }
        Err(e) => {
            steps.push(StepResult::failed("connect", dur, e.to_string()));
            skip_rest(
                &mut steps,
                &[
                    "create_context",
                    "create_page",
                    "navigate",
                    "evaluate",
                    "screenshot",
                    "teardown",
                ],
            );
            return (steps, None, None);
        }
    };

    let (context_result, dur) = time(browser.new_context()).await;
    let context = match context_result {
        Ok(c) => {
            steps.push(StepResult::passed("create_context", dur));
            c
        }
        Err(e) => {
            steps.push(StepResult::failed("create_context", dur, e.to_string()));
            skip_rest(
                &mut steps,
                &[
                    "create_page",
                    "navigate",
                    "evaluate",
                    "screenshot",
                    "teardown",
                ],
            );
            return (steps, None, None);
        }
    };

    let (page_result, dur) = time(context.new_page("about:blank")).await;
    let page = match page_result {
        Ok(p) => {
            steps.push(StepResult::passed("create_page", dur));
            p
        }
        Err(e) => {
            steps.push(StepResult::failed("create_page", dur, e.to_string()));
            skip_rest(
                &mut steps,
                &["navigate", "evaluate", "screenshot", "teardown"],
            );
            let _ = context.dispose().await;
            return (steps, None, None);
        }
    };

    let (nav_result, dur) =
        time(page.navigate_and_wait("https://example.com", Duration::from_secs(15))).await;
    let mut latency = None;
    let mut tree_rss_mb = None;
    match nav_result {
        Ok(()) => {
            steps.push(StepResult::passed("navigate", dur));

            tree_rss_mb = root_pid.and_then(measure_tree_rss_mb);

            let (eval_result, dur) = time(page.evaluate("document.title")).await;
            match eval_result {
                Ok(_) => steps.push(StepResult::passed("evaluate", dur)),
                Err(e) => steps.push(StepResult::failed("evaluate", dur, e.to_string())),
            }

            let (shot_result, dur) = time(page.screenshot()).await;
            match shot_result {
                Ok(bytes) if !bytes.is_empty() => steps.push(StepResult::passed("screenshot", dur)),
                Ok(_) => steps.push(StepResult::failed(
                    "screenshot",
                    dur,
                    "screenshot was empty".into(),
                )),
                Err(e) => steps.push(StepResult::failed("screenshot", dur, e.to_string())),
            }

            latency = Some(measure_latency(&page).await);
        }
        Err(e) => {
            steps.push(StepResult::failed("navigate", dur, e.to_string()));
            skip_rest(&mut steps, &["evaluate", "screenshot"]);
        }
    }

    let (close_result, dur) = time(page.close()).await;
    match close_result {
        Ok(()) => steps.push(StepResult::passed("teardown", dur)),
        Err(e) => steps.push(StepResult::failed("teardown", dur, e.to_string())),
    }
    let _ = context.dispose().await;

    (steps, latency, tree_rss_mb)
}

fn skip_rest(steps: &mut Vec<StepResult>, names: &[&str]) {
    for name in names {
        steps.push(StepResult::skipped(name));
    }
}

/// ≥100 lightweight round trips, reporting p50/p95 (doctor-cli spec:
/// "Latency measurement"). Exit criterion: p50 < 5ms.
async fn measure_latency(page: &Page) -> LatencyReport {
    let mut samples_ms = Vec::with_capacity(LATENCY_SAMPLES);
    for _ in 0..LATENCY_SAMPLES {
        let start = Instant::now();
        let _ = page.evaluate("1+1").await;
        samples_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples_ms.sort_by(|a, b| a.partial_cmp(b).expect("latencies are finite"));

    let p50 = percentile(&samples_ms, 0.50);
    let p95 = percentile(&samples_ms, 0.95);
    let warning = (p50 >= LATENCY_P50_TARGET_MS)
        .then(|| format!("p50 {p50:.2}ms >= {LATENCY_P50_TARGET_MS}ms target"));

    LatencyReport {
        samples: samples_ms.len(),
        p50_ms: p50,
        p95_ms: p95,
        warning,
    }
}

fn percentile(sorted_ms: &[f64], p: f64) -> f64 {
    if sorted_ms.is_empty() {
        return 0.0;
    }
    let idx = (((sorted_ms.len() - 1) as f64) * p).round() as usize;
    sorted_ms[idx.min(sorted_ms.len() - 1)]
}

async fn time<F, T, E>(fut: F) -> (Result<T, E>, f64)
where
    F: Future<Output = Result<T, E>>,
{
    let start = Instant::now();
    let result = fut.await;
    (result, start.elapsed().as_secs_f64() * 1000.0)
}

fn print_text_report(report: &DoctorReport) {
    for browser in &report.browsers {
        let shell_tag = if browser.headless_shell {
            " [headless-shell]"
        } else {
            ""
        };
        println!("== {}{shell_tag} ({}) ==", browser.kind, browser.path);
        for step in &browser.steps {
            let mark = match &step.status {
                StepStatus::Passed => "✓",
                StepStatus::Failed { .. } => "✗",
                StepStatus::Skipped => "-",
            };
            let dur = step
                .duration_ms
                .map(|d| format!(" ({d:.1}ms)"))
                .unwrap_or_default();
            print!("  {mark} {}{dur}", step.name);
            if let StepStatus::Failed { error } = &step.status {
                print!(" — {error}");
            }
            println!();
        }
        if let Some(latency) = &browser.latency {
            println!(
                "  latency: p50={:.2}ms p95={:.2}ms (n={})",
                latency.p50_ms, latency.p95_ms, latency.samples
            );
            if let Some(warning) = &latency.warning {
                println!("  ⚠ {warning}");
            }
        }
        if let Some(rss) = browser.tree_rss_mb {
            println!("  tree memory: {rss:.1} MB (browser + all child processes)");
        }
        println!("  result: {}", if browser.passed { "PASS" } else { "FAIL" });
        println!();
    }
    println!("overall: {}", if report.ok { "PASS" } else { "FAIL" });
}
