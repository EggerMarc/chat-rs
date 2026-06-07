// The C surface of the bridge.
//
// This file is the only place where Rust and Swift meet: each
// `@_cdecl` function below is callable as a plain C function from the
// crate's `src/ffi.rs`. Payloads cross the boundary as JSON strings —
// the same idea as an HTTP provider's wire format, minus the network.
//
// Memory contract: every `char*` returned here is `strdup`-allocated
// and must be released by the caller via `afm_string_free`.
//
// Threading contract: sessions are single-flight — the Rust side holds
// a lock per session across each call, so a stored session is never
// used concurrently.

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

private func cString(_ s: String) -> UnsafeMutablePointer<CChar>? {
    strdup(s)
}

private func availabilityJSON() -> String {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        switch SystemLanguageModel.default.availability {
        case .available:
            return #"{"available":true}"#
        case .unavailable(let reason):
            let why: String
            switch reason {
            case .deviceNotEligible:
                why = "this device is not eligible for Apple Intelligence"
            case .appleIntelligenceNotEnabled:
                why = "Apple Intelligence is not enabled in System Settings"
            case .modelNotReady:
                why = "model assets are not downloaded yet; retry once Apple Intelligence finishes setting up"
            @unknown default:
                why = "the model is unavailable for an unknown reason"
            }
            return #"{"available":false,"reason":"\#(why)"}"#
        @unknown default:
            return #"{"available":false,"reason":"unrecognized availability state"}"#
        }
    } else {
        return #"{"available":false,"reason":"macOS 26 or later is required"}"#
    }
    #else
    return #"{"available":false,"reason":"built against an SDK without the FoundationModels framework"}"#
    #endif
}

/// Probe whether the on-device Apple foundation model can be used.
/// Returns `{"available": bool, "reason"?: string}`.
@_cdecl("afm_availability")
public func afm_availability() -> UnsafeMutablePointer<CChar>? {
    cString(availabilityJSON())
}

/// Create a long-lived session from a `SessionConfig` JSON. Returns
/// `{"session": id}` or an `ErrorReply`. The session keeps conversation
/// history; release it with `afm_session_free`.
@_cdecl("afm_session_create")
public func afm_session_create(_ configJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let configJSON else {
        return cString(errorJSON("decode", "null config"))
    }
    return cString(sessionCreateJSON(String(cString: configJSON)))
}

/// Run one blocking turn against a stored session. Takes a `TurnRequest`
/// JSON, returns a `CompleteReply` or `ErrorReply` JSON.
@_cdecl("afm_session_respond")
public func afm_session_respond(
    _ session: UInt64, _ requestJSON: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>? {
    guard let requestJSON else {
        return cString(errorJSON("decode", "null request"))
    }
    return cString(sessionRespondJSON(session, String(cString: requestJSON)))
}

/// Run one streaming turn against a stored session. The callback is
/// invoked once per event JSON (`delta` / `done` / `error`); it may be
/// invoked from a different thread, and the event pointer is only valid
/// for the duration of each invocation (copy it out). Blocks until the
/// stream finishes.
@_cdecl("afm_session_respond_stream")
public func afm_session_respond_stream(
    _ session: UInt64,
    _ requestJSON: UnsafePointer<CChar>?,
    _ onEvent: (@convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void)?,
    _ context: UnsafeMutableRawPointer?
) {
    guard let onEvent else { return }

    struct RawContext: @unchecked Sendable {
        let pointer: UnsafeMutableRawPointer?
    }
    let rawContext = RawContext(pointer: context)
    let emit: @Sendable (String) -> Void = { event in
        event.withCString { onEvent(rawContext.pointer, $0) }
    }

    guard let requestJSON else {
        emit(encodeJSON(StreamErrorEvent(error: ErrorBody(kind: "decode", message: "null request"))))
        return
    }
    sessionStreamJSON(session, String(cString: requestJSON), emit: emit)
}

/// Release a session previously created by `afm_session_create`.
@_cdecl("afm_session_free")
public func afm_session_free(_ session: UInt64) {
    sessionFree(session)
}

/// Hint the OS to stage model resources for an upcoming turn (e.g. while
/// the user is typing). Pass 0 when no session exists yet. Returns
/// immediately; never fails.
@_cdecl("afm_prewarm")
public func afm_prewarm(_ session: UInt64) {
    sessionPrewarm(session)
}

/// Release a string previously returned by this bridge.
@_cdecl("afm_string_free")
public func afm_string_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}
