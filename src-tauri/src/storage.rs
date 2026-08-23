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
    std::fs::create_dir_all(&dir)
        .map_err(|e| StorageError::DirCreationFailed(e.to_string()))?;
    Ok(dir)
}

/// A family member — a peer the user has explicitly saved as a persistent contact.
/// One stored vault account (secret material still encrypted).
/// Each account's identity secret is wrapped under its OWN passphrase.
#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: i64,
    pub public_key: Vec<u8>,
    pub encrypted_private_key: Vec<u8>,
    pub private_key_nonce: Vec<u8>,
    pub label: Option<String>,
}

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

impl KeyStore {
    /// Open or create the key store.
    /// Note: the private key stored here must already be encrypted by the caller
    /// using a key derived from the user's passphrase (Argon2id + XChaCha20-Poly1305).
    pub fn open(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")?;

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
    /// Returns (public_key, encrypted_secret, nonce).
    pub fn load_x25519_key(&self) -> Result<([u8; 32], Vec<u8>, Vec<u8>), StorageError> {
        let pub_hex: String = self.conn.query_row(
            "SELECT value FROM vault_meta WHERE key = 'x25519_pub'",
            [],
            |row| row.get(0),
        ).map_err(|_| StorageError::KeyNotFound)?;
        let enc_hex: String = self.conn.query_row(
            "SELECT value FROM vault_meta WHERE key = 'x25519_enc'",
            [],
            |row| row.get(0),
        ).map_err(|_| StorageError::KeyNotFound)?;
        let nonce_hex: String = self.conn.query_row(
            "SELECT value FROM vault_meta WHERE key = 'x25519_nonce'",
            [],
            |row| row.get(0),
        ).map_err(|_| StorageError::KeyNotFound)?;

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
            .query_row(
                "SELECT public_key FROM identity WHERE id = 1",
                [],
                |row| row.get(0),
            )
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

        pub fn list_accounts(&self) -> Result<Vec<AccountRow>, StorageError> {
            let mut stmt = self.conn.prepare(
                "SELECT id, public_key, encrypted_private_key, private_key_nonce, label
                 FROM accounts ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(AccountRow {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    encrypted_private_key: row.get(2)?,
                    private_key_nonce: row.get(3)?,
                    label: row.get(4)?,
                })
            })?;
            let mut out: Vec<AccountRow> = Vec::new();
            for row in rows {
                out.push(row.map_err(StorageError::Database)?);
            }
            Ok(out)
        }

        pub fn count_accounts(&self) -> Result<i64, StorageError> {
            let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))?;
            Ok(n)
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
            params![public_key.as_slice(), nickname_stored, now, expires_at, address_stored],
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
                Err(StorageError::Database(rusqlite::Error::SqliteFailure(e, Some("peer already in family".to_string()))))
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
        let rows = stmt.query_map(params![now], |row| {
            let pk_bytes: Vec<u8> = row.get(0)?;
            let mut pk_arr = [0u8; 32];
            if pk_bytes.len() == 32 {
                pk_arr.copy_from_slice(&pk_bytes);
            }
            Ok(FamilyMember {
                public_key_hex: hex::encode(&pk_bytes),
                nickname: row.get(1)?,
                added_at: row.get(2)?,
                expires_at: row.get(3)?,
                last_address: row.get(4)?,
            })
        })?;
        let mut members = Vec::new();
        for row in rows {
            let mut m = row?;
            m.nickname = open_meta_value(key, &m.nickname, AAD_FAMILY)?;
            if let Some(addr) = &m.last_address {
                m.last_address = Some(open_meta_value(key, addr, AAD_FAMILY)?);
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
    ) -> Result<FamilyMember, StorageError> {
        let _now = chrono::Utc::now().timestamp();

        // Update the existing row's key and address
        self.conn.execute(
            "UPDATE family SET public_key = ?1, last_address = ?2 WHERE public_key = ?3",
            params![new_public_key.as_slice(), new_address, old_public_key.as_slice()],
        )?;

        // Read back the updated row
        let mut stmt = self.conn.prepare(
            "SELECT public_key, nickname, added_at, expires_at, last_address
             FROM family WHERE public_key = ?1",
        )?;
        let result = stmt.query_row(params![new_public_key.as_slice()], |row| {
            let pk_bytes: Vec<u8> = row.get(0)?;
            Ok(FamilyMember {
                public_key_hex: hex::encode(&pk_bytes),
                nickname: row.get(1)?,
                added_at: row.get(2)?,
                expires_at: row.get(3)?,
                last_address: row.get(4)?,
            })
        });
        match result {
            Ok(m) => Ok(m),
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
    pub fn list_family_all(&self) -> Result<Vec<FamilyMember>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT public_key, nickname, added_at, expires_at, last_address
             FROM family ORDER BY nickname ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let pk_bytes: Vec<u8> = row.get(0)?;
            Ok(FamilyMember {
                public_key_hex: hex::encode(&pk_bytes),
                nickname: row.get(1)?,
                added_at: row.get(2)?,
                expires_at: row.get(3)?,
                last_address: row.get(4)?,
            })
        })?;
        let mut members = Vec::new();
        for row in rows {
            members.push(row?);
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
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO family (public_key, nickname, added_at, expires_at, last_address)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![public_key, nickname, added_at, expires_at, last_address],
        )?;
        Ok(())
    }

}

