//! Minimal `AppError` so the storage probe can compile `storage.rs`.
//!
//! The real type lives in `src-tauri/src/error.rs` and maps all twelve backend
//! error enums. `storage.rs` only constructs two of its shapes, so this carries
//! the two constructors it needs. The wire format (`{code, message}`) matches so
//! a divergence would be visible rather than silent.

/// Mirror of `crate::error::AppError`, limited to the constructors `storage.rs`
/// uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}
