use crate::error::{CdpError, Result};
use crate::transport::Transport;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};

pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 1024;
pub(crate) const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// An event whose `method`/`params` have not yet been matched against a
/// typed [`crate::session::CdpEvent`] — see `Session::events`.
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub session_id: Option<String>,
    pub method: String,
    pub params: Value,
}

#[derive(Serialize)]
struct OutgoingMessage<'a> {
    id: u64,
    method: &'a str,
    params: Value,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
}

#[derive(Deserialize)]
struct ProtocolError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
struct IncomingMessage {
    id: Option<u64>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    method: Option<String>,
    #[serde(default)]
    params: Value,
    result: Option<Value>,
    error: Option<ProtocolError>,
}

pub(crate) struct Inner {
    out_tx: mpsc::UnboundedSender<String>,
    pending: DashMap<u64, oneshot::Sender<Result<Value>>>,
    next_id: AtomicU64,
    // key = None -> browser-level session (no sessionId on the wire)
    sessions: DashMap<Option<String>, broadcast::Sender<RawEvent>>,
    closed: AtomicBool,
    default_timeout: Duration,
}

/// A live CDP connection: one WebSocket, demuxed by `sessionId` in flatten
/// mode (see design.md Decision #2). Cheap to clone; clones share the same
/// background IO task and state.
#[derive(Clone)]
pub struct Connection {
    pub(crate) inner: Arc<Inner>,
}

impl Connection {
    pub async fn connect(url: &str) -> Result<Self> {
        let transport = crate::transport::WebSocketTransport::connect(url).await?;
        Ok(Self::new(transport))
    }

    pub fn new<T: Transport>(transport: T) -> Self {
        let (out_tx, out_rx) = mpsc::unbounded_channel::<String>();
        let inner = Arc::new(Inner {
            out_tx,
            pending: DashMap::new(),
            next_id: AtomicU64::new(1),
            sessions: DashMap::new(),
            closed: AtomicBool::new(false),
            default_timeout: DEFAULT_COMMAND_TIMEOUT,
        });

        let io_inner = inner.clone();
        tokio::spawn(io_loop(transport, out_rx, io_inner));

        Self { inner }
    }

    /// Root browser-level session (commands/events with no `sessionId`).
    pub fn browser_session(&self) -> crate::session::Session {
        crate::session::Session::new(self.inner.clone(), None)
    }

    pub fn session(&self, session_id: impl Into<String>) -> crate::session::Session {
        crate::session::Session::new(self.inner.clone(), Some(session_id.into()))
    }

    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }
}

async fn io_loop<T: Transport>(
    mut transport: T,
    mut out_rx: mpsc::UnboundedReceiver<String>,
    inner: Arc<Inner>,
) {
    loop {
        tokio::select! {
            biased;
            outgoing = out_rx.recv() => {
                match outgoing {
                    Some(msg) => {
                        if let Err(e) = transport.send(msg).await {
                            tracing::warn!(error = %e, "cdp transport send failed");
                            break;
                        }
                    }
                    None => break, // Connection (and all Sessions) dropped
                }
            }
            incoming = transport.recv() => {
                match incoming {
                    Ok(Some(text)) => handle_incoming(&inner, &text),
                    Ok(None) => {
                        tracing::debug!("cdp transport closed");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "cdp transport recv failed");
                        break;
                    }
                }
            }
        }
    }
    close(&inner);
}

fn handle_incoming(inner: &Arc<Inner>, text: &str) {
    let msg: IncomingMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, raw = %text, "failed to parse CDP message");
            return;
        }
    };

    if let Some(id) = msg.id {
        if let Some((_, sender)) = inner.pending.remove(&id) {
            let result = match msg.error {
                Some(err) => Err(CdpError::Protocol {
                    code: err.code,
                    message: err.message,
                }),
                None => Ok(msg.result.unwrap_or(Value::Null)),
            };
            let _ = sender.send(result);
        }
        return;
    }

    if let Some(method) = msg.method {
        let event = RawEvent {
            session_id: msg.session_id.clone(),
            method,
            params: msg.params,
        };
        if let Some(sender) = inner.sessions.get(&msg.session_id) {
            let _ = sender.send(event); // Err = no receivers, fine to drop
        }
    }
}

/// Fails every in-flight command with [`CdpError::Disconnected`]. Later
/// commands fail too, once `out_tx`'s sole receiver (`out_rx`, owned by
/// `io_loop`) has already been dropped by the time this runs.
fn close(inner: &Arc<Inner>) {
    inner.closed.store(true, Ordering::Release);
    let ids: Vec<u64> = inner.pending.iter().map(|e| *e.key()).collect();
    for id in ids {
        if let Some((_, sender)) = inner.pending.remove(&id) {
            let _ = sender.send(Err(CdpError::Disconnected));
        }
    }
}

