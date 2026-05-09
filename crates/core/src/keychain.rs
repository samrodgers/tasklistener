//! Cross-platform secret storage. Tokens never touch SQLite — they go here.
//! The DB only stores a `keychain_ref` pointing back at the entry by name.
//!
//! Service convention: "TaskListener" + the provider id, account = "token".

use crate::error::{Error, Result};

const SERVICE_PREFIX: &str = "TaskListener";

pub struct Keychain;

impl Keychain {
    pub fn store(provider_id: &str, secret: &str) -> Result<String> {
        let service = service_for(provider_id);
        platform::set(&service, "token", secret)?;
        Ok(service)
    }

    pub fn get(provider_id: &str) -> Result<Option<String>> {
        let service = service_for(provider_id);
        platform::get(&service, "token")
    }

    pub fn delete(provider_id: &str) -> Result<()> {
        let service = service_for(provider_id);
        platform::delete(&service, "token")
    }

    /// Last 4 chars of the secret, for "•••• abcd" UI display.
    pub fn masked_suffix(provider_id: &str) -> Result<Option<String>> {
        Ok(Self::get(provider_id)?.map(|s| {
            let n = s.chars().count();
            if n <= 4 {
                s
            } else {
                s.chars().skip(n - 4).collect()
            }
        }))
    }
}

fn service_for(provider_id: &str) -> String {
    format!("{SERVICE_PREFIX}.{provider_id}")
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{Error, Result};
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    pub fn set(service: &str, account: &str, secret: &str) -> Result<()> {
        set_generic_password(service, account, secret.as_bytes())
            .map_err(|e| Error::Keychain(e.to_string()))
    }

    pub fn get(service: &str, account: &str) -> Result<Option<String>> {
        match get_generic_password(service, account) {
            Ok(bytes) => Ok(Some(
                String::from_utf8(bytes)
                    .map_err(|e| Error::Keychain(format!("non-utf8 secret: {e}")))?,
            )),
            Err(e) => {
                // -25300 = errSecItemNotFound
                if e.code() == -25300 {
                    Ok(None)
                } else {
                    Err(Error::Keychain(e.to_string()))
                }
            }
        }
    }

    pub fn delete(service: &str, account: &str) -> Result<()> {
        match delete_generic_password(service, account) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(Error::Keychain(e.to_string())),
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{Error, Result};
    use windows::core::PWSTR;
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn set(service: &str, _account: &str, secret: &str) -> Result<()> {
        let mut target = to_wide(service);
        let secret_bytes = secret.as_bytes();
        let mut user = to_wide("token");
        unsafe {
            let cred = CREDENTIALW {
                Flags: Default::default(),
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target.as_mut_ptr()),
                Comment: PWSTR(std::ptr::null_mut()),
                LastWritten: FILETIME::default(),
                CredentialBlobSize: secret_bytes.len() as u32,
                CredentialBlob: secret_bytes.as_ptr() as *mut u8,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                AttributeCount: 0,
                Attributes: std::ptr::null_mut(),
                TargetAlias: PWSTR(std::ptr::null_mut()),
                UserName: PWSTR(user.as_mut_ptr()),
            };
            CredWriteW(&cred, 0).map_err(|e| Error::Keychain(e.to_string()))?;
        }
        Ok(())
    }

    pub fn get(service: &str, _account: &str) -> Result<Option<String>> {
        let target = to_wide(service);
        unsafe {
            let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();
            let res = CredReadW(
                windows::core::PCWSTR(target.as_ptr()),
                CRED_TYPE_GENERIC,
                0,
                &mut cred_ptr,
            );
            if res.is_err() {
                return Ok(None);
            }
            let cred = &*cred_ptr;
            let len = cred.CredentialBlobSize as usize;
            let bytes = std::slice::from_raw_parts(cred.CredentialBlob, len).to_vec();
            CredFree(cred_ptr as *const _);
            Ok(Some(String::from_utf8(bytes).map_err(|e| {
                Error::Keychain(format!("non-utf8 secret: {e}"))
            })?))
        }
    }

    pub fn delete(service: &str, _account: &str) -> Result<()> {
        let target = to_wide(service);
        unsafe {
            let _ = CredDeleteW(
                windows::core::PCWSTR(target.as_ptr()),
                CRED_TYPE_GENERIC,
                0,
            );
        }
        Ok(())
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
mod platform {
    //! Dev / CI fallback: in-memory map. Never used in production.
    use super::Result;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::sync::OnceLock;

    fn store() -> &'static Mutex<HashMap<String, String>> {
        static S: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn set(service: &str, _account: &str, secret: &str) -> Result<()> {
        store().lock().insert(service.to_string(), secret.to_string());
        Ok(())
    }

    pub fn get(service: &str, _account: &str) -> Result<Option<String>> {
        Ok(store().lock().get(service).cloned())
    }

    pub fn delete(service: &str, _account: &str) -> Result<()> {
        store().lock().remove(service);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        // Use a unique provider id so tests don't collide on real keychains.
        let id = format!("test-{}", uuid::Uuid::new_v4());
        Keychain::store(&id, "hunter2").unwrap();
        assert_eq!(Keychain::get(&id).unwrap().as_deref(), Some("hunter2"));
        assert_eq!(
            Keychain::masked_suffix(&id).unwrap().as_deref(),
            Some("ter2")
        );
        Keychain::delete(&id).unwrap();
        assert!(Keychain::get(&id).unwrap().is_none());
    }
}
