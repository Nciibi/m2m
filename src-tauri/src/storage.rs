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
        self.conn.execute_batch("VACUUM;")?;
        Ok(())
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