/// The message store: holds chat history (optional).
/// Message contents are encrypted at the application level before storage.
pub struct MessageStore {
    conn: Connection,
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

/// AAD domain for message content ciphertext (must match commands::util::AAD_MSG_STORE).
const AAD_MSG_STORE: &[u8] = b"m2m-msg-v1";
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

/// Encrypt plaintext under `key`; returns (nonce, ciphertext).
fn seal_msg(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<(Vec<u8>, Vec<u8>), StorageError> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::gen_nonce();
    let aead_key = aead::Key::from_slice(key).ok_or(StorageError::KeyNotFound)?;
    let ct = aead::seal(plaintext, Some(aad), &nonce, &aead_key);
    Ok((nonce.0.to_vec(), ct))
}

/// Authenticate-and-decrypt; error on tag mismatch.
fn open_msg(key: &[u8; 32], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, ()> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::Nonce::from_slice(nonce).ok_or(())?;
    let aead_key = aead::Key::from_slice(key).ok_or(())?;
    aead::open(ciphertext, Some(aad), &nonce, &aead_key)
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
fn seal_meta_value(key: Option<&crate::secure_key::StorageKey>, plaintext: &str, aad: &[u8])
    -> Result<String, StorageError>
{
    let Some(k) = key else { return Ok(plaintext.to_string()) };
    let (nonce, ct) = seal_msg(k.as_bytes(), plaintext.as_bytes(), aad)?;
    Ok(format!("{}{}:{}", ENC_PREFIX, hex::encode(nonce), hex::encode(ct)))
}

/// Decrypt a metadata value written by [`seal_meta_value`].
/// Plaintext (legacy) values pass through unchanged.
fn open_meta_value(key: Option<&crate::secure_key::StorageKey>, stored: &str, aad: &[u8])
    -> Result<String, StorageError>
{
    let Some(rest) = stored.strip_prefix(ENC_PREFIX) else {
        return Ok(stored.to_string());
    };
    let k = key.ok_or(StorageError::KeyNotFound)?;
    let mut parts = rest.splitn(2, ':');
    let nonce = parts.next().unwrap_or_default();
    let ct = parts.next().unwrap_or_default();
    let nonce = hex::decode(nonce).map_err(|_| StorageError::DecryptionFailed)?;
    let ct = hex::decode(ct).map_err(|_| StorageError::DecryptionFailed)?;
    let pt = open_msg(k.as_bytes(), &nonce, &ct, aad)
        .map_err(|_| StorageError::DecryptionFailed)?;
    String::from_utf8(pt).map_err(|_| StorageError::DecryptionFailed)
}

impl MessageStore {
    /// Generate a fresh 32-byte content encryption key.
    fn generate_cek() -> [u8; 32] {
        sodiumoxide::crypto::aead::xchacha20poly1305_ietf::gen_key().0
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
    fn unwrap_cek(
        wrapped: &[u8],
        storage_key: &crate::secure_key::StorageKey,
    ) -> Result<[u8; 32], StorageError> {
        if wrapped.len() < 24 + 1 {
            return Err(StorageError::KeyNotFound);
        }
        let mut cek = [0u8; 32];
        let pt = open_msg(storage_key.as_bytes(), &wrapped[..24], &wrapped[24..], AAD_MSG_CEK)
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
            None => {
                open_msg(storage_key.as_bytes(), stored_nonce, stored_content, AAD_MSG_STORE)
                    .map_err(|_| StorageError::KeyNotFound)
            }
        }
    }
    /// Open or create the message store.
    pub fn open(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                peer_id BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                last_message_at INTEGER,
                display_name TEXT,
                peer_display_name TEXT,
                auto_delete_at INTEGER,
                retention_policy TEXT NOT NULL DEFAULT 'none'
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
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conversation
                ON messages(conversation_id, timestamp);
            -- idx_messages_expires_at and idx_messages_read_status are created
            -- in migrate_messages_table() after the expires_at column is guaranteed to exist.
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

        Ok(Self { conn })
    }

    /// Add new columns to the conversations table if they don't exist yet.
    fn migrate_conversations_table(conn: &Connection) -> Result<(), StorageError> {
        let mut stmt = conn.prepare("PRAGMA table_info(conversations)")?;
        let existing_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();

        if !existing_columns.contains(&"last_message_at".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN last_message_at INTEGER", [])?;
        }
        if !existing_columns.contains(&"display_name".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN display_name TEXT", [])?;
        }
        if !existing_columns.contains(&"peer_display_name".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN peer_display_name TEXT", [])?;
        }
        if !existing_columns.contains(&"auto_delete_at".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN auto_delete_at INTEGER", [])?;
        }
        if !existing_columns.contains(&"retention_policy".to_string()) {
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN retention_policy TEXT NOT NULL DEFAULT 'none'",
                [],
            )?;
        }
        if !existing_columns.contains(&"is_favorite".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN is_favorite INTEGER DEFAULT 0", [])?;
        }
        if !existing_columns.contains(&"archived".to_string()) {
            conn.execute("ALTER TABLE conversations ADD COLUMN archived INTEGER DEFAULT 0", [])?;
        }
        Ok(())
    }

    /// Migrate the messages table — add `read_at`, `edited_at`, `deleted`, `expires_at` columns.
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
            conn.execute("ALTER TABLE messages ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0", [])?;
        }
        if !existing_columns.contains(&"expires_at".to_string()) {
            conn.execute("ALTER TABLE messages ADD COLUMN expires_at INTEGER", [])?;
        }
        if !existing_columns.contains(&"content_key_wrapped".to_string()) {
            // Crypto-shredding (H7): per-message content key, wrapped under
            // the vault storage key. Nullable — legacy rows are encrypted
            // directly under the vault key and decrypt via fallback.
            conn.execute("ALTER TABLE messages ADD COLUMN content_key_wrapped BLOB", [])?;
        }
        // Create indexes that depend on the columns above (expires_at, read_at).
        // These are CREATE INDEX IF NOT EXISTS so they're idempotent on re-run.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_expires_at
                ON messages(expires_at);
             CREATE INDEX IF NOT EXISTS idx_messages_read_status
                ON messages(conversation_id, direction, read_at);"
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
                ON group_messages(group_id, timestamp);"
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
            self.conn.execute(
                "UPDATE conversations SET last_message_at = ?1 WHERE id = ?2",
                params![timestamp, conversation_id],
            )?;
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
        let rows = stmt.query_map(params![conversation_id, now, before_timestamp, limit], |row| {
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
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) as msg_count,
                    COALESCE(c.is_favorite, 0) as is_favorite,
                    COALESCE(c.archived, 0) as archived,
                    (SELECT COUNT(*) FROM messages m
                     WHERE m.conversation_id = c.id
                     AND m.direction = 'received' AND m.read_at IS NULL) as unread_count
             FROM conversations c
             ORDER BY archived ASC, COALESCE(c.is_favorite, 0) DESC,
                      COALESCE(c.last_message_at, c.created_at) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                created_at: row.get(2)?,
                last_message_at: row.get(3)?,
                display_name: row.get(4)?,
                peer_display_name: row.get(5)?,
                auto_delete_at: row.get(6)?,
                retention_policy: row.get::<_, Option<String>>(7)?
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
        Ok(())
    }

