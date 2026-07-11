//! Per-character typing cadence synthesis (human-motion spec: "Per-character
//! typing cadence").

use super::{normal_sample, Persona};
use rand_pcg::Pcg64;

#[derive(Debug, Clone, Copy)]
pub struct TimedKey {
    pub at_ms: f64,
    pub ch: char,
}

const MIN_KEY_DELAY_MS: f64 = 15.0;

/// Computes a per-character delay timeline from the persona's log-normal-ish
/// (Gaussian-around-a-positive-mean, floored) distribution. No bigram-class
/// modeling in this synthetic version — see design.md Decision #5.
pub fn typing_timeline(text: &str, persona: &Persona, rng: &mut Pcg64) -> Vec<TimedKey> {
    let mut t = 0.0;
    let mut out = Vec::with_capacity(text.chars().count());

    for (i, ch) in text.chars().enumerate() {
        if i > 0 {
            let delay = persona.key_delay_mean_ms + normal_sample(rng) * persona.key_delay_std_ms;
            t += delay.max(MIN_KEY_DELAY_MS);
        }
        out.push(TimedKey { at_ms: t, ch });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::seeded_rng;

    #[test]
    fn same_seed_reproduces_identical_timeline() {
        let persona = Persona::average();
        let mut rng1 = seeded_rng(99);
        let mut rng2 = seeded_rng(99);

        let t1 = typing_timeline("hello world", &persona, &mut rng1);
        let t2 = typing_timeline("hello world", &persona, &mut rng2);

        assert_eq!(t1.len(), t2.len());
        for (a, b) in t1.iter().zip(t2.iter()) {
            assert_eq!(a.ch, b.ch);
            assert_eq!(a.at_ms, b.at_ms);
        }
    }

    #[test]
    fn delays_are_non_uniform() {
        let mut rng = seeded_rng(3);
        let timeline = typing_timeline("the quick brown fox jumps", &Persona::average(), &mut rng);
        let deltas: Vec<f64> = timeline.windows(2).map(|w| w[1].at_ms - w[0].at_ms).collect();
        let all_equal = deltas.windows(2).all(|w| (w[0] - w[1]).abs() < 0.001);
        assert!(!all_equal, "expected non-uniform inter-character delays");
    }

    #[test]
    fn preserves_character_sequence() {
        let mut rng = seeded_rng(5);
        let timeline = typing_timeline("abc", &Persona::fast(), &mut rng);
        let chars: String = timeline.iter().map(|k| k.ch).collect();
        assert_eq!(chars, "abc");
    }
}
