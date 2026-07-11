//! Console/exception capture to a named JSONL trace (console-capture
//! spec). The collector task mirrors `recording.rs`'s (screencast) and
//! `network::recording`'s collector-task shape.

use crate::error::{EngineError, Result};
use cdp::ops::Page;
use cdp::protocol::runtime::{ConsoleApiCalled, ExceptionThrown, RemoteObject};
use cdp::session::EventItem;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

/// Hard ceiling so a forgotten `browser_console_stop` can't capture
/// indefinitely (mirrors `recording.rs`'s `MAX_RECORDING_DURATION`).
const MAX_CAPTURE_DURATION: Duration = Duration::from_secs(300);

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TraceEntry {
    Console {
        level: String,
        text: String,
        timestamp_ms: f64,
    },
    Exception {
        text: String,
        timestamp_ms: f64,
    },
}

pub struct ConsoleCaptureSummary {
    pub name: String,
    pub entry_count: usize,
    pub path: PathBuf,
}

/// A console capture in progress. `stop()` halts capture and persists the
/// trace -- self-contained, mirroring `NetworkRecording`/`Training::stop`'s
/// "everything happens on the owned value" shape.
pub struct ConsoleCapture {
    name: String,
    entries: Arc<Mutex<Vec<TraceEntry>>>,
    stop_tx: Option<oneshot::Sender<()>>,
    collector: JoinHandle<()>,
}

impl ConsoleCapture {
    pub(crate) async fn start(page: &Page, name: &str) -> Result<Self> {
        let entries = Arc::new(Mutex::new(Vec::new()));
        let (stop_tx, stop_rx) = oneshot::channel();
        let collector = tokio::spawn(collect_console(
            page.clone(),
            entries.clone(),
            stop_rx,
            MAX_CAPTURE_DURATION,
        ));

        Ok(Self {
            name: name.to_string(),
            entries,
            stop_tx: Some(stop_tx),
            collector,
        })
    }

    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub async fn stop(mut self) -> Result<ConsoleCaptureSummary> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.collector).await;

        let entries = std::mem::take(&mut *self.entries.lock().await);
        let entry_count = entries.len();
        let path = save_trace(&self.name, &entries)?;

        Ok(ConsoleCaptureSummary {
            name: self.name.clone(),
            entry_count,
            path,
        })
    }
}

async fn collect_console(
    page: Page,
    entries: Arc<Mutex<Vec<TraceEntry>>>,
    mut stop_rx: oneshot::Receiver<()>,
    max_duration: Duration,
) {
    let mut console = page.events::<ConsoleApiCalled>();
    let mut exceptions = page.events::<ExceptionThrown>();

    let deadline = tokio::time::sleep(max_duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            _ = &mut deadline => break,
            item = console.next() => {
                match item {
                    Some(EventItem::Event(ev)) => {
                        let text = ev.args.iter().map(render_arg).collect::<Vec<_>>().join(" ");
                        entries.lock().await.push(TraceEntry::Console {
                            level: ev.kind,
                            text,
                            timestamp_ms: ev.timestamp,
                        });
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break,
                }
            }
            item = exceptions.next() => {
                match item {
                    Some(EventItem::Event(ev)) => {
                        // exceptionDetails.text is usually just "Uncaught";
                        // the actual message lives on the thrown value's
                        // own description when present.
                        let text = ev
                            .exception_details
                            .exception
                            .as_ref()
                            .and_then(|e| e.description.clone())
                            .unwrap_or(ev.exception_details.text);
                        entries.lock().await.push(TraceEntry::Exception {
                            text,
                            timestamp_ms: ev.timestamp,
                        });
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break,
                }
            }
        }
    }
}

/// Best-effort string rendering of a console argument: prefer the
/// primitive `.value`, fall back to `.description` (objects/functions),
/// else a bare type marker (design.md Decision #2).
fn render_arg(obj: &RemoteObject) -> String {
    if let Some(value) = &obj.value {
        match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    } else if let Some(desc) = &obj.description {
        desc.clone()
    } else {
        format!("<{}>", obj.kind)
    }
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
fn save_trace(name: &str, entries: &[TraceEntry]) -> Result<PathBuf> {
    let dir = cdp::launch::profile_base_dir()?.join("aib").join("traces");
    std::fs::create_dir_all(&dir)
        .map_err(|e| EngineError::Console(format!("failed to create traces dir: {e}")))?;

    let path = dir.join(format!("{name}.jsonl"));
    let mut buf = String::new();
    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|e| EngineError::Console(format!("failed to serialize trace entry: {e}")))?;
        buf.push_str(&line);
        buf.push('\n');
    }
    std::fs::write(&path, buf)
        .map_err(|e| EngineError::Console(format!("failed to write {}: {e}", path.display())))?;

    Ok(path)
}
