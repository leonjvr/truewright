use crate::error::{CdpError, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

/// Seam for the wire protocol under CDP messages. Two implementations ship:
/// [`WebSocketTransport`] over a TCP DevTools endpoint, and (on Unix) the
/// [`PipeTransport`] used for `--remote-debugging-pipe` — either plugs into
/// `Connection` unchanged.
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

/// CDP over a `--remote-debugging-pipe` pair (Unix). Chrome reads command
/// messages from its fd 3 and writes responses/events to its fd 4; each
/// message is a JSON object terminated by a single `\0` byte. This owns the
/// parent-side ends of those two pipes: `writer` feeds Chrome's fd 3, `reader`
/// drains Chrome's fd 4. Framing is handled here (accumulate until `\0`),
/// keeping `Connection` transport-agnostic.
#[cfg(unix)]
pub struct PipeTransport {
    reader: tokio::net::unix::pipe::Receiver,
    writer: tokio::net::unix::pipe::Sender,
    /// Bytes read from `reader` not yet split into a complete `\0`-delimited
    /// message. A single read can carry several messages or a partial one.
    read_buf: Vec<u8>,
}

#[cfg(unix)]
impl PipeTransport {
    /// Builds the transport from the parent-side raw fds: `write_fd` is the
    /// write end of the pipe Chrome reads commands from, `read_fd` the read
    /// end of the pipe Chrome writes to. Takes ownership of both fds (wrapping
    /// each in a `File`, so they close when this transport drops). Must be
    /// called from within a Tokio runtime — the pipe ends register with the
    /// reactor.
    pub(crate) fn from_parent_fds(
        write_fd: std::os::unix::io::RawFd,
        read_fd: std::os::unix::io::RawFd,
    ) -> Result<Self> {
        use std::os::unix::io::FromRawFd;
        let writer_file = unsafe { std::fs::File::from_raw_fd(write_fd) };
        let reader_file = unsafe { std::fs::File::from_raw_fd(read_fd) };
        let writer = tokio::net::unix::pipe::Sender::from_file(writer_file)?;
        let reader = tokio::net::unix::pipe::Receiver::from_file(reader_file)?;
        Ok(Self {
            reader,
            writer,
            read_buf: Vec::with_capacity(16 * 1024),
        })
    }
}

#[cfg(unix)]
impl Transport for PipeTransport {
    async fn send(&mut self, msg: String) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        self.writer.write_all(msg.as_bytes()).await?;
        self.writer.write_all(b"\0").await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        use tokio::io::AsyncReadExt;
        loop {
            // Emit any complete message already buffered before reading more.
            if let Some(pos) = self.read_buf.iter().position(|&b| b == 0) {
                let text = String::from_utf8_lossy(&self.read_buf[..pos]).into_owned();
                self.read_buf.drain(..=pos);
                if text.is_empty() {
                    continue; // skip stray empty frames
                }
                return Ok(Some(text));
            }
            let mut chunk = [0u8; 16 * 1024];
            let n = self.reader.read(&mut chunk).await?;
            if n == 0 {
                return Ok(None); // Chrome closed its write end (exited)
            }
            self.read_buf.extend_from_slice(&chunk[..n]);
        }
    }
}

#[cfg(all(test, unix))]
mod pipe_tests {
    use super::*;

    fn raw_pipe() -> (libc::c_int, libc::c_int) {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe() failed");
        (fds[0], fds[1])
    }

    fn blocking_write(fd: libc::c_int, bytes: &[u8]) {
        let n = unsafe { libc::write(fd, bytes.as_ptr() as *const _, bytes.len()) };
        assert_eq!(n, bytes.len() as isize, "short write");
    }

    fn blocking_read(fd: libc::c_int, buf: &mut [u8]) -> usize {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        assert!(n >= 0, "read failed");
        n as usize
    }

    /// Frames on `\0`, buffers a trailing message across reads, appends the
    /// delimiter on send, and reports EOF as `None`.
    #[tokio::test]
    async fn pipe_transport_frames_on_nul_and_reports_eof() {
        // cmd pipe: transport writes -> test reads.
        let (cmd_r, cmd_w) = raw_pipe();
        // evt pipe: test writes -> transport reads.
        let (evt_r, evt_w) = raw_pipe();

        let mut transport = PipeTransport::from_parent_fds(cmd_w, evt_r).expect("wrap parent fds");

        // Two complete frames delivered in one write: the second must survive
        // in the buffer after the first is drained.
        blocking_write(evt_w, b"{\"a\":1}\0{\"b\":2}\0");
        assert_eq!(
            transport.recv().await.unwrap().as_deref(),
            Some("{\"a\":1}")
        );
        assert_eq!(
            transport.recv().await.unwrap().as_deref(),
            Some("{\"b\":2}")
        );

        // send appends the NUL terminator.
        transport.send("hello".to_string()).await.unwrap();
        let mut buf = [0u8; 16];
        let n = blocking_read(cmd_r, &mut buf);
        assert_eq!(&buf[..n], b"hello\0");

        // Closing the write end Chrome would hold surfaces as a clean EOF.
        unsafe { libc::close(evt_w) };
        assert!(transport.recv().await.unwrap().is_none());

        unsafe { libc::close(cmd_r) };
    }
}
