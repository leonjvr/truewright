//! Screencast-based video recording (browser-recording spec). A background
//! collector task consumes `Page.screencastFrame` events off the same
//! bounded event-stream infrastructure `navigate_and_wait` uses for
//! lifecycle events, acking each frame so CDP keeps streaming.

use crate::error::{EngineError, Result};
use cdp::ops::Page;
use cdp::protocol::page::{ScreencastFrame, StartScreencastParams};
use cdp::session::EventItem;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

/// Hard ceiling on recording length regardless of caller-requested
/// duration, so a forgotten `browser_record_stop` can't grow unbounded
/// (browser-recording spec: "automatic stop after the maximum duration").
pub const MAX_RECORDING_DURATION: Duration = Duration::from_secs(30);
const DEFAULT_QUALITY: u8 = 60;
// Capped well below the viewport's native size: keeps individual frames
// (and so GIF color-quantization/encoding cost) small — a "see the moving
// parts" preview doesn't need full resolution.
const CAPTURE_MAX_WIDTH: i64 = 480;
const CAPTURE_MAX_HEIGHT: i64 = 360;
// CDP streams every repainted frame by default (~60fps for anything
// animating), far denser than a short preview clip needs; keeping roughly
// 1 in 3 still reads as smooth motion while cutting frame count (and GIF
// assembly cost) accordingly.
const CAPTURE_EVERY_NTH_FRAME: i64 = 3;

pub struct RecordingOptions {
    /// Capped to `MAX_RECORDING_DURATION` regardless of what's requested.
    pub max_duration: Duration,
    /// JPEG quality, 0-100.
    pub quality: u8,
}

impl Default for RecordingOptions {
    fn default() -> Self {
        Self {
            max_duration: MAX_RECORDING_DURATION,
            quality: DEFAULT_QUALITY,
        }
    }
}

struct CapturedFrame {
    jpeg_bytes: Vec<u8>,
    timestamp: f64,
}

/// A recording in progress. `stop()` halts capture and writes artifacts.
pub struct Recording {
    frames: std::sync::Arc<Mutex<Vec<CapturedFrame>>>,
    stop_tx: Option<oneshot::Sender<()>>,
    collector: JoinHandle<()>,
    recordings_dir: PathBuf,
}

/// Result of a completed recording (browser-recording spec: "Recording
/// artifacts"). `gif_path`/`preview_jpeg` are `None` only when zero frames
/// were captured.
pub struct RecordingOutput {
    pub dir: PathBuf,
    pub frame_count: usize,
    pub duration_ms: f64,
    pub gif_path: Option<PathBuf>,
    /// Raw JPEG bytes of one frame from partway through the recording, for
    /// returning as inline MCP image content without the whole clip.
    pub preview_jpeg: Option<Vec<u8>>,
}

impl Recording {
    pub(crate) async fn start(
        page: &Page,
        options: RecordingOptions,
        recordings_base: PathBuf,
    ) -> Result<Self> {
        let max_duration = options.max_duration.min(MAX_RECORDING_DURATION);

        page.start_screencast(StartScreencastParams {
            format: Some("jpeg".to_string()),
            quality: Some(options.quality as i64),
            max_width: Some(CAPTURE_MAX_WIDTH),
            max_height: Some(CAPTURE_MAX_HEIGHT),
            every_nth_frame: Some(CAPTURE_EVERY_NTH_FRAME),
        })
        .await?;

        let frames = std::sync::Arc::new(Mutex::new(Vec::new()));
        let (stop_tx, stop_rx) = oneshot::channel();

        let collector = tokio::spawn(collect_frames(
            page.clone(),
            frames.clone(),
            stop_rx,
            max_duration,
        ));

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let recordings_dir = recordings_base.join(id.to_string());

        Ok(Self {
            frames,
            stop_tx: Some(stop_tx),
            collector,
            recordings_dir,
        })
    }

    /// Stops capture (if not already auto-stopped) and writes the frame
    /// sequence, manifest, and GIF to disk. The actual file/image work is
    /// CPU-bound and synchronous, so it runs on the blocking thread pool
    /// rather than tying up an async worker thread.
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub async fn stop(mut self) -> Result<RecordingOutput> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.collector).await;

        let frames = std::mem::take(&mut *self.frames.lock().await);
        let recordings_dir = self.recordings_dir;

        tokio::task::spawn_blocking(move || write_output(recordings_dir, frames))
            .await
            .map_err(|e| {
                EngineError::Recording(format!("recording finalization task panicked: {e}"))
            })?
    }
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
fn write_output(recordings_dir: PathBuf, frames: Vec<CapturedFrame>) -> Result<RecordingOutput> {
    let frame_count = frames.len();
    let duration_ms = match (frames.first(), frames.last()) {
        (Some(first), Some(last)) => (last.timestamp - first.timestamp) * 1000.0,
        _ => 0.0,
    };

    if frames.is_empty() {
        return Ok(RecordingOutput {
            dir: recordings_dir,
            frame_count: 0,
            duration_ms: 0.0,
            gif_path: None,
            preview_jpeg: None,
        });
    }

    std::fs::create_dir_all(&recordings_dir)
        .map_err(|e| EngineError::Recording(format!("failed to create recording dir: {e}")))?;

    let manifest = write_frames(&recordings_dir, &frames)?;
    std::fs::write(
        recordings_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)
            .map_err(|e| EngineError::Recording(format!("failed to serialize manifest: {e}")))?,
    )
    .map_err(|e| EngineError::Recording(format!("failed to write manifest: {e}")))?;

    let gif_path = recordings_dir.join("clip.gif");
    assemble_gif(&frames, &gif_path)?;

    let preview_jpeg = frames[frames.len() / 2].jpeg_bytes.clone();

    Ok(RecordingOutput {
        dir: recordings_dir,
        frame_count,
        duration_ms,
        gif_path: Some(gif_path),
        preview_jpeg: Some(preview_jpeg),
    })
}

