/// M2M — Secure Key Storage
///
/// A wrapper around a fixed-size byte array that provides:
/// - `mlock()`/`VirtualLock()` to prevent paging to swap
/// - Automatic `zeroize()` + `munlock()`/`VirtualUnlock()` on drop
///
/// This prevents the storage encryption key from being written to disk
/// via swapping, which would defeat the at-rest encryption.

use std::sync::atomic::{AtomicBool, Ordering};

use zeroize::Zeroize;

#[cfg(unix)]
extern "C" {
    fn mlock(addr: *const std::ffi::c_void, len: usize) -> i32;
    fn munlock(addr: *const std::ffi::c_void, len: usize) -> i32;
}

#[cfg(windows)]
extern "system" {
    fn VirtualLock(lpAddress: *const std::ffi::c_void, dwSize: usize) -> i32;
    fn VirtualUnlock(lpAddress: *const std::ffi::c_void, dwSize: usize) -> i32;
}

/// A fixed-size byte array that can be pinned in physical RAM.
///
/// - **Locked**: once [`StorageKey::lock_memory`] has been called (by the owner,
///   after the value reaches its final address), the OS will not page this
///   memory to swap.
/// - **Zeroized**: on drop, the contents are overwritten, and the page range is
///   unpinned only if it was pinned.
/// - **Fixed-size**: 32 bytes (a storage encryption key or similar secret).
///
/// # Locking is explicit and best-effort
///
/// `new` does **not** lock. `mlock` works on an address, and a constructor's
/// `self` is moved to its caller, so locking there pinned a stack page that was
/// then abandoned — leaving the live key pageable while leaking
/// `RLIMIT_MEMLOCK` on every unlock. Callers install the key and then call
/// `lock_memory` (see `commands::util::mlock_storage_key`).
///
/// A failed lock only warns: refusing to release a key the user asked for is
/// worse than the loss of swap protection, and `panic = "abort"` would turn a
/// tight `RLIMIT_MEMLOCK` into a process kill.
///
/// # Platform
///
/// - Unix: uses `mlock`/`munlock` (POSIX.1)
/// - Windows: uses `VirtualLock`/`VirtualUnlock`
pub struct StorageKey {
    key: [u8; 32],
    /// Whether `lock()` succeeded on this value at this address.
    ///
    /// `Drop` must only `unlock()` a locked range: `munlock`/`VirtualUnlock` on
    /// a page range that was never pinned is undefined behaviour at the OS
    /// level and can unpin an unrelated mapping. `new` used to lock
    /// unconditionally so this was implicitly always true; now that callers
    /// lock explicitly after installation it has to be tracked.
    ///
    /// `AtomicBool`, not `Cell<bool>`: this struct lives inside
    /// `AppState.storage_key`, a `tokio::sync::RwLock`, and `RwLock<T>: Sync`
    /// requires `T: Send + Sync`. `Cell` is `Send` but **not** `Sync`, so a
    /// `Cell` field would make `AppState: !Sync` and fail
    /// `.manage(app_state)` plus every `tauri::State<'_, Arc<AppState>>`
    /// command. Relaxed ordering is sufficient — this flag only ever gates an
    /// OS call inside this one struct's own lifetime.
    locked: AtomicBool,
}

impl StorageKey {
    /// Build a `StorageKey` from raw bytes. **Does not lock.**
    ///
    /// The caller must call [`Self::lock_memory`] *after* the value has been
    /// placed at its final address. See the move-semantics caveat on
    /// [`lock_range`].
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            key,
            locked: AtomicBool::new(false),
        }
    }

    /// Pin this key's pages so the OS cannot page them to swap.
    ///
    /// Call exactly once, after the value is in its final location. Records
    /// success so `Drop` unlocks the right range. Mirrors
    /// [`crate::crypto::IdentityKeypair::lock_memory`].
    pub fn lock_memory(&self) {
        if self.locked.load(Ordering::Relaxed) {
            return; // already pinned at this address
        }
        if self.lock() {
            self.locked.store(true, Ordering::Relaxed);
        }
    }

    /// Access the key bytes for read-only operations.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    /// Lock the key memory into RAM, best-effort.
    ///
    /// Reached only via [`Self::lock_memory`], never from `new` — `mlock` pins
    /// pages *by address* and the constructor's `self` is moved to its caller,
    /// so calling it there pinned the wrong pages (see `new`).
    ///
    /// It used to `panic!` on failure, and `StorageKey::new` called it on
    /// *every* Argon2id derivation — i.e. on every vault unlock and every
    /// duress-verifier check. `RLIMIT_MEMLOCK` commonly defaults to 64–8192 KB
    /// (and is tight inside containers), so a machine could abort on unlock
    /// because swap protection was unavailable. With `panic = "abort"` that is
    /// a process kill, not a catchable error.
    ///
    /// Failing to mlock means the key *may* be paged to swap. That is a real
    /// loss of protection, but it is not a correctness failure, and refusing to
    /// release a key the user asked for is the worse outcome. The failure is
    /// logged at `warn` so it is visible rather than silent.
    ///
    /// This also makes `StorageKey` consistent with the other two mlock paths in
    /// the codebase, which were already best-effort:
    /// `secure_key::lock_range` returns `bool`, and
    /// `IdentityKeypair::lock_memory` logs a warning.
    /// Returns whether the range is now pinned.
    fn lock(&self) -> bool {
        let ptr = self.key.as_ptr() as *const std::ffi::c_void;
        let len = std::mem::size_of::<[u8; 32]>();
        #[cfg(unix)]
        // SAFETY: mlock is safe to call on any valid memory.
        // Our memory is owned by this struct and valid for its lifetime.
        unsafe {
            if mlock(ptr, len) != 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!(
                    error = %err,
                    "mlock failed — the storage key may be paged to swap. \
                     Raise RLIMIT_MEMLOCK (ulimit -l) to restore swap protection."
                );
                return false;
            }
            true
        }
        #[cfg(windows)]
        // SAFETY: VirtualLock is safe to call on any committed memory in our process.
        unsafe {
            if VirtualLock(ptr, len) == 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!(
                    error = %err,
                    "VirtualLock failed — the storage key may be paged to swap"
                );
                return false;
            }
            true
        }
        #[cfg(not(any(unix, windows)))]
        compile_error!("unsupported platform — StorageKey needs mlock or VirtualLock");
    }

    /// Unlock the key memory from RAM.
    fn unlock(&self) {
        let ptr = self.key.as_ptr() as *const std::ffi::c_void;
        let len = std::mem::size_of::<[u8; 32]>();
        #[cfg(unix)]
        // SAFETY: munlock is safe on memory previously locked with mlock.
        unsafe {
            let _ = munlock(ptr, len); // best-effort on drop
        }
        #[cfg(windows)]
        // SAFETY: VirtualUnlock is safe on memory previously locked with VirtualLock.
        unsafe {
            let _ = VirtualUnlock(ptr, len); // best-effort on drop
        }
    }
}

