/// M2M — Storage Module
///
/// Encrypted local storage using SQLCipher (AES-256 encrypted SQLite).
/// Separate databases for keys and messages.
/// Supports secure deletion and optional history disablement.
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
    let home = dirs_next_or_fallback()?;
    Ok(home.join(DATA_DIR_NAME))
}

/// Fallback home directory resolution.
fn dirs_next_or_fallback() -> Result<PathBuf, StorageError> {
    // Use APPDATA on Windows, HOME on Unix
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
pub struct KeyStore {
    conn: Connection,
}

impl KeyStore {
    /// Open or create the key store with the given encryption key.
    /// The key should be derived from a user passphrase via Argon2id.
    pub fn open(db_path: &Path, encryption_key: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;

        // Set the encryption key for SQLCipher
        conn.pragma_update(None, "key", encryption_key)?;

        // Initialize schema
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS identity (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                public_key BLOB NOT NULL,
                encrypted_private_key BLOB NOT NULL,
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
            );",
        )?;

        Ok(Self { conn })
    }

    /// Store the identity keypair (encrypted private key).
    pub fn store_identity(
        &self,
        public_key: &[u8],
        encrypted_private_key: &[u8],
        created_at: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO identity (id, public_key, encrypted_private_key, created_at)
             VALUES (1, ?1, ?2, ?3)",
            params![public_key, encrypted_private_key, created_at],
        )?;
        Ok(())
    }

    /// Load the stored identity (public key + encrypted private key).
    pub fn load_identity(&self) -> Result<(Vec<u8>, Vec<u8>), StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT public_key, encrypted_private_key FROM identity WHERE id = 1")?;
        let result = stmt
            .query_row([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
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

    /// Mark a peer as verified.
    pub fn set_peer_verified(&self, public_key: &[u8], verified: bool) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE peers SET verified = ?1 WHERE public_key = ?2",
            params![verified as i32, public_key],
        )?;
        Ok(())
    }

    /// Check if an invite nonce has been consumed.
    pub fn is_invite_consumed(&self, nonce: &[u8]) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM consumed_invites WHERE nonce = ?1",
            params![nonce],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Mark an invite nonce as consumed.
    pub fn consume_invite(&self, nonce: &[u8]) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT OR IGNORE INTO consumed_invites (nonce, consumed_at) VALUES (?1, ?2)",
            params![nonce, now],
        )?;
        Ok(())
    }

    /// Securely delete all data and vacuum.
    pub fn secure_delete_all(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "DELETE FROM identity;
             DELETE FROM peers;
             DELETE FROM consumed_invites;
             VACUUM;",
        )?;
        Ok(())
    }
}

/// The message store: holds chat history (optional).
pub struct MessageStore {
    conn: Connection,
}

impl MessageStore {
    /// Open or create the message store with the given encryption key.
    pub fn open(db_path: &Path, encryption_key: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "key", encryption_key)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                peer_id BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                last_message_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                direction TEXT NOT NULL CHECK (direction IN ('sent', 'received')),
                content_encrypted BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                delivered INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conversation
                ON messages(conversation_id, timestamp);",
        )?;

        Ok(Self { conn })
    }

    /// Store a message.
    pub fn store_message(
        &self,
        id: &str,
        conversation_id: &str,
        direction: &str,
        content_encrypted: &[u8],
        timestamp: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, content_encrypted, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, conversation_id, direction, content_encrypted, timestamp],
        )?;
        // Update last_message_at
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
            "INSERT OR IGNORE INTO conversations (id, peer_id, created_at) VALUES (?1, ?2, ?3)",
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
            "SELECT id, direction, content_encrypted, timestamp, delivered
             FROM messages WHERE conversation_id = ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                direction: row.get(1)?,
                content_encrypted: row.get(2)?,
                timestamp: row.get(3)?,
                delivered: row.get::<_, i32>(4)? != 0,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        // Reverse to get chronological order
        messages.reverse();
        Ok(messages)
    }

    /// Delete a conversation and all its messages.
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), StorageError> {
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

    /// Delete all data and vacuum.
    pub fn secure_delete_all(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "DELETE FROM messages;
             DELETE FROM conversations;
             VACUUM;",
        )?;
        Ok(())
    }
}

/// A stored message row.
#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub direction: String,
    pub content_encrypted: Vec<u8>,
    pub timestamp: i64,
    pub delivered: bool,
}
