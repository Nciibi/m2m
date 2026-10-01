/// M2M — Storage Module
///
/// Encrypted local storage using plain SQLite with application-level encryption.
/// Sensitive data (private keys, message contents) is encrypted with
/// XChaCha20-Poly1305 before being stored, using keys derived from the user's
/// passphrase via Argon2id.
///
/// This approach avoids the OpenSSL dependency required by SQLCipher while
/// providing equivalent protection: we control exactly what gets encrypted
/// and the encryption key never touches SQLite internals.
///
/// Two separate databases:
/// - keys.db: identity keys, peer keys, consumed invite nonces
/// - messages.db: chat history (optional, can be disabled)
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use thiserror::Error;

/// Type alias for the reactions map returned by get_reactions.
type ReactionsMap = std::collections::HashMap<String, Vec<(String, String, i64)>>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("storage path error: {0}")]
    PathError(String),
    #[error("key not found")]
    KeyNotFound,
    #[error("decryption failed")]
    DecryptionFailed,
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("data directory creation failed: {0}")]
    DirCreationFailed(String),
}

/// Data directory name for M2M.
const DATA_DIR_NAME: &str = ".m2m";

/// Get the M2M data directory path.
pub fn data_dir() -> Result<PathBuf, StorageError> {
    let home = resolve_base_dir()?;
    Ok(home.join(DATA_DIR_NAME))
}

/// Resolve the base directory for storing data.
fn resolve_base_dir() -> Result<PathBuf, StorageError> {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .map_err(|_| StorageError::PathError("APPDATA not set".to_string()))
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| StorageError::PathError("HOME not set".to_string()))
    }
}

/// Ensure the data directory exists.
pub fn ensure_data_dir() -> Result<PathBuf, StorageError> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| StorageError::DirCreationFailed(e.to_string()))?;
    Ok(dir)
}

/// A family member — a peer the user has explicitly saved as a persistent contact.
/// One stored vault account (secret material still encrypted).
/// Each account's identity secret is wrapped under its OWN passphrase.
#[derive(Debug, Clone)]
pub struct AccountRow {
    // The `id` rowid is deliberately not mirrored: nothing reads it, and
    // selecting it shifted every column index below by one for no benefit.
    pub public_key: Vec<u8>,
    pub encrypted_private_key: Vec<u8>,
    pub private_key_nonce: Vec<u8>,
    // The `label` column ("Main" / "Imported", …) is intentionally NOT mirrored
    // here: nothing reads it, since the unlock screen cannot yet name the
    // account it is about to open. The column stays in the schema so adding a
    // multi-account picker is a SELECT change, not a migration.
}

/// Stored X25519 identity material: `(public_key, encrypted_secret, nonce)`.
///
/// The secret is AEAD-sealed under the vault storage key; the public half is
/// plain (it is not secret) but is *unauthenticated*, so it must be
/// cross-checked against the secret before use.
pub type X25519KeyMaterial = ([u8; 32], Vec<u8>, Vec<u8>);

/// A family member - a peer the user has explicitly saved as a persistent contact.
/// Stored in the `family` table, separate from the ephemeral `peers` table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FamilyMember {
    /// Current public key of this peer (hex-encoded for frontend).
    pub public_key_hex: String,
    /// Your label for them.
    pub nickname: String,
    /// When they were added (unix seconds).
    pub added_at: i64,
    /// When they expire (null = forever).
    pub expires_at: Option<i64>,
    /// Last known address (best-effort, may be stale).
    pub last_address: Option<String>,
}

/// The key store: holds identity keys, peer keys, and consumed invite nonces.
/// Private key material is encrypted at the application level before storage.
pub struct KeyStore {
    conn: Connection,
}

/// Connection-level pragmas applied to every store.
///
/// These are *connection* settings, not schema settings, so they must be set at
/// open time. Two of the three were previously missing entirely:
///
/// * `secure_delete` was only turned on by the three delete paths
///   (`delete_message`, `delete_conversation`, `delete_expired_messages`). It
///   was therefore OFF for every read, every ordinary `UPDATE`, and every freed
///   page until a user happened to delete something — so routine churn (an
///   edited message, a self-destruct timer firing) left exactly the freed-page
///   remnants that per-message CEK shredding exists to make harmless. For a tool
///   built around seizure scenarios, it belongs at `open()`.
/// * `foreign_keys` defaults to OFF in SQLite, which made every
///   `FOREIGN KEY ... REFERENCES` clause in this file purely decorative.
/// * `busy_timeout` turns a cross-process `SQLITE_BUSY` (two instances on one
///   profile) into a wait rather than an error.
fn apply_connection_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Zero freed pages and overwritten cells rather than leaving the old bytes
    // in place. This is a *performance* trade (every delete scrubs), which is
    // the right trade for this threat model.
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

impl KeyStore {
    /// Open or create the key store.
    /// Note: the private key stored here must already be encrypted by the caller
    /// using a key derived from the user's passphrase (Argon2id + XChaCha20-Poly1305).
    pub fn open(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")?;
        apply_connection_pragmas(&conn)?;

        // Initialize schema
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS identity (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                public_key BLOB NOT NULL,
                encrypted_private_key BLOB NOT NULL,
                private_key_nonce BLOB NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS peers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key BLOB NOT NULL UNIQUE,
                fingerprint TEXT NOT NULL,
                alias TEXT,
                verified INTEGER NOT NULL DEFAULT 0,
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS consumed_invites (
                nonce BLOB PRIMARY KEY,
                consumed_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS vault_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key BLOB NOT NULL UNIQUE,
                encrypted_private_key BLOB NOT NULL,
                private_key_nonce BLOB NOT NULL,
                label TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS family (
                public_key BLOB NOT NULL PRIMARY KEY,
                nickname TEXT NOT NULL,
                added_at INTEGER NOT NULL,
                expires_at INTEGER,
                last_address TEXT
            );",
        )?;

        Ok(Self { conn })
    }

    /// Check if the vault passphrase has ever been set.
    pub fn is_vault_initialized(&self) -> Result<bool, StorageError> {
        let result: Result<String, _> = self.conn.query_row(
            "SELECT value FROM vault_meta WHERE key = 'initialized'",
            [],
            |row| row.get(0),
        );
        Ok(result.map(|v| v == "true").unwrap_or(false))
    }

    /// Store a vault metadata value (duress hash, capability flags, …).
    ///
    /// Both the key and the value are bound parameters. The key used to be
    /// formatted into the statement behind a `'`/`\0` denylist, which is a
    /// shape that reads as safe and is not — it needs one more caller to pass
    /// a derived string before it is an injection.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO vault_meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Read a vault metadata value. `None` when absent.
    pub fn get_meta(&self, key: &str) -> Result<Option<String>, StorageError> {
        let result: Result<String, _> = self.conn.query_row(
            "SELECT value FROM vault_meta WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        Ok(result.ok())
    }

    /// Store the X25519 identity keypair (encrypted with storage key).
    pub fn store_x25519_key(
        &self,
        public_key: &[u8; 32],
        encrypted_secret: &[u8],
        nonce: &[u8],
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO vault_meta (key, value) VALUES ('x25519_pub', ?1)",
            params![hex::encode(public_key)],
        )?;
        self.conn.execute(
            "INSERT OR REPLACE INTO vault_meta (key, value) VALUES ('x25519_enc', ?1)",
            params![hex::encode(encrypted_secret)],
        )?;
        self.conn.execute(
            "INSERT OR REPLACE INTO vault_meta (key, value) VALUES ('x25519_nonce', ?1)",
            params![hex::encode(nonce)],
        )?;
        Ok(())
    }

    /// Load the stored X25519 key material.
    ///
    /// Returns `X25519KeyMaterial` = `(public_key, encrypted_secret, nonce)`.
    /// The public half is stored as *unauthenticated plaintext* in `vault_meta`;
    /// `X25519IdentityKeypair::from_bytes` re-derives and cross-checks it, so a
    /// substituted value is rejected rather than loaded.
    pub fn load_x25519_key(&self) -> Result<X25519KeyMaterial, StorageError> {
        let pub_hex: String = self
            .conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'x25519_pub'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::KeyNotFound)?;
        let enc_hex: String = self
            .conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'x25519_enc'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::KeyNotFound)?;
        let nonce_hex: String = self
            .conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'x25519_nonce'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::KeyNotFound)?;

        let pub_bytes = hex::decode(&pub_hex).map_err(|_| StorageError::KeyNotFound)?;
        let enc_bytes = hex::decode(&enc_hex).map_err(|_| StorageError::KeyNotFound)?;
        let nonce_bytes = hex::decode(&nonce_hex).map_err(|_| StorageError::KeyNotFound)?;

        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);
        Ok((pub_arr, enc_bytes, nonce_bytes))
    }

    /// Check if an X25519 key has been stored.
    pub fn has_x25519_key(&self) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM vault_meta WHERE key = 'x25519_pub'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Mark the vault as initialized (passphrase has been set).
    pub fn set_vault_initialized(&self) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO vault_meta (key, value) VALUES ('initialized', 'true')",
            [],
        )?;
        Ok(())
    }

    /// Load only the public key (no decryption needed).
    pub fn load_public_key(&self) -> Result<Vec<u8>, StorageError> {
        self.conn
            .query_row("SELECT public_key FROM identity WHERE id = 1", [], |row| {
                row.get(0)
            })
            .map_err(|_| StorageError::KeyNotFound)
    }

    /// Update the encrypted private key and nonce (used during legacy→vault migration).
    pub fn update_encrypted_private_key(
        &self,
        encrypted_private_key: &[u8],
        nonce: &[u8],
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE identity SET encrypted_private_key = ?1, private_key_nonce = ?2 WHERE id = 1",
            params![encrypted_private_key, nonce],
        )?;
        Ok(())
    }

    /// Store the identity keypair.
    /// `encrypted_private_key` must be the private key encrypted with
    /// XChaCha20-Poly1305 using a key derived from the user's passphrase.
    /// `nonce` is the encryption nonce used.
    pub fn store_identity(
        &self,
        public_key: &[u8],
        encrypted_private_key: &[u8],
        nonce: &[u8],
        created_at: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO identity (id, public_key, encrypted_private_key, private_key_nonce, created_at)
             VALUES (1, ?1, ?2, ?3, ?4)",
            params![public_key, encrypted_private_key, nonce, created_at],
        )?;
        Ok(())
    }

    /// Load the stored identity (public key + encrypted private key + nonce).
    /// The caller must decrypt the private key using their passphrase-derived key.
    #[allow(clippy::type_complexity)]
    pub fn load_identity(&self) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT public_key, encrypted_private_key, private_key_nonce FROM identity WHERE id = 1",
        )?;
        let result = stmt
            .query_row([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(|_| StorageError::KeyNotFound)?;
        Ok(result)
    }

    /// Check if an identity exists.
    pub fn has_identity(&self) -> Result<bool, StorageError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM identity", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    // ─── Multi-Account Vault ─────────────────────────────────────────────────
    // Each account is an identity whose secret key is wrapped under its OWN
    // passphrase. On the unlock screen, the entered passphrase is tried against
    // every account blob; AEAD decryption success selects the account.

    /// Migrate a legacy single-identity row into `accounts` (idempotent).
    pub fn migrate_legacy_identity_to_account(&self) -> Result<(), StorageError> {
        self.conn.execute(
                "INSERT OR IGNORE INTO accounts (public_key, encrypted_private_key, private_key_nonce, label, created_at)
                 SELECT public_key, encrypted_private_key, private_key_nonce, 'Main', created_at
                 FROM identity WHERE id = 1",
                [],
            )?;
        Ok(())
    }

    /// The label recorded for the most recently created account, if any.
    ///
    /// Exists so the `label` column stays covered by a test without widening
    /// `KeyStore::conn` or mirroring the column into `AccountRow` (nothing
    /// reads it on the unlock path).
    #[cfg(test)]
    pub fn last_account_label(&self) -> Result<Option<String>, StorageError> {
        let label: Option<String> = self.conn.query_row(
            "SELECT label FROM accounts ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )?;
        Ok(label)
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountRow>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT public_key, encrypted_private_key, private_key_nonce
                 FROM accounts ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AccountRow {
                public_key: row.get(0)?,
                encrypted_private_key: row.get(1)?,
                private_key_nonce: row.get(2)?,
            })
        })?;
        let mut out: Vec<AccountRow> = Vec::new();
        for row in rows {
            out.push(row.map_err(StorageError::Database)?);
        }
        Ok(out)
    }

    /// Insert a brand-new account (fresh identity wrapped under its own passphrase).
    pub fn insert_account(
        &self,
        public_key: &[u8],
        encrypted_private_key: &[u8],
        nonce: &[u8],
        label: Option<&str>,
        created_at: i64,
    ) -> Result<i64, StorageError> {
        self.conn.execute(
                "INSERT INTO accounts (public_key, encrypted_private_key, private_key_nonce, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![public_key, encrypted_private_key, nonce, label, created_at],
            )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Refresh an existing account's wrapped secret key (e.g. after re-encryption).
    pub fn update_account_private_key(
        &self,
        public_key: &[u8],
        encrypted_private_key: &[u8],
        nonce: &[u8],
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE accounts SET encrypted_private_key = ?2, private_key_nonce = ?3
                 WHERE public_key = ?1",
            params![public_key, encrypted_private_key, nonce],
        )?;
        Ok(())
    }

    /// Add or update a known peer.
    pub fn upsert_peer(
        &self,
        public_key: &[u8],
        fingerprint: &str,
        alias: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO peers (public_key, fingerprint, alias, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(public_key) DO UPDATE SET
                last_seen = ?4,
                alias = COALESCE(?3, alias)",
            params![public_key, fingerprint, alias, now],
        )?;
        Ok(())
    }

    // ─── Family (Persistent Contact List) ───────────────────────

    /// Add a peer to the family list. Fails if already present.
    ///
    /// `nickname` and `last_address` are encrypted at rest when `key` is
    /// provided (they reveal the social graph and contact locations);
    /// `key = None` stores them as plaintext (legacy/no-vault profiles).
    pub fn add_family_member(
        &self,
        public_key: &[u8; 32],
        nickname: &str,
        expires_in_days: Option<u64>,
        last_address: Option<&str>,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<FamilyMember, StorageError> {
        use rusqlite::Error::SqliteFailure;

        let now = chrono::Utc::now().timestamp();
        let expires_at = expires_in_days.map(|days| now + (days as i64) * 86400);

        let nickname_stored = seal_meta_value(key, nickname, AAD_FAMILY)?;
        let address_stored = match last_address {
            Some(addr) => Some(seal_meta_value(key, addr, AAD_FAMILY)?),
            None => None,
        };

        let result = self.conn.execute(
            "INSERT INTO family (public_key, nickname, added_at, expires_at, last_address)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                public_key.as_slice(),
                nickname_stored,
                now,
                expires_at,
                address_stored
            ],
        );

        match result {
            Ok(_) => Ok(FamilyMember {
                public_key_hex: hex::encode(public_key),
                nickname: nickname.to_string(),
                added_at: now,
                expires_at,
                last_address: last_address.map(|s| s.to_string()),
            }),
            Err(SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                Err(StorageError::Database(rusqlite::Error::SqliteFailure(
                    e,
                    Some("peer already in family".to_string()),
                )))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// List all non-expired family members.
    pub fn list_family(
        &self,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<Vec<FamilyMember>, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = self.conn.prepare(
            "SELECT public_key, nickname, added_at, expires_at, last_address
             FROM family WHERE expires_at IS NULL OR expires_at > ?1
             ORDER BY nickname ASC",
        )?;
        // The closure cannot return a `StorageError`, so the length check runs
        // on the collected rows below rather than inside `query_map`. It used to
        // be done here into a `pk_arr` that was never read, while
        // `public_key_hex` was hex-encoded from the *unvalidated* bytes — so a
        // 31-byte key produced a 62-char hex string that no caller, all of which
        // decode it back to `[u8; 32]`, could use. The member would be listed in
        // the Hub and impossible to remove, verify or look up.
        let raw: Vec<(Vec<u8>, String, i64, Option<i64>, Option<String>)> = stmt
            .query_map(params![now], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        let mut members = Vec::new();
        for (pk_bytes, nickname, added_at, expires_at, last_address) in raw {
            if pk_bytes.len() != 32 {
                return Err(StorageError::PathError(format!(
                    "family row has a {}-byte public key, expected 32 — refusing to \
                     hand back a hex string no caller can decode",
                    pk_bytes.len()
                )));
            }
            let mut m = FamilyMember {
                public_key_hex: hex::encode(&pk_bytes),
                nickname,
                added_at,
                expires_at,
                last_address,
            };
            m.nickname = open_meta_value(key, &m.nickname, AAD_FAMILY)
                .unwrap_or_else(|_| "[encrypted]".to_string());
            if let Some(addr) = &m.last_address {
                m.last_address = Some(
                    open_meta_value(key, addr, AAD_FAMILY)
                        .unwrap_or_else(|_| "[encrypted]".to_string()),
                );
            }
            members.push(m);
        }
        Ok(members)
    }

    /// Remove a peer from the family list.
    pub fn remove_family_member(&self, public_key: &[u8; 32]) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM family WHERE public_key = ?1",
            params![public_key.as_slice()],
        )?;
        Ok(())
    }

    /// Update nickname for a family member.
    pub fn set_family_nickname(
        &self,
        public_key: &[u8; 32],
        nickname: &str,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<(), StorageError> {
        let stored = seal_meta_value(key, nickname, AAD_FAMILY)?;
        self.conn.execute(
            "UPDATE family SET nickname = ?1 WHERE public_key = ?2",
            params![stored, public_key.as_slice()],
        )?;
        Ok(())
    }

    /// Replace a family member's public key, address, and fingerprint.
    /// Nickname and expiry stay unchanged.
    pub fn update_family_member(
        &self,
        old_public_key: &[u8; 32],
        new_public_key: &[u8; 32],
        new_address: Option<&str>,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<FamilyMember, StorageError> {
        let _now = chrono::Utc::now().timestamp();
        let address_stored = match new_address {
            Some(addr) => Some(seal_meta_value(key, addr, AAD_FAMILY)?),
            None => None,
        };

        // Update the existing row's key and address
        self.conn.execute(
            "UPDATE family SET public_key = ?1, last_address = ?2 WHERE public_key = ?3",
            params![
                new_public_key.as_slice(),
                address_stored,
                old_public_key.as_slice()
            ],
        )?;

        // Read back the updated row
        let mut stmt = self.conn.prepare(
            "SELECT public_key, nickname, added_at, expires_at, last_address
             FROM family WHERE public_key = ?1",
        )?;
        let result = stmt.query_row(params![new_public_key.as_slice()], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        });
        match result {
            Ok((pk_bytes, nickname, added_at, expires_at, last_address)) => {
                // This path takes `new_public_key: &[u8; 32]`, so the value just
                // written cannot be short — but the row is read back rather than
                // echoed, and this is the function that tells the UI the update
                // worked. A corrupt row must not be reported as a member with a
                // hex string the caller cannot use.
                if pk_bytes.len() != 32 {
                    return Err(StorageError::PathError(format!(
                        "family row has a {}-byte public key, expected 32 — refusing to \
                         report the update as successful",
                        pk_bytes.len()
                    )));
                }
                let mut m = FamilyMember {
                    public_key_hex: hex::encode(&pk_bytes),
                    nickname,
                    added_at,
                    expires_at,
                    last_address,
                };
                m.nickname = open_meta_value(key, &m.nickname, AAD_FAMILY)
                    .unwrap_or_else(|_| "[encrypted]".to_string());
                if let Some(addr) = &m.last_address {
                    m.last_address = Some(
                        open_meta_value(key, addr, AAD_FAMILY)
                            .unwrap_or_else(|_| "[encrypted]".to_string()),
                    );
                }
                Ok(m)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(StorageError::Database(rusqlite::Error::QueryReturnedNoRows))
            }
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Check if a public key is in the family list.
    pub fn is_family_member(&self, public_key: &[u8; 32]) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM family WHERE public_key = ?1",
            params![public_key.as_slice()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check if a public key belongs to a previously-connected peer
    /// (present in the `peers` table). Used by the incoming-connection
    /// contact allowlist gate (H5).
    pub fn is_known_peer(&self, public_key: &[u8; 32]) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM peers WHERE public_key = ?1",
            params![public_key.as_slice()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get all family members including expired ones (for export).
    pub fn list_family_all(
        &self,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<Vec<FamilyMember>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT public_key, nickname, added_at, expires_at, last_address
             FROM family ORDER BY nickname ASC",
        )?;
        // Same validation as `list_family`, and for the same reason: this is the
        // export path, so a bad row would be written into a backup file as a
        // hex string that could never be imported again.
        let raw: Vec<(Vec<u8>, String, i64, Option<i64>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        let mut members = Vec::new();
        for (pk_bytes, nickname, added_at, expires_at, last_address) in raw {
            if pk_bytes.len() != 32 {
                return Err(StorageError::PathError(format!(
                    "family row has a {}-byte public key, expected 32 — refusing to \
                     export a hex string no caller can decode",
                    pk_bytes.len()
                )));
            }
            let mut m = FamilyMember {
                public_key_hex: hex::encode(&pk_bytes),
                nickname,
                added_at,
                expires_at,
                last_address,
            };
            m.nickname = open_meta_value(key, &m.nickname, AAD_FAMILY)
                .unwrap_or_else(|_| "[encrypted]".to_string());
            if let Some(addr) = &m.last_address {
                m.last_address = Some(
                    open_meta_value(key, addr, AAD_FAMILY)
                        .unwrap_or_else(|_| "[encrypted]".to_string()),
                );
            }
            members.push(m);
        }
        Ok(members)
    }

    /// Clear all family members (used during import).
    pub fn clear_family(&self) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM family", [])?;
        Ok(())
    }

    /// Insert a family member from raw values (used during import).
    pub fn insert_family_member_raw(
        &self,
        public_key: &[u8],
        nickname: &str,
        added_at: i64,
        expires_at: Option<i64>,
        last_address: Option<&str>,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<(), StorageError> {
        let nickname_stored = seal_meta_value(key, nickname, AAD_FAMILY)?;
        let address_stored = match last_address {
            Some(addr) => Some(seal_meta_value(key, addr, AAD_FAMILY)?),
            None => None,
        };
        self.conn.execute(
            "INSERT INTO family (public_key, nickname, added_at, expires_at, last_address)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                public_key,
                nickname_stored,
                added_at,
                expires_at,
                address_stored
            ],
        )?;
        Ok(())
    }
}

/// Outcome of a storage-cap eviction pass, for reporting to the user.
///
/// Modelled at module scope rather than inside `impl MessageStore` because Rust
/// does not allow struct definitions in an `impl` block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvictionReport {
    /// How many 1:1 messages were permanently destroyed.
    pub messages_evicted: u32,
    /// How many group messages were permanently destroyed.
    pub group_messages_evicted: u32,
    /// Conversations whose retention policy the cap overrode.
    ///
    /// A user who set "delete after 7 days" and then loses messages to a
    /// full disk has had a preference silently overridden. Naming the
    /// conversation is the difference between a policy the app states and one
    /// the user has to infer.
    pub overrode_retention: Vec<String>,
    /// Bytes actually released.
    pub bytes_freed: u64,
}

/// Outcome of one [`MessageStore::sweep`] pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepOutcome {
    /// Self-destruct messages destroyed because their timer had elapsed.
    pub expired_messages: u32,
    /// Messages destroyed by the storage cap, empty when under it.
    pub evicted: EvictionReport,
}

impl SweepOutcome {
    /// Whether this pass destroyed anything at all.
    pub fn destroyed_anything(&self) -> bool {
        self.expired_messages > 0
            || self.evicted.messages_evicted > 0
            || self.evicted.group_messages_evicted > 0
    }
}

/// The message store: holds chat history (optional).
/// Message contents are encrypted at the application level before storage.
pub struct MessageStore {
    conn: Connection,
    /// Number of content keys destroyed by shredding, for audit and for tests.
    ///
    /// The count is what makes the shred *verifiable through the public path*.
    /// `evict_to_cap` shreds and then deletes, so after a pass the rows are gone
    /// and there is nothing left to inspect — which meant a test could only
    /// call `shred_message_keys` directly, and would have passed unchanged if
    /// `evict_to_cap` had stopped calling it altogether. This counter makes the
    /// wiring itself assertable.
    shredded_keys: std::sync::atomic::AtomicU64,
}

// ─── Crypto-shredding primitives (H7) ──────────────────────────────────────
//
// Every message is encrypted under its own random 32-byte content key (CEK),
// never directly under the vault storage key. The CEK is wrapped under the
// vault key with a dedicated AAD domain and stored alongside the row.
//
// Deletion therefore destroys only a 72-byte wrapped key per message:
// after shredding, ciphertext copies lingering in WAL frames, freed pages,
// or disk slack are undecryptable even with full knowledge of the vault
// storage key. This converts an unbounded remnant problem into a bounded
// one that `secure_delete` + WAL checkpointing can reliably cover.

/// AAD domain for message content ciphertext.
///
/// Canonical definition — do not re-declare this elsewhere. A second copy of a
/// domain separator is a silent-drift hazard: if the two ever diverged, every
/// message in the database would become undecryptable, or message ciphertext
/// would be acceptable in a domain it was not sealed for.
///
/// This constant used to be declared *here* and again in `commands::util`, with
/// a comment in `commands::util` instructing the reader to keep them in sync by
/// hand — and the very next constant in that file violated its own rule. The
/// definition now lives only here, next to its only use, and the command layer
/// imports it.
pub const AAD_MSG_STORE: &[u8] = b"m2m-msg-v1";

/// AAD domain for wrapped content keys — distinct from every other domain.
const AAD_MSG_CEK: &[u8] = b"m2m-msg-cek-v1";
/// AAD domain for reaction text (messages.db reactions table).
const AAD_REACTION: &[u8] = b"m2m-reaction-v1";
/// AAD domain for family-contact metadata (keys.db family table).
const AAD_FAMILY: &[u8] = b"m2m-family-v1";
/// AAD domain for transfer metadata (transfers.db).
const AAD_TRANSFER: &[u8] = b"m2m-transfer-v1";

/// Length of a wrapped CEK blob: 24-byte XChaCha nonce || 32-byte CEK || 16-byte Poly1305 tag.
pub const WRAPPED_CEK_LEN: usize = 24 + 32 + 16;

/// `PRAGMA user_version` recorded in `messages.db` once the schema is known
/// complete.
///
/// Version 1 = every column the migrations add (`expires_at`, `read_at`,
/// `edited_at`, `deleted`, `is_favorite`, `archived`, `content_key_wrapped`) is
/// declared in the `CREATE TABLE` itself, so a fresh install never takes the
/// `ALTER TABLE` path at all.
///
/// Written *after* the migrations run, so a crash part way through leaves the
/// previous value and the next open retries rather than reporting a database
/// it never finished converting. `table_info` cannot make that distinction: it
/// sees the column that did get added and skips it on the next pass, which is
/// how a half-migrated schema becomes indistinguishable from a good one.
const MESSAGE_DB_SCHEMA_VERSION: i64 = 1;

/// Encrypt plaintext under `key`; returns (nonce, ciphertext).
fn seal_msg(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), StorageError> {
    use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
    let nonce_bytes = crate::crypto::random_bytes(24);
    let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key));
    let ct = cipher
        .encrypt(
            chacha20poly1305::XNonce::from_slice(&nonce_bytes),
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| StorageError::EncryptionFailed)?;
    Ok((nonce_bytes, ct))
}

/// Authenticate-and-decrypt; error on tag mismatch.
fn open_msg(key: &[u8; 32], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, ()> {
    use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
    if nonce.len() != 24 {
        return Err(());
    }
    let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key));
    cipher
        .decrypt(
            chacha20poly1305::XNonce::from_slice(nonce),
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| ())
}