    /// Toggle the favorite status of a conversation. Returns the new value.
    pub fn toggle_favorite(&self, peer_key_hex: &str) -> Result<bool, StorageError> {
        // Get current value
        let current: bool = self.conn.query_row(
            "SELECT COALESCE(is_favorite, 0) FROM conversations WHERE id = ?1",
            params![peer_key_hex],
            |row| row.get(0),
        ).unwrap_or(false);
        let new_val = !current;
        self.conn.execute(
            "UPDATE conversations SET is_favorite = ?1 WHERE id = ?2",
            params![new_val as i32, peer_key_hex],
        )?;
        Ok(new_val)
    }

    /// Toggle the archive status of a conversation. Returns the new value.
    pub fn toggle_archive(&self, peer_key_hex: &str) -> Result<bool, StorageError> {
        let current: bool = self.conn.query_row(
            "SELECT COALESCE(archived, 0) FROM conversations WHERE id = ?1",
            params![peer_key_hex],
            |row| row.get(0),
        ).unwrap_or(false);
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
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) as msg_count,
                    COALESCE(c.is_favorite, 0) as is_favorite,
                    COALESCE(c.archived, 0) as archived,
                    (SELECT COUNT(*) FROM messages m
                     WHERE m.conversation_id = c.id
                     AND m.direction = 'received' AND m.read_at IS NULL) as unread_count
             FROM conversations c WHERE c.id = ?1",
        )?;
        let result = stmt.query_row(params![conversation_id], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                created_at: row.get(2)?,
                last_message_at: row.get(3)?,
                display_name: row.get(4)?,
                peer_display_name: row.get(5)?,
                auto_delete_at: row.get(6)?,
                retention_policy: row.get::<_, Option<String>>(7)?
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
    /// SQLite zero freed in-page content). A final checkpoint flushes the
    /// second round of changes.
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), StorageError> {
        self.conn.pragma_update(None, "secure_delete", "ON")?;
        self.conn.execute(
            "UPDATE messages SET content_key_wrapped = ?2 WHERE conversation_id = ?1 AND content_key_wrapped IS NOT NULL",
            params![conversation_id, vec![0u8; WRAPPED_CEK_LEN]],
        )?;
        self.wal_checkpoint_truncate()?;
        self.conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
        )?;
        self.conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![conversation_id],
        )?;
        self.wal_checkpoint_truncate()?;
        Ok(())
    }

    /// Flush and truncate the write-ahead log so previously-modified pages
    /// cannot linger in `messages.db-wal` (H7 companion to secure_delete).
    fn wal_checkpoint_truncate(&self) -> Result<(), StorageError> {
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get::<_, i64>(0))
            .map_err(StorageError::Database)?;
        Ok(())
    }

    // ─── Reactions ─────────────────────────────────────

    /// Store or remove a reaction on a message.
    ///
    /// `conversation_id` scopes the operation: the message must exist in that
    /// conversation or the call is a no-op (returns Ok(false)). This prevents
    /// a peer from reacting to messages in unrelated conversations.
    pub fn upsert_reaction(
        &self,
        message_id: &str,
        reaction: &str,
        peer_key_hex: &str,
        remove: bool,
        conversation_id: &str,
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
            self.conn.execute(
                "DELETE FROM reactions WHERE message_id = ?1 AND reaction = ?2 AND peer_key_hex = ?3",
                rusqlite::params![message_id, reaction, peer_key_hex],
            )?;
        } else {
            let now = chrono::Utc::now().timestamp();
            self.conn.execute(
                "INSERT OR IGNORE INTO reactions (message_id, reaction, peer_key_hex, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![message_id, reaction, peer_key_hex, now],
            )?;
        }
        Ok(true)
    }

    /// Get all reactions for a list of message IDs.
    /// Returns a map of message_id → Vec<(reaction, peer_key_hex, created_at)>.
    pub fn get_reactions(
        &self,
        message_ids: &[String],
    ) -> Result<ReactionsMap, StorageError> {
        if message_ids.is_empty() {
            return Ok(ReactionsMap::new());
        }
        let placeholders: Vec<String> = message_ids.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT message_id, reaction, peer_key_hex, created_at
             FROM reactions WHERE message_id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = message_ids.iter()
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
            let (msg_id, reaction, peer, ts) = row?;
            result.entry(msg_id).or_default().push((reaction, peer, ts));
        }
        Ok(result)
    }

    // ─── Read Receipts ─────────────────────────────────

    /// Mark all unread received messages as read for a conversation.
    pub fn mark_messages_read(&self, conversation_id: &str) -> Result<u32, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let count = self.conn.execute(
            "UPDATE messages SET read_at = ?1
             WHERE conversation_id = ?2 AND direction = 'received' AND read_at IS NULL",
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
                rusqlite::params![ciphertext, nonce, wrapped, now, message_id, conversation_id, expected_direction],
            )?;
            Ok(changed > 0)
        })();
        use zeroize::Zeroize;
        cek.zeroize();
        result
    }

    /// Legacy test-only edit path — content stored exactly as provided and
    /// NOT crypto-shreddable. Production code MUST use [`edit_message_secure`].
    #[cfg(test)]
    pub fn edit_message(
        &self,
        message_id: &str,
        conversation_id: &str,
        expected_direction: &str,
        new_content_encrypted: &[u8],
        new_content_nonce: &[u8],
    ) -> Result<bool, StorageError> {
        let now = chrono::Utc::now().timestamp();
        let changed = self.conn.execute(
            "UPDATE messages SET content_encrypted = ?1, content_nonce = ?2, edited_at = ?3
             WHERE id = ?4 AND conversation_id = ?5 AND direction = ?6",
            rusqlite::params![new_content_encrypted, new_content_nonce, now, message_id, conversation_id, expected_direction],
        )?;
        Ok(changed > 0)
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
            rusqlite::params![message_id, conversation_id, expected_direction, vec![0u8; WRAPPED_CEK_LEN]],
        )?;
        Ok(changed > 0)
    }

    // ─── Self-Destruct (Expired Messages) ─────────────

    /// Permanently delete expired messages from the database, with
    /// crypto-shredding (H7): shred wrapped keys first, truncate the WAL,
    /// then delete the rows.
    pub fn delete_expired_messages(&self) -> Result<u32, StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn.pragma_update(None, "secure_delete", "ON")?;
        self.conn.execute(
            "UPDATE messages SET content_key_wrapped = ?2
             WHERE expires_at IS NOT NULL AND expires_at <= ?1 AND content_key_wrapped IS NOT NULL",
            rusqlite::params![now, vec![0u8; WRAPPED_CEK_LEN]],
        )?;
        self.wal_checkpoint_truncate()?;
        let count = self.conn.execute(
            "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            rusqlite::params![now],
        )?;
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
        let members = stmt.query_map(params![group_id], |row| {
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
    pub fn load_group(
        &self,
        group_id: &str,
    ) -> Result<Option<super::group::Group>, StorageError> {
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
                let mut group = super::group::Group::new(
                    gid, name, created_at as u64, String::new(),
                );
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
    pub fn list_groups(
        &self,
    ) -> Result<Vec<super::group::GroupSummary>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT g.group_id, g.group_name, g.created_at,
                    COALESCE(g.last_message_at, 0), g.last_message_preview,
                    (SELECT COUNT(*) FROM group_members WHERE group_id = g.group_id) as member_count
             FROM groups g ORDER BY COALESCE(g.last_message_at, 0) DESC",
        )?;
        let groups = stmt.query_map([], |row| {
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
        self.conn.execute(
            "DELETE FROM groups WHERE group_id = ?1",
            params![group_id],
        )?;
        Ok(())
    }

    /// Update group metadata.
    pub fn update_group_name(
        &self,
        group_id: &str,
        new_name: &str,
    ) -> Result<(), StorageError> {
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
    pub fn edit_group_message(
        &self,
        message_id: &str,
        edited_at: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE group_messages SET edited_at = ?1 WHERE id = ?2",
            params![edited_at, message_id],
        )?;
        Ok(())
    }

    /// Soft-delete a group message.
    #[allow(dead_code)]
    pub fn delete_group_message(
        &self,
        message_id: &str,
    ) -> Result<(), StorageError> {
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
    ) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO transfers (id, peer_key_hex, filename, total_size, direction, state, chunks_total, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                state = excluded.state,
                chunks_total = excluded.chunks_total",
            params![id, peer_key_hex, filename, total_size, direction, state, chunks_total, now],
        )?;
        Ok(())
    }

    /// Update the transfer state.
    pub fn update_state(
        &self,
        transfer_id: &str,
        state: &str,
        completed_at: Option<i64>,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        match (completed_at, error) {
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
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE transfers SET local_path = ?1 WHERE id = ?2",
            params![local_path, transfer_id],
        )?;
        Ok(())
    }

    /// List all stored transfers, most recent first.
    #[cfg(test)]
    pub fn list_transfers(&self, limit: i64) -> Result<Vec<StoredTransfer>, StorageError> {
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
            transfers.push(row?);
        }
        Ok(transfers)
    }

    /// Get a single transfer by ID.
    #[cfg(test)]
    pub fn get_transfer(&self, transfer_id: &str) -> Result<Option<StoredTransfer>, StorageError> {
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
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Delete a transfer record.
    #[cfg(test)]
    pub fn delete_transfer(&self, transfer_id: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM transfers WHERE id = ?1",
            params![transfer_id],
        )?;
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

        store.store_identity(&pub_key, &enc_pk, &nonce, created).unwrap();

        let (loaded_pub, loaded_enc, loaded_nonce) = store.load_identity().unwrap();
        assert_eq!(loaded_pub, pub_key);
        assert_eq!(loaded_enc, enc_pk);
        assert_eq!(loaded_nonce, nonce);
    }

    #[test]
    fn test_store_identity_overwrite() {
        let store = mem_keystore();
        store.store_identity(&[0xAA; 32], &[0xBB; 64], &[0xCC; 24], 1000).unwrap();
        store.store_identity(&[0xDD; 32], &[0xEE; 64], &[0xFF; 24], 2000).unwrap();

        let (pub_key, enc_pk, nonce) = store.load_identity().unwrap();
        assert_eq!(pub_key, vec![0xDD; 32]);
        assert_eq!(enc_pk, vec![0xEE; 64]);
        assert_eq!(nonce, vec![0xFF; 24]);
    }

    #[test]
    fn test_store_and_load_public_key() {
        let store = mem_keystore();
        store.store_identity(&[0x11; 32], &[0x22; 64], &[0x33; 24], 1000).unwrap();

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
        store.store_identity(&[0xAA; 32], &[0xBB; 64], &[0xCC; 24], 1000).unwrap();
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
        store.upsert_peer(&[0x11; 32], "A1B2:C3D4", Some("Alice")).unwrap();

        // Verify via has_identity (peers table is separate)
        // We can't directly query, but upsert should succeed without error.
        // load_public_key still fails because identity not stored
        assert!(store.load_identity().is_err());
    }

    #[test]
    fn test_upsert_peer_update_alias() {
        let store = mem_keystore();
        store.upsert_peer(&[0x11; 32], "A1B2:C3D4", Some("Alice")).unwrap();
        // Upsert again with new alias — should update, not error
        store.upsert_peer(&[0x11; 32], "A1B2:C3D4", Some("Bob")).unwrap();
    }

    /// H5: is_known_peer must reflect the peers table — unknown keys are
    /// rejected by the contact allowlist gate, previously-connected peers pass.
    #[test]
    fn test_is_known_peer() {
        let store = mem_keystore();
        assert!(!store.is_known_peer(&[0x42; 32]).unwrap(), "empty store: peer must be unknown");

        store.upsert_peer(&[0x42; 32], "AAAA:BBBB", None).unwrap();
        assert!(store.is_known_peer(&[0x42; 32]).unwrap(), "upserted peer must be known");

        // A different key stays unknown.
        assert!(!store.is_known_peer(&[0x43; 32]).unwrap());
    }

    #[test]
    fn test_update_encrypted_private_key() {
        let store = mem_keystore();
        store.store_identity(&[0xAA; 32], &[0xBB; 64], &[0xCC; 24], 1000).unwrap();
        store.update_encrypted_private_key(&[0xDD; 64], &[0xEE; 24]).unwrap();

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
        store.store_message(
            "msg-001", conv_id, "sent", &[0x01; 32], &[0x02; 24], 1000, false,
        ).unwrap();

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
            store.store_message(
                &format!("msg-{:03}", i), "conv-001", "received",
                &[i as u8; 32], &[0xBB; 24], 1000 + i, true,
            ).unwrap();
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
            store.store_message(
                &format!("msg-{:03}", i), "conv-001", "sent",
                &[i as u8; 32], &[0xBB; 24], 1000 + i, true,
            ).unwrap();
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

        store.store_message("m1", "conv-a", "sent", &[0x01; 32], &[0x02; 24], 1000, true).unwrap();
        store.store_message("m2", "conv-a", "sent", &[0x03; 32], &[0x04; 24], 2000, true).unwrap();
        store.store_message("m3", "conv-b", "received", &[0x05; 32], &[0x06; 24], 1500, true).unwrap();

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
        store.store_message("msg-001", "conv-001", "sent", &[0x01; 32], &[0x02; 24], 1000, true).unwrap();

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
        store.store_message("m1", "conv-001", "sent", &[0x01; 32], &[0x02; 24], 1000, true).unwrap();
        store.store_message("m2", "conv-001", "received", &[0x03; 32], &[0x04; 24], 2000, true).unwrap();

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

        store.set_conversation_retention("conv-001", "auto_delete", Some(86400)).unwrap();
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
        store.store_transfer(
            "xfer-001", "alice_pk", "report.pdf", 1048576, "received", "completed", 16,
        ).unwrap();

        let saved = store.get_transfer("xfer-001").unwrap().unwrap();
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
        store.store_transfer(
            "xfer-002", "bob_pk", "photo.jpg", 524288, "sent", "transferring", 8,
        ).unwrap();

        store.update_state("xfer-002", "completed", Some(2000), None).unwrap();

        let saved = store.get_transfer("xfer-002").unwrap().unwrap();
        assert_eq!(saved.state, "completed");
        assert_eq!(saved.completed_at, Some(2000));
    }

    #[test]
    fn test_transfer_store_update_error() {
        let store = mem_transferstore();
        store.store_transfer(
            "xfer-003", "carol_pk", "archive.zip", 2097152, "sent", "transferring", 32,
        ).unwrap();

        store.update_state("xfer-003", "failed", None, Some("connection lost")).unwrap();

        let saved = store.get_transfer("xfer-003").unwrap().unwrap();
        assert_eq!(saved.state, "failed");
        assert_eq!(saved.error, Some("connection lost".to_string()));
    }

    #[test]
    fn test_transfer_store_update_progress() {
        let store = mem_transferstore();
        store.store_transfer(
            "xfer-004", "dave_pk", "video.mp4", 10485760, "sent", "transferring", 40,
        ).unwrap();

        store.update_progress("xfer-004", 15).unwrap();

        let saved = store.get_transfer("xfer-004").unwrap().unwrap();
        assert_eq!(saved.chunks_completed, 15);
    }

    #[test]
    fn test_transfer_store_set_local_path() {
        let store = mem_transferstore();
        store.store_transfer(
            "xfer-005", "eve_pk", "doc.pdf", 65536, "received", "completed", 1,
        ).unwrap();

        store.set_local_path("xfer-005", "/downloads/doc.pdf").unwrap();

        let saved = store.get_transfer("xfer-005").unwrap().unwrap();
        assert_eq!(saved.local_path, Some("/downloads/doc.pdf".to_string()));
    }

    #[test]
    fn test_transfer_store_list_limit() {
        let store = mem_transferstore();
        store.store_transfer("xf-01", "pk1", "a.txt", 100, "sent", "completed", 1).unwrap();
        store.store_transfer("xf-02", "pk2", "b.txt", 200, "received", "failed", 2).unwrap();
        store.store_transfer("xf-03", "pk3", "c.txt", 300, "sent", "transferring", 3).unwrap();

        let limited = store.list_transfers(2).unwrap();
        assert_eq!(limited.len(), 2);

        let all = store.list_transfers(10).unwrap();
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
        store.store_transfer("xf-del", "pk", "nope.txt", 100, "sent", "cancelled", 1).unwrap();
        assert!(store.get_transfer("xf-del").unwrap().is_some());

        store.delete_transfer("xf-del").unwrap();
        assert!(store.get_transfer("xf-del").unwrap().is_none());
    }

    #[test]
    fn test_transfer_store_get_not_found() {
        let store = mem_transferstore();
        let result = store.get_transfer("nonexistent").unwrap();
        assert!(result.is_none());
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
        store.store_message_secure(
            "m1", "conv-sec", "sent", b"shreddable secret", 1000, None, true, &test_key(),
        ).unwrap();

        let msgs = store.load_messages("conv-sec", 10).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].content_key_wrapped.is_some(), "secure rows must carry a wrapped CEK");

        let pt = MessageStore::decrypt_stored_content(
            &msgs[0].content_encrypted, &msgs[0].content_nonce,
            msgs[0].content_key_wrapped.as_deref(), &test_key(),
        ).unwrap();
        assert_eq!(pt, b"shreddable secret");

        // Content must NOT be decryptable directly under the vault key —
        // it is sealed under the per-message CEK only.
        assert!(crate::commands::util::crypto_decrypt_storage(
            &msgs[0].content_encrypted, &msgs[0].content_nonce, &test_key(),
            crate::commands::util::AAD_MSG_STORE,
        ).is_err(), "content must not be encrypted under the vault storage key");
    }

    /// Legacy rows (pre-crypto-shredding) have no wrapped key and were
    /// encrypted directly under the vault key — they must still decrypt.
    #[test]
    fn test_legacy_row_fallback_decrypt() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-old", &[0xBB; 32]).unwrap();

        // Old-style: caller encrypted under the vault key; no wrapped CEK.
        let (nonce, ct) = crate::commands::util::crypto_encrypt_storage(
            b"legacy plaintext", &test_key(), crate::commands::util::AAD_MSG_STORE,
        ).unwrap();
        store.store_message("m1", "conv-old", "received", &ct, &nonce, 1000, true).unwrap();

        let msgs = store.load_messages("conv-old", 10).unwrap();
        assert!(msgs[0].content_key_wrapped.is_none());

        let pt = MessageStore::decrypt_stored_content(
            &msgs[0].content_encrypted, &msgs[0].content_nonce, None, &test_key(),
        ).unwrap();
        assert_eq!(pt, b"legacy plaintext");
    }

    /// THE core H7 assertion: after a soft-delete shred, the tombstone row's
    /// ciphertext is UNRECOVERABLE even with the correct vault storage key,
    /// because its wrapped content key was destroyed.
    #[test]
    fn test_soft_delete_shreds_content_key() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-shred", &[0xCC; 32]).unwrap();
        store.store_message_secure(
            "m1", "conv-shred", "sent", b"doomed message", 1000, None, true, &test_key(),
        ).unwrap();

        // Capture the ciphertext remnants an attacker might recover from disk.
        let msgs = store.load_messages("conv-shred", 10).unwrap();
        let remnant_ct = msgs[0].content_encrypted.clone();
        let remnant_nonce = msgs[0].content_nonce.clone();

        assert!(store.delete_message("m1", "conv-shred", "sent").unwrap());

        // Tombstone remains but carries a zeroed wrapped key.
        let after = store.load_messages("conv-shred", 10).unwrap();
        assert_eq!(after.len(), 1);
        assert!(after[0].deleted);
        let wrapped = after[0].content_key_wrapped.as_deref().expect("wrapped cell present");
        assert_eq!(wrapped.len(), WRAPPED_CEK_LEN);
        assert!(wrapped.iter().all(|&b| b == 0), "wrapped CEK must be zeroed");

        // Simulated remnant attack: original ciphertext + nonce recovered
        // from disk + the CURRENT vault key → decryption MUST fail.
        assert!(MessageStore::decrypt_stored_content(
            &remnant_ct, &remnant_nonce, Some(wrapped), &test_key(),
        ).is_err());
    }

    #[test]
    fn test_edit_message_secure_rekeys_and_shreds_old() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-edit", &[0xDD; 32]).unwrap();
        store.store_message_secure(
            "m1", "conv-edit", "sent", b"original text", 1000, None, true, &test_key(),
        ).unwrap();
        let old_ct = store.load_messages("conv-edit", 10).unwrap()[0].content_encrypted.clone();

        assert!(store.edit_message_secure(
            "m1", "conv-edit", "sent", b"edited text", &test_key(),
        ).unwrap());

        let msgs = store.load_messages("conv-edit", 10).unwrap();
        assert_ne!(msgs[0].content_encrypted, old_ct, "edit must re-encrypt content");
        let pt = MessageStore::decrypt_stored_content(
            &msgs[0].content_encrypted, &msgs[0].content_nonce,
            msgs[0].content_key_wrapped.as_deref(), &test_key(),
        ).unwrap();
        assert_eq!(pt, b"edited text");
    }

    #[test]
    fn test_expired_delete_removes_only_expired() {
        let store = mem_messagestore();
        store.ensure_conversation("conv-exp", &[0xEE; 32]).unwrap();
        let past = 1000i64;
        let future = chrono::Utc::now().timestamp() + 3600;
        store.store_message_secure("m-expired", "conv-exp", "sent", b"gone", past, Some(past), true, &test_key()).unwrap();
        store.store_message_secure("m-live", "conv-exp", "sent", b"kept", past, Some(future), true, &test_key()).unwrap();

        let deleted = store.delete_expired_messages().unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.load_messages("conv-exp", 10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "m-live");
        let pt = MessageStore::decrypt_stored_content(
            &remaining[0].content_encrypted, &remaining[0].content_nonce,
            remaining[0].content_key_wrapped.as_deref(), &test_key(),
        ).unwrap();
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
            ).unwrap();
        }

        let store = MessageStore::open(&db_path).unwrap();
        store.ensure_conversation("c1", &[0x11; 32]).unwrap();
        store.store_message_secure(
            "m1", "c1", "sent", b"post-migration write", 1000, None, true, &test_key(),
        ).unwrap();
        let msgs = store.load_messages("c1", 10).unwrap();
        assert_eq!(
            MessageStore::decrypt_stored_content(
                &msgs[0].content_encrypted, &msgs[0].content_nonce,
                msgs[0].content_key_wrapped.as_deref(), &test_key(),
            ).unwrap(),
            b"post-migration write"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
