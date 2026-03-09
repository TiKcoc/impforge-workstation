// SPDX-License-Identifier: BUSL-1.1
//
// ImpForge AI Engine — License Persistence (SQLite)
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

//! SQLite-backed license persistence for ImpForge.
//!
//! Stores a validated [`LicensePayload`] (and its originating [`LicenseKey`])
//! so the application can restore the active license across restarts without
//! re-entering a key.  The store keeps exactly one active row at a time; a new
//! [`save_license`](LicenseStore::save_license) call replaces any previous
//! record.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::license::{LicenseKey, LicensePayload, LicenseTier};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when reading or writing the license store.
#[derive(Debug)]
pub enum LicenseStoreError {
    /// An underlying SQLite operation failed.
    Sqlite(rusqlite::Error),
    /// The tier string stored in the database is not a recognised variant.
    InvalidTier(String),
}

impl std::fmt::Display for LicenseStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseStoreError::Sqlite(e) => write!(f, "license store: sqlite error: {e}"),
            LicenseStoreError::InvalidTier(s) => {
                write!(f, "license store: unrecognised tier '{s}'")
            }
        }
    }
}

impl std::error::Error for LicenseStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LicenseStoreError::Sqlite(e) => Some(e),
            LicenseStoreError::InvalidTier(_) => None,
        }
    }
}

impl From<rusqlite::Error> for LicenseStoreError {
    fn from(e: rusqlite::Error) -> Self {
        LicenseStoreError::Sqlite(e)
    }
}

// ---------------------------------------------------------------------------
// Tier ↔ TEXT helpers
// ---------------------------------------------------------------------------

/// Serialise a [`LicenseTier`] to the lowercase string stored in the DB.
fn tier_to_text(tier: LicenseTier) -> &'static str {
    match tier {
        LicenseTier::Community => "community",
        LicenseTier::Pro => "pro",
        LicenseTier::Enterprise => "enterprise",
    }
}

/// Deserialise a tier string from the DB back into a [`LicenseTier`].
fn tier_from_text(s: &str) -> Result<LicenseTier, LicenseStoreError> {
    match s {
        "community" => Ok(LicenseTier::Community),
        "pro" => Ok(LicenseTier::Pro),
        "enterprise" => Ok(LicenseTier::Enterprise),
        other => Err(LicenseStoreError::InvalidTier(other.to_owned())),
    }
}

// ---------------------------------------------------------------------------
// LicenseStore
// ---------------------------------------------------------------------------

/// Persists the active license in a local SQLite database.
///
/// The table holds at most one row at any time.  Calling
/// [`save_license`](Self::save_license) replaces any previously stored record.
pub struct LicenseStore {
    conn: Connection,
}

impl LicenseStore {
    /// Open (or create) the license database at `db_path`.
    ///
    /// Enables WAL journal mode and creates the `licenses` table if it does
    /// not already exist.
    pub fn open(db_path: &Path) -> Result<Self, LicenseStoreError> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS licenses (
                 id           INTEGER PRIMARY KEY,
                 tier         TEXT    NOT NULL,
                 email        TEXT    NOT NULL,
                 expires_at   INTEGER NOT NULL,
                 key_b64      TEXT    NOT NULL,
                 activated_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (useful for tests).
    #[cfg(test)]
    fn open_in_memory() -> Result<Self, LicenseStoreError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS licenses (
                 id           INTEGER PRIMARY KEY,
                 tier         TEXT    NOT NULL,
                 email        TEXT    NOT NULL,
                 expires_at   INTEGER NOT NULL,
                 key_b64      TEXT    NOT NULL,
                 activated_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self { conn })
    }

    /// Store a validated license, replacing any existing record.
    ///
    /// `key` is the original [`LicenseKey`] (serialised to JSON and stored in
    /// `key_b64` for potential re-validation on startup).  `payload` is the
    /// already-verified [`LicensePayload`].
    pub fn save_license(
        &self,
        key: &LicenseKey,
        payload: &LicensePayload,
    ) -> Result<(), LicenseStoreError> {
        let key_json = serde_json::to_string(key).unwrap_or_default();
        let now = chrono::Utc::now().timestamp();

        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM licenses", [])?;
        tx.execute(
            "INSERT INTO licenses (tier, email, expires_at, key_b64, activated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                tier_to_text(payload.tier),
                payload.email,
                payload.expires_at,
                key_json,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Retrieve the currently stored license, if any.
    ///
    /// Returns `None` when the table is empty (no license activated or
    /// previously revoked).  Does **not** check expiry — callers should
    /// re-validate the returned payload if freshness matters.
    pub fn get_active_license(&self) -> Result<Option<LicensePayload>, LicenseStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT tier, email, expires_at FROM licenses ORDER BY id DESC LIMIT 1",
        )?;

        let mut rows = stmt.query_map([], |row| {
            let tier_text: String = row.get(0)?;
            let email: String = row.get(1)?;
            let expires_at: i64 = row.get(2)?;
            Ok((tier_text, email, expires_at))
        })?;

        match rows.next() {
            Some(Ok((tier_text, email, expires_at))) => {
                let tier = tier_from_text(&tier_text)?;
                Ok(Some(LicensePayload {
                    tier,
                    email,
                    max_devices: 0, // not persisted — re-read from key if needed
                    issued_at: 0,   // not persisted
                    expires_at,
                    features: Vec::new(), // not persisted
                }))
            }
            Some(Err(e)) => Err(LicenseStoreError::Sqlite(e)),
            None => Ok(None),
        }
    }

