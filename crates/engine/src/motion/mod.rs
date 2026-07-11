//! Seeded, persona-parameterized human-like input synthesis (human-motion
//! spec). Timelines are computed entirely up front from a seeded RNG, then
//! dispatched with real pacing — see `Session::click`/`type_text`'s
//! `human_like` parameter.

mod path;
pub mod profile_store;
pub mod train;
mod typing;

pub use path::{mouse_path, TimedPoint};
pub use train::{fit_persona, Sample, Training};
pub use typing::{typing_timeline, TimedKey};

use rand::SeedableRng;
use rand_pcg::Pcg64;
use serde::{Deserialize, Serialize};

/// Timing/jitter parameters for human-like input. A hand-authored preset
/// (`careful`/`average`/`fast`) or one fitted from a real human's recorded
/// demonstration (`motion::train::fit_persona`) — same shape either way.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Persona {
    /// Fitts's-law duration constants: `duration_ms = a + b * log2(distance / target_size + 1)`.
    pub fitts_a_ms: f64,
    pub fitts_b_ms: f64,
    /// Per-sample perpendicular jitter, pixels (std dev).
    pub jitter_px: f64,
    /// Probability a far-enough move overshoots the target before a short
    /// corrective move back.
    pub overshoot_p: f64,
    /// Per-character typing delay distribution, milliseconds.
    pub key_delay_mean_ms: f64,
    pub key_delay_std_ms: f64,
}

impl Persona {
    pub fn careful() -> Self {
        Self {
            fitts_a_ms: 200.0,
            fitts_b_ms: 170.0,
            jitter_px: 1.5,
            overshoot_p: 0.05,
            key_delay_mean_ms: 140.0,
            key_delay_std_ms: 45.0,
        }
    }

    pub fn average() -> Self {
        Self {
            fitts_a_ms: 120.0,
            fitts_b_ms: 110.0,
            jitter_px: 2.5,
            overshoot_p: 0.15,
            key_delay_mean_ms: 90.0,
            key_delay_std_ms: 30.0,
        }
    }

    pub fn fast() -> Self {
        Self {
            fitts_a_ms: 60.0,
            fitts_b_ms: 70.0,
            jitter_px: 3.5,
            overshoot_p: 0.25,
            key_delay_mean_ms: 55.0,
            key_delay_std_ms: 20.0,
        }
    }

    /// Looks up a persona by name (human-motion spec: "Persona presets").
    /// `None` for anything that isn't one of the built-in presets — callers
    /// turn that into a typed error rather than silently defaulting.
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "careful" => Some(Self::careful()),
            "average" => Some(Self::average()),
            "fast" => Some(Self::fast()),
            _ => None,
        }
    }
}

pub fn seeded_rng(seed: u64) -> Pcg64 {
    Pcg64::seed_from_u64(seed)
}

/// Standard-normal sample via Box-Muller. Small and dependency-free —
/// avoids pulling in `rand_distr` for two distributions.
pub(crate) fn normal_sample(rng: &mut Pcg64) -> f64 {
    use rand::Rng;
    let u1: f64 = rng.gen_range(1e-9..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_name_rejects_unknown_persona() {
        assert!(Persona::by_name("careful").is_some());
        assert!(Persona::by_name("average").is_some());
        assert!(Persona::by_name("fast").is_some());
        assert!(Persona::by_name("ninja").is_none());
    }
}
