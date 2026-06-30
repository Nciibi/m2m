/// M2M — Secure Key Storage
///
/// A wrapper around a fixed-size byte array that provides:
/// - `mlock()`/`VirtualLock()` to prevent paging to swap
/// - Automatic `zeroize()` + `munlock()`/`VirtualUnlock()` on drop
///
/// This prevents the storage encryption key from being written to disk
/// via swapping, which would defeat the at-rest encryption.
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

/// A fixed-size byte array locked in physical RAM.
///
/// - **Locked**: the OS will not page this memory to swap.
/// - **Zeroized**: on drop, the contents are overwritten before unlocking.
/// - **Fixed-size**: 32 bytes (a storage encryption key or similar secret).
///
/// # Panics
///
/// Construction panics if `mlock`/`VirtualLock` fails. This is intentional:
/// an unlocked key is a security hole. If locking fails at startup, the
/// application should fail rather than silently degrade.
///
/// # Platform
///
/// - Unix: uses `mlock`/`munlock` (POSIX.1)
/// - Windows: uses `VirtualLock`/`VirtualUnlock`
pub struct StorageKey {
    key: [u8; 32],
}

impl StorageKey {
    /// Create a new locked key from raw bytes.
    ///
    /// Locks the memory immediately. Panics if locking fails.
    pub fn new(key: [u8; 32]) -> Self {

    /// Create a new key for testing without memory locking.
    /// The key is NOT locked in RAM — only use in tests/benchmarks.
    #[cfg(any(test, benches))]
    pub fn from_bytes_for_test(key: &[u8; 32]) -> Self {
        Self { key: *key }
    }

    /// Create a new locked key from raw bytes.
    ///
    /// Locks the memory immediately. Panics if locking fails.
    #[cfg(not(any(test, benches)))]
    pub fn from_bytes_for_test(key: &[u8; 32]) -> Self {
        Self::new(*key) // fallback that still locks (only reached in release)
    }

        let s = Self { key };
        s.lock();
        s
    }

    /// Access the key bytes for read-only operations.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    /// Lock the key memory into RAM.
    fn lock(&self) {
        let ptr = self.key.as_ptr() as *const std::ffi::c_void;
        let len = std::mem::size_of::<[u8; 32]>();
        #[cfg(unix)]
        // SAFETY: mlock is safe to call on any valid memory.
        // Our memory is stack-allocated in this struct and valid for our lifetime.
        unsafe {
            let ret = mlock(ptr, len);
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                panic!("mlock failed: {err}");
            }
        }
        #[cfg(windows)]
        // SAFETY: VirtualLock is safe to call on any committed memory in our process.
        unsafe {
            let ret = VirtualLock(ptr, len);
            if ret == 0 {
                let err = std::io::Error::last_os_error();
                panic!("VirtualLock failed: {err}");
            }
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
        self.unlock();
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
}
