use crate::error::{CdpError, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

/// Seam for the wire protocol under CDP messages. Phase 0 ships only
/// [`WebSocketTransport`]; a future `--remote-debugging-pipe` transport can
/// implement this trait without touching `Connection`.
pub trait Transport: Send + 'static {
    fn send(&mut self, msg: String) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Returns `Ok(None)` when the underlying stream closed cleanly.
    fn recv(&mut self) -> impl std::future::Future<Output = Result<Option<String>>> + Send;
}

pub struct WebSocketTransport {
    stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
}

impl WebSocketTransport {
    pub async fn connect(url: &str) -> Result<Self> {
        let (stream, _response) = tokio_tungstenite::connect_async(url).await?;
        Ok(Self { stream })
    }
}

impl Transport for WebSocketTransport {
    async fn send(&mut self, msg: String) -> Result<()> {
        self.stream.send(Message::Text(msg)).await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        loop {
            match self.stream.next().await {
                None => return Ok(None),
                Some(Ok(Message::Text(text))) => return Ok(Some(text.to_string())),
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(_)) => continue, // ignore ping/pong/binary frames
                Some(Err(e)) => return Err(CdpError::WebSocket(e)),
            }
        }
    }
}
