//! Training capture and persona fitting (human-motion spec: "Training
//! capture from real trusted input", "Persona fitted from captured
//! samples"). The collector task mirrors `recording.rs`'s `collect_frames`
//! shape; the fitted `Persona` is consumed by the exact same
//! `mouse_path`/`typing_timeline` synthesis the synthetic presets use.

use super::Persona;
use crate::error::{EngineError, Result};
use cdp::ops::Page;
use cdp::protocol::runtime::BindingCalled;
use cdp::session::EventItem;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

const TRAIN_JS: &str = include_str!("../../assets/train.js");
const BINDING_NAME: &str = "__truewrightTrainReport";
/// Hard ceiling so a forgotten `browser_train_stop` can't capture
/// indefinitely (mirrors `recording.rs`'s `MAX_RECORDING_DURATION`).
const MAX_TRAINING_DURATION: Duration = Duration::from_secs(300);
/// A movement shorter than this is noise (mouse settling, tiny wobble), not
/// a deliberate approach to a target — excluded from the Fitts's-law fit.
const MIN_MOVEMENT_DISTANCE_PX: f64 = 5.0;
const MIN_MOVEMENTS_FOR_FIT: usize = 3;
const MIN_KEYSTROKES_FOR_FIT: usize = 5;
/// A movement whose peak progress along its chord exceeds this is treated
/// as having overshot the endpoint before the terminating `mousedown`.
const OVERSHOOT_PROJECTION_THRESHOLD: f64 = 1.05;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Sample {
    Mousemove { x: f64, y: f64, t: f64 },
    Mousedown { x: f64, y: f64, t: f64 },
    Mouseup { x: f64, y: f64, t: f64 },
    Keydown { key: String, t: f64 },
    Keyup { key: String, t: f64 },
}

/// Counts of the samples actually usable in the fit -- distinct from raw
/// sample counts (e.g. a `mousemove` burst well under
/// `MIN_MOVEMENT_DISTANCE_PX` contributes no usable movement).
#[derive(Debug)]
pub struct FitStats {
    pub movements_used: usize,
    pub keystrokes_used: usize,
}

/// A training session in progress. `stop()` halts capture, fits a
/// `Persona`, and persists it by name -- self-contained, mirroring
/// `Recording::stop`'s "everything happens on the owned value, no `Session`
/// needed again" shape.
pub struct Training {
    page: Page,
    name: String,
    samples: Arc<Mutex<Vec<Sample>>>,
    stop_tx: Option<oneshot::Sender<()>>,
    collector: JoinHandle<()>,
    training_active: Arc<AtomicBool>,
}

impl Training {
    pub(crate) async fn start(
        page: &Page,
        name: &str,
        training_active: Arc<AtomicBool>,
    ) -> Result<Self> {
        page.add_binding(BINDING_NAME).await?;
        let install: serde_json::Value = serde_json::from_value(
            page.evaluate(&format!(
                "({TRAIN_JS})({}, {})",
                serde_json::to_string(BINDING_NAME)?,
                serde_json::to_string("start")?
            ))
            .await?,
        )?;
        if install.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            return Err(EngineError::Training(format!(
                "failed to install training recorder: {install}"
            )));
        }

        let samples = Arc::new(Mutex::new(Vec::new()));
        let (stop_tx, stop_rx) = oneshot::channel();
        let collector = tokio::spawn(collect_samples(
            page.clone(),
            samples.clone(),
            stop_rx,
            MAX_TRAINING_DURATION,
        ));

        Ok(Self {
            page: page.clone(),
            name: name.to_string(),
            samples,
            stop_tx: Some(stop_tx),
            collector,
            training_active,
        })
    }

    /// Stops capture, fits a `Persona` from what was captured, and persists
    /// it under this training session's name. Fails without saving anything
    /// if there wasn't enough data to fit (human-motion spec: "Persona
    /// fitted from captured samples").
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub async fn stop(mut self) -> Result<super::profile_store::StoredProfile> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.collector).await;

        let _ = self
            .page
            .evaluate(&format!(
                "({TRAIN_JS})({}, {})",
                serde_json::to_string("")?,
                serde_json::to_string("stop")?
            ))
            .await;
        let _ = self.page.remove_binding(BINDING_NAME).await;
        // Capture is done regardless of whether fitting below succeeds --
        // clear the suppression flag now so a fit failure can't leave
        // Session::click/type_with permanently suppressing themselves.
        self.training_active.store(false, Ordering::SeqCst);

        let samples = std::mem::take(&mut *self.samples.lock().await);
        let (persona, stats) = fit_persona(&samples)?;
        super::profile_store::save(
            &self.name,
            persona,
            stats.movements_used,
            stats.keystrokes_used,
        )
    }
}

