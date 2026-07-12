//! Reads the payload of a JWT we just received ourselves, over TLS, from
//! the token endpoint -- deliberately does NOT verify the signature (no
//! JWT-verification crate pulled in for it). This is not third-party
//! bearer-token validation, where forging a signature would matter; it's
//! reading a claim out of our own freshly-issued token to learn our own
//! account id (oauth-subscription-auth spec: "Account id from the id_token").

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// Decodes a JWT's payload (the middle `.`-delimited segment) as JSON.
/// Returns `None` if the token isn't shaped like a JWT or the payload
/// isn't valid base64url/JSON -- callers treat that as "no claims
/// available," not a hard error, since a malformed id_token shouldn't
/// crash login when the access/refresh tokens themselves are still fine.
pub fn decode_payload(jwt: &str) -> Option<serde_json::Value> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    serde_json::from_slice(&payload_bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fake_jwt(payload_json: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let payload = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        format!("{header}.{payload}.unused-signature")
    }

    #[test]
    fn decodes_a_nested_claim_correctly() {
        let jwt = make_fake_jwt(
            r#"{"email":"user@example.com","https://api.openai.com/auth":{"chatgpt_account_id":"acct_123"}}"#,
        );
        let claims = decode_payload(&jwt).expect("decodes");
        assert_eq!(claims["email"], "user@example.com");
        assert_eq!(
            claims["https://api.openai.com/auth"]["chatgpt_account_id"],
            "acct_123"
        );
    }

    #[test]
    fn malformed_token_returns_none_not_a_panic() {
        assert!(decode_payload("not-a-jwt-at-all").is_none());
        assert!(decode_payload("").is_none());
        assert!(decode_payload("a.b").is_some() || decode_payload("a.b").is_none());
        // two-segment: depends on decodability, must not panic either way
    }
}