async fn collect_frames(
    page: Page,
    frames: std::sync::Arc<Mutex<Vec<CapturedFrame>>>,
    mut stop_rx: oneshot::Receiver<()>,
    max_duration: Duration,
) {
    use base64::Engine;

    let mut events = page.events::<ScreencastFrame>();
    let deadline = tokio::time::sleep(max_duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            _ = &mut deadline => break,
            item = events.next() => {
                match item {
                    Some(EventItem::Event(frame)) => {
                        if let Ok(jpeg_bytes) = base64::engine::general_purpose::STANDARD.decode(&frame.data) {
                            frames.lock().await.push(CapturedFrame {
                                jpeg_bytes,
                                timestamp: frame.metadata.timestamp,
                            });
                        }
                        // Fire-and-forget: CDP just wants the ack sent to keep
                        // the stream flowing. Awaiting its response here
                        // would block this loop (and so the stop/deadline
                        // checks above) on every single frame if that
                        // particular round trip is ever slow.
                        let ack_page = page.clone();
                        let frame_ack_id = frame.frame_ack_id;
                        tokio::spawn(async move {
                            let _ = ack_page.ack_screencast_frame(frame_ack_id).await;
                        });
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break, // connection closed
                }
            }
        }
    }

    let _ = page.stop_screencast().await;
}

#[derive(serde::Serialize)]
struct ManifestEntry {
    index: usize,
    file: String,
    timestamp: f64,
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
fn write_frames(dir: &std::path::Path, frames: &[CapturedFrame]) -> Result<Vec<ManifestEntry>> {
    let mut manifest = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let file = format!("frame_{:04}.jpg", i + 1);
        std::fs::write(dir.join(&file), &frame.jpeg_bytes)
            .map_err(|e| EngineError::Recording(format!("failed to write {file}: {e}")))?;
        manifest.push(ManifestEntry {
            index: i + 1,
            file,
            timestamp: frame.timestamp,
        });
    }
    Ok(manifest)
}

/// Assembles an animated GIF, using per-frame delays derived from the real
/// capture timestamps rather than a fixed rate (browser-recording spec:
/// "Recording artifacts").
#[allow(clippy::result_large_err)]
fn assemble_gif(frames: &[CapturedFrame], out_path: &std::path::Path) -> Result<()> {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame as GifFrame};

    let file = std::fs::File::create(out_path).map_err(|e| {
        EngineError::Recording(format!("failed to create {}: {e}", out_path.display()))
    })?;
    let mut encoder = GifEncoder::new(std::io::BufWriter::new(file));

    for (i, frame) in frames.iter().enumerate() {
        let img = image::load_from_memory_with_format(&frame.jpeg_bytes, image::ImageFormat::Jpeg)
            .map_err(|e| EngineError::Recording(format!("failed to decode frame {}: {e}", i + 1)))?
            .into_rgba8();

        let delay_ms = if i + 1 < frames.len() {
            ((frames[i + 1].timestamp - frame.timestamp) * 1000.0).max(20.0)
        } else {
            100.0
        };
        let gif_frame = GifFrame::from_parts(
            img,
            0,
            0,
            Delay::from_saturating_duration(Duration::from_millis(delay_ms as u64)),
        );

        encoder.encode_frame(gif_frame).map_err(|e| {
            EngineError::Recording(format!("failed to encode frame {}: {e}", i + 1))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_jpeg(color: [u8; 3]) -> Vec<u8> {
        let mut img = image::RgbImage::new(4, 4);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb(color);
        }
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .expect("encode synthetic jpeg");
        bytes
    }

    #[test]
    fn write_frames_and_manifest_round_trip() {
        let dir = std::env::temp_dir().join(format!("aib-rec-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let frames = vec![
            CapturedFrame {
                jpeg_bytes: synthetic_jpeg([255, 0, 0]),
                timestamp: 0.0,
            },
            CapturedFrame {
                jpeg_bytes: synthetic_jpeg([0, 255, 0]),
                timestamp: 0.1,
            },
            CapturedFrame {
                jpeg_bytes: synthetic_jpeg([0, 0, 255]),
                timestamp: 0.2,
            },
        ];

        let manifest = write_frames(&dir, &frames).expect("write frames");
        assert_eq!(manifest.len(), 3);
        assert!(dir.join("frame_0001.jpg").is_file());
        assert!(dir.join("frame_0003.jpg").is_file());
        assert_eq!(manifest[1].timestamp, 0.1);

        let gif_path = dir.join("clip.gif");
        assemble_gif(&frames, &gif_path).expect("assemble gif");
        let gif_bytes = std::fs::read(&gif_path).expect("read gif");
        assert!(!gif_bytes.is_empty());
        assert_eq!(&gif_bytes[..3], b"GIF");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
