// SPDX-License-Identifier: BUSL-1.1
//
// ImpForge AI Engine — License Key Generation
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

//! License Key Generation Utilities
//!
//! Internal tool for generating signed license keys using Ed25519.
//! NOT included in community builds — BSL 1.1 only.
//!
//! This module is the counterpart to [`crate::license::LicenseValidator`]: the
//! validator only *verifies* signatures using a public key, while this module
//! holds the [`LicenseGenerator`] that creates and signs new license keys with
//! the corresponding private (signing) key.

use crate::license::{LicenseKey, LicensePayload, LicenseTier};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use chrono::{Duration, Utc};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};

// ---------------------------------------------------------------------------
// Default feature sets per tier
// ---------------------------------------------------------------------------

/// Features included in every Pro license.
const PRO_FEATURES: &[&str] = &["browser-agent", "gpu-cluster", "advanced-routing"];

/// Features included in every Enterprise license.
const ENTERPRISE_FEATURES: &[&str] = &[
    "browser-agent",
    "gpu-cluster",
    "advanced-routing",
    "priority-support",
    "custom-models",
    "team-management",
];

// ---------------------------------------------------------------------------
// LicenseGenerator
// ---------------------------------------------------------------------------

/// Ed25519-based license key generator.
///
/// Constructed with a 32-byte seed that deterministically derives the signing
/// key. The matching [`VerifyingKey`] (public key) is embedded in the
/// distributed application binary so that licenses can be verified offline.
pub struct LicenseGenerator {
    signing_key: SigningKey,
}

