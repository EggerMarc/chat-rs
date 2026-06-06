//! Safe wrappers over the bridge's C surface.
//!
//! When the crate is built without the bridge (non-macOS target,
//! docs.rs, or `APPLEFM_SKIP_BRIDGE=1`), the stub module below stands in
//! and reports the model as unavailable — same shape, no linking.

#[cfg(applefm_bridge)]
mod real {
    use std::ffi::{CStr, CString, c_char, c_void};

    unsafe extern "C" {
        fn afm_availability() -> *mut c_char;
        fn afm_complete(request_json: *const c_char) -> *mut c_char;
        fn afm_respond_stream(
            request_json: *const c_char,
            on_event: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
            context: *mut c_void,
        );
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
        unsafe { take_bridge_string(afm_complete(request.as_ptr())) }.unwrap_or_else(|| {
            r#"{"error":{"kind":"internal","message":"bridge returned null"}}"#.to_owned()
        })
    }

    /// Run one streaming completion, invoking `on_event` once per event
    /// JSON. Blocks until the stream finishes — call from a worker thread.
    /// `on_event` may be invoked from a different thread than the caller's.
    pub fn stream_json(request: &str, on_event: impl FnMut(&str) + Send) {
        let mut on_event: Box<dyn FnMut(&str) + Send> = Box::new(on_event);
        let Ok(request) = CString::new(request) else {
            on_event(
                r#"{"type":"error","error":{"kind":"decode","message":"request contained a NUL byte"}}"#,
            );
            return;
        };

        unsafe extern "C" fn trampoline(context: *mut c_void, event: *const c_char) {
            if context.is_null() || event.is_null() {
                return;
            }
            // SAFETY: `context` is the `&mut Box<dyn FnMut>` passed below,
            // alive for the whole (blocking) `afm_respond_stream` call;
            // `event` is a NUL-terminated string valid for this invocation.
            unsafe {
                let on_event = &mut *context.cast::<Box<dyn FnMut(&str) + Send>>();
                let event = CStr::from_ptr(event).to_string_lossy();
                on_event(&event);
            }
        }

        let context = (&raw mut on_event).cast::<c_void>();
        // SAFETY: the bridge blocks until the stream completes, so the
        // closure outlives every trampoline invocation.
        unsafe { afm_respond_stream(request.as_ptr(), Some(trampoline), context) }
    }
}

#[cfg(not(applefm_bridge))]
mod stub {
    const STUB_REASON: &str = "chat-applefm was built without the Swift bridge (non-macOS target, docs build, or APPLEFM_SKIP_BRIDGE set)";

    pub fn availability_json() -> String {
        format!(r#"{{"available":false,"reason":"{STUB_REASON}"}}"#)
    }

    pub fn complete_json(_request: &str) -> String {
        format!(r#"{{"error":{{"kind":"unavailable","message":"{STUB_REASON}"}}}}"#)
    }

    pub fn stream_json(_request: &str, mut on_event: impl FnMut(&str) + Send) {
        on_event(&format!(
            r#"{{"type":"error","error":{{"kind":"unavailable","message":"{STUB_REASON}"}}}}"#
        ));
    }
}

#[cfg(applefm_bridge)]
pub(crate) use real::{availability_json, complete_json, stream_json};
#[cfg(not(applefm_bridge))]
pub(crate) use stub::{availability_json, complete_json, stream_json};
