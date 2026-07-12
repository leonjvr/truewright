//! Minimal hand-rolled OpenAI-compatible `/chat/completions` stub server
//! (mcp-task-delegation spec: verified against a real Chrome + a real
//! local HTTP LLM stub over a real streamable-HTTP MCP client, not an
//! in-process mock). Duplicated from `crates/agent/tests/support/llm_stub.rs`
//! rather than shared -- that one is private to its own crate's `tests/`
//! directory, the same reason `crates/agent` couldn't reuse
//! `crates/llm`'s or `crates/engine`'s own stub servers either. Returns a
//! scripted FIFO sequence of full chat-completion response bodies -- one
//! per request received, repeating the last entry once exhausted -- and
//! records every request body for assertions.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

struct State {
    responses: Vec<serde_json::Value>,
    next_response: usize,
    requests: Vec<serde_json::Value>,
}

pub struct LlmStub {
    port: u16,
    stop_tx: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
    state: Arc<Mutex<State>>,
}

impl LlmStub {
    pub async fn start(script: Vec<serde_json::Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind llm stub");
        let port = listener.local_addr().expect("local addr").port();
        let state = Arc::new(Mutex::new(State {
            responses: script,
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

    /// `CompatClient` posts to `{base_url}/chat/completions` -- this is
    /// the `base_url` half, matching `llm::ProviderConfig.base_url`.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    #[allow(dead_code)]
    pub async fn requests(&self) -> Vec<serde_json::Value> {
        self.state.lock().await.requests.clone()
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

    let mut headers = std::collections::HashMap::new();
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

    let response_body = {
        let mut guard = state.lock().await;
        guard.requests.push(body);
        let idx = guard
            .next_response
            .min(guard.responses.len().saturating_sub(1));
        let resp = guard.responses.get(idx).cloned().unwrap_or_else(|| serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "no scripted response left"}, "finish_reason": "stop"}]
        }));
        if guard.next_response < guard.responses.len() {
            guard.next_response += 1;
        }
        resp
    };

    let body_text = response_body.to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_text}",
        body_text.len()
    );
    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.shutdown().await;
}
