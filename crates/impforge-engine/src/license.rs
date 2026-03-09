// SPDX-License-Identifier: BUSL-1.1
//
// ImpForge AI Engine — License Validation
// Copyright (c) 2026 AiImp Development. All rights reserved.
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://mariadb.com/bsl11/
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Ed25519-based license validation for ImpForge.
//!
//! This module provides offline-first license verification. The validator only
//! *verifies* signatures using a baked-in public key — private key generation
//! and license signing are handled by a separate CLI tool (see Task 17).

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// License Tier
// ---------------------------------------------------------------------------

/// The three tiers available in ImpForge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseTier {
    /// Free tier — core features, single device.
    Community,
    /// Paid tier — advanced features, multiple devices.
    Pro,
    /// Organisation tier — all features, unlimited devices.
    Enterprise,
}

impl fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseTier::Community => write!(f, "Community"),
            LicenseTier::Pro => write!(f, "Pro"),
            LicenseTier::Enterprise => write!(f, "Enterprise"),
        }
    }
}

// ---------------------------------------------------------------------------
// License Payload
// ---------------------------------------------------------------------------

/// The signed payload embedded in every license key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    /// Which tier this license grants.
    pub tier: LicenseTier,
    /// Email address of the licensee.
    pub email: String,
    /// Maximum number of devices this license may be activated on.
    pub max_devices: u8,
    /// Unix timestamp (seconds) when the license was issued.
    pub issued_at: i64,
    /// Unix timestamp (seconds) when the license expires.
    pub expires_at: i64,
    /// Feature flags enabled by this license (e.g. `["browser-agent", "gpu-cluster"]`).
    pub features: Vec<String>,
}

// ---------------------------------------------------------------------------
// License Key (wire format)
// ---------------------------------------------------------------------------

/// The portable license key exchanged between the signing CLI and the app.
///
/// Both fields are standard Base64-encoded strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    /// Base64-encoded JSON of [`LicensePayload`].
    pub payload_b64: String,
    /// Base64-encoded Ed25519 signature over the raw payload bytes.
    pub signature_b64: String,
}

// ---------------------------------------------------------------------------
// License Error
// ---------------------------------------------------------------------------

/// Errors returned by [`LicenseValidator::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseError {
    /// The Ed25519 signature does not match the payload.
    InvalidSignature,
    /// The license has expired (current time > `expires_at`).
    Expired,
    /// The payload bytes are not valid JSON or cannot be deserialized.
    MalformedPayload,
    /// Base64 decoding of payload or signature failed.
    DecodingError,
}

impl fmt::Display for LicenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseError::InvalidSignature => write!(f, "invalid Ed25519 signature"),
            LicenseError::Expired => write!(f, "license has expired"),
            LicenseError::MalformedPayload => write!(f, "malformed license payload"),
            LicenseError::DecodingError => write!(f, "base64 decoding error"),
        }
    }
}

impl std::error::Error for LicenseError {}

// ---------------------------------------------------------------------------
// License Validator
// ---------------------------------------------------------------------------

/// Offline Ed25519 license validator.
///
/// Constructed once at app start with the baked-in public key, then reused for
/// every validation call.
pub struct LicenseValidator {
    verifying_key: VerifyingKey,
}

impl LicenseValidator {
    /// Create a new validator from a 32-byte Ed25519 public key.
    ///
    /// # Panics
    ///
    /// This will never panic for a well-formed 32-byte key. If the bytes do
    /// not represent a valid curve point the constructor returns a
    /// `LicenseValidator` that will reject every signature.
    pub fn new(public_key_bytes: &[u8; 32]) -> Result<Self, LicenseError> {
        let verifying_key =
            VerifyingKey::from_bytes(public_key_bytes).map_err(|_| LicenseError::InvalidSignature)?;
        Ok(Self { verifying_key })
    }

