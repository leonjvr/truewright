//! End-to-end recording test against an animated local fixture: start a
//! recording, let the animation run briefly, stop, and verify real frames
//! and a playable GIF were produced. Skips (not fails) when no browser is
//! installed, matching the project's integration-test convention.

use engine::{RecordingOptions, Session};
use std::time::Duration;

fn fixture_url() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/animated.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

#[tokio::test]
async fn records_an_animated_page() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping records_an_animated_page: no installed browser found");
        return;
    }

    let session = Session::launch("engine-recording-test", true)
        .await
        .expect("session launches");

    session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");

    let recording = session
        .start_recording(RecordingOptions {
            max_duration: Duration::from_millis(1500),
            quality: 60,
        })
        .await
        .expect("recording starts");

    tokio::time::sleep(Duration::from_millis(1200)).await;

    let output = recording.stop().await.expect("recording stops");

    assert!(
        output.frame_count > 1,
        "expected more than one captured frame, got {}",
        output.frame_count
    );
    assert!(
        output.dir.is_dir(),
        "recording dir should exist: {:?}",
        output.dir
    );
    assert!(output.dir.join("manifest.json").is_file());
    assert!(output.dir.join("frame_0001.jpg").is_file());

    let gif_path = output
        .gif_path
        .expect("gif path present for non-empty recording");
    let gif_bytes = std::fs::read(&gif_path).expect("read gif file");
    assert!(!gif_bytes.is_empty(), "gif should not be empty");
    assert_eq!(&gif_bytes[..3], b"GIF", "should be a valid GIF file header");

    let preview = output
        .preview_jpeg
        .expect("preview frame present for non-empty recording");
    assert!(!preview.is_empty());

    let manifest_text = std::fs::read_to_string(output.dir.join("manifest.json")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).expect("valid manifest json");
    assert_eq!(manifest.as_array().unwrap().len(), output.frame_count);

    session.close().await.expect("close succeeds");
}
