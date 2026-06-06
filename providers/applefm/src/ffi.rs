//! Safe wrappers over the bridge's C surface.
//!
//! When the crate is built without the bridge (non-macOS target,
//! docs.rs, or `APPLEFM_SKIP_BRIDGE=1`), the stub module below stands in
//! and reports the model as unavailable — same shape, no linking.

#[cfg(applefm_bridge)]
mod real {
    use std::ffi::{CStr, CString, c_char};

    unsafe extern "C" {
        fn afm_availability() -> *mut c_char;
        fn afm_complete(request_json: *const c_char) -> *mut c_char;
        fn afm_string_free(ptr: *mut c_char);
    }

    /// Copy a bridge-owned string out and release it. Maps a null return
    /// to `None`.
    ///
    /// SAFETY: `ptr` must come from this bridge (strdup-allocated,
    /// NUL-terminated) and must not be used after this call.
    unsafe fn take_bridge_string(ptr: *mut c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        // SAFETY: per contract above.
        unsafe {
            let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            afm_string_free(ptr);
            Some(s)
        }
    }

    pub fn availability_json() -> String {
        // SAFETY: afm_availability returns a bridge-owned string.
        unsafe { take_bridge_string(afm_availability()) }
            .unwrap_or_else(|| r#"{"available":false,"reason":"bridge returned null"}"#.to_owned())
    }

    pub fn complete_json(request: &str) -> String {
        let Ok(request) = CString::new(request) else {
            return r#"{"error":{"kind":"decode","message":"request contained a NUL byte"}}"#
                .to_owned();
        };
        // SAFETY: the pointer is valid for the duration of the call and
        // the reply is a bridge-owned string.
        unsafe { take_bridge_string(afm_complete(request.as_ptr())) }
            .unwrap_or_else(|| {
                r#"{"error":{"kind":"internal","message":"bridge returned null"}}"#.to_owned()
            })
    }
}

#[cfg(not(applefm_bridge))]
mod stub {
    const STUB_REASON: &str =
        "chat-applefm was built without the Swift bridge (non-macOS target, docs build, or APPLEFM_SKIP_BRIDGE set)";

    pub fn availability_json() -> String {
        format!(r#"{{"available":false,"reason":"{STUB_REASON}"}}"#)
    }

    pub fn complete_json(_request: &str) -> String {
        format!(r#"{{"error":{{"kind":"unavailable","message":"{STUB_REASON}"}}}}"#)
    }
}

#[cfg(applefm_bridge)]
pub(crate) use real::{availability_json, complete_json};
#[cfg(not(applefm_bridge))]
pub(crate) use stub::{availability_json, complete_json};
