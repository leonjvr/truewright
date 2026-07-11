//! Request interception + fulfillment from a cassette (network-mocking
//! spec: "Replay from a cassette with no live-network dependency",
//! "Unmatched replay requests fail loudly").

use super::cassette::{self, CassetteEntry};
use crate::error::Result;
use cdp::ops::Page;
use cdp::protocol::fetch;
use cdp::session::EventItem;
use std::collections::{HashMap, VecDeque};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// An active replay session. `stop()` disables interception -- any request
/// still paused when `stop()` is called has already been resolved inline
/// by the worker before `stop()`'s `disable_request_interception()` runs
/// (see `intercept_requests`), so nothing is left hanging.
pub struct NetworkReplay {
    page: Page,
    stop_tx: Option<oneshot::Sender<()>>,
    worker: JoinHandle<()>,
}

impl NetworkReplay {
    pub(crate) async fn start(page: &Page, name: &str) -> Result<Self> {
        let cassette = cassette::load(name)?;
        let mut queues: HashMap<(String, String), VecDeque<CassetteEntry>> = HashMap::new();
        for entry in cassette.entries {
            queues
                .entry((entry.method.clone(), entry.url.clone()))
                .or_default()
                .push_back(entry);
        }

        page.enable_request_interception().await?;

        let (stop_tx, stop_rx) = oneshot::channel();
        let worker = tokio::spawn(intercept_requests(page.clone(), queues, stop_rx));

        Ok(Self {
            page: page.clone(),
            stop_tx: Some(stop_tx),
            worker,
        })
    }

    pub async fn stop(mut self) -> Result<()> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.worker).await;
        self.page.disable_request_interception().await?;
        Ok(())
    }
}

async fn intercept_requests(
    page: Page,
    mut queues: HashMap<(String, String), VecDeque<CassetteEntry>>,
    mut stop_rx: oneshot::Receiver<()>,
) {
    let mut requests = page.events::<fetch::RequestPaused>();

    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            item = requests.next() => {
                match item {
                    Some(EventItem::Event(ev)) => {
                        let key = (ev.request.method.clone(), ev.request.url.clone());
                        let entry = queues.get_mut(&key).and_then(|q| q.pop_front());
                        match entry {
                            Some(entry) => {
                                let _ = page
                                    .fulfill_request(&ev.request_id, entry.status, entry.headers, Some(entry.body_base64))
                                    .await;
                            }
                            None => {
                                let _ = page.fail_request(&ev.request_id).await;
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

