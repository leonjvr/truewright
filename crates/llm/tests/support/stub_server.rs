//! Minimal hand-rolled HTTP/1.1 stub server for testing `CompatClient`
//! against a real local socket (llm-providers spec: verified against a
//! real local server, not just serialized-shape assertions -- same
//! discipline as `crates/engine/tests/support/http_server.rs`, but this one
//! reads request bodies/headers and returns a scripted sequence of
//! status/body pairs, which the engine test server never needed to do).

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

pub struct RecordedRequest {
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
}

struct State {
    /// Scripted (status, body) pairs returned in order; once exhausted, the
    /// last entry repeats for any further requests.
    responses: Vec<(u16, serde_json::Value)>,
    next_response: usize,
    requests: Vec<RecordedRequest>,
}

pub struct StubServer {
    port: u16,
    stop_tx: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
    state: Arc<Mutex<State>>,
}

impl StubServer {
    /// Starts the server with a scripted sequence of `(status, body)`
    /// responses, one per request received, repeating the last entry once
    /// exhausted.
    pub async fn start(responses: Vec<(u16, serde_json::Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stub server");
        let port = listener.local_addr().expect("local addr").port();
        let state = Arc::new(Mutex::new(State {
            responses,
            next_response: 0,
            requests: Vec::new(),
        }));
        let (stop_tx, mut stop_rx) = oneshot::channel();

        let server_state = state.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, _)) = accepted {
                            tokio::spawn(handle_connection(stream, server_state.clone()));
                        }
                    }
                }
            }
        });

        Self {
            port,
            stop_tx: Some(stop_tx),
            handle: Some(handle),
            state,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub async fn requests(&self) -> Vec<RecordedRequest> {
        let mut guard = self.state.lock().await;
        std::mem::take(&mut guard.requests)
            .into_iter()
            .map(|r| RecordedRequest {
                headers: r.headers,
                body: r.body,
            })
            .collect()
    }

    pub async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }
}

async fn handle_connection(stream: TcpStream, state: Arc<Mutex<State>>) {
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.is_err() || request_line.is_empty() {
        return;
    }

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => return,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {
                if let Some((name, value)) = line.split_once(':') {
                    headers.insert(name.trim().to_lowercase(), value.trim().to_string());
                }
            }
            Err(_) => return,
        }
    }

    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body_bytes = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body_bytes).await.is_err() {
        return;
    }
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).unwrap_or(serde_json::Value::Null);

    let (status, resp_body) = {
        let mut guard = state.lock().await;
        guard.requests.push(RecordedRequest {
            headers: headers.clone(),
            body: body.clone(),
        });
        let idx = guard
            .next_response
            .min(guard.responses.len().saturating_sub(1));
        let entry = guard
            .responses
            .get(idx)
            .cloned()
            .unwrap_or((500, serde_json::json!({"error": "no scripted response"})));
        if guard.next_response < guard.responses.len() {
            guard.next_response += 1;
        }
        entry
    };

    let status_line = match status {
        200 => "HTTP/1.1 200 OK",
        400 => "HTTP/1.1 400 Bad Request",
        401 => "HTTP/1.1 401 Unauthorized",
        429 => "HTTP/1.1 429 Too Many Requests",
        500 => "HTTP/1.1 500 Internal Server Error",
        503 => "HTTP/1.1 503 Service Unavailable",
        _ => "HTTP/1.1 500 Internal Server Error",
    };
    let body_text = resp_body.to_string();
    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_text}",
        body_text.len()
    );
    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.shutdown().await;
}