impl Drop for Training {
    /// Safety net: if a `Training` is dropped without `stop()` being called
    /// (e.g. an MCP client disconnects mid-session), `training_active` must
    /// not stay stuck `true` -- that would permanently suppress this
    /// session's own click/type training-recorder toggle forever. Mirrors
    /// `LaunchedBrowser`'s `Drop` safety net from Phase 1.
    fn drop(&mut self) {
        self.training_active.store(false, Ordering::SeqCst);
    }
}

async fn collect_samples(
    page: Page,
    samples: Arc<Mutex<Vec<Sample>>>,
    mut stop_rx: oneshot::Receiver<()>,
    max_duration: Duration,
) {
    let mut events = page.events::<BindingCalled>();
    let deadline = tokio::time::sleep(max_duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut stop_rx => break,
            _ = &mut deadline => break,
            item = events.next() => {
                match item {
                    Some(EventItem::Event(call)) => {
                        if call.name != BINDING_NAME {
                            continue;
                        }
                        if let Ok(sample) = serde_json::from_str::<Sample>(&call.payload) {
                            samples.lock().await.push(sample);
                        }
                    }
                    Some(EventItem::Lagged(_)) => continue,
                    None => break,
                }
            }
        }
    }
}

struct Movement {
    start: (f64, f64),
    end: (f64, f64),
    duration_ms: f64,
    points: Vec<(f64, f64)>,
    overshot: bool,
}

/// Fits a `Persona` from raw captured samples (human-motion spec: "Persona
/// fitted from captured samples"). Fails without persisting anything if
/// there isn't enough data for a stable fit -- see `MIN_MOVEMENTS_FOR_FIT`/
/// `MIN_KEYSTROKES_FOR_FIT`.
#[allow(clippy::result_large_err)]
pub fn fit_persona(samples: &[Sample]) -> Result<(Persona, FitStats)> {
    let movements = segment_movements(samples);
    let usable: Vec<&Movement> = movements
        .iter()
        .filter(|m| chord_distance(m) >= MIN_MOVEMENT_DISTANCE_PX)
        .collect();
    let keydowns: Vec<f64> = samples
        .iter()
        .filter_map(|s| match s {
            Sample::Keydown { t, .. } => Some(*t),
            _ => None,
        })
        .collect();

    if usable.len() < MIN_MOVEMENTS_FOR_FIT || keydowns.len() < MIN_KEYSTROKES_FOR_FIT {
        return Err(EngineError::Training(format!(
            "not enough training data to fit a persona: {} usable mouse movement(s) (need {}), \
             {} keystroke(s) (need {})",
            usable.len(),
            MIN_MOVEMENTS_FOR_FIT,
            keydowns.len(),
            MIN_KEYSTROKES_FOR_FIT
        )));
    }

    let (fitts_a_ms, fitts_b_ms) = fit_fitts_law(&usable);
    let jitter_px = mean_jitter(&usable);
    let overshoot_p = usable.iter().filter(|m| m.overshot).count() as f64 / usable.len() as f64;
    let (key_delay_mean_ms, key_delay_std_ms) = key_delay_stats(&keydowns);

    let stats = FitStats {
        movements_used: usable.len(),
        keystrokes_used: keydowns.len(),
    };

    let persona = Persona {
        fitts_a_ms,
        fitts_b_ms,
        jitter_px,
        overshoot_p,
        key_delay_mean_ms,
        key_delay_std_ms,
    };

    Ok((persona, stats))
}

/// A movement is the run of `mousemove` samples between one `mousedown` and
/// the next (or from capture-start to the first `mousedown`); the
/// terminating `mousedown` closes it out as the movement's endpoint.
fn segment_movements(samples: &[Sample]) -> Vec<Movement> {
    let mut movements = Vec::new();
    let mut current: Vec<(f64, f64, f64)> = Vec::new(); // (x, y, t)

    for sample in samples {
        match sample {
            Sample::Mousemove { x, y, t } => current.push((*x, *y, *t)),
            Sample::Mousedown { x, y, t } => {
                current.push((*x, *y, *t));
                if current.len() >= 2 {
                    movements.push(build_movement(&current));
                }
                current.clear();
            }
            _ => {}
        }
    }

    movements
}

fn build_movement(points: &[(f64, f64, f64)]) -> Movement {
    let start = (points[0].0, points[0].1);
    let end = (points[points.len() - 1].0, points[points.len() - 1].1);
    let duration_ms = (points[points.len() - 1].2 - points[0].2).max(0.0);
    let xy: Vec<(f64, f64)> = points.iter().map(|p| (p.0, p.1)).collect();

    let chord = (end.0 - start.0, end.1 - start.1);
    let chord_len_sq = chord.0 * chord.0 + chord.1 * chord.1;
    let overshot = chord_len_sq > 0.0
        && xy.iter().any(|p| {
            let proj = ((p.0 - start.0) * chord.0 + (p.1 - start.1) * chord.1) / chord_len_sq;
            proj > OVERSHOOT_PROJECTION_THRESHOLD
        });

    Movement {
        start,
        end,
        duration_ms,
        points: xy,
        overshot,
    }
}