impl Drop for StorageKey {
    fn drop(&mut self) {
        // Zeroize before unlocking: ensure key material is gone if unlocking fails
        self.key.zeroize();
        // Only unpin what was actually pinned. Unlocking a range that was never
        // locked is undefined behaviour at the OS level and can unpin an
        // unrelated mapping — and with `new` no longer locking, an unlocked
        // `StorageKey` is the common case for tests and for any key that was
        // dropped before its installation completed.
        if self.locked.load(Ordering::Relaxed) {
            self.unlock();
        }
    }
}

/// Lock an arbitrary fixed-size secret buffer into physical RAM
/// (mlock/VirtualLock). Best-effort: returns false instead of panicking so
/// callers can degrade gracefully for non-critical secrets.
///
/// # Move semantics caveat
/// Rust moves invalidate addresses — callers MUST lock only AFTER placing
/// the secret at its final stable location (e.g., inside a heap-backed
/// RwLock) and unlock BEFORE removing it.
pub fn lock_range(ptr: *const std::ffi::c_void, len: usize) -> bool {
    #[cfg(unix)]
    unsafe {
        mlock(ptr, len) == 0
    }
    #[cfg(windows)]
    unsafe {
        VirtualLock(ptr, len) != 0
    }
}

/// Inverse of [`lock_range`]. Best-effort; failures are non-fatal.
pub fn unlock_range(ptr: *const std::ffi::c_void, len: usize) {
    #[cfg(unix)]
    unsafe {
        let _ = munlock(ptr, len);
    }
    #[cfg(windows)]
    unsafe {
        let _ = VirtualUnlock(ptr, len);
    }
}

impl std::fmt::Debug for StorageKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StorageKey").field(&"[redacted]").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_key_creation_and_clone() {
        let key = [0xABu8; 32];
        let sk = StorageKey::new(key);
        assert_eq!(sk.as_bytes(), &key);
    }

    #[test]
    fn test_storage_key_debug_redacted() {
        let sk = StorageKey::new([0xAB; 32]);
        let dbg = format!("{:?}", sk);
        assert!(dbg.contains("[redacted]"));
        assert!(!dbg.contains("ab"));
    }

    #[test]
    fn test_storage_key_drop_zeroizes() {
        let key = [0xCDu8; 32];
        {
            let sk = StorageKey::new(key);
            assert_eq!(sk.as_bytes(), &key);
        }
        // key was moved into StorageKey, then zeroized on drop.
        // key still holds the original value (it was copied).
        // This test confirms the Drop doesn't panic.
        assert_eq!(key, [0xCDu8; 32]);
    }

    #[test]
    fn test_new_does_not_lock_and_drop_of_unlocked_does_not_unlock() {
        // Regression: `new` used to `mlock` its own stack local, which was then
        // moved. Two failures followed — the live key stayed pageable, and the
        // abandoned page stayed pinned forever, leaking `RLIMIT_MEMLOCK` until
        // every later `mlock` failed. The Drop side matters too: unlocking a
        // range that was never pinned is undefined behaviour at the OS level.
        let sk = StorageKey::new([0x11; 32]);
        assert!(
            !sk.locked.load(Ordering::Relaxed),
            "StorageKey::new must not pin anything; the caller pins after install"
        );
        // Dropping must not attempt to unpin. If this regressed it would be
        // undefined behaviour rather than a clean test failure, so the assertion
        // above is the real guard; this line just exercises the path.
        drop(sk);
    }

    #[test]
    fn test_lock_memory_is_idempotent() {
        // Calling it twice must not double-pin (which would need two unlocks) and
        // must not fail. Best-effort: a locked `RLIMIT_MEMLOCK` makes `lock()`
        // return false, and the test must still pass.
        let sk = StorageKey::new([0x22; 32]);
        sk.lock_memory();
        let after_first = sk.locked.load(Ordering::Relaxed);
        sk.lock_memory();
        assert_eq!(
            sk.locked.load(Ordering::Relaxed),
            after_first,
            "a second lock_memory must be a no-op, not a second pin"
        );
    }
}