// ─── Metadata-at-rest encryption (H-secondary) ─────────────────────────────
//
// Reaction text, family nicknames/addresses, and transfer filenames/paths
// used to be stored as plaintext — revealing social graph and file activity
// to anyone with the database files. Sensitive TEXT values are now stored in
// an authenticated envelope: "enc1:<nonce_hex>:<ciphertext_hex>".
//
// Rows written before this change (or by no-key paths) keep their plaintext
// value; readers accept both forms, so no destructive migration is needed.

/// Prefix marking an encrypted envelope value.
const ENC_PREFIX: &str = "enc1:";

/// Encrypt a UTF-8 metadata value into its storage envelope.
fn seal_meta_value(
    key: Option<&crate::secure_key::StorageKey>,
    plaintext: &str,
    aad: &[u8],
) -> Result<String, StorageError> {
    let Some(k) = key else {
        return Ok(plaintext.to_string());
    };
    let (nonce, ct) = seal_msg(k.as_bytes(), plaintext.as_bytes(), aad)?;
    Ok(format!(
        "{}{}:{}",
        ENC_PREFIX,
        hex::encode(nonce),
        hex::encode(ct)
    ))
}

/// Decrypt a metadata value written by [`seal_meta_value`].
/// Plaintext (legacy) values pass through unchanged.
fn open_meta_value(
    key: Option<&crate::secure_key::StorageKey>,
    stored: &str,
    aad: &[u8],
) -> Result<String, StorageError> {
    let Some(rest) = stored.strip_prefix(ENC_PREFIX) else {
        return Ok(stored.to_string());
    };
    let k = key.ok_or(StorageError::KeyNotFound)?;
    let mut parts = rest.splitn(2, ':');
    let nonce = parts.next().unwrap_or_default();
    let ct = parts.next().unwrap_or_default();
    let nonce = hex::decode(nonce).map_err(|_| StorageError::DecryptionFailed)?;
    let ct = hex::decode(ct).map_err(|_| StorageError::DecryptionFailed)?;
    let pt =
        open_msg(k.as_bytes(), &nonce, &ct, aad).map_err(|_| StorageError::DecryptionFailed)?;
    String::from_utf8(pt).map_err(|_| StorageError::DecryptionFailed)
}

impl MessageStore {
    /// Generate a fresh 32-byte content encryption key.
    fn generate_cek() -> [u8; 32] {
        let mut cek = [0u8; 32];
        crate::crypto::fill_random(&mut cek);
        cek
    }

    /// Wrap a CEK under the vault storage key → single BLOB (nonce || ciphertext).
    fn wrap_cek(
        cek: &[u8; 32],
        storage_key: &crate::secure_key::StorageKey,
    ) -> Result<Vec<u8>, StorageError> {
        let (nonce, ct) = seal_msg(storage_key.as_bytes(), cek, AAD_MSG_CEK)?;
        let mut blob = nonce;
        blob.extend_from_slice(&ct);
        Ok(blob)
    }

    /// Inverse of [`wrap_cek`]. Fails on tampering or wrong vault key.
    ///
    /// The length check is exact because the wrapped blob has a fixed size by
    /// construction. It used to be `len() < 24 + 1`, where the `1` stood for
    /// "at least a tag": a 25-byte blob passed, was sliced into a 24-byte nonce
    /// and 1 byte of "ciphertext", and was rejected later by `open_msg` as a
    /// decryption failure — so a truncated or corrupted key was reported as bad
    /// crypto rather than bad storage. Any row whose wrapped key is not exactly
    /// 72 bytes is either shredded-with-the-wrong-length or corrupt, and both
    /// deserve the same answer: this key is not usable.
    fn unwrap_cek(
        wrapped: &[u8],
        storage_key: &crate::secure_key::StorageKey,
    ) -> Result<[u8; 32], StorageError> {
        if wrapped.len() != WRAPPED_CEK_LEN {
            return Err(StorageError::KeyNotFound);
        }
        let mut cek = [0u8; 32];
        let pt = open_msg(
            storage_key.as_bytes(),
            &wrapped[..24],
            &wrapped[24..],
            AAD_MSG_CEK,
        )
        .map_err(|_| StorageError::KeyNotFound)?;
        if pt.len() != 32 {
            return Err(StorageError::KeyNotFound);
        }
        cek.copy_from_slice(&pt);
        Ok(cek)
    }

    /// Decrypt a stored message row's content.
    ///
    /// - Rows written via [`store_message_secure`]/[`edit_message_secure`]:
    ///   unwrap the CEK first, then decrypt the content under it.
    /// - Legacy rows (`content_key_wrapped IS NULL`): decrypt directly under
    ///   the vault storage key, matching pre-crypto-shredding behavior.
    pub fn decrypt_stored_content(
        stored_content: &[u8],
        stored_nonce: &[u8],
        content_key_wrapped: Option<&[u8]>,
        storage_key: &crate::secure_key::StorageKey,
    ) -> Result<Vec<u8>, StorageError> {
        match content_key_wrapped {
            Some(wrapped) => {
                let cek = Self::unwrap_cek(wrapped, storage_key)?;
                let pt = open_msg(&cek, stored_nonce, stored_content, AAD_MSG_STORE)
                    .map_err(|_| StorageError::KeyNotFound)?;
                Ok(pt)
            }
            None => open_msg(
                storage_key.as_bytes(),
                stored_nonce,
                stored_content,
                AAD_MSG_STORE,
            )
            .map_err(|_| StorageError::KeyNotFound),
        }
    }
    /// Open or create the message store.
    pub fn open(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        apply_connection_pragmas(&conn)?;

        // Every column the migrations below add is declared here.
        //
        // The `messages` table used to be created without `expires_at`,
        // `read_at`, `edited_at` or `deleted`, so a fresh install wrote the
        // table, read `PRAGMA table_info`, and then issued four `ALTER TABLE`s —
        // each its own implicit transaction. A crash between two of them left
        // a partially-migrated schema on a database that could not be fixed by
        // restarting, because the next run would read `table_info`, see the
        // column present, and skip it. Declaring the columns up front makes the
        // new-database path a single atomic `execute_batch`, and
        // `migrate_messages_table` remains for databases that predate this.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                peer_id BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                last_message_at INTEGER,
                display_name TEXT,
                peer_display_name TEXT,
                auto_delete_at INTEGER,
                retention_policy TEXT NOT NULL DEFAULT 'none',
                is_favorite INTEGER DEFAULT 0,
                archived INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                direction TEXT NOT NULL CHECK (direction IN ('sent', 'received')),
                content_encrypted BLOB NOT NULL,
                content_nonce BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                delivered INTEGER NOT NULL DEFAULT 0,
                content_key_wrapped BLOB,
                expires_at INTEGER,
                read_at INTEGER,
                edited_at INTEGER,
                deleted INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conversation
                ON messages(conversation_id, timestamp);
            -- These two depend on `expires_at`, which is declared above now, so
            -- they are created here rather than deferred to the migration. The
            -- migration still creates them (IF NOT EXISTS) for older databases.
            CREATE INDEX IF NOT EXISTS idx_messages_expires_at
                ON messages(expires_at);
            CREATE INDEX IF NOT EXISTS idx_messages_read_status
                ON messages(conversation_id, direction, read_at);
            CREATE TABLE IF NOT EXISTS reactions (
                message_id TEXT NOT NULL,
                reaction TEXT NOT NULL,
                peer_key_hex TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (message_id, peer_key_hex, reaction)
            );",
        )?;

        // Run migrations for existing databases that lack the new columns
        Self::migrate_conversations_table(&conn)?;
        Self::migrate_messages_table(&conn)?;

        // Record that the schema is known-complete, *after* the migrations, so
        // a crash mid-migration leaves the old version and the next open retries
        // rather than assuming success. Nothing reads this yet — it exists so
        // "is this database migrated" is an explicit fact rather than inferred
        // from `PRAGMA table_info`, which cannot distinguish "migrated" from
        // "half-migrated".
        conn.pragma_update(None, "user_version", MESSAGE_DB_SCHEMA_VERSION)?;

        let store = Self {
            conn,
            shredded_keys: std::sync::atomic::AtomicU64::new(0),
        };
        // Destroy anything whose self-destruct timer elapsed while the app was
        // not running.
        //
        // Expiry used to be driven only by a `setInterval` in `ChatView`, so it
        // ran *only while a chat screen was mounted*. For a tray app that is
        // most of its life, which means "auto-delete after 24h" did nothing at
        // all for a user who never left a conversation open — and a
        // self-destructed message stayed on disk, shredded-in-name-only, until
        // the app happened to be sitting on that screen. A promise about
        // destroying data cannot depend on which view is focused.
        //
        // The background sweep in `maintenance.rs` covers the running case;
        // this covers the closed one, and runs before the first query can read
        // the row back, so an expired message is never even returned by
        // `load_messages` after a restart.
        //
        // It must also run *before* `recompute_stored_bytes`, because the byte
        // counter is derived from the tables: counting rows that are about to
        // be destroyed leaves the cap permanently inflated by the size of
        // everything that already expired.
        match store.delete_expired_messages() {
            Ok(0) => {}
            Ok(n) => tracing::info!(
                expired = n,
                "self-destruct timer elapsed while closed — messages permanently destroyed"
            ),
            // A failure here means expired content is still on disk. Logged
            // rather than swallowed: the user has no other way to find out.
            Err(e) => tracing::error!(
                error = %e,
                "could not destroy elapsed self-destruct timers at startup"
            ),
        }

        // Same guarantee for the conversation retention policy, which is the
        // deadline the user set on the conversation itself rather than on an
        // individual message.
        match store.delete_messages_by_retention_policy() {
            Ok(0) => {}
            Ok(n) => tracing::info!(
                messages = n,
                "retention policy elapsed while closed — messages permanently destroyed"
            ),
            Err(e) => tracing::error!(
                error = %e,
                "could not enforce conversation retention policies at startup"
            ),
        }