impl Inner {
    pub(crate) async fn execute_raw(
        &self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CdpError::Disconnected);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, tx);

        let payload = OutgoingMessage {
            id,
            method,
            params,
            session_id,
        };
        let text = serde_json::to_string(&payload)?;

        if self.out_tx.send(text).is_err() {
            self.pending.remove(&id);
            return Err(CdpError::Disconnected);
        }

        match tokio::time::timeout(self.default_timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CdpError::Disconnected), // sender dropped without a reply
            Err(_) => {
                self.pending.remove(&id);
                Err(CdpError::Timeout(self.default_timeout))
            }
        }
    }

    pub(crate) fn subscribe(&self, session_id: Option<String>) -> broadcast::Receiver<RawEvent> {
        self.sessions
            .entry(session_id)
            .or_insert_with(|| broadcast::channel(EVENT_CHANNEL_CAPACITY).0)
            .subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An in-memory `Transport` driven by two unbounded channels, so tests
    /// can script server behavior and inspect what the client sent.
    struct MockTransport {
        incoming: mpsc::UnboundedReceiver<String>,
        outgoing: mpsc::UnboundedSender<String>,
    }

    impl Transport for MockTransport {
        async fn send(&mut self, msg: String) -> Result<()> {
            self.outgoing.send(msg).map_err(|_| CdpError::Disconnected)
        }

        async fn recv(&mut self) -> Result<Option<String>> {
            Ok(self.incoming.recv().await)
        }
    }

    fn mock_pair() -> (
        MockTransport,
        mpsc::UnboundedSender<String>,
        mpsc::UnboundedReceiver<String>,
    ) {
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();
        (
            MockTransport {
                incoming: incoming_rx,
                outgoing: outgoing_tx,
            },
            incoming_tx,
            outgoing_rx,
        )
    }

    fn sent_id(sent: &str) -> u64 {
        serde_json::from_str::<Value>(sent).unwrap()["id"]
            .as_u64()
            .unwrap()
    }

    #[tokio::test]
    async fn successful_command_round_trip() {
        let (transport, incoming_tx, mut outgoing_rx) = mock_pair();
        let conn = Connection::new(transport);
        let session = conn.browser_session();

        let handle = tokio::spawn(async move {
            session
                .execute_raw("Browser.getVersion", serde_json::json!({}))
                .await
        });

        let sent = outgoing_rx
            .recv()
            .await
            .expect("client should send a command");
        let id = sent_id(&sent);

        incoming_tx
            .send(serde_json::json!({"id": id, "result": {"foo": "bar"}}).to_string())
            .unwrap();

        let result = handle.await.unwrap().expect("command should succeed");
        assert_eq!(result["foo"], "bar");
    }

    #[tokio::test]
    async fn protocol_error_surfaces_as_typed_error() {
        let (transport, incoming_tx, mut outgoing_rx) = mock_pair();
        let conn = Connection::new(transport);
        let session = conn.browser_session();

        let handle =
            tokio::spawn(
                async move { session.execute_raw("Foo.bar", serde_json::json!({})).await },
            );

        let sent = outgoing_rx
            .recv()
            .await
            .expect("client should send a command");
        let id = sent_id(&sent);

        incoming_tx
            .send(
                serde_json::json!({"id": id, "error": {"code": -32601, "message": "not found"}})
                    .to_string(),
            )
            .unwrap();

        match handle.await.unwrap() {
            Err(CdpError::Protocol { code, message }) => {
                assert_eq!(code, -32601);
                assert_eq!(message, "not found");
            }
            other => panic!("expected Protocol error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn disconnect_fails_in_flight_commands() {
        let (transport, incoming_tx, mut outgoing_rx) = mock_pair();
        let conn = Connection::new(transport);
        let session = conn.browser_session();

        let handle = tokio::spawn(async move {
            session
                .execute_raw("Browser.getVersion", serde_json::json!({}))
                .await
        });

        // execute_raw registers the pending entry before sending, so by the
        // time we observe the outgoing message it's already in the map.
        let _sent = outgoing_rx
            .recv()
            .await
            .expect("client should send a command");

        drop(incoming_tx); // MockTransport::recv now returns Ok(None) -> io_loop closes

        match handle.await.unwrap() {
            Err(CdpError::Disconnected) => {}
            other => panic!("expected Disconnected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn commands_issued_after_close_fail_immediately() {
        let (transport, incoming_tx, _outgoing_rx) = mock_pair();
        let conn = Connection::new(transport);
        drop(incoming_tx);

        // Give the io_loop a chance to observe the close before we issue a
        // fresh command against an already-dead connection.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let session = conn.browser_session();
        match session
            .execute_raw("Browser.getVersion", serde_json::json!({}))
            .await
        {
            Err(CdpError::Disconnected) => {}
            other => panic!("expected Disconnected, got {other:?}"),
        }
    }
}
