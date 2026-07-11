use crate::connection::{Inner, RawEvent};
use crate::error::{LagInfo, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::broadcast;

/// A typed CDP command: `METHOD` plus its request/response shapes. Hand
/// written per command we actually use (see design.md Decision #1) — not
/// codegen'd from the full protocol.
pub trait Command {
    const METHOD: &'static str;
    type Params: Serialize;
    type Response: DeserializeOwned;
}

/// A typed CDP event, matched against [`RawEvent::method`] and deserialized
/// from `params`.
pub trait CdpEvent: DeserializeOwned + Send + 'static {
    const METHOD: &'static str;
}

/// One CDP session: either the browser-level session (`sessionId: None`) or
/// a target's attached session. Cheap to clone.
#[derive(Clone)]
pub struct Session {
    inner: Arc<Inner>,
    session_id: Option<String>,
}

impl Session {
    pub(crate) fn new(inner: Arc<Inner>, session_id: Option<String>) -> Self {
        Self { inner, session_id }
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// A view of this same connection scoped to a different (typically
    /// newly-attached) session id.
    pub fn for_session(&self, session_id: impl Into<String>) -> Session {
        Session {
            inner: self.inner.clone(),
            session_id: Some(session_id.into()),
        }
    }

    pub async fn execute<C: Command>(&self, params: C::Params) -> Result<C::Response> {
        let value = serde_json::to_value(params)?;
        let raw = self
            .inner
            .execute_raw(self.session_id.as_deref(), C::METHOD, value)
            .await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// Escape hatch for methods we haven't written typed structs for.
    pub async fn execute_raw(&self, method: &str, params: Value) -> Result<Value> {
        self.inner
            .execute_raw(self.session_id.as_deref(), method, params)
            .await
    }

    pub fn events<E: CdpEvent>(&self) -> EventStream<E> {
        let rx = self.inner.subscribe(self.session_id.clone());
        EventStream {
            rx,
            _marker: PhantomData,
        }
    }
}

pub enum EventItem<E> {
    Event(E),
    /// The subscriber fell behind and the broadcast channel dropped events
    /// on its behalf (cdp-client spec: "Bounded event subscription").
    Lagged(LagInfo),
}

pub struct EventStream<E> {
    rx: broadcast::Receiver<RawEvent>,
    _marker: PhantomData<E>,
}

impl<E: CdpEvent> EventStream<E> {
    /// Awaits the next event matching `E::METHOD`. Returns `None` only once
    /// the connection has closed and no more events will ever arrive.
    pub async fn next(&mut self) -> Option<EventItem<E>> {
        loop {
            match self.rx.recv().await {
                Ok(raw) => {
                    if raw.method != E::METHOD {
                        continue;
                    }
                    match serde_json::from_value::<E>(raw.params) {
                        Ok(ev) => return Some(EventItem::Event(ev)),
                        Err(e) => {
                            tracing::warn!(error = %e, method = %raw.method, "failed to decode CDP event");
                            continue;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    return Some(EventItem::Lagged(LagInfo { skipped }));
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    /// Non-blocking drain: returns the next already-buffered event matching
    /// `E::METHOD`, or `None` if nothing is pending right now (popup-attach
    /// spec Decision #2 -- this project has no persistent event loop, so
    /// callers poll this at the point they actually want to know "has
    /// anything new arrived", rather than a background task pushing state).
    pub fn try_next(&mut self) -> Option<EventItem<E>> {
        loop {
            match self.rx.try_recv() {
                Ok(raw) => {
                    if raw.method != E::METHOD {
                        continue;
                    }
                    match serde_json::from_value::<E>(raw.params) {
                        Ok(ev) => return Some(EventItem::Event(ev)),
                        Err(e) => {
                            tracing::warn!(error = %e, method = %raw.method, "failed to decode CDP event");
                            continue;
                        }
                    }
                }
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    return Some(EventItem::Lagged(LagInfo { skipped }));
                }
                Err(broadcast::error::TryRecvError::Empty)
                | Err(broadcast::error::TryRecvError::Closed) => return None,
            }
        }
    }
}