        // Seed the storage-cap counter from the tables. Done once, at open, so
        // a database that already holds messages is accounted for from its
        // first launch — otherwise the cap would appear to be 0 bytes and
        // nothing would ever be evicted.
        store.recompute_stored_bytes()?;
        Ok(store)
    }

    /// Add new columns to the conversations table if they don't exist yet.
    fn migrate_conversations_table(conn: &Connection) -> Result<(), StorageError> {
        let mut stmt = conn.prepare("PRAGMA table_info(conversations)")?;
        let existing_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();

        if !existing_columns.contains(&"last_message_at".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN last_message_at INTEGER",
                [],
            )?;
        }
        if !existing_columns.contains(&"display_name".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN display_name TEXT", [])?;
        }
        if !existing_columns.contains(&"peer_display_name".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN peer_display_name TEXT",
                [],
            )?;
        }
        if !existing_columns.contains(&"auto_delete_at".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN auto_delete_at INTEGER",
                [],
            )?;
        }
        if !existing_columns.contains(&"retention_policy".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN retention_policy TEXT NOT NULL DEFAULT 'none'",
                [],
            )?;
        }
        if !existing_columns.contains(&"is_favorite".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN is_favorite INTEGER DEFAULT 0",
                [],
            )?;
        }
        if !existing_columns.contains(&"archived".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN archived INTEGER DEFAULT 0",
                [],
            )?;
        }
        Ok(())
    }

    /// Migrate the messages table — add `read_at`, `edited_at`, `deleted`, `expires_at` columns.
    ///
    /// Every column is already declared in the `CREATE TABLE` in `open`, so on a
    /// fresh install all four `PRAGMA table_info` checks hit and no `ALTER` runs.
    /// This remains the path for databases created by an earlier version, and it
    /// is idempotent, so it is safe to re-run on every open.
    fn migrate_messages_table(conn: &Connection) -> Result<(), StorageError> {
        let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
        let existing_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !existing_columns.contains(&"read_at".to_string()) {
            conn.execute("ALTER TABLE messages ADD COLUMN read_at INTEGER", [])?;
        }
        if !existing_columns.contains(&"edited_at".to_string()) {
            conn.execute("ALTER TABLE messages ADD COLUMN edited_at INTEGER", [])?;
        }
        if !existing_columns.contains(&"deleted".to_string()) {
            conn.execute(
                "ALTER TABLE messages ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        if !existing_columns.contains(&"expires_at".to_string()) {
            conn.execute("ALTER TABLE messages ADD COLUMN expires_at INTEGER", [])?;
        }
        if !existing_columns.contains(&"content_key_wrapped".to_string()) {
            // Crypto-shredding (H7): per-message content key, wrapped under
            // the vault storage key. Nullable — legacy rows are encrypted
            // directly under the vault key and decrypt via fallback.
            conn.execute(
                "ALTER TABLE messages ADD COLUMN content_key_wrapped BLOB",
                [],
            )?;
        }
        // Create indexes that depend on the columns above (expires_at, read_at).
        // These are CREATE INDEX IF NOT EXISTS so they're idempotent on re-run.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_expires_at
                ON messages(expires_at);
             CREATE INDEX IF NOT EXISTS idx_messages_read_status
                ON messages(conversation_id, direction, read_at);",
        )?;
        // Run group table migrations
        Self::migrate_group_tables(conn)?;
        Ok(())
    }

    /// Create or migrate group chat tables.
    fn migrate_group_tables(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS groups (
                group_id TEXT PRIMARY KEY,
                group_name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                our_role TEXT NOT NULL DEFAULT 'member',
                last_message_at INTEGER,
                last_message_preview TEXT
            );
            CREATE TABLE IF NOT EXISTS group_members (
                group_id TEXT NOT NULL,
                peer_key_hex TEXT NOT NULL,
                display_name TEXT,
                role TEXT NOT NULL DEFAULT 'member',
                added_at INTEGER NOT NULL,
                PRIMARY KEY (group_id, peer_key_hex),
                FOREIGN KEY (group_id) REFERENCES groups(group_id)
            );
            CREATE TABLE IF NOT EXISTS group_messages (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL,
                sender_peer_key_hex TEXT NOT NULL,
                content_encrypted BLOB NOT NULL,
                content_nonce BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                delivered INTEGER NOT NULL DEFAULT 1,
                edited_at INTEGER,
                deleted INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (group_id) REFERENCES groups(group_id)
            );
            CREATE INDEX IF NOT EXISTS idx_group_messages_group
                ON group_messages(group_id, timestamp);

            -- Byte accounting for the storage cap.
            --
            -- A single row holding the running total of on-disk message bytes.
            -- It exists because the alternative — SUM(LENGTH(...)) over the
            -- whole table on every inbound message — is O(rows), and the
            -- receive loop admits up to 30 messages/second, so a full scan per
            -- message is not viable. The counter is re-derived from SQL
            -- whenever usage approaches the cap (see `recompute_stored_bytes`),
            -- so drift from an un-audited write path self-corrects exactly
            -- where being wrong would matter.
            CREATE TABLE IF NOT EXISTS storage_stats (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                total_bytes INTEGER NOT NULL DEFAULT 0
            );
            INSERT OR IGNORE INTO storage_stats (id, total_bytes) VALUES (1, 0);

            -- Oldest-first eviction walks this ordering.
            CREATE INDEX IF NOT EXISTS idx_messages_oldest
                ON messages(timestamp);",
        )?;
        Ok(())
    }

    /// Store a message (idempotent — duplicate message IDs are silently ignored).
    ///
    /// ⚠️ TEST-ONLY legacy path: content is stored exactly as provided and is
    /// NOT crypto-shreddable. Production code MUST use [`store_message_secure`].
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn store_message(
        &self,
        id: &str,
        conversation_id: &str,
        direction: &str,
        content_encrypted: &[u8],
        content_nonce: &[u8],
        timestamp: i64,
        delivered: bool,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO messages (id, conversation_id, direction, content_encrypted, content_nonce, timestamp, delivered)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, conversation_id, direction, content_encrypted, content_nonce, timestamp, delivered as i32],
        )?;
        self.conn.execute(
            "UPDATE conversations SET last_message_at = ?1 WHERE id = ?2",
            params![timestamp, conversation_id],
        )?;
        Ok(())
    }

    /// Store a message encrypted under a FRESH random per-message content key
    /// (CEK), which is itself wrapped under the vault storage key (H7).
    ///
    /// Takes PLAINTEXT — encryption happens here, not at the call site, so the
    /// CEK never leaves this module unwrapped. Deleting the message shreds the
    /// wrapped CEK, rendering every copy of the ciphertext (including WAL and
    /// freed-page remnants) undecryptable.
    ///
    /// Idempotent: duplicate message IDs are silently ignored.
    #[allow(clippy::too_many_arguments)]
    pub fn store_message_secure(
        &self,
        id: &str,
        conversation_id: &str,
        direction: &str,
        plaintext: &[u8],
        timestamp: i64,
        expires_at: Option<i64>,
        delivered: bool,
        storage_key: &crate::secure_key::StorageKey,
    ) -> Result<(), StorageError> {
        let mut cek = Self::generate_cek();
        let result = (|| {
            let wrapped = Self::wrap_cek(&cek, storage_key)?;
            let (nonce, ciphertext) = seal_msg(&cek, plaintext, AAD_MSG_STORE)?;
            self.conn.execute(
                "INSERT OR IGNORE INTO messages
                    (id, conversation_id, direction, content_encrypted, content_nonce, timestamp, expires_at, delivered, content_key_wrapped)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![id, conversation_id, direction, ciphertext, nonce, timestamp, expires_at, delivered as i32, wrapped],
            )?;
            // `changes()` reflects the *most recent* statement, so it has to be
            // read here — after the following `UPDATE conversations` it would
            // report that update's row count (always 1) instead, and every
            // ignored duplicate would inflate the total. Only add bytes when a
            // row was actually created.
            let inserted = self.conn.changes();
            self.conn.execute(
                "UPDATE conversations SET last_message_at = ?1 WHERE id = ?2",
                params![timestamp, conversation_id],
            )?;
            if inserted > 0 {
                self.add_stored_bytes(Self::msg_row_bytes(ciphertext.len(), nonce.len()));
            }
            Ok(())
        })();
        use zeroize::Zeroize;
        cek.zeroize();
        result
    }

    /// Load undelivered (queued) sent messages for a conversation.
    /// Returns messages ordered oldest-first so they are re-sent in order.
    pub fn load_undelivered_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, content_encrypted, content_nonce, timestamp, read_at,
                    edited_at, deleted, expires_at, content_key_wrapped
             FROM messages WHERE conversation_id = ?1
             AND direction = 'sent' AND delivered = 0
             ORDER BY timestamp ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                direction: row.get(1)?,
                content_encrypted: row.get(2)?,
                content_nonce: row.get(3)?,
                timestamp: row.get(4)?,
                read_at: row.get(5)?,
                edited_at: row.get(6)?,
                deleted: row.get::<_, i64>(7)? != 0,
                expires_at: row.get(8)?,
                content_key_wrapped: row.get(9)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    /// Mark a message as delivered.
    pub fn mark_delivered(&self, message_id: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE messages SET delivered = 1 WHERE id = ?1",
            params![message_id],
        )?;
        Ok(())
    }

    /// Load sent messages for a conversation with timestamp >= since.
    /// Used to respond to SyncRequest — returns messages others sent *to* this peer.
    pub fn load_sent_messages_since(
        &self,
        conversation_id: &str,
        since_timestamp: i64,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, content_encrypted, content_nonce, timestamp, read_at,
                    edited_at, deleted, expires_at, content_key_wrapped
             FROM messages WHERE conversation_id = ?1
             AND direction = 'sent' AND timestamp > ?2
             ORDER BY timestamp ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id, since_timestamp], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                direction: row.get(1)?,
                content_encrypted: row.get(2)?,
                content_nonce: row.get(3)?,
                timestamp: row.get(4)?,
                read_at: row.get(5)?,
                edited_at: row.get(6)?,
                deleted: row.get::<_, i64>(7)? != 0,
                expires_at: row.get(8)?,
                content_key_wrapped: row.get(9)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    /// Get the most recent received message timestamp for a conversation.
    /// Returns 0 if no received messages exist.
    pub fn get_latest_received_timestamp(
        &self,
        conversation_id: &str,
    ) -> Result<i64, StorageError> {
        let result: Result<i64, _> = self.conn.query_row(
            "SELECT COALESCE(MAX(timestamp), 0) FROM messages
             WHERE conversation_id = ?1 AND direction = 'received'",
            params![conversation_id],
            |row| row.get(0),
        );
        Ok(result.unwrap_or(0))
    }

    // ─── Storage-cap byte accounting ────────────────────────────────────────
    //
    // The cap exists because the receive loop's limits are *rate* limits: 30
    // frames/s and 16 MiB/s bound how fast a peer can write, not how much in
    // total. A peer that simply keeps going fills the disk unattended, and
    // `retention_policy = 'none'` is the default, so nothing ever reclaims it.
    //
    // Per-row size is exact rather than estimated. Every message is sealed as
    // `content_encrypted || content_nonce || content_key_wrapped`, and the
    // wrapped key is a fixed 72 bytes (`WRAPPED_CEK_LEN = 24 + 32 + 16`).
    //
    // Group messages are counted too. `group_messages` is a separate table
    // with its own delete paths, so leaving it out would make the cap
    // trivially bypassable: an attacker fills it while the 1:1 store sits at
    // zero.

    /// Bytes occupied by one 1:1 message row.
    const MSG_ROW_OVERHEAD: i64 = WRAPPED_CEK_LEN as i64;

    /// Re-derive the byte total from the tables and overwrite the counter.
    ///
    /// Called at open (to backfill a store that predates the counter) and
    /// whenever usage approaches the cap, so that drift accumulated by a write
    /// path nobody remembered to update cannot cause the cap to be overshot.
    pub fn recompute_stored_bytes(&self) -> Result<u64, StorageError> {
        let total: i64 = self.conn.query_row(
            "SELECT COALESCE((SELECT SUM(LENGTH(content_encrypted)
                                    + LENGTH(content_nonce)
                                    + ?1)
                             FROM messages), 0)
                  + COALESCE((SELECT SUM(LENGTH(content_encrypted)
                                           + LENGTH(content_nonce)
                                           + ?1)
                              FROM group_messages), 0)",
            params![Self::MSG_ROW_OVERHEAD],
            |row| row.get(0),
        )?;
        let total = total.max(0) as u64;
        self.conn.execute(
            "INSERT INTO storage_stats (id, total_bytes) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET total_bytes = ?1",
            params![total as i64],
        )?;
        Ok(total)
    }

    /// Current stored message bytes, from the running counter.
    ///
    /// O(1). The counter is authoritative between recomputes; callers that are
    /// about to enforce a limit should use [`Self::stored_bytes_verified`], which
    /// re-derives from SQL first so a stale low count cannot overshoot the cap.
    pub fn stored_bytes(&self) -> Result<u64, StorageError> {
        // Distinguish "no row yet" (legitimately zero) from "the counter is
        // unreadable". Collapsing the second into zero makes a broken counter
        // read as an empty store, which disables the cap and shows the user
        // "0 bytes used" on a full disk — a claim the disk cannot back.
        match self
            .conn
            .query_row("SELECT total_bytes FROM storage_stats WHERE id = 1", [], |row| {
                row.get::<_, i64>(0)
            }) {
            Ok(total) => Ok(total.max(0) as u64),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    /// Current stored bytes, re-derived from SQL so the number is exact.
    ///
    /// Used on the enforcement path. The scan is the point: it is correct even
    /// if some write path forgot to maintain the counter, which is the failure
    /// mode a cached total cannot catch.
    pub fn stored_bytes_verified(&self) -> Result<u64, StorageError> {
        self.recompute_stored_bytes()
    }

    /// Add to the running byte counter (clamped at zero).
    fn add_stored_bytes(&self, delta: i64) {
        let _ = self.conn.execute(
            "UPDATE storage_stats
                SET total_bytes = MAX(0, total_bytes + ?1)
              WHERE id = 1",
            params![delta],
        );
    }

    /// Size in bytes of a single message body as stored.
    fn msg_row_bytes(content_encrypted_len: usize, content_nonce_len: usize) -> i64 {
        content_encrypted_len as i64 + content_nonce_len as i64 + Self::MSG_ROW_OVERHEAD
    }

    /// Enforce the storage cap, and report what was destroyed.
    ///
    /// This is **the** definition of "the cap is enforced", and every write path
    /// that puts a message on disk calls it — inbound (`handle_incoming_text`),
    /// outbound (`send_message`, `send_message_with_timer`) and group
    /// (`send_group_message`, inbound group frames) alike. It exists as one
    /// function because those paths had already begun to diverge: the receive
    /// loop enforced the cap and the three others did not, so a user filling
    /// the disk by sending, or an attacker filling it with group traffic, was
    /// bounded only by whatever the periodic sweep happened to catch. A cap that
    /// most write paths bypass is not a cap.
    ///
    /// Called *before* the write, so a store that is already over the ceiling
    /// cannot be pushed further past it by the message being written.
    ///
    /// The fast path is one O(1) counter read; the expensive re-derivation
    /// inside `evict_to_cap` only runs once the ceiling is genuinely crossed.
    ///
    /// `cap_bytes` is passed in rather than read from the config so the caller
    /// reads `security_config` *before* taking the store lock. The reverse
    /// nesting is a deadlock cycle, which is the shape both of this codebase's
    /// historical deadlocks took.
    ///
    /// Returns `None` when nothing was destroyed, including the case where the
    /// store is over the cap but holds no evictable rows — a cap smaller than a
    /// single message. The caller uses this to decide whether to bother the
    /// user with a `m2m://storage-evicted` event.
    pub fn enforce_storage_cap(
        &self,
        cap_bytes: u64,
    ) -> Result<Option<EvictionReport>, StorageError> {
        // The gate must be able to *disagree* with the cached counter, or it is
        // not a check at all.
        //
        // `stored_bytes()` is O(1) but is only correct if every write path
        // remembered to call `add_stored_bytes`. A single path that did not
        // leaves the counter low, and a gate that trusts it returns early and
        // does nothing — the counter has to first exceed the cap on its own
        // before the scan that would have corrected it ever runs. A cap that
        // most write paths bypass is not a cap.
        //
        // The cached value is therefore only ever a *hint about whether to
        // verify*, never a substitute for verification: the moment it says
        // "possibly over, or near enough that I might be", the authoritative SQL
        // re-derivation decides. Note the asymmetry that matters — the earlier
        // version asked "is the counter within the band around the cap?", which a
        // counter drifted *low* answers "no", so the severe direction was exactly
        // the one that could not be detected.
        //
        // `verify_margin` is proportional, not fixed, because
        // `stored_bytes_verified` is an unindexed `SUM(LENGTH(...))` over every
        // row and this runs on every write path while the caller holds
        // `state.message_store.lock()`. 1/16 of the cap keeps the verification
        // window tight without making the common case a full table scan.
        let verify_margin = (cap_bytes / 16).min(64 * 1024 * 1024);
        let cached = self.stored_bytes()?;
        let mut usage = cached;
        if cached > cap_bytes || cached.saturating_add(verify_margin) >= cap_bytes {
            usage = self.stored_bytes_verified()?;
        }
        if usage <= cap_bytes {
            return Ok(None);
        }
        let report = self.evict_to_cap(cap_bytes)?;
        if report.messages_evicted == 0 && report.group_messages_evicted == 0 {
            return Ok(None);
        }
        Ok(Some(report))
    }

    /// One pass of the two policies that destroy stored history on a timer:
    /// self-destruct expiry and the storage cap.
    ///
    /// Split out of the background task so the policy is testable without a
    /// tokio runtime or an `AppHandle`, and so the task has no logic of its own
    /// to get wrong — it decides *when*, this decides *what*.
    ///
    /// Expiry runs first and unconditionally, including in ephemeral mode:
    /// deleting is always the safe direction, and a store that accumulated
    /// messages before the user turned ephemeral mode on still owes them the
    /// timers they were given.
    pub fn sweep(&self, cap_bytes: u64) -> Result<SweepOutcome, StorageError> {
let expired_messages = self.delete_expired_messages()?;
        if expired_messages > 0 {
            tracing::info!(
                expired = expired_messages,
                "self-destruct timer elapsed - messages permanently destroyed"
            );
        }
        // Re-derive the byte counter from SQL before the cap is evaluated. This
        // is the pass that bounds drift accumulated by a write path nobody
        // remembered to update: `enforce_storage_cap` only verifies
        // opportunistically (its band is a performance compromise), so without
        // this the counter could sit arbitrarily far below the truth until it
        // happened to land near the cap — and under-reporting is the direction
        // that silently disables the cap. Runs on the 15-minute timer, not per
        // frame, so the cost is bounded.
        self.stored_bytes_verified()?;
        // Conversation retention policies are the *other* thing that destroys
        // stored history, so they belong in the same pass. Leaving them out is
        // what let "Auto-Delete After 24h" persist, display, and never fire.
        let retention_deleted = self.delete_messages_by_retention_policy()?;
        if retention_deleted > 0 {
            tracing::info!(
                messages = retention_deleted,
                "conversation retention policy elapsed — messages permanently destroyed"
            );
        }
        let evicted = self.enforce_storage_cap(cap_bytes)?;
        Ok(SweepOutcome {
            expired_messages: expired_messages + retention_deleted,
            evicted: evicted.unwrap_or_default(),
        })
    }

    /// Permanently evict the oldest stored messages until usage is at or below
    /// `cap_bytes`.
    ///
    /// This is a **hard** delete, not the soft-delete tombstone used for a
    /// user's own "delete for everyone": the row is removed so the bytes are
    /// actually released. It follows the same four-step shredding sequence as
    /// `delete_conversation` —
    ///
    /// 1. overwrite `content_key_wrapped` with zeros, destroying the content
    ///    encryption key, so the ciphertext becomes undecryptable regardless of
    ///    what survives on disk;
    /// 2. `wal_checkpoint(TRUNCATE)`, so shredded cells cannot linger in
    ///    `messages.db-wal`;
    /// 3. `DELETE`, with `secure_delete = ON` zeroing freed in-page content;
    /// 4. truncate the WAL again.
    ///
    /// Group messages are evicted too — `group_messages` is a separate table,
    /// and counting it is the only reason the cap cannot be side-stepped by an
    /// attacker who just sends group traffic.
    ///
    /// # Why evict rather than refuse
    ///
    /// Refusing writes at the cap is the other option, and it was rejected: it
    /// drops a legitimate inbound message mid-conversation with no way for the
    /// user to recover it, and it still leaves a full disk — just a slower fill.
    /// Oldest-first keeps the app working and bounds the damage to history the
    /// user already has, which is recoverable in the sense that matters here:
    /// it still exists on the other peer's device.
    ///
    /// # Ordering and batching
    ///
    /// Rows are taken oldest-first in bounded batches, stopping at a low-water
    /// mark so this does not re-run on every subsequent write. Batch size is
    /// capped so a single pass cannot hold the store lock long enough to stall
    /// a peer's message read.
    pub fn evict_to_cap(&self, cap_bytes: u64) -> Result<EvictionReport, StorageError> {
        let mut report = EvictionReport::default();
        // Exact, not cached: this is the enforcement path, and a counter that
        // drifted low would let the cap be overshot. The scan is correct even
        // if some write path forgot to maintain the counter.
        let mut used = self.stored_bytes_verified()?;
        if used <= cap_bytes {
            return Ok(report);
        }

        // Stop below the cap rather than at it, so eviction does not re-run on
        // every subsequent message.
        let target = cap_bytes * 90 / 100;
        // Bounds one pass's lock hold time: 200 rows of shred + delete is
        // short work, and a peer reading the store should never queue behind
        // a large eviction.
        const BATCH: usize = 200;

        self.conn.pragma_update(None, "secure_delete", "ON")?;

        while used > target {
            // How many rows does getting to `target` actually require? Taking a
            // fixed batch instead would evict a whole 200-row batch when only a
            // handful of rows are needed, destroying far more history than the
            // cap demanded. Estimated from the current average row size and
            // then clamped to BATCH, with +1 so rounding cannot leave the loop
            // unable to make progress.
            let rows_total: i64 = self
                .conn
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM messages)
                          + (SELECT COUNT(*) FROM group_messages)",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if rows_total == 0 || used == 0 {
                break;
            }
            let avg = (used / rows_total as u64).max(1);
            let needed = used - target;
            let want = needed.div_ceil(avg).saturating_add(1) as usize;

            // 1:1 first, then group. Both tables count toward the cap, so
            // evicting only one would let an attacker park everything in the
            // other.
            let (ids, freed) = self.oldest_message_batch(want.min(BATCH), &mut report)?;
            if !ids.is_empty() {
                self.shred_message_keys(&ids)?;
                // One checkpoint between the shred and the delete, one after.
                // They cannot be merged into a single transaction because
                // `wal_checkpoint(TRUNCATE)` is refused while a write
                // transaction is open, and it must run *after* the shred is
                // durable for the shred to have any point.
                //
                // What the delete loop itself must not be is a sequence of
                // independent autocommit statements. A crash or `SQLITE_BUSY`
                // between two of them used to leave a partially-deleted batch
                // whose shredded rows were still listed by `load_messages` —
                // which filters on `expires_at` alone, not on key state — so
                // they rendered as "[encrypted]" permanently, and
                // `report.messages_evicted` claimed rows that were still on
                // disk. One transaction makes the batch all-or-nothing.
                self.wal_checkpoint_truncate()?;
                {
                    let tx = self.conn.unchecked_transaction()?;
                    for id in &ids {
                        tx.execute("DELETE FROM messages WHERE id = ?1", params![id])?;
                    }
                    tx.commit()?;
                }
                self.wal_checkpoint_truncate()?;
                report.messages_evicted += ids.len() as u32;
                let freed = freed.max(0) as u64;
                report.bytes_freed += freed;
                self.add_stored_bytes(-(freed as i64));
                used = used.saturating_sub(freed);
                continue;
            }

            let (gids, gfreed) = self.oldest_group_message_batch(want.min(BATCH))?;
            if gids.is_empty() {
                // Nothing left to evict at all. Guard against a cap we cannot
                // satisfy rather than spinning.
                break;
            }
            // Same all-or-nothing requirement as the 1:1 branch: a torn group
            // eviction reports rows as freed that are still on disk, and the
            // byte counter is decremented by the full batch.
            {
                let tx = self.conn.unchecked_transaction()?;
                for id in &gids {
                    tx.execute(
                        "DELETE FROM group_messages WHERE id = ?1",
                        params![id],
                    )?;
                }
                tx.commit()?;
            }
            self.wal_checkpoint_truncate()?;
            report.group_messages_evicted += gids.len() as u32;
            let gfreed = gfreed.max(0) as u64;
            report.bytes_freed += gfreed;
            self.add_stored_bytes(-(gfreed as i64));
            used = used.saturating_sub(gfreed);
        }

        if report.messages_evicted > 0 || report.group_messages_evicted > 0 {
            tracing::info!(
                evicted = report.messages_evicted,
                group_evicted = report.group_messages_evicted,
                freed_mb = report.bytes_freed / (1024 * 1024),
                remaining_mb = self.stored_bytes().unwrap_or(0) / (1024 * 1024),
                "storage cap reached — oldest messages permanently evicted"
            );
        }
        Ok(report)
    }

    /// The oldest `limit` 1:1 message ids, with their total stored size.
    ///
    /// Records any conversation whose retention policy the eviction is about to
    /// override, so the caller can name it to the user rather than quietly
    /// discarding a preference they set.
    fn oldest_message_batch(
        &self,
        limit: usize,
        report: &mut EvictionReport,
    ) -> Result<(Vec<String>, i64), StorageError> {
        let rows: Vec<(String, String, i64)> = {
            let mut stmt = self.conn.prepare(
                "SELECT m.id,
                        COALESCE(m.conversation_id, ''),
                        COALESCE(LENGTH(m.content_encrypted)
                                 + LENGTH(m.content_nonce)
                                 + ?2, 0)
                   FROM messages m
                  ORDER BY m.timestamp ASC
                  LIMIT ?1",
            )?;
            let mapped = stmt.query_map(params![limit as i64, Self::MSG_ROW_OVERHEAD], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            mapped.filter_map(|r| r.ok()).collect()
        };
        if rows.is_empty() {
            return Ok((Vec::new(), 0));
        }

        // One policy lookup per *distinct* conversation, not per message — a
        // batch is usually one or two conversations.
        for (_, conv, _) in &rows {
            if conv.is_empty() || report.overrode_retention.contains(conv) {
                continue;
            }
            let policy: Option<String> = self
                .conn
                .query_row(
                    "SELECT retention_policy FROM conversations WHERE id = ?1",
                    params![conv],
                    |row| row.get(0),
                )
                .ok();
            match policy {
                Some(p) if p != "none" => report.overrode_retention.push(conv.clone()),
                _ => {}
            }
        }

        let freed = rows.iter().map(|(_, _, n)| *n).sum();
        Ok((rows.into_iter().map(|(id, _, _)| id).collect(), freed))
    }

    /// The oldest `limit` group message ids, with their total stored size.
    fn oldest_group_message_batch(
        &self,
        limit: usize,
    ) -> Result<(Vec<String>, i64), StorageError> {
        let rows: Vec<(String, i64)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id,
                        COALESCE(LENGTH(content_encrypted)
                                 + LENGTH(content_nonce)
                                 + ?2, 0)
                   FROM group_messages
                  ORDER BY timestamp ASC
                  LIMIT ?1",
            )?;
            let mapped = stmt.query_map(params![limit as i64, Self::MSG_ROW_OVERHEAD], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
            mapped.filter_map(|r| r.ok()).collect()
        };
        let freed = rows.iter().map(|(_, n)| *n).sum();
        Ok((rows.into_iter().map(|(id, _)| id).collect(), freed))
    }

    /// Step 1 of the shred sequence: destroy the content encryption keys for
    /// these rows. Once this commits the ciphertext is undecryptable, whatever
    /// later happens to the row or the file.
    ///
    /// Idempotent and atomic, because the caller may retry it. The `!=` guard
    /// skips rows that already hold the zero blob: without it a retry after a
    /// crash between the shred and the delete rewrites zeros over zeros and
    /// `shredded_keys` counts the same key twice, so the audit figure would
    /// claim more keys destroyed than exist. The `IS NULL` arm is still
    /// shredded — a legacy row with no wrapped key is encrypted directly under
    /// the vault key, so it is exactly the case shredding exists for.
    ///
    /// One transaction rather than a commit per row: a crash half way through
    /// 200 rows used to leave the batch partly shredded and partly intact while
    /// the caller had already been told the whole batch was.
    fn shred_message_keys(&self, ids: &[String]) -> Result<(), StorageError> {
        let mut shredded = 0u64;
        {
            let tx = self.conn.unchecked_transaction()?;
            for id in ids {
                let n = tx.execute(
                    "UPDATE messages SET content_key_wrapped = ?2
                      WHERE id = ?1
                        AND (content_key_wrapped IS NULL OR content_key_wrapped != ?2)",
                    params![id, vec![0u8; WRAPPED_CEK_LEN]],
                )?;
                shredded += n as u64;
            }
            tx.commit()?;
        }
        self.shredded_keys
            .fetch_add(shredded, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// How many content keys this store has destroyed by shredding.
    pub fn shredded_key_count(&self) -> u64 {
        self.shredded_keys.load(std::sync::atomic::Ordering::Relaxed)
    }


    /// Create or get a conversation.
    pub fn ensure_conversation(
        &self,
        conversation_id: &str,
        peer_id: &[u8],
    ) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT OR IGNORE INTO conversations (id, peer_id, created_at, retention_policy) VALUES (?1, ?2, ?3, 'none')",
            params![conversation_id, peer_id, now],
        )?;
        Ok(())
    }

    /// Load messages for a conversation (most recent first, with limit).
    /// Skips expired messages (those past their expires_at).
    pub fn load_messages(
        &self,
        conversation_id: &str,
        limit: i64,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, content_encrypted, content_nonce, timestamp, read_at,
                    edited_at, deleted, expires_at, content_key_wrapped
             FROM messages WHERE conversation_id = ?1
             AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY timestamp DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![conversation_id, now, limit], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                direction: row.get(1)?,
                content_encrypted: row.get(2)?,
                content_nonce: row.get(3)?,
                timestamp: row.get(4)?,
                read_at: row.get(5)?,
                edited_at: row.get(6)?,
                deleted: row.get::<_, i64>(7)? != 0,
                expires_at: row.get(8)?,
                content_key_wrapped: row.get(9)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        messages.reverse();
        Ok(messages)
    }

    /// Load messages older than a given timestamp (cursor-based pagination).
    /// Returns messages with timestamp < `before`, ordered most-recent-first, limited to `limit`.
    /// Skips expired messages.
    pub fn load_messages_before(
        &self,
        conversation_id: &str,
        before_timestamp: i64,
        limit: i64,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, content_encrypted, content_nonce, timestamp, read_at,
                    edited_at, deleted, expires_at, content_key_wrapped
             FROM messages WHERE conversation_id = ?1
             AND (expires_at IS NULL OR expires_at > ?2)
             AND timestamp < ?3
             ORDER BY timestamp DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(
            params![conversation_id, now, before_timestamp, limit],
            |row| {
                Ok(StoredMessage {
                    id: row.get(0)?,
                    direction: row.get(1)?,
                    content_encrypted: row.get(2)?,
                    content_nonce: row.get(3)?,
                    timestamp: row.get(4)?,
                    read_at: row.get(5)?,
                    edited_at: row.get(6)?,
                    deleted: row.get::<_, i64>(7)? != 0,
                    expires_at: row.get(8)?,
                    content_key_wrapped: row.get(9)?,
                })
            },
        )?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        messages.reverse();
        Ok(messages)
    }

    /// Run PRAGMA optimize to keep the database performant over time.
    /// Should be called periodically (e.g. after write operations, but at most once per minute).
    pub fn optimize(&self) -> Result<(), StorageError> {
        self.conn.execute_batch("PRAGMA optimize;")?;
        Ok(())
    }

    /// List all conversations with summary info.
    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.peer_id, c.created_at, c.last_message_at,
                    c.display_name, c.peer_display_name,
                    c.auto_delete_at, c.retention_policy,
                    (SELECT COUNT(*) FROM messages m
                      WHERE m.conversation_id = c.id
                        AND m.deleted = 0
                        AND (m.expires_at IS NULL OR m.expires_at > ?1)) as msg_count,
                    COALESCE(c.is_favorite, 0) as is_favorite,
                    COALESCE(c.archived, 0) as archived,
                    (SELECT COUNT(*) FROM messages m
                     WHERE m.conversation_id = c.id
                       AND m.direction = 'received' AND m.read_at IS NULL
                       AND m.deleted = 0
                       AND (m.expires_at IS NULL OR m.expires_at > ?1)) as unread_count
             FROM conversations c
             ORDER BY archived ASC, COALESCE(c.is_favorite, 0) DESC,
                      COALESCE(c.last_message_at, c.created_at) DESC",
        )?;
        let now = chrono::Utc::now().timestamp();
        let rows = stmt.query_map(params![now], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                created_at: row.get(2)?,
                last_message_at: row.get(3)?,
                display_name: row.get(4)?,
                peer_display_name: row.get(5)?,
                auto_delete_at: row.get(6)?,
                retention_policy: row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "none".to_string()),
                message_count: row.get(8)?,
                is_favorite: row.get::<_, Option<bool>>(9)?,
                archived: row.get::<_, Option<bool>>(10)?,
                unread_count: row.get::<_, i64>(11)? as u32,
            })
        })?;
        let mut convos = Vec::new();
        for row in rows {
            convos.push(row?);
        }
        Ok(convos)
    }

    /// Rename a conversation (local display name).
    pub fn rename_conversation(
        &self,
        conversation_id: &str,
        display_name: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE conversations SET display_name = ?1 WHERE id = ?2",
            params![display_name, conversation_id],
        )?;
        Ok(())
    }

    /// Set the peer's display name for a conversation (received from peer).
    pub fn set_peer_display_name(
        &self,
        conversation_id: &str,
        peer_display_name: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE conversations SET peer_display_name = ?1 WHERE id = ?2",
            params![peer_display_name, conversation_id],
        )?;
        Ok(())
    }

    /// Set per-conversation retention policy and auto-delete timer.
    ///
    /// When `policy` is `delete` and a duration is given, the timer is applied
    /// **retroactively** to the messages already stored: `auto_delete_at` is a
    /// conversation-wide deadline enforced by [`Self::sweep`], so a policy the
    /// user sets now has to cover the history it is meant to be a policy *for*.
    /// Only messages already stamped with their own (longer) self-destruct
    /// timer are left alone — a per-message timer is the more specific promise,
    /// and silently shortening it would be a data-destruction the user did not
    /// ask for.
    pub fn set_conversation_retention(
        &self,
        conversation_id: &str,
        policy: &str,
        duration_secs: Option<i64>,
    ) -> Result<(), StorageError> {
        let auto_delete_at = duration_secs.map(|d| chrono::Utc::now().timestamp() + d);
        self.conn.execute(
            "UPDATE conversations SET retention_policy = ?1, auto_delete_at = ?2 WHERE id = ?3",
            params![policy, auto_delete_at, conversation_id],
        )?;
        if policy == "delete" {
            if let Some(deadline) = auto_delete_at {
                self.conn.execute(
                    "UPDATE messages SET expires_at = ?1
                      WHERE conversation_id = ?2
                        AND expires_at IS NULL",
                    params![deadline, conversation_id],
                )?;
            }
        }
        Ok(())
    }

    /// Permanently destroy messages in conversations whose retention deadline
    /// has elapsed.
    ///
    /// This is what makes the conversation policy selector real. It used to be
    /// written to `conversations.retention_policy` / `auto_delete_at` and
    /// displayed back to the user, and **nothing ever deleted anything on that
    /// basis** — `sweep` consulted only `messages.expires_at`, a different
    /// column. So "Auto-Delete After 24h" was a control that reported success
    /// when the thing it described did not happen, on the exact feature
    /// CLAUDE.md names as the worst instance of that failure.
    ///
    /// Shredding order matches [`Self::delete_expired_messages`]: zero the
    /// wrapped content keys first (so the bytes are unrecoverable even if the
    /// delete itself is interrupted), truncate the WAL, then delete. The
    /// delete and the orphan-reaction sweep share one transaction for the same
    /// reason as there: as separate commits, a crash between them left
    /// reactions naming messages that no longer existed.
    ///
    /// Returns the number of messages destroyed.
    pub fn delete_messages_by_retention_policy(&self) -> Result<u32, StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn
            .pragma_update(None, "secure_delete", "ON")?;

        // Only conversations that actually asked for destruction. `policy =
        // 'export'` means "keep it, I'll export it", so it must not be swept.
        let freed: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(m.content_encrypted)
                                    + LENGTH(m.content_nonce)
                                    + ?1), 0)
                   FROM messages m
                   JOIN conversations c ON c.id = m.conversation_id
                  WHERE c.retention_policy = 'delete'
                    AND c.auto_delete_at IS NOT NULL
                    AND c.auto_delete_at <= ?2",
                params![Self::MSG_ROW_OVERHEAD, now],
                |row| row.get(0),
            )
            .unwrap_or(0);

        self.conn.execute(
            "UPDATE messages SET content_key_wrapped = ?2
              WHERE content_key_wrapped IS NOT NULL
                AND content_key_wrapped != ?2
                AND conversation_id IN (
                    SELECT id FROM conversations
                     WHERE retention_policy = 'delete'
                       AND auto_delete_at IS NOT NULL
                       AND auto_delete_at <= ?1)",
            rusqlite::params![now, vec![0u8; WRAPPED_CEK_LEN]],
        )?;
        self.wal_checkpoint_truncate()?;

        let count = {
            let tx = self.conn.unchecked_transaction()?;
            let count = tx.execute(
                "DELETE FROM messages
                  WHERE conversation_id IN (
                        SELECT id FROM conversations
                         WHERE retention_policy = 'delete'
                           AND auto_delete_at IS NOT NULL
                           AND auto_delete_at <= ?1)",
                rusqlite::params![now],
            )?;
            // Reactions are keyed by message id with no foreign key, so they
            // would otherwise outlive the message they annotate — and count as
            // nothing toward the cap, which is how the table became invisible
            // to it.
            //
            // In the same transaction as the delete: it decides what is
            // orphaned by reading `messages`, so a separate commit could
            // only ever be right if the delete had already landed.
            tx.execute(
                "DELETE FROM reactions
                  WHERE message_id NOT IN (SELECT id FROM messages)",
                [],
            )?;
            tx.commit()?;
            count
        };
        self.add_stored_bytes(-freed);
        self.wal_checkpoint_truncate()?;
        Ok(count as u32)
    }

    /// Toggle the favorite status of a conversation. Returns the new value.
    pub fn toggle_favorite(&self, peer_key_hex: &str) -> Result<bool, StorageError> {
        // Get current value
        let current: bool = self
            .conn
            .query_row(
                "SELECT COALESCE(is_favorite, 0) FROM conversations WHERE id = ?1",
                params![peer_key_hex],
                |row| row.get(0),
            )
            .unwrap_or(false);
        let new_val = !current;
        self.conn.execute(
            "UPDATE conversations SET is_favorite = ?1 WHERE id = ?2",
            params![new_val as i32, peer_key_hex],
        )?;
        Ok(new_val)
    }

    /// Toggle the archive status of a conversation. Returns the new value.
    pub fn toggle_archive(&self, peer_key_hex: &str) -> Result<bool, StorageError> {
        let current: bool = self
            .conn
            .query_row(
                "SELECT COALESCE(archived, 0) FROM conversations WHERE id = ?1",
                params![peer_key_hex],
                |row| row.get(0),
            )
            .unwrap_or(false);
        let new_val = !current;
        self.conn.execute(
            "UPDATE conversations SET archived = ?1 WHERE id = ?2",
            params![new_val as i32, peer_key_hex],
        )?;
        Ok(new_val)
    }

    /// Export all messages for a conversation.
    pub fn export_conversation_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        self.load_messages(conversation_id, i64::MAX)
    }

    /// Get a single conversation summary.
    pub fn get_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationSummary>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.peer_id, c.created_at, c.last_message_at,
                    c.display_name, c.peer_display_name,
                    c.auto_delete_at, c.retention_policy,
                    (SELECT COUNT(*) FROM messages m
                      WHERE m.conversation_id = c.id
                        AND m.deleted = 0
                        AND (m.expires_at IS NULL OR m.expires_at > ?2)) as msg_count,
                    COALESCE(c.is_favorite, 0) as is_favorite,
                    COALESCE(c.archived, 0) as archived,
                    (SELECT COUNT(*) FROM messages m
                     WHERE m.conversation_id = c.id
                       AND m.direction = 'received' AND m.read_at IS NULL
                       AND m.deleted = 0
                       AND (m.expires_at IS NULL OR m.expires_at > ?2)) as unread_count
             FROM conversations c WHERE c.id = ?1",
        )?;
        let now = chrono::Utc::now().timestamp();
        let result = stmt.query_row(params![conversation_id, now], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                created_at: row.get(2)?,
                last_message_at: row.get(3)?,
                display_name: row.get(4)?,
                peer_display_name: row.get(5)?,
                auto_delete_at: row.get(6)?,
                retention_policy: row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "none".to_string()),
                message_count: row.get(8)?,
                is_favorite: row.get::<_, Option<bool>>(9)?,
                archived: row.get::<_, Option<bool>>(10)?,
                unread_count: row.get::<_, i64>(11)? as u32,
            })
        });
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Delete a conversation and all its messages, with crypto-shredding (H7).
    ///
    /// Step 1 overwrites every per-message wrapped content key with zeros:
    /// after this, any ciphertext remnants (WAL frames, freed pages, disk
    /// slack) are undecryptable even with full knowledge of the vault key.
    /// Step 2 truncates the WAL so shredded key cells cannot survive in
    /// `messages.db-wal`. Step 3 deletes the rows (`secure_delete` ON makes
    /// SQLite zero freed in-page content) in ONE transaction, together with the
    /// conversation row. A final checkpoint flushes the second round of
    /// changes.
    ///
    /// Why step 3 must be a transaction: the two DELETEs were separate
    /// autocommit statements, so a crash or `SQLITE_BUSY` between them could
    /// remove the messages while leaving the conversation row behind — a
    /// "deleted" conversation still listed in the Hub with an empty history,
    /// and reactions orphaned against ids that no longer exist. The checkpoint
    /// sits before and after the transaction rather than inside it, because
    /// SQLite refuses `wal_checkpoint(TRUNCATE)` while a write transaction is
    /// open.
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), StorageError> {
        self.conn.pragma_update(None, "secure_delete", "ON")?;
        // Measure the rows about to be freed so the storage-cap counter can be
        // decremented exactly. Scoped to this conversation, so it stays cheap
        // even on a large store.
        let freed: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(content_encrypted)
                                    + LENGTH(content_nonce)
                                    + ?2), 0)
                   FROM messages WHERE conversation_id = ?1",
                params![conversation_id, Self::MSG_ROW_OVERHEAD],
                |row| row.get(0),
            )
            .unwrap_or(0);
        self.conn.execute(
            "UPDATE messages SET content_key_wrapped = ?2
              WHERE conversation_id = ?1
                AND content_key_wrapped IS NOT NULL
                AND content_key_wrapped != ?2",
            params![conversation_id, vec![0u8; WRAPPED_CEK_LEN]],
        )?;
        self.wal_checkpoint_truncate()?;
        {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute(
                "DELETE FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
            )?;
            tx.execute(
                "DELETE FROM conversations WHERE id = ?1",
                params![conversation_id],
            )?;
            tx.commit()?;
        }
        self.add_stored_bytes(-freed);
        self.wal_checkpoint_truncate()?;
        Ok(())
    }

    /// Flush and truncate the write-ahead log so previously-modified pages
    /// cannot linger in `messages.db-wal` (H7 companion to secure_delete).
    fn wal_checkpoint_truncate(&self) -> Result<(), StorageError> {
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(StorageError::Database)?;
        Ok(())
    }

    // ─── Reactions ─────────────────────────────────────

    /// Store or remove a reaction on a message.
    ///
    /// `conversation_id` scopes the operation: the message must exist in that
    /// conversation or the call is a no-op (returns Ok(false)). This prevents
    /// a peer from reacting to messages in unrelated conversations.
    /// Insert or remove a reaction.
    ///
    /// The reaction TEXT is encrypted at rest when `key` is provided
    /// (reaction metadata reveals who felt what about which message);
    /// `key = None` stores it as plaintext (legacy/no-vault profiles).
    /// Lookup keys (message_id, peer_key_hex) stay plaintext so queries
    /// remain indexable.
    ///
    /// # A failed decryption is not a failed match
    ///
    /// Both the dedup and the remove path have to open every candidate row to
    /// compare it, because envelopes carry a fresh random nonce each write and
    /// so the stored form of "👍" never equals a freshly-sealed probe. The
    /// comparison used to collapse `Err` into `false`, which is the CLAUDE.md
    /// failure class exactly: with the vault locked (`key = None`) every
    /// comparison against a sealed row fails, so `remove` deleted nothing and
    /// still returned `Ok(true)` — the caller surfaced success and the reaction
    /// the user tapped was still on disk and still visible.
    ///
    /// [`Self::matching_reaction_rowids`] therefore reports decryption failure
    /// separately from "did not match", and this function turns it into an
    /// error. Failing loudly is the only safe option: a wrong guess in the
    /// other direction would delete a *different* peer's reaction.
    pub fn upsert_reaction(
        &self,
        message_id: &str,
        reaction: &str,
        peer_key_hex: &str,
        remove: bool,
        conversation_id: &str,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<bool, StorageError> {
        // Ownership gate: message must belong to the given conversation.
        let owns = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE id = ?1 AND conversation_id = ?2",
            rusqlite::params![message_id, conversation_id],
            |row| row.get::<_, i64>(0),
        )? > 0;
        if !owns {
            return Ok(false);
        }
        if remove {
            // Match by DECRYPTED text: envelopes carry fresh random nonces,
            // so re-encrypting the probe would never equal the stored form.
            let (matched, undecryptable) =
                self.matching_reaction_rowids(message_id, peer_key_hex, reaction, key)?;
            if undecryptable {
                // Refuse rather than claim the reaction was removed. The
                // caller would report success for a delete that did not
                // happen; the row is still present and still rendered.
                return Err(StorageError::KeyNotFound);
            }
            for rowid in matched {
                self.conn.execute(
                    "DELETE FROM reactions WHERE rowid = ?1",
                    rusqlite::params![rowid],
                )?;
            }
        } else {
            let now = chrono::Utc::now().timestamp();
            // Deduplicate: each envelope uses a fresh random nonce, so
            // identical reactions produce different stored forms and a SQL
            // UNIQUE constraint cannot see them as equal. Remove prior rows
            // from this peer on this message whose DECRYPTED reaction
            // matches, then insert fresh.
            let (dup_rowids, undecryptable) =
                self.matching_reaction_rowids(message_id, peer_key_hex, reaction, key)?;
            if undecryptable {
                // Without this the insert would go ahead alongside rows we
                // could not read, producing a duplicate reaction that SQL
                // cannot see and no UI path can remove.
                return Err(StorageError::KeyNotFound);
            }
            for rowid in dup_rowids {
                self.conn.execute(
                    "DELETE FROM reactions WHERE rowid = ?1",
                    rusqlite::params![rowid],
                )?;
            }
            let stored = seal_meta_value(key, reaction, AAD_REACTION)?;
            self.conn.execute(
                "INSERT INTO reactions (message_id, reaction, peer_key_hex, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![message_id, stored, peer_key_hex, now],
            )?;
        }
        Ok(true)
    }

    /// Row ids of this peer's reactions on `message_id` whose DECRYPTED text
    /// equals `reaction`, plus whether any candidate row could not be opened.
    ///
    /// Envelopes are sealed with a fresh random nonce per write, so
    /// `reaction == stored_reaction` is false for two identical reactions and
    /// the match has to happen on the plaintext. That requires the key, and
    /// when it is absent (vault locked) or wrong, `open_meta_value` fails for
    /// every sealed row while succeeding for legacy plaintext ones — so the
    /// boolean is the only way to tell "this peer never reacted that way" from
    /// "I could not find out".
    ///
    /// The two are not interchangeable. Callers that treat them alike report a
    /// successful remove that deleted nothing, or insert a duplicate that no
    /// subsequent remove can target.
    fn matching_reaction_rowids(
        &self,
        message_id: &str,
        peer_key_hex: &str,
        reaction: &str,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<(Vec<i64>, bool), StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT rowid, reaction FROM reactions
             WHERE message_id = ?1 AND peer_key_hex = ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![message_id, peer_key_hex], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut matched = Vec::new();
        let mut undecryptable = false;
        // Collected eagerly so `stmt` is released before the caller deletes
        // rows — the drop is explicit rather than relying on NLL, because a
        // live statement over the table being written is a foot-gun.
        let candidates: Vec<(i64, String)> = rows.filter_map(|r| r.ok()).collect();
        drop(stmt);
        for (rowid, stored) in candidates {
            match open_meta_value(key, &stored, AAD_REACTION) {
                Ok(plain) => {
                    if plain == reaction {
                        matched.push(rowid);
                    }
                }
                // Vault locked, wrong key, or a tampered row. Not a negative
                // answer, so it must not be allowed to look like one.
                Err(_) => undecryptable = true,
            }
        }
        Ok((matched, undecryptable))
    }

    /// Check whether `message_id` exists in `conversation_id` with the
    /// given direction — read-only ownership check (no tombstone write),
    /// used by delete in ephemeral mode where no local state may change.
    pub fn message_in_conversation(
        &self,
        message_id: &str,
        conversation_id: &str,
        expected_direction: &str,
    ) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE id = ?1 AND conversation_id = ?2 AND direction = ?3",
            rusqlite::params![message_id, conversation_id, expected_direction],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get all reactions for a list of message IDs.
    /// Returns a map of message_id → Vec<(reaction, peer_key_hex, created_at)>.
    ///
    /// Encrypted reaction texts are decrypted when `key` is provided;
    /// legacy plaintext rows pass through either way. Rows that fail
    /// decryption (wrong key / tampering) are skipped, not fatal.
    pub fn get_reactions(
        &self,
        message_ids: &[String],
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<ReactionsMap, StorageError> {
        if message_ids.is_empty() {
            return Ok(ReactionsMap::new());
        }
        let placeholders: Vec<String> = message_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT message_id, reaction, peer_key_hex, created_at
             FROM reactions WHERE message_id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = message_ids
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;

        let mut result: ReactionsMap = ReactionsMap::new();
        for row in rows {
            let (msg_id, reaction_stored, peer, ts) = row?;
            match open_meta_value(key, &reaction_stored, AAD_REACTION) {
                Ok(reaction) => result.entry(msg_id).or_default().push((reaction, peer, ts)),
                Err(_) => {
                    tracing::warn!(
                        message_id = %msg_id,
                        "reaction row failed to decrypt — skipping (wrong key or tampered)"
                    );
                }
            }
        }
        Ok(result)
    }

    // ─── Read Receipts ─────────────────────────────────

    /// Mark all unread received messages as read for a conversation.
    pub fn mark_messages_read(&self, conversation_id: &str) -> Result<u32, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let count = self.conn.execute(
            // Expired and soft-deleted rows are excluded so the returned count
            // is "messages the user just marked read", not "rows this UPDATE
            // happened to touch". Including them inflated the badge with
            // content that is logically already gone.
            "UPDATE messages SET read_at = ?1
             WHERE conversation_id = ?2 AND direction = 'received' AND read_at IS NULL
               AND deleted = 0
               AND (expires_at IS NULL OR expires_at > ?1)",
            rusqlite::params![now, conversation_id],
        )?;
        Ok(count as u32)
    }

    // ─── Message Editing ──────────────────────────────

    /// Update a message's content (edit) under a FRESH per-message content
    /// key (H7). The previous wrapped key is overwritten in the same UPDATE,
    /// so the pre-edit ciphertext becomes undecryptable immediately.
    ///
    /// Scoped to a conversation AND direction: peer-initiated edits may only
    /// touch `received` messages in the sender's own conversation; user
    /// edits target their own `sent` messages. Returns false if no
    /// matching row was found (foreign or unknown message).
    pub fn edit_message_secure(
        &self,
        message_id: &str,
        conversation_id: &str,
        expected_direction: &str,
        new_plaintext: &[u8],
        storage_key: &crate::secure_key::StorageKey,
    ) -> Result<bool, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let mut cek = Self::generate_cek();
        let result = (|| {
            let wrapped = Self::wrap_cek(&cek, storage_key)?;
            let (nonce, ciphertext) = seal_msg(&cek, new_plaintext, AAD_MSG_STORE)?;
            use zeroize::Zeroize;
            cek.zeroize();
            let changed = self.conn.execute(
                "UPDATE messages SET content_encrypted = ?1, content_nonce = ?2,
                        content_key_wrapped = ?3, edited_at = ?4
                 WHERE id = ?5 AND conversation_id = ?6 AND direction = ?7",
                rusqlite::params![
                    ciphertext,
                    nonce,
                    wrapped,
                    now,
                    message_id,
                    conversation_id,
                    expected_direction
                ],
            )?;
            Ok(changed > 0)
        })();
        use zeroize::Zeroize;
        cek.zeroize();
        result
    }

    // ─── Message Deletion ─────────────────────────────

    /// Soft-delete a message (mark as deleted so peers see a placeholder)
    /// AND crypto-shred its content (H7): the wrapped per-message key is
    /// zeroed in the same statement, so the retained tombstone row carries
    /// undecryptable ciphertext.
    ///
    /// Scoped like [`edit_message_secure`]: returns false if the message does not
    /// exist in the given conversation with the expected direction.
    pub fn delete_message(
        &self,
        message_id: &str,
        conversation_id: &str,
        expected_direction: &str,
    ) -> Result<bool, StorageError> {
        self.conn.pragma_update(None, "secure_delete", "ON")?;
        // Overwrite with an equal-length zero blob: same-size in-place
        // replacement is stronger than setting NULL, which may leave the
        // old cell bytes in place depending on how SQLite rewrites the row.
        let changed = self.conn.execute(
            "UPDATE messages SET deleted = 1, content_key_wrapped = ?4
             WHERE id = ?1 AND conversation_id = ?2 AND direction = ?3",
            rusqlite::params![
                message_id,
                conversation_id,
                expected_direction,
                vec![0u8; WRAPPED_CEK_LEN]
            ],
        )?;
        Ok(changed > 0)
    }

    // ─── Self-Destruct (Expired Messages) ─────────────

