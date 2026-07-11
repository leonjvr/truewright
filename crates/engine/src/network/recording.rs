//! Passive network capture (network-mocking spec: "Passive network
//! recording to a named cassette"). The collector task mirrors
//! `recording.rs`'s (screencast) collector-task shape: correlates
//! `RequestWillBeSent` (method/url) -> `ResponseReceived` (status/headers)
//! -> `LoadingFinished` (body now fetchable) by `requestId`, all local to
//! the collector task -- only the finished entries list is shared.

use super::cassette::{self, Cassette, CassetteEntry};
use crate::error::Result;
use cdp::ops::Page;
use cdp::protocol::network;
use cdp::session::EventItem;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

/// Hard ceiling so a forgotten `browser_network_record_stop` can't capture
/// indefinitely (mirrors `recording.rs`'s `MAX_RECORDING_DURATION`).
const MAX_NETWORK_RECORDING_DURATION: Duration = Duration::from_secs(300);

pub struct NetworkRecordingSummary {
    pub name: String,
    pub entry_count: usize,
    pub path: PathBuf,
}

/// A network recording in progress. `stop()` halts capture and persists the
/// cassette -- self-contained, mirroring `Recording`/`Training::stop`'s
/// "everything happens on the owned value" shape.
pub struct NetworkRecording {
    name: String,
    entries: Arc<Mutex<Vec<CassetteEntry>>>,
    stop_tx: Option<oneshot::Sender<()>>,
    collector: JoinHandle<()>,
}

impl NetworkRecording {
    pub(crate) async fn start(page: &Page, name: &str) -> Result<Self> {
        page.enable_network_capture().await?;

        let entries = Arc::new(Mutex::new(Vec::new()));
        let (stop_tx, stop_rx) = oneshot::channel();
        let collector = tokio::spawn(collect_network(
            page.clone(),
            entries.clone(),
            stop_rx,
            MAX_NETWORK_RECORDING_DURATION,
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
    pub async fn stop(mut self) -> Result<NetworkRecordingSummary> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.collector).await;

        let entries = std::mem::take(&mut *self.entries.lock().await);
        let entry_count = entries.len();
        let path = cassette::save(&self.name, &Cassette { entries })?;

        Ok(NetworkRecordingSummary {
            name: self.name.clone(),
            entry_count,
            path,
        })
    }
}

struct PendingEntry {
    method: String,
    url: String,
    status: i64,
    headers: Vec<(String, String)>,
}

async fn collect_network(
    page: Page,
    entries: Arc<Mutex<Vec<CassetteEntry>>>,
    mut stop_rx: oneshot::Receiver<()>,
    max_duration: Duration,
) {
    let mut request_started = page.events::<network::RequestWillBeSent>();
    let mut response_received = page.events::<network::ResponseReceived>();
    let mut loading_finished = page.events::<network::LoadingFinished>();
    let mut pending: HashMap<String, PendingEntry> = HashMap::new();

    let deadline = tokio::time::sleep(max_duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            _ = &mut deadline => break,
            item = request_started.next() => {
                match item {
                    Some(EventItem::Event(ev)) => {
                        pending.insert(ev.request_id, PendingEntry {
                            method: ev.request.method,
                            url: ev.request.url,
                            status: 0,
                            headers: Vec::new(),
                        });
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break,
                }
            }
            item = response_received.next() => {
                match item {
                    Some(EventItem::Event(ev)) => {
                        if let Some(p) = pending.get_mut(&ev.request_id) {
                            p.status = ev.response.status;
                            p.headers = ev.response.headers.into_iter().collect();
                        }
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break,
                }
            }
            item = loading_finished.next() => {
                match item {
                    Some(EventItem::Event(ev)) => {
                        if let Some(p) = pending.remove(&ev.request_id) {
                            // Awaited inline, unlike the screencast
                            // collector's fire-and-forget acks: network
                            // requests arrive far less often than
                            // screencast frames, so blocking briefly here
                            // isn't the throughput risk it was there --
                            // and awaiting inline means a request that
                            // finishes right as `stop()` is called still
                            // gets its entry pushed before `stop()` reads
                            // the list, which a detached spawn wouldn't
                            // guarantee.
                            if let Ok((body, is_base64)) = page.get_response_body(&ev.request_id).await {
                                let body_base64 = if is_base64 {
                                    body
                                } else {
                                    use base64::Engine;
                                    base64::engine::general_purpose::STANDARD.encode(body.as_bytes())
                                };
                                entries.lock().await.push(CassetteEntry {
                                    method: p.method,
                                    url: p.url,
                                    status: p.status,
                                    headers: p.headers,
                                    body_base64,
                                });
                            }
                        }
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break,
                }
            }
        }
    }
}
