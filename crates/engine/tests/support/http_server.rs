//! Minimal hand-rolled HTTP/1.1 test server (network-mocking spec:
//! verified against a real local server, not just mocked-in-test). Serves
//! canned responses keyed by exact path -- test-only, not shipped in the
//! release binary.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub struct Route {
    pub content_type: &'static str,
    pub body: String,
}

pub struct TestServer {
    port: u16,
    stop_tx: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    pub async fn start(routes: HashMap<String, Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let port = listener.local_addr().expect("local addr").port();
        let routes = Arc::new(routes);
        let (stop_tx, mut stop_rx) = oneshot::channel();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, _)) = accepted {
                            tokio::spawn(handle_connection(stream, routes.clone()));
                        }
                    }
                }
            }
        });

        Self {
            port,
            stop_tx: Some(stop_tx),
            handle: Some(handle),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
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

async fn handle_connection(stream: TcpStream, routes: Arc<HashMap<String, Route>>) {
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.is_err() || request_line.is_empty() {
        return;
    }
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => return,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
        }
    }

    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();

    let (status_line, content_type, body) = match routes.get(&path) {
        Some(route) => ("HTTP/1.1 200 OK", route.content_type, route.body.clone()),
        None => ("HTTP/1.1 404 Not Found", "text/plain", "not found".to_string()),
    };

    let response = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n{body}",
        body.len()
    );
    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.shutdown().await;
}