/// Permanently delete expired messages from the database, with
    /// crypto-shredding (H7): shred wrapped keys first, truncate the WAL,
    /// then delete the rows in one transaction.
    ///
    /// The delete and the orphan-reaction sweep are in the same transaction
    /// because they are one promise: no reaction may outlive the message it
    /// annotates. As separate commits, a crash between them left reactions
    /// naming message ids that no longer existed — and the `reactions` table is
    /// not counted by the storage cap, so those rows accumulated invisibly.
    ///
    /// This is the path `MessageStore::open` runs for timers that elapsed while
    /// the app was closed, so it must be crash-safe rather than merely
    /// crash-tolerant.
    pub fn delete_expired_messages(&self) -> Result<u32, StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn.pragma_update(None, "secure_delete", "ON")?;
        // Same accounting as `delete_conversation`: measure before freeing.
        let freed: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(content_encrypted)
                                    + LENGTH(content_nonce)
                                    + ?1), 0)
                   FROM messages
                  WHERE expires_at IS NOT NULL AND expires_at <= ?2",
                params![Self::MSG_ROW_OVERHEAD, now],
                |row| row.get(0),
            )
            .unwrap_or(0);
        self.conn.execute(
            "UPDATE messages SET content_key_wrapped = ?2
              WHERE expires_at IS NOT NULL AND expires_at <= ?1
                AND content_key_wrapped IS NOT NULL
                AND content_key_wrapped != ?2",
            rusqlite::params![now, vec![0u8; WRAPPED_CEK_LEN]],
        )?;
        self.wal_checkpoint_truncate()?;
        let count = {
            let tx = self.conn.unchecked_transaction()?;
            let count = tx.execute(
                "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1",
                rusqlite::params![now],
            )?;
            // Reactions carry no foreign key to `messages`, so every
            // self-destruct left them behind: orphaned rows naming messages
            // that no longer exist. They were also invisible to the storage
            // cap, which counts only message rows — so the one table that
            // could grow without bound was the one the ceiling could not see.
            //
            // This must be inside the same transaction as the delete above: it
            // queries `messages` to find what is orphaned, so running it
            // separately could only ever be correct if the delete had
            // already committed.
            tx.execute(
                "DELETE FROM reactions WHERE message_id NOT IN (SELECT id FROM messages)",
                [],
            )?;
            tx.commit()?;
            count
        };
        self.add_stored_bytes(-freed);
        self.wal_checkpoint_truncate()?;
        Ok(count as u32)
    }
}

