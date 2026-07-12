//! PKCE (RFC 7636) verifier/challenge generation, plus the anti-CSRF
//! `state` value (oauth-subscription-auth spec: "PKCE authorization
//! flow"). Pure math, no I/O -- kept separate from `login.rs`'s
//! orchestration so it's trivially unit-testable without a real network
//! call or callback server.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    /// Generates a fresh verifier (32 random bytes, base64url-encoded --
    /// 43 characters, within RFC 7636's 43-128 range) and its S256
    /// challenge.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = challenge_for(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

fn challenge_for(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// A random anti-CSRF token to round-trip through the authorize
/// redirect and validate on the callback.
pub fn random_state() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_is_within_rfc_7636_length_bounds() {
        let pkce = Pkce::generate();
        assert!(
            pkce.verifier.len() >= 43 && pkce.verifier.len() <= 128,
            "{}",
            pkce.verifier.len()
        );
        // Only unreserved URL-safe characters, per RFC 7636 section 4.1.
        assert!(pkce.verifier.chars().all(|c| c.is_ascii_alphanumeric()
            || c == '-'
            || c == '_'
            || c == '~'
            || c == '.'));
    }

    #[test]
    fn challenge_is_deterministic_given_the_same_verifier() {
        let a = challenge_for("fixed-verifier-value-for-this-test");
        let b = challenge_for("fixed-verifier-value-for-this-test");
        assert_eq!(a, b);
        // And different verifiers produce different challenges.
        assert_ne!(a, challenge_for("a-different-verifier-value"));
    }

    #[test]
    fn two_generated_pkce_pairs_are_never_identical() {
        let a = Pkce::generate();
        let b = Pkce::generate();
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.challenge, b.challenge);
    }

    #[test]
    fn state_values_are_random_and_reasonably_long() {
        let a = random_state();
        let b = random_state();
        assert_ne!(a, b);
        assert!(a.len() >= 32);
    }
}
