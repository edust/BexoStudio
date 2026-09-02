use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        normalize_access_key_id, normalize_access_key_secret, normalize_credential_ref,
        OssCredential,
    },
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Default)]
pub struct OssCredentialStore;

#[derive(Debug, Serialize, Deserialize)]
struct StoredCredential {
    access_key_id: String,
    access_key_secret: String,
}

const MAX_CREDENTIAL_PAYLOAD_BYTES: usize = 4 * 1024;

impl OssCredentialStore {
    pub fn new() -> Self {
        Self
    }

    pub fn write(&self, credential_ref: &str, credential: &OssCredential) -> AppResult<()> {
        let credential_ref = normalize_credential_ref(credential_ref.to_string())?;
        let stored = StoredCredential {
            access_key_id: normalize_access_key_id(credential.access_key_id.clone())?,
            access_key_secret: normalize_access_key_secret(credential.access_key_secret.clone())?,
        };
        let payload = serde_json::to_vec(&stored).map_err(|error| {
            AppError::new(
                "OSS_CREDENTIAL_STORE_UNAVAILABLE",
                "failed to encode OSS credential",
            )
            .with_detail("reason", error.to_string())
        })?;

        write_native_credential(&credential_ref, &payload)
    }

    pub fn read(&self, credential_ref: &str) -> AppResult<OssCredential> {
        let credential_ref = normalize_credential_ref(credential_ref.to_string())?;
        let payload = read_native_credential(&credential_ref)?;
        if payload.len() > MAX_CREDENTIAL_PAYLOAD_BYTES {
            return Err(AppError::new(
                "OSS_CREDENTIAL_INVALID",
                "stored OSS credential is too large",
            ));
        }
        let stored: StoredCredential = serde_json::from_slice(&payload).map_err(|error| {
            AppError::new(
                "OSS_CREDENTIAL_INVALID",
                "stored OSS credential is invalid or corrupted",
            )
            .with_detail("reason", error.to_string())
        })?;
        let access_key_id = normalize_access_key_id(stored.access_key_id).map_err(|_| {
            AppError::new(
                "OSS_CREDENTIAL_INVALID",
                "stored OSS credential has an invalid AccessKey ID",
            )
        })?;
        let access_key_secret =
            normalize_access_key_secret(stored.access_key_secret).map_err(|_| {
                AppError::new(
                    "OSS_CREDENTIAL_INVALID",
                    "stored OSS credential has an invalid AccessKey Secret",
                )
            })?;
        Ok(OssCredential {
            access_key_id,
            access_key_secret,
        })
    }

    pub fn delete(&self, credential_ref: &str) -> AppResult<()> {
        let credential_ref = normalize_credential_ref(credential_ref.to_string())?;
        delete_native_credential(&credential_ref)
    }
}

#[cfg(windows)]
fn write_native_credential(credential_ref: &str, payload: &[u8]) -> AppResult<()> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};

    const CRED_TYPE_GENERIC: u32 = 1;
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;

    if payload.len() > MAX_CREDENTIAL_PAYLOAD_BYTES {
        return Err(AppError::new(
            "OSS_CREDENTIAL_STORE_UNAVAILABLE",
            "OSS credential payload is too large",
        ));
    }

    #[repr(C)]
    struct FileTime {
        low_date_time: u32,
        high_date_time: u32,
    }

    #[repr(C)]
    struct CredentialAttribute {
        keyword: *mut u16,
        flags: u32,
        value_size: u32,
        value: *mut u8,
    }

    #[repr(C)]
    struct Credential {
        flags: u32,
        credential_type: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: FileTime,
        credential_blob_size: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut CredentialAttribute,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    unsafe extern "system" {
        fn CredWriteW(credential: *const Credential, flags: u32) -> i32;
    }

    let mut target_name: Vec<u16> = std::ffi::OsStr::new(credential_ref)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut user_name: Vec<u16> = std::ffi::OsStr::new("BexoStudio")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let credential = Credential {
        flags: 0,
        credential_type: CRED_TYPE_GENERIC,
        target_name: target_name.as_mut_ptr(),
        comment: null_mut(),
        last_written: FileTime {
            low_date_time: 0,
            high_date_time: 0,
        },
        credential_blob_size: u32::try_from(payload.len()).map_err(|_| {
            AppError::new(
                "OSS_CREDENTIAL_STORE_UNAVAILABLE",
                "OSS credential payload is too large",
            )
        })?,
        credential_blob: payload.as_ptr() as *mut u8,
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: null_mut(),
        target_alias: null_mut(),
        user_name: user_name.as_mut_ptr(),
    };

    let success = unsafe { CredWriteW(&credential, 0) };
    if success == 0 {
        return Err(AppError::new(
            "OSS_CREDENTIAL_STORE_UNAVAILABLE",
            "Windows Credential Manager rejected the OSS credential",
        )
        .with_detail("reason", std::io::Error::last_os_error().to_string()));
    }
    Ok(())
}