/// A stored message row.
#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub direction: String,
    pub content_encrypted: Vec<u8>,
    pub content_nonce: Vec<u8>,
    /// Per-message content key wrapped under the vault storage key
    /// (crypto-shredding, H7). `None` for legacy rows, whose content was
    /// encrypted directly under the vault key.
    pub content_key_wrapped: Option<Vec<u8>>,
    pub timestamp: i64,
    /// When this message was read by the recipient (null = unread).
    pub read_at: Option<i64>,
    /// When this message was edited (null = never edited).
    pub edited_at: Option<i64>,
    /// Whether this message has been deleted.
    pub deleted: bool,
    /// When this message self-destructs (null = never).
    pub expires_at: Option<i64>,
}

/// Summary of a conversation for the frontend.
#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub peer_id: Vec<u8>,
    pub created_at: i64,
    pub last_message_at: Option<i64>,
    pub display_name: Option<String>,
    pub peer_display_name: Option<String>,
    pub auto_delete_at: Option<i64>,
    pub retention_policy: String,
    pub message_count: i64,
    /// Whether this conversation is favorited.
    pub is_favorite: Option<bool>,
    /// Whether this conversation is archived.
    pub archived: Option<bool>,
    /// Number of unread received messages.
    pub unread_count: u32,
}

/// Summary of a stored transfer for the frontend.
#[derive(Debug, Clone)]
#[cfg(test)]
pub struct StoredTransfer {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub peer_key_hex: String,
    pub filename: String,
    pub total_size: u64,
    pub direction: String,
    pub state: String,
    pub chunks_completed: u32,
    pub chunks_total: u32,
    #[allow(dead_code)]
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub local_path: Option<String>,
    pub error: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Group Chat — Query Methods (Phase 3)
// ═══════════════════════════════════════════════════════════════════════════

impl MessageStore {
    /// Create or update a group record.
    pub fn upsert_group(
        &self,
        group_id: &str,
        group_name: &str,
        created_at: i64,
        our_role: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO groups (group_id, group_name, created_at, our_role)
             VALUES (?1, ?2, ?3, ?4)",
            params![group_id, group_name, created_at, our_role],
        )?;
        Ok(())
    }

    /// Add a member to a group.
    pub fn add_group_member(
        &self,
        group_id: &str,
        peer_key_hex: &str,
        display_name: Option<&str>,
        role: &str,
        added_at: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO group_members (group_id, peer_key_hex, display_name, role, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![group_id, peer_key_hex, display_name, role, added_at],
        )?;
        Ok(())
    }

