//! Curved, timed mouse-path synthesis (human-motion spec: "Curved, timed
//! mouse movement to a target").

use super::{normal_sample, Persona};
use rand::Rng;
use rand_pcg::Pcg64;

#[derive(Debug, Clone, Copy)]
pub struct TimedPoint {
    pub at_ms: f64,
    pub x: f64,
    pub y: f64,
}

const FRAME_MS: f64 = 16.0;
const MIN_STEPS: usize = 6;
const MAX_STEPS: usize = 40;
const OVERSHOOT_MIN_DISTANCE_PX: f64 = 40.0;
const CORRECTION_DURATION_MS: f64 = 120.0;
const CORRECTION_STEPS: usize = 4;

/// Computes a Bezier path from `from` to `to`, duration sized by Fitts's
/// law from `target_size` (larger targets get shorter approach times),
/// with jitter and a persona-probability overshoot+correction.
pub fn mouse_path(
    from: (f64, f64),
    to: (f64, f64),
    target_size: f64,
    persona: &Persona,
    rng: &mut Pcg64,
) -> Vec<TimedPoint> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance < 1.0 {
        return vec![TimedPoint { at_ms: 0.0, x: to.0, y: to.1 }];
    }

    let duration_ms =
        persona.fitts_a_ms + persona.fitts_b_ms * (distance / target_size.max(1.0) + 1.0).log2();

    let overshoots = distance > OVERSHOOT_MIN_DISTANCE_PX && rng.gen_bool(persona.overshoot_p);
    let (end_x, end_y) = if overshoots {
        let overshoot_frac = 1.0 + rng.gen_range(0.05..0.15);
        (from.0 + dx * overshoot_frac, from.1 + dy * overshoot_frac)
    } else {
        (to.0, to.1)
    };

    // Perpendicular unit vector, for bowing the curve off the straight line.
    let perp_len = (dy * dy + dx * dx).sqrt().max(1.0);
    let perp_unit = (-dy / perp_len, dx / perp_len);
    let curvature = rng.gen_range(-1.0..1.0) * (distance * 0.15).min(60.0);

    let c1 = (
        from.0 + dx * 0.3 + perp_unit.0 * curvature,
        from.1 + dy * 0.3 + perp_unit.1 * curvature,
    );
    let c2 = (
        from.0 + dx * 0.7 + perp_unit.0 * curvature * 0.6,
        from.1 + dy * 0.7 + perp_unit.1 * curvature * 0.6,
    );

    let steps = ((duration_ms / FRAME_MS).round() as usize).clamp(MIN_STEPS, MAX_STEPS);
    let mut points = Vec::with_capacity(steps + 1 + if overshoots { CORRECTION_STEPS } else { 0 });

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let (bx, by) = cubic_bezier(from, c1, c2, (end_x, end_y), t);
        let jitter_x = normal_sample(rng) * persona.jitter_px;
        let jitter_y = normal_sample(rng) * persona.jitter_px;
        points.push(TimedPoint { at_ms: t * duration_ms, x: bx + jitter_x, y: by + jitter_y });
    }

    if overshoots {
        for i in 1..=CORRECTION_STEPS {
            let t = i as f64 / CORRECTION_STEPS as f64;
            points.push(TimedPoint {
                at_ms: duration_ms + t * CORRECTION_DURATION_MS,
                x: end_x + (to.0 - end_x) * t,
                y: end_y + (to.1 - end_y) * t,
            });
        }
    }

    points
}

fn cubic_bezier(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    let x = mt * mt * mt * p0.0 + 3.0 * mt * mt * t * p1.0 + 3.0 * mt * t * t * p2.0 + t * t * t * p3.0;
    let y = mt * mt * mt * p0.1 + 3.0 * mt * mt * t * p1.1 + 3.0 * mt * t * t * p2.1 + t * t * t * p3.1;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::seeded_rng;

    #[test]
    fn same_seed_reproduces_identical_path() {
        let persona = Persona::average();
        let mut rng1 = seeded_rng(42);
        let mut rng2 = seeded_rng(42);

        let path1 = mouse_path((0.0, 0.0), (300.0, 200.0), 40.0, &persona, &mut rng1);
        let path2 = mouse_path((0.0, 0.0), (300.0, 200.0), 40.0, &persona, &mut rng2);

        assert_eq!(path1.len(), path2.len());
        for (a, b) in path1.iter().zip(path2.iter()) {
            assert_eq!(a.x, b.x);
            assert_eq!(a.y, b.y);
            assert_eq!(a.at_ms, b.at_ms);
        }
    }

    #[test]
    fn careful_takes_longer_than_fast_for_the_same_distance() {
        let mut rng_careful = seeded_rng(7);
        let mut rng_fast = seeded_rng(7);

        let careful = mouse_path((0.0, 0.0), (400.0, 0.0), 40.0, &Persona::careful(), &mut rng_careful);
        let fast = mouse_path((0.0, 0.0), (400.0, 0.0), 40.0, &Persona::fast(), &mut rng_fast);

        let careful_duration = careful.last().unwrap().at_ms;
        let fast_duration = fast.last().unwrap().at_ms;
        assert!(
            careful_duration > fast_duration,
            "careful ({careful_duration}ms) should take longer than fast ({fast_duration}ms)"
        );
    }

    #[test]
    fn path_has_multiple_points_not_a_single_jump() {
        let mut rng = seeded_rng(1);
        let path = mouse_path((0.0, 0.0), (500.0, 400.0), 40.0, &Persona::average(), &mut rng);
        assert!(path.len() > 2, "expected a multi-point path, got {} points", path.len());
    }

    #[test]
    fn negligible_distance_returns_a_single_point() {
        let mut rng = seeded_rng(1);
        let path = mouse_path((10.0, 10.0), (10.2, 10.1), 40.0, &Persona::average(), &mut rng);
        assert_eq!(path.len(), 1);
    }
}