impl LicenseGenerator {
    /// Create a generator from a 32-byte seed.
    ///
    /// The seed is expanded into an Ed25519 signing key via the standard
    /// `ed25519-dalek` derivation. The same seed always produces the same
    /// keypair, which makes key management reproducible.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        Self { signing_key }
    }

    /// Return the [`VerifyingKey`] (public key) that corresponds to this
    /// generator's signing key.
    ///
    /// This is the key that must be baked into the application binary for
    /// offline license validation.
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Return the public key as a standard Base64-encoded string.
    ///
    /// Useful for configuration files and CLI output.
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.signing_key.verifying_key().to_bytes())
    }

    /// Generate a signed [`LicenseKey`].
    ///
    /// # Arguments
    ///
    /// * `customer_id` — email address or unique identifier of the licensee.
    /// * `tier` — the [`LicenseTier`] to grant.
    /// * `valid_days` — number of days from *now* until the license expires.
    ///
    /// The `max_devices` and `features` fields are derived from the tier:
    ///
    /// | Tier | Devices | Features |
    /// |------|---------|----------|
    /// | Community | 1 | *(none)* |
    /// | Pro | 3 | browser-agent, gpu-cluster, advanced-routing |
    /// | Enterprise | 255 | all Pro features + priority-support, custom-models, team-management |
    pub fn generate(
        &self,
        customer_id: &str,
        tier: LicenseTier,
        valid_days: i64,
    ) -> LicenseKey {
        let now = Utc::now();
        let expires = now + Duration::days(valid_days);

        let (max_devices, features) = match tier {
            LicenseTier::Community => (1_u8, Vec::new()),
            LicenseTier::Pro => (
                3,
                PRO_FEATURES.iter().map(|s| (*s).to_owned()).collect(),
            ),
            LicenseTier::Enterprise => (
                255,
                ENTERPRISE_FEATURES.iter().map(|s| (*s).to_owned()).collect(),
            ),
        };

        let payload = LicensePayload {
            tier,
            email: customer_id.to_owned(),
            max_devices,
            issued_at: now.timestamp(),
            expires_at: expires.timestamp(),
            features,
        };

        self.sign_payload(&payload)
    }

    /// Generate a 30-day trial license at the [`LicenseTier::Pro`] tier.
    ///
    /// This is a convenience wrapper around [`Self::generate`].
    pub fn generate_trial(&self, customer_id: &str) -> LicenseKey {
        self.generate(customer_id, LicenseTier::Pro, 30)
    }

    // -- internal helpers ---------------------------------------------------

    /// Serialize a [`LicensePayload`] to JSON, sign the raw bytes, and return
    /// the resulting [`LicenseKey`] with both fields Base64-encoded.
    fn sign_payload(&self, payload: &LicensePayload) -> LicenseKey {
        let payload_json = serde_json::to_vec(payload)
            .expect("LicensePayload serialization is infallible");
        let signature = self.signing_key.sign(&payload_json);

        LicenseKey {
            payload_b64: BASE64.encode(&payload_json),
            signature_b64: BASE64.encode(signature.to_bytes()),
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::LicenseValidator;
    use rand::rngs::OsRng;
    use rand::RngCore;

    /// Helper: create a [`LicenseGenerator`] with a random seed.
    fn random_generator() -> LicenseGenerator {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        LicenseGenerator::from_seed(seed)
    }

    // -----------------------------------------------------------------------
    // 1. Round-trip: generate → validate
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_and_verify_roundtrip() {
        let gen = random_generator();
        let license = gen.generate("alice@example.com", LicenseTier::Pro, 365);

        // Build a validator from the generator's public key.
        let pub_bytes = gen.public_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");

        let payload = validator
            .validate(&license)
            .expect("license should validate");

        assert_eq!(payload.tier, LicenseTier::Pro);
        assert_eq!(payload.email, "alice@example.com");
        assert_eq!(payload.max_devices, 3);
        assert!(payload.features.contains(&"browser-agent".to_owned()));
        assert!(payload.features.contains(&"gpu-cluster".to_owned()));
        assert!(payload.features.contains(&"advanced-routing".to_owned()));
    }

    // -----------------------------------------------------------------------
    // 2. Trial license is 30 days
    // -----------------------------------------------------------------------

    #[test]
    fn test_trial_is_30_days() {
        let gen = random_generator();
        let license = gen.generate_trial("trial@example.com");

        let pub_bytes = gen.public_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");
        let payload = validator
            .validate(&license)
            .expect("trial license should validate");

        assert_eq!(payload.tier, LicenseTier::Pro);
        assert_eq!(payload.email, "trial@example.com");

        // The difference between expires_at and issued_at must be ~30 days.
        // Allow 2 seconds of clock skew for slow CI.
        let duration_secs = payload.expires_at - payload.issued_at;
        let thirty_days_secs = 30 * 24 * 3600;
        assert!(
            (duration_secs - thirty_days_secs).unsigned_abs() < 2,
            "expected ~{thirty_days_secs}s, got {duration_secs}s",
        );
    }

    // -----------------------------------------------------------------------
    // 3. Public key Base64 round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_public_key_base64_roundtrip() {
        let gen = random_generator();
        let b64 = gen.public_key_base64();

        // Decode back to bytes.
        let decoded = BASE64.decode(&b64).expect("valid base64");
        assert_eq!(decoded.len(), 32, "Ed25519 public key is 32 bytes");

        // The decoded bytes must match the raw public key.
        assert_eq!(decoded.as_slice(), gen.public_key().as_bytes());
    }

    // -----------------------------------------------------------------------
    // 4. Different tiers produce correct metadata
    // -----------------------------------------------------------------------

    #[test]
    fn test_different_tiers() {
        let gen = random_generator();
        let pub_bytes = gen.public_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");

        // -- Pro --
        let pro = gen.generate("pro@example.com", LicenseTier::Pro, 90);
        let pro_payload = validator.validate(&pro).expect("pro should validate");
        assert_eq!(pro_payload.tier, LicenseTier::Pro);
        assert_eq!(pro_payload.max_devices, 3);
        assert_eq!(pro_payload.features.len(), 3);

        // -- Enterprise --
        let ent = gen.generate("ent@corp.com", LicenseTier::Enterprise, 365);
        let ent_payload = validator.validate(&ent).expect("enterprise should validate");
        assert_eq!(ent_payload.tier, LicenseTier::Enterprise);
        assert_eq!(ent_payload.max_devices, 255);
        assert_eq!(ent_payload.features.len(), 6);
        assert!(ent_payload.features.contains(&"team-management".to_owned()));

        // -- Community --
        let comm = gen.generate("free@example.com", LicenseTier::Community, 365);
        let comm_payload = validator.validate(&comm).expect("community should validate");
        assert_eq!(comm_payload.tier, LicenseTier::Community);
        assert_eq!(comm_payload.max_devices, 1);
        assert!(comm_payload.features.is_empty());
    }

    // -----------------------------------------------------------------------
    // 5. Different customers produce different keys
    // -----------------------------------------------------------------------

    #[test]
    fn test_different_customers_different_keys() {
        let gen = random_generator();

        let key_a = gen.generate("alice@example.com", LicenseTier::Pro, 365);
        let key_b = gen.generate("bob@example.com", LicenseTier::Pro, 365);

        // The payload contains the email, so the Base64-encoded payloads must
        // differ. The signatures will also differ because the signed content
        // differs.
        assert_ne!(
            key_a.payload_b64, key_b.payload_b64,
            "different customers must produce different payloads"
        );
        assert_ne!(
            key_a.signature_b64, key_b.signature_b64,
            "different payloads must produce different signatures"
        );

        // Both must still validate.
        let pub_bytes = gen.public_key().to_bytes();
        let validator = LicenseValidator::new(&pub_bytes).expect("valid public key");
        assert!(validator.validate(&key_a).is_ok());
        assert!(validator.validate(&key_b).is_ok());
    }

    // -----------------------------------------------------------------------
    // 6. Deterministic seed produces deterministic public key
    // -----------------------------------------------------------------------

    #[test]
    fn test_deterministic_seed() {
        let seed = [42u8; 32];
        let gen_a = LicenseGenerator::from_seed(seed);
        let gen_b = LicenseGenerator::from_seed(seed);

        assert_eq!(
            gen_a.public_key().to_bytes(),
            gen_b.public_key().to_bytes(),
            "same seed must produce same public key"
        );
        assert_eq!(gen_a.public_key_base64(), gen_b.public_key_base64());
    }

    // -----------------------------------------------------------------------
    // 7. Key generated by one seed is rejected by a different seed's pubkey
    // -----------------------------------------------------------------------

    #[test]
    fn test_wrong_key_rejects() {
        let gen_a = LicenseGenerator::from_seed([1u8; 32]);
        let gen_b = LicenseGenerator::from_seed([2u8; 32]);

        let license = gen_a.generate("user@example.com", LicenseTier::Pro, 365);

        // Validate with gen_b's public key — must fail.
        let wrong_pub = gen_b.public_key().to_bytes();
        let validator = LicenseValidator::new(&wrong_pub).expect("valid public key");
        let result = validator.validate(&license);

        assert!(result.is_err(), "wrong public key must reject the license");
    }
}