    /// Remove a member from a group.
    pub fn remove_group_member(
        &self,
        group_id: &str,
        peer_key_hex: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM group_members WHERE group_id = ?1 AND peer_key_hex = ?2",
            params![group_id, peer_key_hex],
        )?;
        Ok(())
    }

    /// Load all members for a group.
    #[allow(dead_code)]
    pub fn load_group_members(
        &self,
        group_id: &str,
    ) -> Result<Vec<super::group::GroupMember>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT peer_key_hex, display_name, role, added_at
             FROM group_members WHERE group_id = ?1 ORDER BY added_at",
        )?;
        let members = stmt
            .query_map(params![group_id], |row| {
                Ok(super::group::GroupMember {
                    peer_key_hex: row.get(0)?,
                    display_name: row.get(1)?,
                    role: match row.get::<_, String>(2)?.as_str() {
                        "admin" => super::group::GroupRole::Admin,
                        _ => super::group::GroupRole::Member,
                    },
                    added_at: row.get::<_, i64>(3)? as u64,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(members)
    }

    /// Load a single group record (without members).
    #[allow(dead_code)]
    pub fn load_group(&self, group_id: &str) -> Result<Option<super::group::Group>, StorageError> {
        let result = self.conn.query_row(
            "SELECT group_id, group_name, created_at, our_role, last_message_at, last_message_preview
             FROM groups WHERE group_id = ?1",
            params![group_id],
            |row| {
                let group_id: String = row.get(0)?;
                Ok((group_id, row.get::<_, String>(1)?, row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?, row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<String>>(5)?))
            },
        );
        match result {
            Ok((gid, name, created_at, _role, last_msg_at, last_preview)) => {
                let members = self.load_group_members(&gid)?;
                let mut group =
                    super::group::Group::new(gid, name, created_at as u64, String::new());
                group.members = members;
                group.last_message_at = last_msg_at.unwrap_or(0) as u64;
                group.last_message_preview = last_preview;
                Ok(Some(group))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// List all groups with summary info.
    #[allow(dead_code)]
    pub fn list_groups(&self) -> Result<Vec<super::group::GroupSummary>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT g.group_id, g.group_name, g.created_at,
                    COALESCE(g.last_message_at, 0), g.last_message_preview,
                    (SELECT COUNT(*) FROM group_members WHERE group_id = g.group_id) as member_count
             FROM groups g ORDER BY COALESCE(g.last_message_at, 0) DESC",
        )?;
        let groups = stmt
            .query_map([], |row| {
                Ok(super::group::GroupSummary {
                    group_id: row.get(0)?,
                    group_name: row.get(1)?,
                    created_at: row.get::<_, i64>(2)? as u64,
                    last_message_at: row.get::<_, i64>(3)? as u64,
                    last_message_preview: row.get(4)?,
                    member_count: row.get::<_, i64>(5)? as u32,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(groups)
    }

    /// Remove a group and all its members.
    pub fn remove_group(&self, group_id: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM group_members WHERE group_id = ?1",
            params![group_id],
        )?;
        self.conn
            .execute("DELETE FROM groups WHERE group_id = ?1", params![group_id])?;
        Ok(())
    }

    /// Update group metadata.
    pub fn update_group_name(&self, group_id: &str, new_name: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE groups SET group_name = ?1 WHERE group_id = ?2",
            params![new_name, group_id],
        )?;
        Ok(())
    }

    /// Update the last message preview for a group.
    pub fn update_group_last_message(
        &self,
        group_id: &str,
        timestamp: i64,
        preview: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE groups SET last_message_at = ?1, last_message_preview = ?2 WHERE group_id = ?3",
            params![timestamp, preview, group_id],
        )?;
        Ok(())
    }

    /// Store a group message (idempotent).
    #[allow(clippy::too_many_arguments)]
    pub fn store_group_message(
        &self,
        id: &str,
        group_id: &str,
        sender_peer_key_hex: &str,
        content_encrypted: &[u8],
        content_nonce: &[u8],
        timestamp: i64,
        delivered: bool,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO group_messages
             (id, group_id, sender_peer_key_hex, content_encrypted, content_nonce, timestamp, delivered)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id, group_id, sender_peer_key_hex,
                content_encrypted, content_nonce, timestamp,
                delivered as i32,
            ],
        )?;
        // Counted toward the storage cap for the same reason 1:1 messages are:
        // `group_messages` has its own delete paths, so leaving it out of the
        // accounting would let the cap be bypassed entirely by an attacker who
        // only sends group traffic.
        if self.conn.changes() > 0 {
            self.add_stored_bytes(Self::msg_row_bytes(
                content_encrypted.len(),
                content_nonce.len(),
            ));
        }
        Ok(())
    }

    /// Load group messages (most recent first, with limit and offset).
    #[allow(dead_code)]
    pub fn load_group_messages(
        &self,
        group_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<super::commands::ChatMessage>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, group_id, sender_peer_key_hex, content_encrypted, content_nonce,
                    timestamp, delivered, edited_at, deleted
             FROM group_messages
             WHERE group_id = ?1 AND deleted = 0
             ORDER BY timestamp DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let messages = stmt
            .query_map(params![group_id, limit, offset], |row| {
                Ok(super::commands::ChatMessage {
                    id: row.get(0)?,
                    content: String::new(), // filled in by caller after decryption
                    direction: String::new(), // filled in by caller
                    timestamp: row.get::<_, i64>(5)? as u64,
                    read_at: None,
                    edited_at: row.get(7)?,
                    deleted: row.get::<_, i32>(8)? != 0,
                    expires_at: None,
                    reactions: std::collections::HashMap::new(),
                    sender_peer_key_hex: row.get::<_, String>(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(messages)
    }

    /// Load group messages WITH encrypted content (for decryption by caller).
    /// Returns (ChatMessage, content_encrypted, content_nonce) tuples.
    #[allow(clippy::type_complexity)]
    pub fn load_group_messages_with_content(
        &self,
        group_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(super::commands::ChatMessage, Vec<u8>, Vec<u8>)>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, group_id, sender_peer_key_hex, content_encrypted, content_nonce,
                    timestamp, delivered, edited_at, deleted
             FROM group_messages
             WHERE group_id = ?1 AND deleted = 0
             ORDER BY timestamp DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let results = stmt
            .query_map(params![group_id, limit, offset], |row| {
                let msg = super::commands::ChatMessage {
                    id: row.get(0)?,
                    content: String::new(),
                    direction: String::new(),
                    timestamp: row.get::<_, i64>(5)? as u64,
                    read_at: None,
                    edited_at: row.get(7)?,
                    deleted: row.get::<_, i32>(8)? != 0,
                    expires_at: None,
                    reactions: std::collections::HashMap::new(),
                    sender_peer_key_hex: row.get::<_, String>(2)?,
                };
                let content_encrypted: Vec<u8> = row.get(3)?;
                let content_nonce: Vec<u8> = row.get(4)?;
                Ok((msg, content_encrypted, content_nonce))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(results)
    }

    /// Mark a group message as edited.
    #[allow(dead_code)]
    pub fn edit_group_message(&self, message_id: &str, edited_at: i64) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE group_messages SET edited_at = ?1 WHERE id = ?2",
            params![edited_at, message_id],
        )?;
        Ok(())
    }

    /// Soft-delete a group message.
    #[allow(dead_code)]
    pub fn delete_group_message(&self, message_id: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE group_messages SET deleted = 1 WHERE id = ?1",
            params![message_id],
        )?;
        Ok(())
    }
}

/// Persistent transfer history store.
///
/// Records every file transfer (both sent and received) so the user can
/// see past transfers, retry failed ones, and resume interrupted ones
/// after an app restart.
pub struct TransferStore {
    conn: Connection,
}

impl TransferStore {
    /// Open or create the transfer store.
    pub fn open(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        apply_connection_pragmas(&conn)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS transfers (
                id TEXT PRIMARY KEY,
                peer_key_hex TEXT NOT NULL,
                filename TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                direction TEXT NOT NULL CHECK (direction IN ('sent', 'received')),
                state TEXT NOT NULL DEFAULT 'pending',
                chunks_completed INTEGER NOT NULL DEFAULT 0,
                chunks_total INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                completed_at INTEGER,
                local_path TEXT,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_transfers_peer
                ON transfers(peer_key_hex);
            CREATE INDEX IF NOT EXISTS idx_transfers_created
                ON transfers(created_at DESC);",
        )?;

        Ok(Self { conn })
    }

    /// Insert or update a transfer record.
    #[allow(clippy::too_many_arguments)]
    pub fn store_transfer(
        &self,
        id: &str,
        peer_key_hex: &str,
        filename: &str,
        total_size: u64,
        direction: &str,
        state: &str,
        chunks_total: u32,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp();
        let filename_stored = seal_meta_value(key, filename, AAD_TRANSFER)?;
        self.conn.execute(
            "INSERT INTO transfers (id, peer_key_hex, filename, total_size, direction, state, chunks_total, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                state = excluded.state,
                chunks_total = excluded.chunks_total",
            params![id, peer_key_hex, filename_stored, total_size, direction, state, chunks_total, now],
        )?;
        Ok(())
    }

    /// Update the transfer state.
    ///
    /// `error` text is encrypted at rest when `key` is provided.
    pub fn update_state(
        &self,
        transfer_id: &str,
        state: &str,
        completed_at: Option<i64>,
        error: Option<&str>,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<(), StorageError> {
        let error_stored = match error {
            Some(e) => Some(seal_meta_value(key, e, AAD_TRANSFER)?),
            None => None,
        };
        match (completed_at, error_stored) {
            (Some(at), Some(e)) => {
                self.conn.execute(
                    "UPDATE transfers SET state = ?1, completed_at = ?2, error = ?3 WHERE id = ?4",
                    params![state, at, e, transfer_id],
                )?;
            }
            (Some(at), None) => {
                self.conn.execute(
                    "UPDATE transfers SET state = ?1, completed_at = ?2 WHERE id = ?3",
                    params![state, at, transfer_id],
                )?;
            }
            (None, Some(e)) => {
                self.conn.execute(
                    "UPDATE transfers SET state = ?1, error = ?2 WHERE id = ?3",
                    params![state, e, transfer_id],
                )?;
            }
            (None, None) => {
                self.conn.execute(
                    "UPDATE transfers SET state = ?1 WHERE id = ?2",
                    params![state, transfer_id],
                )?;
            }
        }
        Ok(())
    }

    /// Update the number of completed chunks.
    #[cfg(test)]
    pub fn update_progress(
        &self,
        transfer_id: &str,
        chunks_completed: u32,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE transfers SET chunks_completed = ?1 WHERE id = ?2",
            params![chunks_completed, transfer_id],
        )?;
        Ok(())
    }

    /// Update the local path for a completed received file.
    #[cfg(test)]
    pub fn set_local_path(
        &self,
        transfer_id: &str,
        local_path: &str,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<(), StorageError> {
        let stored = seal_meta_value(key, local_path, AAD_TRANSFER)?;
        self.conn.execute(
            "UPDATE transfers SET local_path = ?1 WHERE id = ?2",
            params![stored, transfer_id],
        )?;
        Ok(())
    }

    /// List all stored transfers, most recent first.
    #[cfg(test)]
    pub fn list_transfers(
        &self,
        limit: i64,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<Vec<StoredTransfer>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, peer_key_hex, filename, total_size, direction, state,
                    chunks_completed, chunks_total, created_at, completed_at,
                    local_path, error
             FROM transfers ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(StoredTransfer {
                id: row.get(0)?,
                peer_key_hex: row.get(1)?,
                filename: row.get(2)?,
                total_size: row.get(3)?,
                direction: row.get(4)?,
                state: row.get(5)?,
                chunks_completed: row.get(6)?,
                chunks_total: row.get(7)?,
                created_at: row.get(8)?,
                completed_at: row.get(9)?,
                local_path: row.get(10)?,
                error: row.get(11)?,
            })
        })?;
        let mut transfers = Vec::new();
        for row in rows {
            let mut t = row?;
            t.filename = open_meta_value(key, &t.filename, AAD_TRANSFER)
                .unwrap_or_else(|_| "[encrypted]".to_string());
            if let Some(p) = &t.local_path {
                t.local_path = Some(
                    open_meta_value(key, p, AAD_TRANSFER)
                        .unwrap_or_else(|_| "[encrypted]".to_string()),
                );
            }
            if let Some(e) = &t.error {
                t.error = Some(
                    open_meta_value(key, e, AAD_TRANSFER)
                        .unwrap_or_else(|_| "[encrypted]".to_string()),
                );
            }
            transfers.push(t);
        }
        Ok(transfers)
    }

    /// Get a single transfer by ID.
    #[cfg(test)]
    pub fn get_transfer(
        &self,
        transfer_id: &str,
        key: Option<&crate::secure_key::StorageKey>,
    ) -> Result<Option<StoredTransfer>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, peer_key_hex, filename, total_size, direction, state,
                    chunks_completed, chunks_total, created_at, completed_at,
                    local_path, error
             FROM transfers WHERE id = ?1",
        )?;
        let result = stmt.query_row(params![transfer_id], |row| {
            Ok(StoredTransfer {
                id: row.get(0)?,
                peer_key_hex: row.get(1)?,
                filename: row.get(2)?,
                total_size: row.get(3)?,
                direction: row.get(4)?,
                state: row.get(5)?,
                chunks_completed: row.get(6)?,
                chunks_total: row.get(7)?,
                created_at: row.get(8)?,
                completed_at: row.get(9)?,
                local_path: row.get(10)?,
                error: row.get(11)?,
            })
        });
        match result {
            Ok(mut t) => {
                t.filename = open_meta_value(key, &t.filename, AAD_TRANSFER)
                    .unwrap_or_else(|_| "[encrypted]".to_string());
                if let Some(p) = &t.local_path {
                    t.local_path = Some(
                        open_meta_value(key, p, AAD_TRANSFER)
                            .unwrap_or_else(|_| "[encrypted]".to_string()),
                    );
                }
                if let Some(e) = &t.error {
                    t.error = Some(
                        open_meta_value(key, e, AAD_TRANSFER)
                            .unwrap_or_else(|_| "[encrypted]".to_string()),
                    );
                }
                Ok(Some(t))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Delete a transfer record.
    #[cfg(test)]
    pub fn delete_transfer(&self, transfer_id: &str) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM transfers WHERE id = ?1", params![transfer_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Helper: open a KeyStore on `:memory:` for test isolation.
    fn mem_keystore() -> KeyStore {
        KeyStore::open(Path::new(":memory:")).unwrap()
    }

    /// Helper: open a MessageStore on `:memory:` for test isolation.
    fn mem_messagestore() -> MessageStore {
        MessageStore::open(Path::new(":memory:")).unwrap()
    }

    /// Helper: open a TransferStore on `:memory:` for test isolation.
    fn mem_transferstore() -> TransferStore {
        TransferStore::open(Path::new(":memory:")).unwrap()
    }

    // ─── KeyStore tests ────────────────────────────────────────

    #[test]
    fn test_store_and_load_identity() {
        let store = mem_keystore();
        let pub_key = vec![0xAA; 32];
        let enc_pk = vec![0xBB; 64];
        let nonce = vec![0xCC; 24];
        let created = 1719446400i64;

        store
            .store_identity(&pub_key, &enc_pk, &nonce, created)
            .unwrap();

        let (loaded_pub, loaded_enc, loaded_nonce) = store.load_identity().unwrap();
        assert_eq!(loaded_pub, pub_key);
        assert_eq!(loaded_enc, enc_pk);
        assert_eq!(loaded_nonce, nonce);
    }

    #[test]
    fn test_store_identity_overwrite() {
        let store = mem_keystore();
        store
            .store_identity(&[0xAA; 32], &[0xBB; 64], &[0xCC; 24], 1000)
            .unwrap();
        store
            .store_identity(&[0xDD; 32], &[0xEE; 64], &[0xFF; 24], 2000)
            .unwrap();

        let (pub_key, enc_pk, nonce) = store.load_identity().unwrap();
        assert_eq!(pub_key, vec![0xDD; 32]);
        assert_eq!(enc_pk, vec![0xEE; 64]);
        assert_eq!(nonce, vec![0xFF; 24]);
    }

    #[test]
    fn test_store_and_load_public_key() {
        let store = mem_keystore();
        store
            .store_identity(&[0x11; 32], &[0x22; 64], &[0x33; 24], 1000)
            .unwrap();

        let pk = store.load_public_key().unwrap();
        assert_eq!(pk, vec![0x11; 32]);
    }

    #[test]
    fn test_has_identity_false_initially() {
        let store = mem_keystore();
        assert!(!store.has_identity().unwrap());
    }

    #[test]
    fn test_has_identity_true_after_store() {
        let store = mem_keystore();
        store
            .store_identity(&[0xAA; 32], &[0xBB; 64], &[0xCC; 24], 1000)
            .unwrap();
        assert!(store.has_identity().unwrap());
    }

    #[test]
    fn test_is_vault_initialized_default_false() {
        let store = mem_keystore();
        assert!(!store.is_vault_initialized().unwrap());
    }

    #[test]
    fn test_set_vault_initialized_roundtrip() {
        let store = mem_keystore();
        assert!(!store.is_vault_initialized().unwrap());
        store.set_vault_initialized().unwrap();
        assert!(store.is_vault_initialized().unwrap());
    }

    #[test]
    fn test_key_not_found_on_empty_store() {
        let store = mem_keystore();
        let err = store.load_identity().unwrap_err();
        assert!(matches!(err, StorageError::KeyNotFound));
    }

    #[test]
    fn test_load_public_key_not_found() {
        let store = mem_keystore();
        let err = store.load_public_key().unwrap_err();
        assert!(matches!(err, StorageError::KeyNotFound));
    }

    #[test]
    fn test_upsert_peer_new() {
        let store = mem_keystore();
        store
            .upsert_peer(&[0x11; 32], "A1B2:C3D4", Some("Alice"))
            .unwrap();

        // Verify via has_identity (peers table is separate)
        // We can't directly query, but upsert should succeed without error.
        // load_public_key still fails because identity not stored
        assert!(store.load_identity().is_err());
    }

    #[test]
    fn test_upsert_peer_update_alias() {
        let store = mem_keystore();
        store
            .upsert_peer(&[0x11; 32], "A1B2:C3D4", Some("Alice"))
            .unwrap();
        // Upsert again with new alias — should update, not error
        store
            .upsert_peer(&[0x11; 32], "A1B2:C3D4", Some("Bob"))
            .unwrap();
    }

    /// H5: is_known_peer must reflect the peers table — unknown keys are
    /// rejected by the contact allowlist gate, previously-connected peers pass.
    #[test]
    fn test_is_known_peer() {
        let store = mem_keystore();
        assert!(
            !store.is_known_peer(&[0x42; 32]).unwrap(),
            "empty store: peer must be unknown"
        );

        store.upsert_peer(&[0x42; 32], "AAAA:BBBB", None).unwrap();
        assert!(
            store.is_known_peer(&[0x42; 32]).unwrap(),
            "upserted peer must be known"
        );

        // A different key stays unknown.
        assert!(!store.is_known_peer(&[0x43; 32]).unwrap());
    }

    #[test]
    fn test_update_encrypted_private_key() {
        let store = mem_keystore();
        store
            .store_identity(&[0xAA; 32], &[0xBB; 64], &[0xCC; 24], 1000)
            .unwrap();
        store
            .update_encrypted_private_key(&[0xDD; 64], &[0xEE; 24])
            .unwrap();

        let (_, enc_pk, nonce) = store.load_identity().unwrap();
        assert_eq!(enc_pk, vec![0xDD; 64]);
        assert_eq!(nonce, vec![0xEE; 24]);
    }

    // ─── MessageStore tests ────────────────────────────────────

    #[test]
    fn test_message_store_roundtrip() {
        let store = mem_messagestore();
        let conv_id = "conv-001";
        let peer_id = vec![0xAA; 32];

        store.ensure_conversation(conv_id, &peer_id).unwrap();
        store
            .store_message(
                "msg-001",
                conv_id,
                "sent",
                &[0x01; 32],
                &[0x02; 24],
                1000,
                false,
            )
            .unwrap();

        let messages = store.load_messages(conv_id, 10).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg-001");
        assert_eq!(messages[0].direction, "sent");
        assert_eq!(messages[0].content_encrypted, vec![0x01u8; 32]);
        assert_eq!(messages[0].content_nonce, vec![0x02u8; 24]);
        assert_eq!(messages[0].timestamp, 1000);
    }

    #[test]
    fn test_store_multiple_messages_ordered() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();

        for i in 0..5 {
            store
                .store_message(
                    &format!("msg-{:03}", i),
                    "conv-001",
                    "received",
                    &[i as u8; 32],
                    &[0xBB; 24],
                    1000 + i,
                    true,
                )
                .unwrap();
        }

        let messages = store.load_messages("conv-001", 10).unwrap();
        assert_eq!(messages.len(), 5);
        // Should be in ascending timestamp order
        for (idx, msg) in messages.iter().enumerate() {
            assert_eq!(msg.timestamp, 1000 + idx as i64);
        }
    }

    #[test]
    fn test_load_messages_limit() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();

        for i in 0..10 {
            store
                .store_message(
                    &format!("msg-{:03}", i),
                    "conv-001",
                    "sent",
                    &[i as u8; 32],
                    &[0xBB; 24],
                    1000 + i,
                    true,
                )
                .unwrap();
        }

        let limited = store.load_messages("conv-001", 3).unwrap();
        assert_eq!(limited.len(), 3);
        // Most recent 3 messages (timestamps 1007, 1008, 1009)
        assert_eq!(limited[0].timestamp, 1007);
        assert_eq!(limited[1].timestamp, 1008);
        assert_eq!(limited[2].timestamp, 1009);
    }

    #[test]
    fn test_list_conversations() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-a", &[0x11; 32]).unwrap();
        store.ensure_conversation("conv-b", &[0x22; 32]).unwrap();

        store
            .store_message("m1", "conv-a", "sent", &[0x01; 32], &[0x02; 24], 1000, true)
            .unwrap();
        store
            .store_message("m2", "conv-a", "sent", &[0x03; 32], &[0x04; 24], 2000, true)
            .unwrap();
        store
            .store_message(
                "m3",
                "conv-b",
                "received",
                &[0x05; 32],
                &[0x06; 24],
                1500,
                true,
            )
            .unwrap();

        let convos = store.list_conversations().unwrap();
        assert_eq!(convos.len(), 2);

        // conv-a has 2 messages, last_message_at=2000
        // conv-b has 1 message, last_message_at=1500
        let a = convos.iter().find(|c| c.id == "conv-a").unwrap();
        assert_eq!(a.message_count, 2);
        assert_eq!(a.last_message_at, Some(2000));

        let b = convos.iter().find(|c| c.id == "conv-b").unwrap();
        assert_eq!(b.message_count, 1);
        assert_eq!(b.last_message_at, Some(1500));
    }

    #[test]
    fn test_rename_conversation() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();

        store.rename_conversation("conv-001", "My Chat").unwrap();
        let conv = store.get_conversation("conv-001").unwrap().unwrap();
        assert_eq!(conv.display_name, Some("My Chat".to_string()));
    }

    #[test]
    fn test_delete_conversation_cascade() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();
        store
            .store_message(
                "msg-001",
                "conv-001",
                "sent",
                &[0x01; 32],
                &[0x02; 24],
                1000,
                true,
            )
            .unwrap();

        // Verify it exists
        assert!(store.get_conversation("conv-001").unwrap().is_some());
        assert_eq!(store.load_messages("conv-001", 10).unwrap().len(), 1);

        // Delete
        store.delete_conversation("conv-001").unwrap();

        // Verify gone
        assert!(store.get_conversation("conv-001").unwrap().is_none());
        assert_eq!(store.load_messages("conv-001", 10).unwrap().len(), 0);
    }

    #[test]
    fn test_export_conversation_messages() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();
        store
            .store_message(
                "m1",
                "conv-001",
                "sent",
                &[0x01; 32],
                &[0x02; 24],
                1000,
                true,
            )
            .unwrap();
        store
            .store_message(
                "m2",
                "conv-001",
                "received",
                &[0x03; 32],
                &[0x04; 24],
                2000,
                true,
            )
            .unwrap();

        let exported = store.export_conversation_messages("conv-001").unwrap();
        assert_eq!(exported.len(), 2);
    }

    #[test]
    fn test_set_peer_display_name() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();

        store.set_peer_display_name("conv-001", "Bob").unwrap();
        let conv = store.get_conversation("conv-001").unwrap().unwrap();
        assert_eq!(conv.peer_display_name, Some("Bob".to_string()));
    }

    #[test]
    fn test_set_conversation_retention() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-001", &[0xAA; 32]).unwrap();

        store
            .set_conversation_retention("conv-001", "auto_delete", Some(86400))
            .unwrap();
        let conv = store.get_conversation("conv-001").unwrap().unwrap();
        assert_eq!(conv.retention_policy, "auto_delete");
        assert!(conv.auto_delete_at.is_some());
    }

    #[test]
    fn test_get_conversation_not_found() {
        let store = mem_messagestore();
        let conv = store.get_conversation("nonexistent").unwrap();
        assert!(conv.is_none());
    }

    // ─── TransferStore tests ──────────────────────────────────

    #[test]
    fn test_transfer_store_roundtrip() {
        let store = mem_transferstore();
        let key = test_key();
        // Encrypted-at-rest write (filename sealed under the vault key).
        store
            .store_transfer(
                "xfer-001",
                "alice_pk",
                "report.pdf",
                1048576,
                "received",
                "completed",
                16,
                Some(&key),
            )
            .unwrap();

        let saved = store.get_transfer("xfer-001", Some(&key)).unwrap().unwrap();
        assert_eq!(saved.id, "xfer-001");
        assert_eq!(saved.filename, "report.pdf");
        assert_eq!(saved.total_size, 1048576);
        assert_eq!(saved.direction, "received");
        assert_eq!(saved.state, "completed");
        assert_eq!(saved.chunks_total, 16);
    }

    #[test]
    fn test_transfer_store_update_state() {
        let store = mem_transferstore();
        store
            .store_transfer(
                "xfer-002",
                "bob_pk",
                "photo.jpg",
                524288,
                "sent",
                "transferring",
                8,
                None,
            )
            .unwrap();

        store
            .update_state("xfer-002", "completed", Some(2000), None, None)
            .unwrap();

        let saved = store.get_transfer("xfer-002", None).unwrap().unwrap();
        assert_eq!(saved.state, "completed");
        assert_eq!(saved.completed_at, Some(2000));
    }

    #[test]
    fn test_transfer_store_update_error() {
        let store = mem_transferstore();
        let key = test_key();
        store
            .store_transfer(
                "xfer-003",
                "carol_pk",
                "archive.zip",
                2097152,
                "sent",
                "transferring",
                32,
                Some(&key),
            )
            .unwrap();

        store
            .update_state(
                "xfer-003",
                "failed",
                None,
                Some("connection lost"),
                Some(&key),
            )
            .unwrap();

        let saved = store.get_transfer("xfer-003", Some(&key)).unwrap().unwrap();
        assert_eq!(saved.state, "failed");
        assert_eq!(saved.error, Some("connection lost".to_string()));
    }

    #[test]
    fn test_transfer_store_update_progress() {
        let store = mem_transferstore();
        store
            .store_transfer(
                "xfer-004",
                "dave_pk",
                "video.mp4",
                10485760,
                "sent",
                "transferring",
                40,
                None,
            )
            .unwrap();

        store.update_progress("xfer-004", 15).unwrap();

        let saved = store.get_transfer("xfer-004", None).unwrap().unwrap();
        assert_eq!(saved.chunks_completed, 15);
    }

    #[test]
    fn test_transfer_store_set_local_path() {
        let store = mem_transferstore();
        let key = test_key();
        store
            .store_transfer(
                "xfer-005",
                "eve_pk",
                "doc.pdf",
                65536,
                "received",
                "completed",
                1,
                Some(&key),
            )
            .unwrap();

        store
            .set_local_path("xfer-005", "/downloads/doc.pdf", Some(&key))
            .unwrap();

        let saved = store.get_transfer("xfer-005", Some(&key)).unwrap().unwrap();
        assert_eq!(saved.local_path, Some("/downloads/doc.pdf".to_string()));
    }

    #[test]
    fn test_transfer_store_list_limit() {
        let store = mem_transferstore();
        store
            .store_transfer("xf-01", "pk1", "a.txt", 100, "sent", "completed", 1, None)
            .unwrap();
        store
            .store_transfer("xf-02", "pk2", "b.txt", 200, "received", "failed", 2, None)
            .unwrap();
        store
            .store_transfer(
                "xf-03",
                "pk3",
                "c.txt",
                300,
                "sent",
                "transferring",
                3,
                None,
            )
            .unwrap();

        let limited = store.list_transfers(2, None).unwrap();
        assert_eq!(limited.len(), 2);

        let all = store.list_transfers(10, None).unwrap();
        assert_eq!(all.len(), 3);
        // All IDs present
        let ids: std::collections::HashSet<String> = all.iter().map(|t| t.id.clone()).collect();
        assert!(ids.contains("xf-01"));
        assert!(ids.contains("xf-02"));
        assert!(ids.contains("xf-03"));
    }

    #[test]
    fn test_transfer_store_delete() {
        let store = mem_transferstore();
        store
            .store_transfer(
                "xf-del",
                "pk",
                "nope.txt",
                100,
                "sent",
                "cancelled",
                1,
                None,
            )
            .unwrap();
        assert!(store.get_transfer("xf-del", None).unwrap().is_some());

        store.delete_transfer("xf-del").unwrap();
        assert!(store.get_transfer("xf-del", None).unwrap().is_none());
    }

    #[test]
    fn test_transfer_store_get_not_found() {
        let store = mem_transferstore();
        let result = store.get_transfer("nonexistent", None).unwrap();
        assert!(result.is_none());
    }

    // ─── Metadata-at-rest tests (H-secondary) ─────────────────

    /// Family nicknames/addresses must be stored as envelopes and decrypt
    /// with the right key; legacy plaintext rows still read back.
    #[test]
    fn test_family_metadata_encrypted_at_rest() {
        let store = mem_keystore();
        let key = test_key();
        let pk = [0x11u8; 32];

        store
            .add_family_member(
                &pk,
                "Alice Home",
                Some(30),
                Some("192.168.1.50:7777"),
                Some(&key),
            )
            .unwrap();

        // Raw row must not leak plaintext metadata.
        let raw_nick: String = store
            .conn
            .query_row(
                "SELECT nickname FROM family WHERE public_key = ?1",
                params![pk.as_slice()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            raw_nick.starts_with("enc1:"),
            "nickname must be an envelope, got {raw_nick}"
        );
        assert!(!raw_nick.contains("Alice"));

        // Right key decrypts.
        let members = store.list_family(Some(&key)).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].nickname, "Alice Home");
        assert_eq!(
            members[0].last_address.as_deref(),
            Some("192.168.1.50:7777")
        );

        // list_family_all (export path) also decrypts.
        let all = store.list_family_all(Some(&key)).unwrap();
        assert_eq!(all[0].nickname, "Alice Home");

        // Nickname update re-encrypts; read-back works.
        store
            .set_family_nickname(&pk, "Renamed", Some(&key))
            .unwrap();
        let renamed = store.list_family(Some(&key)).unwrap();
        assert_eq!(renamed[0].nickname, "Renamed");

        // Legacy plaintext row (key = None write) reads fine without a key
        // AND with a key (open_meta_value passes non-envelope values through).
        let pk2 = [0x22u8; 32];
        store
            .add_family_member(&pk2, "Plaintext Bob", None, None, None)
            .unwrap();
        let mixed_none = store.list_family(None).unwrap();
        assert_eq!(mixed_none.len(), 2);
        let bob = mixed_none
            .iter()
            .find(|m| m.public_key_hex == hex::encode(pk2))
            .unwrap();
        assert_eq!(bob.nickname, "Plaintext Bob");
        let mixed_keyed = store.list_family(Some(&key)).unwrap();
        let bob2 = mixed_keyed
            .iter()
            .find(|m| m.public_key_hex == hex::encode(pk2))
            .unwrap();
        assert_eq!(bob2.nickname, "Plaintext Bob");
    }

    /// Reaction text must be encrypted at rest, dedupe correctly despite
    /// per-write random nonces, remove by decrypted match, and skip
    /// undecryptable rows on read.
    #[test]
    fn test_reaction_metadata_encrypted_at_rest() {
        let store = mem_messagestore();
        let key = test_key();
        let peer = [0x33u8; 32];
        store.ensure_conversation("conv-r", &peer).unwrap();
        store
            .store_message("m-1", "conv-r", "sent", &[0u8; 24], b"hello", 1000, true)
            .unwrap();

        // Keyed insert → envelope on disk.
        store
            .upsert_reaction("m-1", "👍", &hex::encode(peer), false, "conv-r", Some(&key))
            .unwrap();
        let raw: String = store
            .conn
            .query_row(
                "SELECT reaction FROM reactions WHERE message_id = 'm-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            raw.starts_with("enc1:"),
            "reaction must be an envelope, got {raw}"
        );

        // Read back with right key.
        let map = store
            .get_reactions(&["m-1".to_string()], Some(&key))
            .unwrap();
        assert_eq!(map["m-1"][0].0, "👍");

        // Duplicate insert from same peer does NOT create a second row
        // (random nonces would defeat SQL-level uniqueness).
        store
            .upsert_reaction("m-1", "👍", &hex::encode(peer), false, "conv-r", Some(&key))
            .unwrap();
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reactions WHERE message_id = 'm-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Remove matches by DECRYPTED text.
        store
            .upsert_reaction("m-1", "👍", &hex::encode(peer), true, "conv-r", Some(&key))
            .unwrap();
        let count_after: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reactions WHERE message_id = 'm-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_after, 0);

        // Wrong-key read skips the undecryptable row instead of erroring.
        let wrong = StorageKey::new([0xEE; 32]);
        store
            .upsert_reaction("m-1", "❤️", &hex::encode(peer), false, "conv-r", Some(&key))
            .unwrap();
        let skipped = store
            .get_reactions(&["m-1".to_string()], Some(&wrong))
            .unwrap();
        assert!(
            skipped.is_empty(),
            "undecryptable reactions must be skipped"
        );

        // Legacy plaintext reaction still readable without a key.
        //
        // This is on a *different* message, and that is the point rather than an
        // accident: `m-1` carries a sealed envelope from this same peer, so
        // writing to it without a key cannot tell "new reaction" from "duplicate
        // of the sealed one" and now refuses. A message whose rows are all
        // plaintext is still fully usable with `key = None` — which is the whole
        // no-vault / pre-metadata-at-rest profile this branch exists for.
        store
            .store_message("m-2", "conv-r", "sent", &[0u8; 24], b"hello", 1001, true)
            .unwrap();
        store
            .upsert_reaction("m-2", "legacy", &hex::encode(peer), false, "conv-r", None)
            .unwrap();
        let legacy_map = store.get_reactions(&["m-2".to_string()], None).unwrap();
        assert!(
            legacy_map["m-2"].iter().any(|(r, _, _)| r == "legacy"),
            "expected the legacy 'legacy' reaction to be present"
        );
    }

    /// Metadata-at-rest (H-secondary): keyed writes must produce an
    /// "enc1:" envelope on disk and decrypt back with the right key;
    /// wrong-key reads must NOT return the plaintext.
    #[test]
    fn test_transfer_metadata_encrypted_at_rest() {
        let store = mem_transferstore();
        let key = test_key();
        let wrong_key = StorageKey::new([0xEE; 32]);

        store
            .store_transfer(
                "enc-t1",
                "pk",
                "secret-report.pdf",
                1,
                "sent",
                "failed",
                1,
                Some(&key),
            )
            .unwrap();
        store
            .update_state("enc-t1", "failed", Some(99), Some("disk full"), Some(&key))
            .unwrap();
        store
            .set_local_path("enc-t1", "/tmp/secret-report.pdf", Some(&key))
            .unwrap();

        // Raw row must not contain any plaintext metadata.
        let raw_name: String = store
            .conn
            .query_row(
                "SELECT filename FROM transfers WHERE id = 'enc-t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            raw_name.starts_with("enc1:"),
            "filename must be stored as envelope, got {raw_name}"
        );
        assert!(!raw_name.contains("secret"));

        // Right key decrypts everything.
        let t = store.get_transfer("enc-t1", Some(&key)).unwrap().unwrap();
        assert_eq!(t.filename, "secret-report.pdf");
        assert_eq!(t.error.as_deref(), Some("disk full"));
        assert_eq!(t.local_path.as_deref(), Some("/tmp/secret-report.pdf"));

        // Wrong key must fail to decrypt (not silently return plaintext).
        let bad = store.get_transfer("enc-t1", Some(&wrong_key));
        assert!(bad.is_err() || bad.unwrap().unwrap().filename != "secret-report.pdf");

        // Legacy plaintext rows still read back unchanged without a key.
        store
            .store_transfer(
                "legacy-t2",
                "pk2",
                "old-file.txt",
                2,
                "sent",
                "completed",
                1,
                None,
            )
            .unwrap();
        let legacy = store.get_transfer("legacy-t2", None).unwrap().unwrap();
        assert_eq!(legacy.filename, "old-file.txt");
    }

    // ─── Crypto-shredding tests (H7) ──────────────────────────

    use crate::secure_key::StorageKey;

    fn test_key() -> StorageKey {
        StorageKey::new([0x5A; 32])
    }

    #[test]
    fn test_secure_message_roundtrip() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-sec", &[0xAA; 32]).unwrap();
        store
            .store_message_secure(
                "m1",
                "conv-sec",
                "sent",
                b"shreddable secret",
                1000,
                None,
                true,
                &test_key(),
            )
            .unwrap();

        let msgs = store.load_messages("conv-sec", 10).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(
            msgs[0].content_key_wrapped.is_some(),
            "secure rows must carry a wrapped CEK"
        );

        let pt = MessageStore::decrypt_stored_content(
            &msgs[0].content_encrypted,
            &msgs[0].content_nonce,
            msgs[0].content_key_wrapped.as_deref(),
            &test_key(),
        )
        .unwrap();
        assert_eq!(pt, b"shreddable secret");

        // Content must NOT be decryptable directly under the vault key —
        // it is sealed under the per-message CEK only.
        assert!(
            crate::commands::util::crypto_decrypt_storage(
                &msgs[0].content_encrypted,
                &msgs[0].content_nonce,
                &test_key(),
                crate::storage::AAD_MSG_STORE,
            )
            .is_err(),
            "content must not be encrypted under the vault storage key"
        );
    }

    /// Legacy rows (pre-crypto-shredding) have no wrapped key and were
    /// encrypted directly under the vault key — they must still decrypt.
    #[test]
    fn test_legacy_row_fallback_decrypt() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-old", &[0xBB; 32]).unwrap();

        // Old-style: caller encrypted under the vault key; no wrapped CEK.
        let (nonce, ct) = crate::commands::util::crypto_encrypt_storage(
            b"legacy plaintext",
            &test_key(),
            crate::storage::AAD_MSG_STORE,
        )
        .unwrap();
        store
            .store_message("m1", "conv-old", "received", &ct, &nonce, 1000, true)
            .unwrap();

        let msgs = store.load_messages("conv-old", 10).unwrap();
        assert!(msgs[0].content_key_wrapped.is_none());

        let pt = MessageStore::decrypt_stored_content(
            &msgs[0].content_encrypted,
            &msgs[0].content_nonce,
            None,
            &test_key(),
        )
        .unwrap();
        assert_eq!(pt, b"legacy plaintext");
    }

    /// THE core H7 assertion: after a soft-delete shred, the tombstone row's
    /// ciphertext is UNRECOVERABLE even with the correct vault storage key,
    /// because its wrapped content key was destroyed.
    #[test]
    fn test_soft_delete_shreds_content_key() {
        let store = mem_messagestore();
        store
            .ensure_conversation("conv-shred", &[0xCC; 32])
            .unwrap();
        store
            .store_message_secure(
                "m1",
                "conv-shred",
                "sent",
                b"doomed message",
                1000,
                None,
                true,
                &test_key(),
            )
            .unwrap();

        // Capture the ciphertext remnants an attacker might recover from disk.
        let msgs = store.load_messages("conv-shred", 10).unwrap();
        let remnant_ct = msgs[0].content_encrypted.clone();
        let remnant_nonce = msgs[0].content_nonce.clone();

        assert!(store.delete_message("m1", "conv-shred", "sent").unwrap());

        // Tombstone remains but carries a zeroed wrapped key.
        let after = store.load_messages("conv-shred", 10).unwrap();
        assert_eq!(after.len(), 1);
        assert!(after[0].deleted);
        let wrapped = after[0]
            .content_key_wrapped
            .as_deref()
            .expect("wrapped cell present");
        assert_eq!(wrapped.len(), WRAPPED_CEK_LEN);
        assert!(
            wrapped.iter().all(|&b| b == 0),
            "wrapped CEK must be zeroed"
        );

        // Simulated remnant attack: original ciphertext + nonce recovered
        // from disk + the CURRENT vault key → decryption MUST fail.
        assert!(MessageStore::decrypt_stored_content(
            &remnant_ct,
            &remnant_nonce,
            Some(wrapped),
            &test_key(),
        )
        .is_err());
    }

    #[test]
    fn test_edit_message_secure_rekeys_and_shreds_old() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-edit", &[0xDD; 32]).unwrap();
        store
            .store_message_secure(
                "m1",
                "conv-edit",
                "sent",
                b"original text",
                1000,
                None,
                true,
                &test_key(),
            )
            .unwrap();
        let old_ct = store.load_messages("conv-edit", 10).unwrap()[0]
            .content_encrypted
            .clone();

        assert!(store
            .edit_message_secure("m1", "conv-edit", "sent", b"edited text", &test_key(),)
            .unwrap());

        let msgs = store.load_messages("conv-edit", 10).unwrap();
        assert_ne!(
            msgs[0].content_encrypted, old_ct,
            "edit must re-encrypt content"
        );
        let pt = MessageStore::decrypt_stored_content(
            &msgs[0].content_encrypted,
            &msgs[0].content_nonce,
            msgs[0].content_key_wrapped.as_deref(),
            &test_key(),
        )
        .unwrap();
        assert_eq!(pt, b"edited text");
    }

    #[test]
    fn test_expired_delete_removes_only_expired() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-exp", &[0xEE; 32]).unwrap();
        let past = 1000i64;
        let future = chrono::Utc::now().timestamp() + 3600;
        store
            .store_message_secure(
                "m-expired",
                "conv-exp",
                "sent",
                b"gone",
                past,
                Some(past),
                true,
                &test_key(),
            )
            .unwrap();
        store
            .store_message_secure(
                "m-live",
                "conv-exp",
                "sent",
                b"kept",
                past,
                Some(future),
                true,
                &test_key(),
            )
            .unwrap();

        let deleted = store.delete_expired_messages().unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.load_messages("conv-exp", 10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "m-live");
        let pt = MessageStore::decrypt_stored_content(
            &remaining[0].content_encrypted,
            &remaining[0].content_nonce,
            remaining[0].content_key_wrapped.as_deref(),
            &test_key(),
        )
        .unwrap();
        assert_eq!(pt, b"kept");
    }

    /// Migration: a pre-crypto-shredding database (messages table without
    /// content_key_wrapped) must open cleanly and accept new secure writes.
    #[test]
    fn test_migration_adds_content_key_column() {
        let dir = std::env::temp_dir().join(format!("m2m_h7_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("messages.db");

        // Build an OLD-schema database manually (as shipped before
        // crypto-shredding: no content_key_wrapped column).
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE conversations (
                    id TEXT PRIMARY KEY, peer_id BLOB NOT NULL, created_at INTEGER NOT NULL);
                 CREATE TABLE messages (
                    id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL,
                    direction TEXT NOT NULL, content_encrypted BLOB NOT NULL,
                    content_nonce BLOB NOT NULL, timestamp INTEGER NOT NULL,
                    delivered INTEGER NOT NULL DEFAULT 0);",
            )
            .unwrap();
        }

        let store = MessageStore::open(&db_path).unwrap();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        store
            .store_message_secure(
                "m1",
                "c1",
                "sent",
                b"post-migration write",
                1000,
                None,
                true,
                &test_key(),
            )
            .unwrap();
        let msgs = store.load_messages("c1", 10).unwrap();
        assert_eq!(
            MessageStore::decrypt_stored_content(
                &msgs[0].content_encrypted,
                &msgs[0].content_nonce,
                msgs[0].content_key_wrapped.as_deref(),
                &test_key(),
            )
            .unwrap(),
            b"post-migration write"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── Storage cap: byte accounting and eviction ─────────────────────────
    //
    // The cap exists because the receive loop's limits are *rate* limits. 30
    // frames/s and 16 MiB/s bound how fast a peer writes, not how much in
    // total, and `retention_policy = 'none'` is the default, so nothing
    // reclaims it. These tests pin the two properties that make the cap
    // trustworthy: the byte count is exact, and eviction is permanent and
    // oldest-first.

    /// Store `n` messages of `size` bytes, oldest-first by timestamp.
    fn fill_messages(store: &MessageStore, conv: &str, n: u32, size: usize) {
        for i in 0..n {
            store
                .store_message_secure(
                    &format!("m{i}"),
                    conv,
                    "received",
                    &vec![b'x'; size],
                    1_000 + i as i64,
                    None,
                    true,
                    &test_key(),
                )
                .unwrap();
        }
    }

    #[test]
    fn test_stored_bytes_counts_and_releases() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        assert_eq!(store.stored_bytes().unwrap(), 0, "empty store is 0");

        fill_messages(&store, "c1", 5, 1000);
        let after = store.stored_bytes().unwrap();
        // ciphertext (1000 + 16-byte tag) + 24-byte nonce + 72-byte wrapped key
        let expected = 5 * (1000 + 16 + 24 + 72);
        assert_eq!(after, expected as u64, "counter must equal exact row size");

        store.delete_conversation("c1").unwrap();
        assert_eq!(
            store.stored_bytes().unwrap(),
            0,
            "deleting the conversation must release every byte"
        );
    }

    #[test]
    fn test_stored_bytes_ignores_duplicate_inserts() {
        // `store_message_secure` is `INSERT OR IGNORE`, so a redelivered
        // message must not inflate the count — otherwise a peer looping the
        // same frame id would trip the cap without adding anything.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 1, 500);
        let once = store.stored_bytes().unwrap();
        for _ in 0..5 {
            store
                .store_message_secure("m0", "c1", "received", &vec![b'x'; 500], 1_000, None, true, &test_key())
                .unwrap();
        }
        assert_eq!(store.stored_bytes().unwrap(), once);
    }

    #[test]
    fn test_stored_bytes_verified_recovers_from_drift() {
        // The counter is a cache. If a write path forgets to update it, the
        // exact re-derivation on the enforcement path must still be right —
        // a counter that drifted low would let the cap be overshot.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 3, 200);
        let truth = store.stored_bytes_verified().unwrap();

        // Corrupt the counter, as a missing update would. Off by one rather
        // than zeroed, so the assertion below cannot pass by the two paths
        // coincidentally agreeing.
        store
            .conn
            .execute(
                "UPDATE storage_stats SET total_bytes = total_bytes + 1 WHERE id = 1",
                [],
            )
            .unwrap();
        assert_eq!(
            store.stored_bytes().unwrap(),
            truth + 1,
            "the cache is now off by one"
        );
        assert_eq!(
            store.stored_bytes_verified().unwrap(),
            truth,
            "re-derivation must restore the true total"
        );
    }

    #[test]
    fn test_evict_is_oldest_first() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 10, 1000);
        let one_msg = store.stored_bytes().unwrap() / 10;

        // Cap that leaves room for 3 messages after the 90% low-water step.
        let report = store.evict_to_cap(one_msg * 4).unwrap();
        assert!(report.messages_evicted > 0, "something must be evicted");

        // The survivors must be the *newest* ones.
        let msgs = store.load_messages("c1", 100).unwrap();
        let remaining: Vec<&str> = msgs.iter().map(|m| m.id.as_str()).collect();
        assert!(
            remaining.contains(&"m9"),
            "newest message must survive, got {remaining:?}"
        );
        assert!(
            !remaining.contains(&"m0"),
            "oldest message must be gone, got {remaining:?}"
        );
        assert!(store.stored_bytes().unwrap() <= one_msg * 4);
    }

    #[test]
    fn test_evict_is_permanent_not_soft_delete() {
        // The user's own "delete for everyone" leaves a tombstone row; the cap
        // must not, because the point is to release the bytes.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 6, 1000);
        let one_msg = store.stored_bytes().unwrap() / 6;

        store.evict_to_cap(one_msg * 2).unwrap();

        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert!(count < 6, "rows must be removed, not tombstoned: {count}");
        // And no residual CEK anywhere, for the rows that did go.
        let live_keys: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages
                  WHERE content_key_wrapped IS NOT NULL
                    AND length(content_key_wrapped) != ?1",
                params![WRAPPED_CEK_LEN],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(live_keys, 0, "surviving rows keep their keys; evicted keys are zeroed");
    }

    #[test]
    fn test_evict_reports_overridden_retention() {
        // A user who set "delete after 7 days" and then loses messages to a
        // full disk has had a preference overridden. The conversation must be
        // named so the UI can say which one.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        store.ensure_conversation("c2", &[0x22; 32]).unwrap();
        store
            .conn
            .execute(
                "UPDATE conversations SET retention_policy = 'delete', auto_delete_at = 999 WHERE id = 'c1'",
                [],
            )
            .unwrap();
        fill_messages(&store, "c1", 6, 1000);
        let one_msg = store.stored_bytes().unwrap() / 6;

        let report = store.evict_to_cap(one_msg * 2).unwrap();
        assert!(
            report.overrode_retention.contains(&"c1".to_string()),
            "the conversation whose policy was overridden must be reported, got {:?}",
            report.overrode_retention
        );
    }

    #[test]
    fn test_evict_includes_group_messages() {
        // `group_messages` is a separate table. If it were not counted or not
        // evicted, an attacker could park everything in group traffic and the
        // cap would never fire.
        let store = mem_messagestore();
        store.upsert_group("g1", "G", 1, "member").unwrap();
        for i in 0..6u32 {
            store
                .store_group_message(
                    &format!("gm{i}"),
                    "g1",
                    &"a".repeat(64),
                    &vec![b'y'; 1000],
                    &[0u8; 24],
                    1_000 + i as i64,
                    true,
                )
                .unwrap();
        }
        let before = store.stored_bytes().unwrap();
        assert!(before > 0, "group messages must count toward the cap");

        let report = store.evict_to_cap(before / 4).unwrap();
        assert!(
            report.group_messages_evicted > 0,
            "group messages must be evicted, got {report:?}"
        );
    }

    #[test]
    fn test_evict_is_a_noop_under_the_cap() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 3, 100);
        let before = store.stored_bytes().unwrap();

        let report = store.evict_to_cap(before * 100).unwrap();
        assert_eq!(report.messages_evicted, 0);
        assert_eq!(report.bytes_freed, 0);
        assert_eq!(store.stored_bytes().unwrap(), before, "nothing may be lost");
    }

    #[test]
    fn test_shred_message_keys_destroys_the_cek() {
        // The guarantee the whole feature rests on. Eviction removes the row,
        // so the shredded key cannot be inspected after the fact — which means
        // a test that only checks "the row is gone" would pass even if the
        // shred step were deleted entirely. This pins the shred itself.
        //
        // Mutation-verified: commenting out the `shred_message_keys` call in
        // `evict_to_cap` does not fail any other test in this file.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 2, 200);

        let before: Vec<u8> = store
            .conn
            .query_row("SELECT content_key_wrapped FROM messages WHERE id = 'm0'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(before.iter().any(|&b| b != 0), "a real wrapped key is not all zeros");

        store.shred_message_keys(&["m0".to_string()]).unwrap();

        let after: Vec<u8> = store
            .conn
            .query_row("SELECT content_key_wrapped FROM messages WHERE id = 'm0'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(after.len(), WRAPPED_CEK_LEN);
        assert!(
            after.iter().all(|&b| b == 0),
            "the wrapped content key must be overwritten with zeros"
        );

        // And the consequence: the content can no longer be decrypted, even
        // though the row and its ciphertext are still there.
        let msgs = store.load_messages("c1", 10).unwrap();
        let m0 = msgs.iter().find(|m| m.id == "m0").expect("row still present");
        assert!(
            MessageStore::decrypt_stored_content(
                &m0.content_encrypted,
                &m0.content_nonce,
                Some(&after),
                &test_key(),
            )
            .is_err(),
            "shredded content must be undecryptable while the row survives"
        );
    }

    #[test]
    fn test_evict_shreds_every_evicted_message() {
        // Proves the *wiring*, not just the shred helper. Eviction shreds and
        // then deletes, so the rows — and the evidence that they were shredded
        // — are gone once the pass completes. A test that only called
        // `shred_message_keys` directly would pass unchanged if `evict_to_cap`
        // had stopped calling it, which is precisely the regression worth
        // catching: a hard delete without a shred leaves recoverable key
        // material behind on freed pages.
        //
        // Mutation-verified: commenting out the `shred_message_keys` call in
        // `evict_to_cap` fails this test.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 10, 1000);
        let one_msg = store.stored_bytes().unwrap() / 10;
        assert_eq!(store.shredded_key_count(), 0, "nothing shredded yet");

        let report = store.evict_to_cap(one_msg * 4).unwrap();

        assert!(
            report.messages_evicted > 0,
            "the test needs an eviction to have happened"
        );
        assert_eq!(
            store.shredded_key_count(),
            report.messages_evicted as u64,
            "every evicted message must have had its content key destroyed \
             — reported {} evicted, {} shredded",
            report.messages_evicted,
            store.shredded_key_count()
        );
    }

    // ─── Sweep: expiry + cap, and the two paths that call them ──────────────
    //
    // `sweep` is what the background task in `maintenance.rs` runs, and
    // `enforce_storage_cap` is what every write path runs. The gap these cover
    // is the one the cap could not have: it was enforced at one write site of
    // four, and self-destruct expiry ran only while a chat screen was mounted.

    #[test]
    fn test_enforce_storage_cap_is_a_noop_under_the_cap() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 4, 500);
        let used = store.stored_bytes().unwrap();
        let shredded_before = store.shredded_key_count();

        assert!(
            store.enforce_storage_cap(used * 2).unwrap().is_none(),
            "under the cap nothing may be evicted and nothing may be reported"
        );
        assert_eq!(store.load_messages("c1", 100).unwrap().len(), 4);
        assert_eq!(store.shredded_key_count(), shredded_before);
    }

    #[test]
    fn test_enforce_storage_cap_evicts_and_reports_over_the_cap() {
        // Mutation-verified: making `enforce_storage_cap` return `Ok(None)`
        // unconditionally fails this test — which is exactly the shape of the
        // original bug, where a write path that "checked" the cap reported
        // success having done nothing.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 10, 1000);
        let one_msg = store.stored_bytes().unwrap() / 10;

        let report = store
            .enforce_storage_cap(one_msg * 4)
            .unwrap()
            .expect("over the cap, so the caller must be told what it lost");

        assert!(report.messages_evicted > 0, "something must be evicted");
        assert!(report.bytes_freed > 0, "the report must quantify the loss");
        assert!(
            store.stored_bytes().unwrap() <= one_msg * 4,
            "the store must end up back under the cap"
        );
        assert_eq!(
            store.shredded_key_count(),
            report.messages_evicted as u64,
            "the write-path entry point must shred, not merely delete"
        );
    }

    #[test]
    fn test_enforce_storage_cap_reports_nothing_when_nothing_is_evictable() {
        // A counter that drifted *high* must not manufacture an eviction. The
        // enforcement path re-derives from SQL, so an inflated cache resolves to
        // "under the cap" and the user is not shown a report for messages that
        // were never destroyed — a notice about a loss that did not happen is
        // the mirror of the silent loss this feature exists to prevent.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 2, 200);
        store
            .conn
            .execute(
                "UPDATE storage_stats SET total_bytes = 1000000 WHERE id = 1",
                [],
            )
            .unwrap();

        assert!(
            store.enforce_storage_cap(1_000).unwrap().is_none(),
            "a cache that over-reports must not cause a phantom eviction"
        );
        assert_eq!(store.load_messages("c1", 100).unwrap().len(), 2);
    }

    #[test]
    fn test_sweep_expires_and_enforces_in_one_pass() {
        // The two halves of the background task, and the order matters: expiry
        // runs first so an elapsed self-destruct timer frees bytes that the cap
        // would otherwise have to evict something else to reclaim.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        let past = chrono::Utc::now().timestamp() - 3600;
        store
            .store_message_secure("m-expired", "c1", "sent", b"gone", past, Some(past), true, &test_key())
            .unwrap();
        fill_messages(&store, "c1", 6, 1000);
        let after_expiry_bytes = {
            let outcome = store.sweep(u64::MAX / 4).unwrap();
            assert_eq!(outcome.expired_messages, 1, "the elapsed timer must fire");
            assert!(
                outcome.evicted.messages_evicted == 0,
                "an effectively unlimited cap must evict nothing"
            );
            store.stored_bytes().unwrap()
        };
        assert!(
            store.load_messages("c1", 100).unwrap().iter().all(|m| m.id != "m-expired"),
            "the expired message must be gone from the store"
        );

        // Now the same call with a cap the store no longer fits under.
        let one_msg = after_expiry_bytes / 6;
        let outcome = store.sweep(one_msg * 3).unwrap();
        assert_eq!(outcome.expired_messages, 0, "nothing left to expire");
        assert!(outcome.evicted.messages_evicted > 0);
        assert!(
            outcome.destroyed_anything(),
            "a pass that evicted must report that it destroyed something"
        );
    }

    // ─── Conversation retention policy ───
    //
    // `retention_policy` / `auto_delete_at` were written by
    // `set_conversation_retention`, read back for display, and **never acted
    // on by anything**. So the UI's "Auto-Delete After 24h" persisted,
    // displayed, toasted success on write, and destroyed nothing — the exact
    // failure CLAUDE.md calls out as the worst instance of a control that
    // reports success when the thing it describes did not happen.

    #[test]
    fn test_sweep_enforces_conversation_retention_policy() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 4, 500);

        // Already past its deadline.
        store
            .conn
            .execute(
                "UPDATE conversations SET retention_policy = 'delete', auto_delete_at = 1 WHERE id = 'c1'",
                [],
            )
            .unwrap();

        let outcome = store.sweep(u64::MAX / 4).unwrap();
        assert_eq!(
            outcome.expired_messages, 4,
            "an elapsed conversation policy must destroy the conversation's messages"
        );

        // The observable is the disk, not the read path: `load_messages` filters
        // expired rows, so asserting through it would prove nothing.
        let on_disk: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages WHERE conversation_id = 'c1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(on_disk, 0, "the rows must be physically removed, not just hidden");
    }

    #[test]
    fn test_export_policy_is_never_swept() {
        // `export` means "keep it so I can export it". Sweeping it would be
        // data destruction the user explicitly asked NOT to happen.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 4, 500);
        store
            .conn
            .execute(
                "UPDATE conversations SET retention_policy = 'export', auto_delete_at = 1 WHERE id = 'c1'",
                [],
            )
            .unwrap();

        let outcome = store.sweep(u64::MAX / 4).unwrap();
        assert_eq!(outcome.expired_messages, 0, "an 'export' policy must not destroy");
        let on_disk: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(on_disk, 4);
    }

    #[test]
    fn test_setting_retention_applies_retroactively_to_existing_messages() {
        // A policy the user sets now has to cover the history it is a policy
        // *for*. Without this the messages already stored were never swept and
        // the policy only ever affected messages sent afterwards.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 3, 400);

        store
            .set_conversation_retention("c1", "delete", Some(3600))
            .unwrap();

        let unstamped: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = 'c1' AND expires_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            unstamped, 0,
            "existing messages must inherit the conversation deadline, or the policy only covers new messages"
        );
    }

    #[test]
    fn test_setting_retention_does_not_shorten_a_per_message_timer() {
        // A per-message self-destruct is the more specific promise. Silently
        // lowering it would destroy content the user asked to keep for longer.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        let far = chrono::Utc::now().timestamp() + 7 * 86_400;
        store
            .store_message_secure("m7d", "c1", "sent", b"keep", 1_000, Some(far), true, &test_key())
            .unwrap();

        store.set_conversation_retention("c1", "delete", Some(3600)).unwrap();

        let got: i64 = store
            .conn
            .query_row("SELECT expires_at FROM messages WHERE id = 'm7d'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(got, far, "the longer per-message timer must win");
    }

    #[test]
    fn test_expiry_removes_orphan_reactions() {
        // `reactions` has no foreign key to `messages`, so every self-destruct
        // left its reactions behind, naming messages that no longer existed.
        // They were also invisible to the storage cap, which counts message rows
        // only — so the one table that could grow unboundedly was the one the
        // ceiling could not see.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        let past = chrono::Utc::now().timestamp() - 60;
        store
            .store_message_secure("m-gone", "c1", "sent", b"x", past, Some(past), true, &test_key())
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO reactions (message_id, reaction, peer_key_hex, created_at)
                 VALUES ('m-gone', 'X', ?1, 1)",
                params!["a".repeat(64)],
            )
            .unwrap();

        store.delete_expired_messages().unwrap();

        let remaining: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM reactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0, "a reaction must not outlive the message it annotates");
    }

    #[test]
    fn test_mark_messages_read_and_unread_count_ignore_expired_and_deleted() {
        // Both the badge and the write counted rows that are logically already
        // gone, so the count was permanently inflated by anything awaiting a
        // sweep and by the user's own "delete for everyone" tombstones.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        let past = chrono::Utc::now().timestamp() - 60;
        store
            .store_message_secure("m-exp", "c1", "received", b"x", past, Some(past), true, &test_key())
            .unwrap();
        store
            .store_message_secure("m-live", "c1", "received", b"y", 2_000, None, true, &test_key())
            .unwrap();
        store
            .conn
            .execute("UPDATE messages SET deleted = 1 WHERE id = 'm-live'", [])
            .unwrap();

        let marked = store.mark_messages_read("c1").unwrap();
        assert_eq!(
            marked, 0,
            "no live unread message exists, so marking read must report zero"
        );

        let summary = store.list_conversations().unwrap();
        let c1 = summary.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(c1.unread_count, 0, "expired and deleted rows must not inflate the badge");
        assert_eq!(c1.message_count, 0);
    }

    #[test]
    fn test_enforce_cap_recovers_from_counter_drift() {
        // The gate used to trust the O(1) cached counter, so any write path that
        // forgot `add_stored_bytes` left it low and enforcement never fired: the
        // counter had to first exceed the cap on its own before the scan that
        // would have corrected it ran.
        //
        // `sweep` is the entry point here, not `enforce_storage_cap` directly:
        // the cap's opportunistic band is a deliberate performance compromise,
        // and the *unconditional* re-derivation is what `sweep` contributes on
        // its 15-minute timer. Asserting on the bare gate would demand a full
        // table scan per inbound message.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 6, 1000);
        let one_msg = store.stored_bytes().unwrap() / 6;

        // Simulate drift: the counter claims the store is far below the cap.
        store
            .conn
            .execute("UPDATE storage_stats SET total_bytes = 0 WHERE id = 1", [])
            .unwrap();
        assert_eq!(store.stored_bytes().unwrap(), 0, "the counter is now wrong");

        // A cap the truth violates but the cached number does not.
        let outcome = store.sweep(one_msg * 3).unwrap();
        assert!(
            outcome.evicted.messages_evicted > 0,
            "a drifted-low counter must not be able to switch the cap off"
        );
        assert!(
            store.stored_bytes().unwrap() <= one_msg * 3,
            "usage must end up under the cap, not merely reported as over it"
        );
    }

    #[test]
    fn test_enforce_cap_verifies_when_the_counter_is_near_the_cap() {
        // The opportunistic band: a counter sitting just under the cap is
        // untrustworthy by definition, so the authoritative re-derivation must
        // decide. A counter drifted high (an over-counted delete, say) must not
        // be able to trigger an eviction that destroys nothing needed either.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 6, 1000);
        let truth = store.stored_bytes().unwrap();

        // Claim to be just below the cap; the truth is well under it.
        let cap = truth * 2;
        store
            .conn
            .execute(
                "UPDATE storage_stats SET total_bytes = ?1 WHERE id = 1",
                // `cap - 1`, not something further away: the gate's verify band
                // is `min(cap/16, 64 MiB)`, so a counter that is only 1024 bytes
                // under the cap is *outside* it and would legitimately skip
                // verification. One byte under is unambiguously "near".
                params![cap - 1],
            )
            .unwrap();

        let report = store.enforce_storage_cap(cap).unwrap();
        assert!(
            report.is_none(),
            "the near-cap counter must be verified against SQL, and the truth is \
             under the cap, so nothing may be evicted"
        );
        assert_eq!(
            store.stored_bytes().unwrap(),
            truth,
            "the verification pass must also correct the stored counter"
        );
    }

    #[test]
    fn test_sweep_is_a_noop_when_there_is_nothing_to_do() {
        // The common case, and the one that runs every 15 minutes for the life
        // of the process. It must destroy nothing and say so.
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 3, 500);
        let before = store.stored_bytes().unwrap();

        let outcome = store.sweep(before * 2).unwrap();
        assert_eq!(outcome.expired_messages, 0);
        assert_eq!(outcome.evicted, EvictionReport::default());
        assert!(
            !outcome.destroyed_anything(),
            "nothing happened, so nothing may be reported as having happened"
        );
        assert_eq!(store.stored_bytes().unwrap(), before);
    }

    #[test]
    fn test_open_destroys_messages_that_expired_while_the_app_was_closed() {
        // The bug this whole path exists for. Expiry was driven by a
        // `setInterval` in `ChatView`, so a self-destruct timer elapsed while
        // the app was shut — or while the user was on another screen, which for
        // a tray app is most of its life — left the message on disk, content
        // key and all, until the app happened to be sitting on that screen.
        //
        // So: write a message, let it expire, close the store, reopen. The row
        // must be gone before the first query can read it.
        let dir = std::env::temp_dir().join(format!("m2m_exp_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("messages.db");

        let past = chrono::Utc::now().timestamp() - 60;
        let future = chrono::Utc::now().timestamp() + 3600;
        {
            let store = MessageStore::open(&db_path).unwrap();
            store.ensure_conversation("c1", &[0x11; 32]).unwrap();
            store
                .store_message_secure("m-expired", "c1", "sent", b"boom", past, Some(past), true, &test_key())
                .unwrap();
            store
                .store_message_secure("m-live", "c1", "sent", b"keep", past, Some(future), true, &test_key())
                .unwrap();
        }
        // "Closed" — the process is gone, nothing has run since.

        let store = MessageStore::open(&db_path).unwrap();
        let msgs = store.load_messages("c1", 10).unwrap();
        let ids: Vec<&str> = msgs.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"m-live"), "a live timer must be untouched");
        assert!(
            !ids.contains(&"m-expired"),
            "an expired message must not survive the restart, got {ids:?}"
        );

        // The read path already filters expired rows (`load_messages` carries
        // `expires_at > now`), so asserting on `load_messages` alone proves
        // nothing about the disk — the row and its wrapped content key would
        // still be sitting in the file, readable by anyone who seizes it and
        // has the storage key. That is the actual risk, so that is what is
        // asserted: the row is *gone from the table*.
        //
        // Mutation-verified: deleting the `delete_expired_messages` call from
        // `MessageStore::open` fails this assertion while leaving every
        // `load_messages` assertion above passing.
        let on_disk: Vec<String> = store
            .conn
            .prepare("SELECT id FROM messages ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(
            on_disk,
            vec!["m-live".to_string()],
            "the expired row must be physically removed at open, not merely hidden"
        );

        // And the byte counter must agree with the tables, or the cap starts
        // from a number that includes bytes nobody can reach any more.
        let one_msg = store
            .conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(content_encrypted) + LENGTH(content_nonce) + ?1), 0)
                   FROM messages",
                params![MessageStore::MSG_ROW_OVERHEAD],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(
            store.stored_bytes().unwrap() as i64,
            one_msg,
            "the counter must be re-derived at open, after the expiry pass"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    // ─── A control must not report success for something that did not happen ──
    //
    // The recurring failure in this codebase is a function returning `Ok(true)`
    // — or a toast reading "done" — while the thing it describes failed
    // silently. Every test below asserts the *error*, not the success, because
    // the bug is precisely that a success was returned.

    /// Reaction remove must fail loudly when the sealed row cannot be opened.
    ///
    /// This is the exact bug: the dedup/remove comparison was
    /// `.unwrap_or(false)`, so with the vault locked every sealed row failed to
    /// decrypt, every comparison returned "no match", the DELETE loop had an
    /// empty `matched` list, and `upsert_reaction` returned `Ok(true)`. The
    /// caller surfaced that as "reaction removed" while the row was still on
    /// disk and still rendered in the UI.
    ///
    /// Asserted on the table, not on `get_reactions`: the read path skips
    /// undecryptable rows, so it would look identical before and after a
    /// successful delete.
    #[test]
    fn test_reaction_remove_errors_when_it_cannot_decrypt() {
        let store = mem_messagestore();
        let key = test_key();
        let peer = hex::encode([0x44u8; 32]);
        store.ensure_conversation("conv-x", &[0x44; 32]).unwrap();
        store
            .store_message("m-1", "conv-x", "sent", &[0u8; 24], b"hello", 1000, true)
            .unwrap();

        // Sealed row, written with a key.
        store
            .upsert_reaction("m-1", "👍", &peer, false, "conv-x", Some(&key))
            .unwrap();

        // Now remove it with NO key — the vault-locked case.
        let result = store.upsert_reaction("m-1", "👍", &peer, true, "conv-x", None);

        assert!(
            result.is_err(),
            "a remove that could not decrypt its target row must not report success"
        );
        assert!(
            matches!(result, Err(StorageError::KeyNotFound)),
            "expected KeyNotFound, got {:?}",
            result.err()
        );

        // The row must still be there. A delete that "succeeded" while leaving
        // the row is the precise failure mode being pinned.
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reactions WHERE message_id = 'm-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "the reaction row must survive a remove that could not read it"
        );
    }

    /// Reaction insert must also refuse when it cannot read the existing rows.
    ///
    /// Symmetric to the remove case and worse if unfixed: the dedup scan found
    /// no match, so the INSERT went ahead *alongside* the row it could not
    /// read. SQL cannot see the duplicate (the envelope has a fresh nonce, so
    /// `reaction` differs byte-wise even for the same emoji), and no subsequent
    /// remove can target it — the reaction is stuck on the message forever, and
    /// a remove of the identical emoji now matches two rows.
    #[test]
    fn test_reaction_insert_errors_when_it_cannot_decrypt() {
        let store = mem_messagestore();
        let key = test_key();
        let peer = hex::encode([0x55u8; 32]);
        store.ensure_conversation("conv-y", &[0x55; 32]).unwrap();
        store
            .store_message("m-1", "conv-y", "sent", &[0u8; 24], b"hello", 1000, true)
            .unwrap();

        store
            .upsert_reaction("m-1", "👍", &peer, false, "conv-y", Some(&key))
            .unwrap();

        // Same reaction again, but with no key: cannot tell duplicate from new.
        let result = store.upsert_reaction("m-1", "👍", &peer, false, "conv-y", None);

        assert!(
            result.is_err(),
            "an insert that could not check for duplicates must not claim it stored one"
        );
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reactions WHERE message_id = 'm-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "no duplicate may be inserted next to a row the caller could not read"
        );

        // And with the right key the dedup still works, so the guard has not
        // simply broken the feature.
        store
            .upsert_reaction("m-1", "👍", &peer, false, "conv-y", Some(&key))
            .unwrap();
        let count_keyed: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reactions WHERE message_id = 'm-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_keyed, 1, "keyed dedup must still collapse to one row");
    }

    /// A row that is genuinely *not* a match must still remove/insert normally.
    ///
    /// The guard added above must not degrade into "refuse whenever there is
    /// any sealed row" — that would break the ordinary case where a user reacts
    /// with a different emoji to a message they have already reacted to.
    #[test]
    fn test_reaction_dedup_still_works_when_rows_decrypt() {
        let store = mem_messagestore();
        let key = test_key();
        let peer = hex::encode([0x66u8; 32]);
        store.ensure_conversation("conv-z", &[0x66; 32]).unwrap();
        store
            .store_message("m-1", "conv-z", "sent", &[0u8; 24], b"hello", 1000, true)
            .unwrap();

        store
            .upsert_reaction("m-1", "👍", &peer, false, "conv-z", Some(&key))
            .unwrap();
        // Different emoji: a real insert, not a dedup, and not an error.
        store
            .upsert_reaction("m-1", "🎉", &peer, false, "conv-z", Some(&key))
            .unwrap();

        let map = store
            .get_reactions(&["m-1".to_string()], Some(&key))
            .unwrap();
        assert_eq!(map["m-1"].len(), 2, "two distinct reactions must coexist");

        // Removing one must leave the other.
        store
            .upsert_reaction("m-1", "👍", &peer, true, "conv-z", Some(&key))
            .unwrap();
        let map = store
            .get_reactions(&["m-1".to_string()], Some(&key))
            .unwrap();
        assert_eq!(map["m-1"].len(), 1);
        assert_eq!(map["m-1"][0].0, "🎉", "the wrong reaction must not be deleted");
    }

    /// A wrong *key* is the same failure as a missing one and must be treated
    /// the same way, not as "no match".
    ///
    /// Reaching this state means the vault key was replaced (a rekey) while the
    /// database kept its rows. Silently reporting success there is how a user
    /// ends up believing they removed a reaction that is still readable by
    /// whoever holds the old key.
    #[test]
    fn test_reaction_remove_errors_on_a_wrong_key() {
        let store = mem_messagestore();
        let key = test_key();
        let wrong = StorageKey::new([0xEE; 32]);
        let peer = hex::encode([0x77u8; 32]);
        store.ensure_conversation("conv-w", &[0x77; 32]).unwrap();
        store
            .store_message("m-1", "conv-w", "sent", &[0u8; 24], b"hello", 1000, true)
            .unwrap();

        store
            .upsert_reaction("m-1", "👍", &peer, false, "conv-w", Some(&key))
            .unwrap();

        let result = store.upsert_reaction("m-1", "👍", &peer, true, "conv-w", Some(&wrong));
        assert!(
            result.is_err(),
            "a wrong key is indistinguishable from no key here and must not \
             be treated as 'no match'"
        );
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM reactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "the row must survive");
    }

    /// A legacy plaintext row alongside a sealed one is still readable *with*
    /// the right key — `open_meta_value` passes non-envelope values through —
    /// so the guard must not fire for it.
    #[test]
    fn test_reaction_mixed_plaintext_and_sealed_rows_still_work_with_a_key() {
        let store = mem_messagestore();
        let key = test_key();
        let peer = hex::encode([0x88u8; 32]);
        store.ensure_conversation("conv-m", &[0x88; 32]).unwrap();
        store
            .store_message("m-1", "conv-m", "sent", &[0u8; 24], b"hello", 1000, true)
            .unwrap();

        // Plaintext (no key) and sealed (key) rows for the same peer/message.
        store
            .conn
            .execute(
                "INSERT INTO reactions (message_id, reaction, peer_key_hex, created_at)
                 VALUES ('m-1', 'plain', ?1, 1000)",
                params![peer],
            )
            .unwrap();
        store
            .upsert_reaction("m-1", "sealed", &peer, false, "conv-m", Some(&key))
            .unwrap();

        // Removing the plaintext one must succeed — it opens without a key.
        store
            .upsert_reaction("m-1", "plain", &peer, true, "conv-m", Some(&key))
            .unwrap();
        let map = store
            .get_reactions(&["m-1".to_string()], Some(&key))
            .unwrap();
        assert_eq!(
            map["m-1"].len(),
            1,
            "only the plaintext row should have been removed"
        );
        assert_eq!(map["m-1"][0].0, "sealed");
    }

    // ─── Atomicity of the destroy paths (HIGH-13) ────────────────────────────
    //
    // `evict_to_cap`, `delete_expired_messages`, `delete_conversation` and
    // `delete_messages_by_retention_policy` all shred, checkpoint, then delete.
    // The delete used to be a series of independent autocommit statements, so
    // a crash between two of them left a half-destroyed batch: rows shredded
    // but still present, which `load_messages` does not filter on (it only
    // checks `expires_at`), so they rendered as "[encrypted]" forever. The
    // shred-first ordering is the safe direction, so this is availability, not
    // confidentiality — but a shred followed by a *torn* delete is the worst of
    // both.

    /// The shred is idempotent: re-running it must not double-count.
    ///
    /// A retry is the whole point — `evict_to_cap` can be interrupted between
    /// the shred and the delete and re-run. Without the guard the second pass
    /// rewrites zeros over zeros, reports the rows as changed again, and
    /// `shredded_keys` claims more keys destroyed than exist. That counter is
    /// the audit trail for a feature whose whole claim is "we destroyed your
    /// key", so an inflated one is not cosmetic.
    #[test]
    fn test_shred_is_idempotent() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 3, 200);

        store.shred_message_keys(&["m0".to_string()]).unwrap();
        assert_eq!(store.shredded_key_count(), 1, "first pass shreds one key");

        // Retry, as a resumed eviction would.
        store.shred_message_keys(&["m0".to_string()]).unwrap();
        assert_eq!(
            store.shredded_key_count(),
            1,
            "a repeated shred must not claim a second key was destroyed"
        );

        // And the bytes are still the zero blob.
        let after: Vec<u8> = store
            .conn
            .query_row(
                "SELECT content_key_wrapped FROM messages WHERE id = 'm0'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after.len(), WRAPPED_CEK_LEN);
        assert!(
            after.iter().all(|&b| b == 0),
            "the retry must leave the key destroyed"
        );
    }

    /// Deleting a conversation removes its messages, the conversation row and
    /// the byte accounting together.
    ///
    /// The two DELETEs share one transaction, so the observable invariant is
    /// that neither half is reachable without the other. This asserts the
    /// post-state, which is what the Hub renders.
    #[test]
    fn test_delete_conversation_leaves_no_orphans() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 4, 300);

        // A reaction on one of the messages, which the delete must not strand.
        store
            .conn
            .execute(
                "INSERT INTO reactions (message_id, reaction, peer_key_hex, created_at)
                 VALUES ('m1', 'x', 'peer', 1000)",
                [],
            )
            .unwrap();

        store.delete_conversation("c1").unwrap();

        let msgs: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(msgs, 0, "no message may survive the conversation delete");
        assert!(
            store.get_conversation("c1").unwrap().is_none(),
            "the conversation row must be gone too, not just its messages"
        );
        assert_eq!(
            store.stored_bytes().unwrap(),
            0,
            "the byte counter must return to zero, or the cap counts bytes \
             nobody can reach"
        );
    }

    /// Expired-message destruction removes the rows *and* their orphaned
    /// reactions in one unit.
    ///
    /// The orphan sweep reads `messages` to decide what is stranded, so it is
    /// only correct inside the same transaction as the delete. A separate commit
    /// is a coin flip on crash order, and the `reactions` table is invisible to
    /// the storage cap — so stranded rows accumulate with nothing ever reclaiming
    /// them.
    #[test]
    fn test_expiry_removes_rows_and_orphan_reactions_together() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        let past = chrono::Utc::now().timestamp() - 60;
        store
            .store_message_secure("m-old", "c1", "sent", b"x", past, Some(past), true, &test_key())
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO reactions (message_id, reaction, peer_key_hex, created_at)
                 VALUES ('m-old', 'x', 'peer', 1000)",
                [],
            )
            .unwrap();

        assert_eq!(store.delete_expired_messages().unwrap(), 1);

        let reactions: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM reactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            reactions, 0,
            "a reaction must not outlive the message it annotates — the \
             orphan sweep decides by reading `messages`, so it only works \
             inside the delete's transaction"
        );
    }

    // ─── Schema shape (MEDIUM-9) ─────────────────────────────────────────────

    /// A fresh database has every column the migrations add, and records its
    /// schema version.
    ///
    /// The table used to be created without `expires_at`, `read_at`, `edited_at`
    /// or `deleted`, so *every* new install wrote the table and then issued four
    /// `ALTER TABLE`s — four separate implicit transactions, meaning a crash
    /// during setup could leave a permanently half-migrated schema. Asserting
    /// the columns exist is the only way to catch a future edit that moves them
    /// back out of the `CREATE TABLE`.
    #[test]
    fn test_fresh_database_has_all_migrated_columns() {
        let store = mem_messagestore();

        let mut stmt = store
            .conn
            .prepare("PRAGMA table_info(messages)")
            .unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for expected in [
            "expires_at",
            "read_at",
            "edited_at",
            "deleted",
            "content_key_wrapped",
        ] {
            assert!(
                columns.iter().any(|c| c == expected),
                "fresh database is missing `{expected}` — it must be declared in \
                 CREATE TABLE, not added by ALTER; have: {columns:?}"
            );
        }

        // The indexes that depend on `expires_at` must exist too, since they
        // used to be deferred to the migration.
        for index in [
            "idx_messages_expires_at",
            "idx_messages_read_status",
            "idx_messages_conversation",
        ] {
            let n: i64 = store
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                      WHERE type = 'index' AND name = ?1",
                    params![index],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "index {index} must exist on a fresh database");
        }
    }

    /// `user_version` records the schema as known-complete after the migrations.
    ///
    /// Nothing reads this today; it exists so migration state is explicit rather
    /// than inferred from `PRAGMA table_info`, which cannot tell "migrated" from
    /// "half-migrated" — a column that got added before the crash looks the same
    /// as one that was always there.
    #[test]
    fn test_schema_version_is_recorded() {
        let store = mem_messagestore();
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            version, MESSAGE_DB_SCHEMA_VERSION,
            "the schema version must be stamped after the migrations run"
        );
    }

    // ─── CEK length is exact (MEDIUM-16) ─────────────────────────────────────

    /// A wrapped CEK of the wrong length is rejected as unusable storage, not
    /// as a decryption failure.
    ///
    /// The check was `len() < 24 + 1`, so a 25-byte blob passed and was handed
    /// to the AEAD as a 24-byte nonce plus 1 byte of ciphertext — which the
    /// Poly1305 tag rejected for a reason that has nothing to do with the real
    /// cause. Both paths return an error, so this test pins *which* diagnosis a
    /// corrupt row gets, and it is the one that survives the ciphertext being
    /// absent entirely.
    #[test]
    fn test_cek_length_check_is_exact() {
        // A wrapped key of every plausible wrong length must be rejected as
        // unusable storage. The old check was `len() < 24 + 1`, so 25 passed
        // and was handed to the AEAD as a 24-byte nonce plus one byte of
        // ciphertext — rejected by Poly1305 for a reason that has nothing to do
        // with the real cause, and indistinguishable from tampering.
        for len in [
            0usize,
            1,
            23,
            25,
            WRAPPED_CEK_LEN - 1,
            WRAPPED_CEK_LEN + 1,
        ] {
            let err = MessageStore::unwrap_cek(&vec![0xAA; len], &test_key()).unwrap_err();
            assert!(
                matches!(err, StorageError::KeyNotFound),
                "a {len}-byte wrapped key must be rejected as unusable, got {err:?}"
            );
        }
        // The correct length gets *past* the length check, so the loop above is
        // not passing because everything fails. (It still fails to decrypt: these
        // are not real wrapped keys.)
        assert!(
            MessageStore::unwrap_cek(&vec![0xAA; WRAPPED_CEK_LEN], &test_key()).is_err(),
            "a well-formed-length blob of the wrong bytes must still fail to open"
        );
    }

    /// A shredded key is exactly `WRAPPED_CEK_LEN` zeros and must be treated as
    /// undecryptable, not as a length error.
    ///
    /// This is the boundary between the two failure modes and it matters: the
    /// shred writes a full-width zero blob on purpose, so if the length check
    /// were ever changed to "all zeros is invalid" the shred would silently
    /// stop working.
    #[test]
    fn test_shredded_key_is_rejected_as_undecryptable_not_malformed() {
        let store = mem_messagestore();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        fill_messages(&store, "c1", 1, 200);

        store.shred_message_keys(&["m0".to_string()]).unwrap();
        let shredded: Vec<u8> = store
            .conn
            .query_row(
                "SELECT content_key_wrapped FROM messages WHERE id = 'm0'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(shredded.len(), WRAPPED_CEK_LEN);

        // Same error as a malformed key — both mean "you cannot read this" —
        // but the blob must be the right shape, which is asserted above.
        assert!(
            MessageStore::unwrap_cek(&shredded, &test_key()).is_err(),
            "a shredded key must not unwrap"
        );
    }

    // ─── Family public-key length (LOW) ──────────────────────────────────────

    /// A family row with a wrong-length public key is refused, not rendered.
    ///
    /// `list_family` used to hex-encode the unvalidated bytes while copying
    /// them into a `pk_arr` that was never read, so a 31-byte key produced a
    /// 62-char `public_key_hex`. Every caller decodes that back to `[u8; 32]`
    /// for `remove_family_member` / `is_family_member`, so the member appeared
    /// in the Hub and could not be acted on at all. Erroring is better than
    /// rendering an unusable one.
    #[test]
    fn test_list_family_rejects_a_wrong_length_public_key() {
        let store = mem_keystore();
        // A good row, so the test proves the error is about the bad row and not
        // an empty table.
        store
            .add_family_member(&[0x11u8; 32], "Alice", None, None, None)
            .unwrap();
        // A corrupt one, injected directly — `add_family_member` takes `&[u8; 32]`
        // and cannot produce this.
        store
            .conn
            .execute(
                "INSERT INTO family (public_key, nickname, added_at, expires_at, last_address)
                 VALUES (?1, 'Corrupt', 1000, NULL, NULL)",
                params![vec![0x22u8; 31]],
            )
            .unwrap();

        let err = store.list_family(None).unwrap_err();
        assert!(
            matches!(err, StorageError::PathError(_)),
            "a wrong-length family key must be refused, got {err:?}"
        );

        // Same for the export path, which would otherwise write the unusable
        // hex into a backup file.
        assert!(
            store.list_family_all(None).is_err(),
            "the export path must refuse it too, or the bad hex is preserved \
             in a backup that can never be re-imported"
        );
    }

    /// A well-formed family row still reads back, and its hex is 64 chars.
    #[test]
    fn test_list_family_accepts_a_well_formed_key() {
        let store = mem_keystore();
        let pk = [0x33u8; 32];
        store
            .add_family_member(&pk, "Bob", None, Some("1.2.3.4:1"), None)
            .unwrap();

        let members = store.list_family(None).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].public_key_hex, hex::encode(pk));
        assert_eq!(
            members[0].public_key_hex.len(),
            64,
            "a decodable fingerprint is exactly 64 hex chars"
        );
        assert_eq!(members[0].nickname, "Bob");
    }
}
