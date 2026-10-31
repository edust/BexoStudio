//! Keychain Services bindings verified against the macOS SDK headers.
//! Secret bytes never enter a command line, log, or temporary file.
use crate::error::{AppError, AppResult};
use std::{
    ffi::{c_char, c_void},
    ptr,
};
const SERVICE: &[u8] = b"studio.bexo.desktop.oss";
const NOT_FOUND: i32 = -25300;
const DUPLICATE: i32 = -25299;
type Item = *mut c_void;
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainFindGenericPassword(
        keychain: Item,
        service_len: u32,
        service: *const c_char,
        account_len: u32,
        account: *const c_char,
        len: *mut u32,
        data: *mut *mut c_void,
        item: *mut Item,
    ) -> i32;
    fn SecKeychainAddGenericPassword(
        keychain: Item,
        service_len: u32,
        service: *const c_char,
        account_len: u32,
        account: *const c_char,
        len: u32,
        data: *const c_void,
        item: *mut Item,
    ) -> i32;
    fn SecKeychainItemModifyAttributesAndData(
        item: Item,
        attrs: *const c_void,
        len: u32,
        data: *const c_void,
    ) -> i32;
    fn SecKeychainItemFreeContent(attrs: *mut c_void, data: *mut c_void) -> i32;
    fn SecKeychainItemDelete(item: Item) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(item: *const c_void);
}
struct OwnedItem(Item);
impl Drop for OwnedItem {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}
fn check(status: i32) -> AppResult<()> {
    if status == 0 {
        return Ok(());
    }
    Err(AppError::new(
        if status == NOT_FOUND {
            "OSS_CREDENTIAL_NOT_FOUND"
        } else {
            "OSS_CREDENTIAL_STORE_UNAVAILABLE"
        },
        "无法访问 macOS 钥匙串凭据，请检查钥匙串是否解锁及应用访问授权",
    )
    .with_detail("osStatus", status.to_string()))
}
fn find(account: &str, len: *mut u32, data: *mut *mut c_void, item: *mut Item) -> i32 {
    unsafe {
        SecKeychainFindGenericPassword(
            ptr::null_mut(),
            SERVICE.len() as u32,
            SERVICE.as_ptr().cast(),
            account.len() as u32,
            account.as_ptr().cast(),
            len,
            data,
            item,
        )
    }
}
pub(super) fn write_native_credential(account: &str, payload: &[u8]) -> AppResult<()> {
    if payload.len() > 4096 {
        return Err(AppError::new(
            "OSS_CREDENTIAL_INVALID",
            "OSS credential payload is too large",
        ));
    }
    let status = unsafe {
        SecKeychainAddGenericPassword(
            ptr::null_mut(),
            SERVICE.len() as u32,
            SERVICE.as_ptr().cast(),
            account.len() as u32,
            account.as_ptr().cast(),
            payload.len() as u32,
            payload.as_ptr().cast(),
            ptr::null_mut(),
        )
    };
    if status != DUPLICATE {
        return check(status);
    }
    let mut item = OwnedItem(ptr::null_mut());
    check(find(account, ptr::null_mut(), ptr::null_mut(), &mut item.0))?;
    check(unsafe {
        SecKeychainItemModifyAttributesAndData(
            item.0,
            ptr::null(),
            payload.len() as u32,
            payload.as_ptr().cast(),
        )
    })
}
pub(super) fn read_native_credential(account: &str) -> AppResult<Vec<u8>> {
    let mut len = 0;
    let mut data = ptr::null_mut();
    check(find(account, &mut len, &mut data, ptr::null_mut()))?;
    let result = if len > 4096 || (len > 0 && data.is_null()) {
        Err(AppError::new(
            "OSS_CREDENTIAL_INVALID",
            "stored OSS credential is invalid or too large",
        ))
    } else if len == 0 {
        Ok(Vec::new())
    } else {
        Ok(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len as usize) }.to_vec())
    };
    let cleanup = unsafe { SecKeychainItemFreeContent(ptr::null_mut(), data) };
    check(cleanup)?;
    result
}
pub(super) fn delete_native_credential(account: &str) -> AppResult<()> {
    let mut item = OwnedItem(ptr::null_mut());
    let status = find(account, ptr::null_mut(), ptr::null_mut(), &mut item.0);
    if status == NOT_FOUND {
        return Ok(());
    }
    check(status)?;
    check(unsafe { SecKeychainItemDelete(item.0) })
}
