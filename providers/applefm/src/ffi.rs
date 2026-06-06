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
        fn afm_session_create(config_json: *const c_char) -> *mut c_char;
        fn afm_session_respond(session: u64, request_json: *const c_char) -> *mut c_char;
        fn afm_session_respond_stream(
            session: u64,
            request_json: *const c_char,
            on_event: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
            context: *mut c_void,
        );
        fn afm_session_free(session: u64);
        fn afm_prewarm(session: u64);
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

    fn null_reply() -> String {
        r#"{"error":{"kind":"internal","message":"bridge returned null"}}"#.to_owned()
    }

    fn nul_byte_reply() -> String {
        r#"{"error":{"kind":"decode","message":"request contained a NUL byte"}}"#.to_owned()
    }

    pub fn availability_json() -> String {
        // SAFETY: afm_availability returns a bridge-owned string.
        unsafe { take_bridge_string(afm_availability()) }
            .unwrap_or_else(|| r#"{"available":false,"reason":"bridge returned null"}"#.to_owned())
    }

    /// Create a long-lived session; returns `{"session": id}` or an
    /// error reply. Free with [`session_free`].
    pub fn session_create(config_json: &str) -> String {
        let Ok(config) = CString::new(config_json) else {
            return nul_byte_reply();
        };
        // SAFETY: valid pointer for the call; bridge-owned reply.
        unsafe { take_bridge_string(afm_session_create(config.as_ptr())) }
            .unwrap_or_else(null_reply)
    }

    /// One blocking turn against a stored session.
    pub fn session_respond(session: u64, request_json: &str) -> String {
        let Ok(request) = CString::new(request_json) else {
            return nul_byte_reply();
        };
        // SAFETY: valid pointer for the call; bridge-owned reply.
        unsafe { take_bridge_string(afm_session_respond(session, request.as_ptr())) }
            .unwrap_or_else(null_reply)
    }

    /// One streaming turn against a stored session, invoking `on_event`
    /// once per event JSON. Blocks until the stream finishes — call from
    /// a worker thread. `on_event` may be invoked from another thread.
    pub fn session_stream(session: u64, request_json: &str, on_event: impl FnMut(&str) + Send) {
        let mut on_event: Box<dyn FnMut(&str) + Send> = Box::new(on_event);
        let Ok(request) = CString::new(request_json) else {
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
            // alive for the whole (blocking) call; `event` is a
            // NUL-terminated string valid for this invocation.
            unsafe {
                let on_event = &mut *context.cast::<Box<dyn FnMut(&str) + Send>>();
                let event = CStr::from_ptr(event).to_string_lossy();
                on_event(&event);
            }
        }

        let context = (&raw mut on_event).cast::<c_void>();
        // SAFETY: the bridge blocks until the stream completes, so the
        // closure outlives every trampoline invocation.
        unsafe { afm_session_respond_stream(session, request.as_ptr(), Some(trampoline), context) }
    }

    pub fn session_free(session: u64) {
        // SAFETY: trivially safe; unknown ids are ignored bridge-side.
        unsafe { afm_session_free(session) }
    }

    /// Hint the OS to stage model resources (0 = no session yet).
    /// Returns immediately; the bridge detaches the actual work.
    pub fn prewarm(session: u64) {
        // SAFETY: trivially safe; unknown ids prewarm a default session.
        unsafe { afm_prewarm(session) }
    }
}

#[cfg(not(applefm_bridge))]
mod stub {
    const STUB_REASON: &str = "chat-applefm was built without the Swift bridge (non-macOS target, docs build, or APPLEFM_SKIP_BRIDGE set)";

    pub fn availability_json() -> String {
        format!(r#"{{"available":false,"reason":"{STUB_REASON}"}}"#)
    }

    pub fn session_create(_config_json: &str) -> String {
        format!(r#"{{"error":{{"kind":"unavailable","message":"{STUB_REASON}"}}}}"#)
    }

    pub fn session_respond(_session: u64, _request_json: &str) -> String {
        format!(r#"{{"error":{{"kind":"unavailable","message":"{STUB_REASON}"}}}}"#)
    }

    pub fn session_stream(
        _session: u64,
        _request_json: &str,
        mut on_event: impl FnMut(&str) + Send,
    ) {
        on_event(&format!(
            r#"{{"type":"error","error":{{"kind":"unavailable","message":"{STUB_REASON}"}}}}"#
        ));
    }

    pub fn session_free(_session: u64) {}

    pub fn prewarm(_session: u64) {}
}

#[cfg(applefm_bridge)]
pub(crate) use real::{
    availability_json, prewarm, session_create, session_free, session_respond, session_stream,
};
#[cfg(not(applefm_bridge))]
pub(crate) use stub::{
    availability_json, prewarm, session_create, session_free, session_respond, session_stream,
};
