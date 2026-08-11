//! [`bhk_core::platform::Storage`] for the T-Embed, backed by ESP-IDF's
//! NVS (non-volatile storage) partition.
//!
//! The core's `Storage` trait is an opaque key/value blob store (`get`
//! returns `Option<Vec<u8>>`, `set` takes ownership of a `Vec<u8>`) — it
//! doesn't know or care that the underlying store is NVS. This adapter
//! is the thing that does: it uses `EspNvs`'s `RawStorage` impl
//! (`embedded_svc::storage::RawStorage`, re-exported via the `nvs`
//! module) to read/write raw byte blobs under a fixed namespace.
//!
//! **Untested on hardware.** NVS is one of the more mature, heavily used
//! parts of ESP-IDF, so functional risk here is lower than the display
//! or encoder adapters, but this has not been flashed or run against a
//! real NVS partition — only compiled.

use bhk_core::platform::Storage;
use embedded_svc::storage::RawStorage;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use esp_idf_svc::sys::EspError;

/// ESP-IDF's NVS keys are capped at 15 bytes (16 with the implicit NUL
/// terminator) — this is a documented ESP-IDF constraint, not something
/// discovered on hardware. Silently truncating a longer key would risk
/// two distinct keys colliding into the same NVS entry, which is worse
/// than failing loudly, so `NvsStorage::set` rejects them instead (see
/// [`NvsStorageError::KeyTooLong`]).
const NVS_MAX_KEY_LEN: usize = 15;

/// Errors from [`NvsStorage::set`]. `Storage::get` can't report errors
/// (the core trait returns a bare `Option`, not a `Result` — frozen in
/// W1), so a failed read is logged and treated as "key absent" instead;
/// this type only covers the fallible write path.
#[derive(Debug)]
pub enum NvsStorageError {
    /// The key is longer than NVS's 15-byte limit.
    KeyTooLong,
    /// The underlying NVS write failed.
    Esp(EspError),
}

impl From<EspError> for NvsStorageError {
    fn from(e: EspError) -> Self {
        Self::Esp(e)
    }
}

/// Opaque-blob key/value storage backed by ESP-IDF's default NVS
/// partition, namespaced so this app's keys can't collide with any other
/// NVS consumer sharing the same partition.
pub struct NvsStorage {
    nvs: EspNvs<NvsDefault>,
}

impl NvsStorage {
    /// The NVS namespace this app's keys live under. NVS namespaces are
    /// also capped at 15 bytes; `"bhk"` is comfortably under that.
    const NAMESPACE: &'static str = "bhk";

    /// # Errors
    ///
    /// Returns `EspError` if the default NVS partition can't be taken
    /// (e.g. it's already taken elsewhere in the process, or the
    /// partition is corrupt and re-init fails) or the namespace can't be
    /// opened read/write.
    pub fn new(partition: EspDefaultNvsPartition) -> Result<Self, EspError> {
        let nvs = EspNvs::new(partition, Self::NAMESPACE, true)?;
        Ok(Self { nvs })
    }
}

impl Storage for NvsStorage {
    type Error = NvsStorageError;

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        if key.len() > NVS_MAX_KEY_LEN {
            log::warn!("NvsStorage::get: key {key:?} exceeds NVS's 15-byte limit, treating as absent");
            return None;
        }

        let len = match RawStorage::len(&self.nvs, key) {
            Ok(len) => len?,
            Err(e) => {
                log::warn!("NvsStorage::get({key:?}): NVS read failed: {e:?}");
                return None;
            }
        };

        let mut buf = vec![0u8; len];
        match self.nvs.get_raw(key, &mut buf) {
            Ok(Some(bytes)) => Some(bytes.to_vec()),
            Ok(None) => None,
            Err(e) => {
                log::warn!("NvsStorage::get({key:?}): NVS read failed: {e:?}");
                None
            }
        }
    }

    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), Self::Error> {
        if key.len() > NVS_MAX_KEY_LEN {
            return Err(NvsStorageError::KeyTooLong);
        }

        self.nvs.set_raw(key, &value)?;
        Ok(())
    }
}