    /// Quick lookup of the current license tier.
    ///
    /// Returns [`LicenseTier::Community`] when no license is stored (i.e. the
    /// free-tier default).
    pub fn get_tier(&self) -> Result<LicenseTier, LicenseStoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT tier FROM licenses ORDER BY id DESC LIMIT 1")?;

        let mut rows = stmt.query_map([], |row| {
            let tier_text: String = row.get(0)?;
            Ok(tier_text)
        })?;

        match rows.next() {
            Some(Ok(tier_text)) => tier_from_text(&tier_text),
            Some(Err(e)) => Err(LicenseStoreError::Sqlite(e)),
            None => Ok(LicenseTier::Community),
        }
    }

    /// Remove the stored license, effectively reverting to Community tier.
    pub fn revoke(&self) -> Result<(), LicenseStoreError> {
        self.conn.execute("DELETE FROM licenses", [])?;
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::{LicenseKey, LicensePayload, LicenseTier};

    /// Helper: build a sample payload and key for testing persistence.
    fn sample_payload_and_key() -> (LicensePayload, LicenseKey) {
        let payload = LicensePayload {
            tier: LicenseTier::Pro,
            email: "user@example.com".into(),
            max_devices: 3,
            issued_at: 1_700_000_000,
            expires_at: 1_800_000_000,
            features: vec!["browser-agent".into()],
        };
        let key = LicenseKey {
            payload_b64: "dGVzdC1wYXlsb2Fk".into(),
            signature_b64: "dGVzdC1zaWduYXR1cmU=".into(),
        };
        (payload, key)
    }

    #[test]
    fn store_and_retrieve_license() {
        let store = LicenseStore::open_in_memory().expect("open in-memory db");
        let (payload, key) = sample_payload_and_key();

        store.save_license(&key, &payload).expect("save");
        let loaded = store
            .get_active_license()
            .expect("get")
            .expect("should be Some");

        assert_eq!(loaded.tier, LicenseTier::Pro);
        assert_eq!(loaded.email, "user@example.com");
        assert_eq!(loaded.expires_at, 1_800_000_000);
    }

    #[test]
    fn revoke_clears_license() {
        let store = LicenseStore::open_in_memory().expect("open in-memory db");
        let (payload, key) = sample_payload_and_key();

        store.save_license(&key, &payload).expect("save");
        store.revoke().expect("revoke");

        let loaded = store.get_active_license().expect("get");
        assert!(loaded.is_none(), "expected None after revoke");
    }

    #[test]
    fn get_tier_defaults_to_community() {
        let store = LicenseStore::open_in_memory().expect("open in-memory db");

        let tier = store.get_tier().expect("get_tier");
        assert_eq!(tier, LicenseTier::Community);
    }

    #[test]
    fn get_tier_returns_stored_tier() {
        let store = LicenseStore::open_in_memory().expect("open in-memory db");
        let (payload, key) = sample_payload_and_key();

        store.save_license(&key, &payload).expect("save");
        let tier = store.get_tier().expect("get_tier");
        assert_eq!(tier, LicenseTier::Pro);
    }

    #[test]
    fn save_replaces_previous_license() {
        let store = LicenseStore::open_in_memory().expect("open in-memory db");
        let (payload1, key1) = sample_payload_and_key();

        store.save_license(&key1, &payload1).expect("save first");

        let payload2 = LicensePayload {
            tier: LicenseTier::Enterprise,
            email: "admin@corp.com".into(),
            max_devices: 100,
            issued_at: 1_700_000_000,
            expires_at: 1_900_000_000,
            features: vec![],
        };
        let key2 = LicenseKey {
            payload_b64: "c2Vjb25k".into(),
            signature_b64: "c2lnMg==".into(),
        };
        store.save_license(&key2, &payload2).expect("save second");

        let loaded = store
            .get_active_license()
            .expect("get")
            .expect("should be Some");
        assert_eq!(loaded.tier, LicenseTier::Enterprise);
        assert_eq!(loaded.email, "admin@corp.com");

        // Verify only one row exists.
        let tier = store.get_tier().expect("get_tier");
        assert_eq!(tier, LicenseTier::Enterprise);
    }

    #[test]
    fn revoke_then_get_tier_returns_community() {
        let store = LicenseStore::open_in_memory().expect("open in-memory db");
        let (payload, key) = sample_payload_and_key();

        store.save_license(&key, &payload).expect("save");
        assert_eq!(store.get_tier().expect("tier"), LicenseTier::Pro);

        store.revoke().expect("revoke");
        assert_eq!(store.get_tier().expect("tier after revoke"), LicenseTier::Community);
    }
}