fn chord_distance(m: &Movement) -> f64 {
    let dx = m.end.0 - m.start.0;
    let dy = m.end.1 - m.start.1;
    (dx * dx + dy * dy).sqrt()
}

/// Least-squares fit of `duration_ms = a + b * log2(distance + 1)`. Real
/// captured movements carry no element-geometry ("target size") data, so
/// distance is used directly rather than `distance/size` -- equivalent to
/// the synthetic model's `target_size.max(1.0)` floor case.
fn fit_fitts_law(movements: &[&Movement]) -> (f64, f64) {
    let n = movements.len() as f64;
    let points: Vec<(f64, f64)> = movements
        .iter()
        .map(|m| ((chord_distance(m) + 1.0).log2(), m.duration_ms))
        .collect();

    let sum_u: f64 = points.iter().map(|(u, _)| u).sum();
    let sum_d: f64 = points.iter().map(|(_, d)| d).sum();
    let sum_uu: f64 = points.iter().map(|(u, _)| u * u).sum();
    let sum_ud: f64 = points.iter().map(|(u, d)| u * d).sum();

    let denom = n * sum_uu - sum_u * sum_u;
    if denom.abs() < 1e-6 {
        // Degenerate: movements all cover ~the same distance, so a/b can't
        // be separated. Fall back to the built-in "average" persona's
        // constants rather than dividing by ~zero.
        let avg = Persona::average();
        return (avg.fitts_a_ms, avg.fitts_b_ms);
    }

    let b = (n * sum_ud - sum_u * sum_d) / denom;
    let a = (sum_d - b * sum_u) / n;
    (a.max(0.0), b.max(0.0))
}

/// RMS perpendicular deviation of each movement's intermediate points from
/// its own straight start->end chord, averaged across movements.
fn mean_jitter(movements: &[&Movement]) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;

    for m in movements {
        let dx = m.end.0 - m.start.0;
        let dy = m.end.1 - m.start.1;
        let chord_len = (dx * dx + dy * dy).sqrt();
        if chord_len < 1e-6 {
            continue;
        }
        for &(px, py) in &m.points {
            let cross = dx * (py - m.start.1) - dy * (px - m.start.0);
            total += (cross / chord_len).powi(2);
            count += 1;
        }
    }

    if count == 0 {
        return Persona::average().jitter_px;
    }
    (total / count as f64).sqrt()
}

/// A gap this large between consecutive keystrokes is a pause between
/// distinct typing bursts (deciding what to type next, clicking elsewhere),
/// not part of typing cadence itself -- excluded from the fit so one such
/// pause during a longer training window (which may capture more than one
/// discrete action) doesn't dominate the fitted mean/stddev.
const MAX_KEY_DELAY_GAP_MS: f64 = 800.0;