#[cfg(not(windows))]
fn write_native_credential(_credential_ref: &str, _payload: &[u8]) -> AppResult<()> {
    Err(AppError::new(
        "OSS_CREDENTIAL_STORE_UNAVAILABLE",
        "secure OSS credential storage is only implemented for Windows",
    ))
}

#[cfg(windows)]
fn read_native_credential(credential_ref: &str) -> AppResult<Vec<u8>> {
    use std::{
        os::windows::ffi::OsStrExt,
        ptr::{null_mut, slice_from_raw_parts},
    };

    const CRED_TYPE_GENERIC: u32 = 1;
    const ERROR_NOT_FOUND: i32 = 1168;

    #[repr(C)]
    struct FileTime {
        low_date_time: u32,
        high_date_time: u32,
    }

    #[repr(C)]
    struct CredentialAttribute {
        keyword: *mut u16,
        flags: u32,
        value_size: u32,
        value: *mut u8,
    }

    #[repr(C)]
    struct Credential {
        flags: u32,
        credential_type: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: FileTime,
        credential_blob_size: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut CredentialAttribute,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    unsafe extern "system" {
        fn CredReadW(
            target_name: *const u16,
            credential_type: u32,
            flags: u32,
            credential: *mut *mut Credential,
        ) -> i32;
        fn CredFree(buffer: *mut std::ffi::c_void);
    }

    let target_name: Vec<u16> = std::ffi::OsStr::new(credential_ref)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut credential_ptr: *mut Credential = null_mut();
    let success = unsafe {
        CredReadW(
            target_name.as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &mut credential_ptr,
        )
    };
    if success == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_NOT_FOUND) {
            return Err(AppError::new(
                "OSS_CREDENTIAL_NOT_FOUND",
                "OSS credential was not found in Windows Credential Manager",
            ));
        }
        return Err(AppError::new(
            "OSS_CREDENTIAL_STORE_UNAVAILABLE",
            "failed to read OSS credential from Windows Credential Manager",
        )
        .with_detail("reason", error.to_string()));
    }

    let payload = unsafe {
        if credential_ptr.is_null() {
            return Err(AppError::new(
                "OSS_CREDENTIAL_INVALID",
                "Windows Credential Manager returned an empty credential",
            ));
        }
        let credential = &*credential_ptr;
        if credential.credential_blob.is_null() {
            CredFree(credential_ptr.cast());
            return Err(AppError::new(
                "OSS_CREDENTIAL_INVALID",
                "stored OSS credential has no credential blob",
            ));
        }
        if credential.credential_blob_size as usize > MAX_CREDENTIAL_PAYLOAD_BYTES {
            CredFree(credential_ptr.cast());
            return Err(AppError::new(
                "OSS_CREDENTIAL_INVALID",
                "stored OSS credential is too large",
            ));
        }
        let bytes = slice_from_raw_parts(
            credential.credential_blob,
            credential.credential_blob_size as usize,
        );
        let value = (*bytes).to_vec();
        CredFree(credential_ptr.cast());
        value
    };
    Ok(payload)
}

#[cfg(not(windows))]
fn read_native_credential(_credential_ref: &str) -> AppResult<Vec<u8>> {
    Err(AppError::new(
        "OSS_CREDENTIAL_STORE_UNAVAILABLE",
        "secure OSS credential storage is only implemented for Windows",
    ))
}

#[cfg(windows)]
fn delete_native_credential(credential_ref: &str) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;

    const CRED_TYPE_GENERIC: u32 = 1;
    const ERROR_NOT_FOUND: i32 = 1168;

    unsafe extern "system" {
        fn CredDeleteW(target_name: *const u16, credential_type: u32, flags: u32) -> i32;
    }

    let target_name: Vec<u16> = std::ffi::OsStr::new(credential_ref)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let success = unsafe { CredDeleteW(target_name.as_ptr(), CRED_TYPE_GENERIC, 0) };
    if success == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_NOT_FOUND) {
            return Ok(());
        }
        return Err(AppError::new(
            "OSS_CREDENTIAL_STORE_UNAVAILABLE",
            "failed to delete OSS credential from Windows Credential Manager",
        )
        .with_detail("reason", error.to_string()));
    }
    Ok(())
}

#[cfg(not(windows))]
fn delete_native_credential(_credential_ref: &str) -> AppResult<()> {
    Err(AppError::new(
        "OSS_CREDENTIAL_STORE_UNAVAILABLE",
        "secure OSS credential storage is only implemented for Windows",
    ))
}
