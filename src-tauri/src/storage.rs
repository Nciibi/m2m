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

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("storage path error: {0}")]
    PathError(String),
    #[error("key not found")]
    KeyNotFound,
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

}

/// The message store: holds chat history (optional).
/// Message contents are encrypted at the application level before storage.
pub struct MessageStore {
    conn: Connection,
}

impl MessageStore {
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
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conversation
                ON messages(conversation_id, timestamp);",
        )?;

        // Run migrations for existing databases that lack the new columns
        Self::migrate_conversations_table(&conn)?;

        Ok(Self { conn })
    }

    /// Add new columns to the conversations table if they don't exist yet.
    fn migrate_conversations_table(conn: &Connection) -> Result<(), StorageError> {
        let mut stmt = conn.prepare("PRAGMA table_info(conversations)")?;
        let existing_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();

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
        Ok(())
    }

    /// Store a message.
    pub fn store_message(
        &self,
        id: &str,
        conversation_id: &str,
        direction: &str,
        content_encrypted: &[u8],
        content_nonce: &[u8],
        timestamp: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, content_encrypted, content_nonce, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, conversation_id, direction, content_encrypted, content_nonce, timestamp],
        )?;
        self.conn.execute(
            "UPDATE conversations SET last_message_at = ?1 WHERE id = ?2",
            params![timestamp, conversation_id],
        )?;
        Ok(())
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
    pub fn load_messages(
        &self,
        conversation_id: &str,
        limit: i64,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, content_encrypted, content_nonce, timestamp
             FROM messages WHERE conversation_id = ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                direction: row.get(1)?,
                content_encrypted: row.get(2)?,
                content_nonce: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        messages.reverse();
        Ok(messages)
    }

    /// List all conversations with summary info.
    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.peer_id, c.created_at, c.last_message_at,
                    c.display_name, c.peer_display_name,
                    c.auto_delete_at, c.retention_policy,
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) as msg_count
             FROM conversations c
             ORDER BY COALESCE(c.last_message_at, c.created_at) DESC",
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
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) as msg_count
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
            })
        });
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Delete a conversation and all its messages.
    ///
    /// Uses `secure_delete` to overwrite deleted data on disk.
    /// Does NOT run `VACUUM` — that rebuilds the entire database file (O(db_size))
    /// and should only be done as a periodic maintenance task, not per-deletion.
    /// SQLite automatically marks freed pages for reuse.
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), StorageError> {
        self.conn.pragma_update(None, "secure_delete", "ON")?;
        self.conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
        )?;
        self.conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![conversation_id],
        )?;
        Ok(())
    }

    /// Load messages by direction (used for flushing pending messages on reconnect).
    pub fn load_messages_by_direction(
        &self,
        conversation_id: &str,
        direction: &str,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, content_encrypted, content_nonce, timestamp
             FROM messages WHERE conversation_id = ?1 AND direction = ?2
             ORDER BY timestamp ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id, direction], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                direction: row.get(1)?,
                content_encrypted: row.get(2)?,
                content_nonce: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    /// Update the direction field of a message (e.g., "pending" → "sent" after flush).
    pub fn update_message_direction(
        &self,
        message_id: &str,
        new_direction: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE messages SET direction = ?1 WHERE id = ?2",
            params![new_direction, message_id],
        )?;
        Ok(())
    }

    /// Store a message with direction="pending" (queued for delivery when peer comes online).
    pub fn store_pending_message(
        &self,
        id: &str,
        conversation_id: &str,
        content_encrypted: &[u8],
        content_nonce: &[u8],
        timestamp: i64,
    ) -> Result<(), StorageError> {
        self.store_message(id, conversation_id, "pending", content_encrypted, content_nonce, timestamp)
    }

}

/// A stored message row.
#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub direction: String,
    pub content_encrypted: Vec<u8>,
    pub content_nonce: Vec<u8>,
    pub timestamp: i64,
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
}

/// Summary of a stored transfer for the frontend.
#[derive(Debug, Clone)]
pub struct StoredTransfer {
    pub id: String,
    pub peer_key_hex: String,
    pub filename: String,
    pub total_size: u64,
    pub direction: String,
    pub state: String,
    pub chunks_completed: u32,
    pub chunks_total: u32,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub local_path: Option<String>,
    pub error: Option<String>,
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
            "msg-001", conv_id, "sent", &[0x01; 32], &[0x02; 24], 1000,
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
                &[i as u8; 32], &[0xBB; 24], 1000 + i,
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
                &[i as u8; 32], &[0xBB; 24], 1000 + i,
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

        store.store_message("m1", "conv-a", "sent", &[0x01; 32], &[0x02; 24], 1000).unwrap();
        store.store_message("m2", "conv-a", "sent", &[0x03; 32], &[0x04; 24], 2000).unwrap();
        store.store_message("m3", "conv-b", "received", &[0x05; 32], &[0x06; 24], 1500).unwrap();

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
        store.store_message("msg-001", "conv-001", "sent", &[0x01; 32], &[0x02; 24], 1000).unwrap();

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
        store.store_message("m1", "conv-001", "sent", &[0x01; 32], &[0x02; 24], 1000).unwrap();
        store.store_message("m2", "conv-001", "received", &[0x03; 32], &[0x04; 24], 2000).unwrap();

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
}