fn key_delay_stats(keydown_times: &[f64]) -> (f64, f64) {
    let mut sorted = keydown_times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let deltas: Vec<f64> = sorted
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d <= MAX_KEY_DELAY_GAP_MS)
        .collect();

    if deltas.is_empty() {
        let avg = Persona::average();
        return (avg.key_delay_mean_ms, avg.key_delay_std_ms);
    }

    let n = deltas.len() as f64;
    let mean = deltas.iter().sum::<f64>() / n;
    let variance = deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mousemove(x: f64, y: f64, t: f64) -> Sample {
        Sample::Mousemove { x, y, t }
    }
    fn mousedown(x: f64, y: f64, t: f64) -> Sample {
        Sample::Mousedown { x, y, t }
    }
    fn keydown(key: &str, t: f64) -> Sample {
        Sample::Keydown {
            key: key.to_string(),
            t,
        }
    }

    /// A straight-line, evenly-timed movement from `from` to `to`, ending
    /// in a `mousedown` -- as close to "textbook Fitts's law" as a fixture
    /// gets, so the fitted constants should be sane and non-degenerate.
    fn synthetic_movement(
        from: (f64, f64),
        to: (f64, f64),
        start_t: f64,
        duration_ms: f64,
    ) -> Vec<Sample> {
        let steps = 10;
        let mut out = Vec::new();
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = from.0 + (to.0 - from.0) * t;
            let y = from.1 + (to.1 - from.1) * t;
            let at = start_t + t * duration_ms;
            if i == steps {
                out.push(mousedown(x, y, at));
            } else {
                out.push(mousemove(x, y, at));
            }
        }
        out
    }

    fn synthetic_typing(start_t: f64, count: usize, delay_ms: f64) -> Vec<Sample> {
        (0..count)
            .map(|i| keydown("a", start_t + i as f64 * delay_ms))
            .collect()
    }

    #[test]
    fn insufficient_movements_is_rejected() {
        let mut samples = synthetic_movement((0.0, 0.0), (100.0, 0.0), 0.0, 200.0);
        samples.extend(synthetic_typing(1000.0, 10, 100.0));
        let err = fit_persona(&samples).unwrap_err();
        assert!(matches!(err, EngineError::Training(_)));
    }

    #[test]
    fn insufficient_keystrokes_is_rejected() {
        let mut samples = Vec::new();
        for i in 0..3 {
            samples.extend(synthetic_movement(
                (0.0, 0.0),
                (100.0 + i as f64 * 50.0, 0.0),
                i as f64 * 500.0,
                200.0,
            ));
        }
        samples.extend(synthetic_typing(2000.0, 2, 100.0));
        let err = fit_persona(&samples).unwrap_err();
        assert!(matches!(err, EngineError::Training(_)));
    }

    #[test]
    fn sufficient_data_fits_a_usable_persona() {
        let mut samples = Vec::new();
        for i in 0..4 {
            samples.extend(synthetic_movement(
                (0.0, 0.0),
                (80.0 + i as f64 * 60.0, 0.0),
                i as f64 * 1000.0,
                150.0 + i as f64 * 40.0,
            ));
        }
        samples.extend(synthetic_typing(5000.0, 8, 120.0));

        let (persona, stats) = fit_persona(&samples).expect("should fit");
        assert!(persona.fitts_a_ms >= 0.0);
        assert!(persona.fitts_b_ms >= 0.0);
        assert!(persona.key_delay_mean_ms > 0.0);
        // Perfectly straight synthetic movements have ~zero jitter.
        assert!(persona.jitter_px < 1.0);
        assert_eq!(stats.movements_used, 4);
        assert_eq!(stats.keystrokes_used, 8);
    }

    #[test]
    fn a_pause_between_typing_bursts_does_not_inflate_the_fitted_cadence() {
        let mut samples = Vec::new();
        for i in 0..4 {
            samples.extend(synthetic_movement(
                (0.0, 0.0),
                (80.0 + i as f64 * 60.0, 0.0),
                i as f64 * 1000.0,
                150.0,
            ));
        }
        // Two fast (100ms-cadence) bursts of 5 keystrokes each, separated
        // by a 5-second pause -- e.g. the user typed, thought for a bit,
        // then typed again during one training window.
        samples.extend(synthetic_typing(5000.0, 5, 100.0));
        samples.extend(synthetic_typing(5000.0 + 4.0 * 100.0 + 5000.0, 5, 100.0));

        let (persona, _stats) = fit_persona(&samples).expect("should fit");
        assert!(
            (persona.key_delay_mean_ms - 100.0).abs() < 5.0,
            "expected the fitted cadence to reflect the fast bursts (~100ms), got {}",
            persona.key_delay_mean_ms
        );
    }

    #[test]
    fn typing_cadence_matches_the_synthetic_delay() {
        let mut samples = Vec::new();
        for i in 0..4 {
            samples.extend(synthetic_movement(
                (0.0, 0.0),
                (80.0 + i as f64 * 60.0, 0.0),
                i as f64 * 1000.0,
                150.0,
            ));
        }
        samples.extend(synthetic_typing(5000.0, 10, 100.0));

        let (persona, _stats) = fit_persona(&samples).expect("should fit");
        assert!((persona.key_delay_mean_ms - 100.0).abs() < 0.01);
        assert!(persona.key_delay_std_ms < 0.01);
    }

    #[test]
    fn overshoot_is_detected_when_the_path_passes_the_endpoint() {
        let mut samples = Vec::new();
        // Three clean movements, no overshoot.
        for i in 0..3 {
            samples.extend(synthetic_movement(
                (0.0, 0.0),
                (100.0 + i as f64 * 40.0, 0.0),
                i as f64 * 1000.0,
                150.0,
            ));
        }
        // One movement that visibly overshoots past x=150 before landing at x=100.
        let overshoot_points = vec![
            mousemove(0.0, 0.0, 3000.0),
            mousemove(80.0, 0.0, 3050.0),
            mousemove(160.0, 0.0, 3100.0),
            mousemove(120.0, 0.0, 3150.0),
            mousedown(100.0, 0.0, 3200.0),
        ];
        samples.extend(overshoot_points);
        samples.extend(synthetic_typing(5000.0, 6, 90.0));

        let (persona, _stats) = fit_persona(&samples).expect("should fit");
        assert!(
            persona.overshoot_p > 0.0,
            "expected at least one overshoot to be detected"
        );
    }
}
