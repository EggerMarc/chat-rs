//! Safe wrappers over the bridge's C surface.
//!
//! When the crate is built without the bridge (non-macOS target,
//! docs.rs, or `APPLEFM_SKIP_BRIDGE=1`), the stub module below stands in
//! and reports the model as unavailable — same shape, no linking.

#[cfg(applefm_bridge)]
mod real {
    use std::ffi::{CStr, c_char};

    unsafe extern "C" {
        fn afm_availability() -> *mut c_char;
        fn afm_string_free(ptr: *mut c_char);
    }

    pub fn availability_json() -> String {
        // SAFETY: the bridge returns a NUL-terminated string allocated
        // with `strdup`; we copy it out and hand it straight back to
        // `afm_string_free`. A null return is mapped to an error value.
        unsafe {
            let ptr = afm_availability();
            if ptr.is_null() {
                return r#"{"available":false,"reason":"bridge returned null"}"#.to_owned();
            }
            let json = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            afm_string_free(ptr);
            json
        }
    }
}

#[cfg(not(applefm_bridge))]
mod stub {
    pub fn availability_json() -> String {
        r#"{"available":false,"reason":"chat-applefm was built without the Swift bridge (non-macOS target, docs build, or APPLEFM_SKIP_BRIDGE set)"}"#
            .to_owned()
    }
}

#[cfg(applefm_bridge)]
pub(crate) use real::availability_json;
#[cfg(not(applefm_bridge))]
pub(crate) use stub::availability_json;