    /// Validate a [`LicenseKey`]: verify the Ed25519 signature, decode the
    /// payload, and check that the license has not expired.
    ///
    /// Returns the decoded [`LicensePayload`] on success.
    pub fn validate(&self, key: &LicenseKey) -> Result<LicensePayload, LicenseError> {
        // 1. Decode base64 fields.
        let payload_bytes = BASE64
            .decode(&key.payload_b64)
            .map_err(|_| LicenseError::DecodingError)?;
        let sig_bytes = BASE64
            .decode(&key.signature_b64)
            .map_err(|_| LicenseError::DecodingError)?;

        // 2. Reconstruct the Ed25519 signature (exactly 64 bytes).
        let signature = Signature::from_slice(&sig_bytes)
            .map_err(|_| LicenseError::DecodingError)?;

        // 3. Verify.
        self.verifying_key
            .verify(&payload_bytes, &signature)
            .map_err(|_| LicenseError::InvalidSignature)?;

        // 4. Deserialize the JSON payload.
        let payload: LicensePayload =
            serde_json::from_slice(&payload_bytes).map_err(|_| LicenseError::MalformedPayload)?;

        // 5. Check expiry against wall-clock time.
        let now = chrono::Utc::now().timestamp();
        if now > payload.expires_at {
            return Err(LicenseError::Expired);
        }

        Ok(payload)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    /// Helper: create a signing/verifying keypair and a signed license key.
    fn make_signed_key(payload: &LicensePayload) -> (SigningKey, LicenseKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let payload_json = serde_json::to_vec(payload).unwrap();
        let signature = signing_key.sign(&payload_json);

        let key = LicenseKey {
            payload_b64: BASE64.encode(&payload_json),
            signature_b64: BASE64.encode(signature.to_bytes()),
        };
        (signing_key, key)
    }

    /// Helper: build a payload that expires far in the future.
    fn future_payload() -> LicensePayload {
        let now = chrono::Utc::now().timestamp();
        LicensePayload {
            tier: LicenseTier::Pro,
            email: "test@example.com".into(),
            max_devices: 3,
            issued_at: now,
            expires_at: now + 365 * 86400, // +1 year
            features: vec!["browser-agent".into(), "gpu-cluster".into()],
        }
    }

    /// Helper: build a payload that has already expired.
    fn expired_payload() -> LicensePayload {
        LicensePayload {
            tier: LicenseTier::Community,
            email: "expired@example.com".into(),
            max_devices: 1,
            issued_at: 1_000_000,
            expires_at: 1_000_001, // long in the past
            features: vec![],
        }
    }

    #[test]
    fn test_valid_signature_verification() {
        let payload = future_payload();
        let (signing_key, license_key) = make_signed_key(&payload);

        let pub_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");
        let result = validator.validate(&license_key);

        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let decoded = result.unwrap();
        assert_eq!(decoded.tier, LicenseTier::Pro);
        assert_eq!(decoded.email, "test@example.com");
        assert_eq!(decoded.max_devices, 3);
        assert_eq!(decoded.features, vec!["browser-agent", "gpu-cluster"]);
    }

    #[test]
    fn test_expired_license_rejected() {
        let payload = expired_payload();
        let (signing_key, license_key) = make_signed_key(&payload);

        let pub_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");
        let result = validator.validate(&license_key);

        assert_eq!(result.unwrap_err(), LicenseError::Expired);
    }

    #[test]
    fn test_invalid_signature_rejected() {
        let payload = future_payload();
        let (_signing_key, license_key) = make_signed_key(&payload);

        // Use a DIFFERENT keypair for validation — signature will not match.
        let other_key = SigningKey::generate(&mut OsRng);
        let other_pub: [u8; 32] = other_key.verifying_key().to_bytes();
        let validator = LicenseValidator::new(&other_pub).expect("valid public key");
        let result = validator.validate(&license_key);

        assert_eq!(result.unwrap_err(), LicenseError::InvalidSignature);
    }

    #[test]
    fn test_malformed_base64_rejected() {
        let bad_key = LicenseKey {
            payload_b64: "not-valid-base64!!!".into(),
            signature_b64: "also-bad!!!".into(),
        };

        let key_bytes = SigningKey::generate(&mut OsRng);
        let pub_bytes: [u8; 32] = key_bytes.verifying_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");
        let result = validator.validate(&bad_key);

        assert_eq!(result.unwrap_err(), LicenseError::DecodingError);
    }

    #[test]
    fn test_license_tier_display() {
        assert_eq!(LicenseTier::Community.to_string(), "Community");
        assert_eq!(LicenseTier::Pro.to_string(), "Pro");
        assert_eq!(LicenseTier::Enterprise.to_string(), "Enterprise");
    }

    #[test]
    fn test_license_tier_serde_roundtrip() {
        let tier = LicenseTier::Enterprise;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"enterprise\"");
        let back: LicenseTier = serde_json::from_str(&json).unwrap();
        assert_eq!(back, LicenseTier::Enterprise);
    }
}
