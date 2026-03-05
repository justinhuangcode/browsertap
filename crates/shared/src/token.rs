use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Token scopes - browser sessions get short-lived tokens, CLI gets longer-lived ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenScope {
    /// Browser runtime token (short TTL: 5 minutes)
    Session,
    /// CLI/Agent token (longer TTL: 1 hour)
    Cli,
}

impl TokenScope {
    pub fn ttl(&self) -> Duration {
        match self {
            TokenScope::Session => Duration::minutes(5),
            TokenScope::Cli => Duration::hours(1),
        }
    }
}

/// Token payload - the claims inside a signed token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub token_id: Uuid,
    pub scope: TokenScope,
    pub subject: String,
    pub session_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl TokenPayload {
    pub fn new(scope: TokenScope, subject: impl Into<String>, session_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            token_id: Uuid::new_v4(),
            scope,
            subject: subject.into(),
            session_id,
            issued_at: now,
            expires_at: now + scope.ttl(),
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("invalid token format")]
    InvalidFormat,
    #[error("invalid base64 encoding")]
    InvalidBase64,
    #[error("invalid JSON payload")]
    InvalidPayload,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("token expired")]
    Expired,
    #[error("scope mismatch: expected {expected:?}, got {actual:?}")]
    ScopeMismatch {
        expected: TokenScope,
        actual: TokenScope,
    },
}

/// Sign a token payload with HMAC-SHA256.
///
/// Format: `{base64url(json_payload)}.{base64url(hmac_signature)}`
pub fn sign_token(payload: &TokenPayload, secret: &[u8]) -> Result<String, TokenError> {
    let json = serde_json::to_vec(payload).map_err(|_| TokenError::InvalidPayload)?;
    let encoded_payload = URL_SAFE_NO_PAD.encode(&json);

    let mut mac =
        HmacSha256::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(encoded_payload.as_bytes());
    let signature = mac.finalize().into_bytes();
    let encoded_sig = URL_SAFE_NO_PAD.encode(signature);

    Ok(format!("{encoded_payload}.{encoded_sig}"))
}

/// Verify and decode a token string.
///
/// Uses constant-time comparison for the signature to prevent timing attacks.
pub fn verify_token(token: &str, secret: &[u8]) -> Result<TokenPayload, TokenError> {
    let (encoded_payload, encoded_sig) = token
        .split_once('.')
        .ok_or(TokenError::InvalidFormat)?;

    // Verify signature (constant-time via hmac crate)
    let mut mac =
        HmacSha256::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(encoded_payload.as_bytes());
    let expected_sig = URL_SAFE_NO_PAD
        .decode(encoded_sig)
        .map_err(|_| TokenError::InvalidBase64)?;
    mac.verify_slice(&expected_sig)
        .map_err(|_| TokenError::InvalidSignature)?;

    // Decode payload
    let json = URL_SAFE_NO_PAD
        .decode(encoded_payload)
        .map_err(|_| TokenError::InvalidBase64)?;
    let payload: TokenPayload =
        serde_json::from_slice(&json).map_err(|_| TokenError::InvalidPayload)?;

    if payload.is_expired() {
        return Err(TokenError::Expired);
    }

    Ok(payload)
}

/// Verify a token and enforce a specific scope.
pub fn verify_token_with_scope(
    token: &str,
    secret: &[u8],
    expected_scope: TokenScope,
) -> Result<TokenPayload, TokenError> {
    let payload = verify_token(token, secret)?;
    if payload.scope != expected_scope {
        return Err(TokenError::ScopeMismatch {
            expected: expected_scope,
            actual: payload.scope,
        });
    }
    Ok(payload)
}

/// Generate a cryptographically secure random secret (32 bytes).
pub fn generate_secret() -> Vec<u8> {
    use rand::RngCore;
    let mut secret = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    secret
}

/// Encode a secret as hex string for storage.
pub fn secret_to_hex(secret: &[u8]) -> String {
    secret.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decode a hex string back to secret bytes.
pub fn secret_from_hex(hex: &str) -> Result<Vec<u8>, TokenError> {
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| TokenError::InvalidFormat)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let secret = generate_secret();
        let session_id = Uuid::new_v4();
        let payload = TokenPayload::new(TokenScope::Session, "test-browser", session_id);

        let token = sign_token(&payload, &secret).unwrap();
        let verified = verify_token(&token, &secret).unwrap();

        assert_eq!(verified.token_id, payload.token_id);
        assert_eq!(verified.scope, TokenScope::Session);
        assert_eq!(verified.session_id, session_id);
    }

    #[test]
    fn wrong_secret_fails() {
        let secret = generate_secret();
        let wrong_secret = generate_secret();
        let payload = TokenPayload::new(TokenScope::Cli, "test-cli", Uuid::new_v4());

        let token = sign_token(&payload, &secret).unwrap();
        let result = verify_token(&token, &wrong_secret);

        assert!(matches!(result, Err(TokenError::InvalidSignature)));
    }

    #[test]
    fn scope_enforcement() {
        let secret = generate_secret();
        let payload = TokenPayload::new(TokenScope::Session, "browser", Uuid::new_v4());
        let token = sign_token(&payload, &secret).unwrap();

        let result = verify_token_with_scope(&token, &secret, TokenScope::Cli);
        assert!(matches!(result, Err(TokenError::ScopeMismatch { .. })));
    }

    #[test]
    fn secret_hex_roundtrip() {
        let secret = generate_secret();
        let hex = secret_to_hex(&secret);
        let decoded = secret_from_hex(&hex).unwrap();
        assert_eq!(secret, decoded);
    }
}
