//! One-shot local callback listener for the OAuth authorize redirect
//! (oauth-subscription-auth spec: "Local callback server"). Hand-rolled,
//! not `axum` -- this crate makes real outbound HTTPS calls but has no
//! other reason to depend on a web framework, and a real socket accepting
//! exactly one GET request is ~60 lines (mirrors the style already used
//! for this project's own test HTTP servers).

use crate::error::{LlmError, Result};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

pub struct CallbackResult {
    pub code: String,
    pub state: String,
}

const SUCCESS_PAGE: &str = "<html><body><h1>Signed in</h1><p>You can close this tab and return to the terminal.</p></body></html>";
const ERROR_PAGE: &str = "<html><body><h1>Sign-in failed</h1><p>You can close this tab and return to the terminal.</p></body></html>";

/// Binds `127.0.0.1:port` -- separate from waiting for the actual request
/// so the caller can bind first, *then* print/open the authorize URL,
/// rather than racing a fast browser redirect against a listener that
/// isn't up yet.
pub async fn bind(port: u16) -> Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| LlmError::OAuthCallbackServer(format!("failed to bind 127.0.0.1:{port}: {e}")))
}

/// Waits for exactly one request to `expected_path` carrying `code`/`state`
/// (or an `error` param if the user denied consent), and returns it.
/// Non-matching requests (a stray favicon fetch, a browser prefetch) are
/// answered with a plain 404 and don't count -- the listener keeps
/// waiting for the real callback until `timeout` elapses.
pub async fn accept_one(
    listener: TcpListener,
    expected_path: &str,
    timeout: Duration,
) -> Result<CallbackResult> {
    let accept_loop = async {
        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| LlmError::OAuthCallbackServer(e.to_string()))?;
            if let Some(outcome) = handle_one_connection(stream, expected_path).await {
                return outcome;
            }
        }
    };

    tokio::time::timeout(timeout, accept_loop)
        .await
        .map_err(|_| {
            LlmError::OAuthCallbackServer(
                "timed out waiting for the browser sign-in to complete".to_string(),
            )
        })?
}

/// Returns `None` for a request that isn't the callback we're waiting for
/// (keep listening), `Some(Ok(..))`/`Some(Err(..))` once the real
/// callback arrives.
async fn handle_one_connection(
    stream: tokio::net::TcpStream,
    expected_path: &str,
) -> Option<Result<CallbackResult>> {
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.is_err() || request_line.is_empty() {
        return None;
    }
    // Drain remaining headers so the client sees a clean connection close.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let path_and_query = request_line.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    if path != expected_path {
        write_response(&mut write_half, 404, "not found").await;
        return None;
    }

    let params = parse_query(query);
    if let Some(error) = params.get("error") {
        write_response(&mut write_half, 200, ERROR_PAGE).await;
        return Some(Err(LlmError::OAuthLoginFailed {
            provider: "".to_string(),
            reason: error.clone(),
        }));
    }
    let (Some(code), Some(state)) = (params.get("code"), params.get("state")) else {
        write_response(&mut write_half, 400, ERROR_PAGE).await;
        return Some(Err(LlmError::OAuthLoginFailed {
            provider: "".to_string(),
            reason: "callback was missing code/state".to_string(),
        }));
    };

    write_response(&mut write_half, 200, SUCCESS_PAGE).await;
    Some(Ok(CallbackResult {
        code: code.clone(),
        state: state.clone(),
    }))
}

async fn write_response(write_half: &mut (impl AsyncWriteExt + Unpin), status: u16, body: &str) {
    let status_line = match status {
        200 => "HTTP/1.1 200 OK",
        400 => "HTTP/1.1 400 Bad Request",
        _ => "HTTP/1.1 404 Not Found",
    };
    let response = format!(
        "{status_line}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.shutdown().await;
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_handles_plus_and_percent_escapes() {
        assert_eq!(percent_decode("hello+world"), "hello world");
        assert_eq!(percent_decode("a%3Db"), "a=b");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn parse_query_extracts_multiple_params() {
        let params = parse_query("code=abc123&state=xyz789");
        assert_eq!(params.get("code"), Some(&"abc123".to_string()));
        assert_eq!(params.get("state"), Some(&"xyz789".to_string()));
    }
}
