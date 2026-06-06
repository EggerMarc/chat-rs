// Streaming completion against the on-device model.
//
// FoundationModels streams *cumulative snapshots* ("The", "The meeting",
// …), while the wire protocol carries *deltas*. This file diffs
// consecutive snapshots and emits delta events through the caller's
// callback, ending with a `done` event carrying the authoritative full
// text (so downstream state is correct even if a snapshot revised
// earlier output).
//
// Blocking like the complete path: the function returns only after the
// stream finishes. Events are emitted from the streaming task's thread —
// the callback must be thread-safe (the Rust side feeds a channel).

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

func streamJSON(_ requestJSON: String, emit: @escaping @Sendable (String) -> Void) {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        guard let data = requestJSON.data(using: .utf8),
            let request = try? JSONDecoder().decode(CompleteRequest.self, from: data)
        else {
            emit(encodeJSON(StreamErrorEvent(error: ErrorBody(
                kind: "decode", message: "malformed request JSON"))))
            return
        }
        runStream(request, emit: emit)
    } else {
        emit(encodeJSON(StreamErrorEvent(error: ErrorBody(
            kind: "unavailable", message: "macOS 26 or later is required"))))
    }
    #else
    emit(encodeJSON(StreamErrorEvent(error: ErrorBody(
        kind: "unavailable",
        message: "built against an SDK without the FoundationModels framework"))))
    #endif
}

#if canImport(FoundationModels)
@available(macOS 26.0, *)
private func runStream(_ request: CompleteRequest, emit: @escaping @Sendable (String) -> Void) {
    let semaphore = DispatchSemaphore(value: 0)

    Task.detached {
        defer { semaphore.signal() }

        let session: LanguageModelSession
        switch makeSession(for: request) {
        case .failure(let body):
            emit(encodeJSON(StreamErrorEvent(error: body)))
            return
        case .success(let s):
            session = s
        }

        do {
            let stream = session.streamResponse(
                to: renderPrompt(request.messages),
                options: generationOptions(request.options)
            )

            var previous = ""
            for try await snapshot in stream {
                let current: String = snapshot.content
                let delta = suffixDelta(previous: previous, current: current)
                previous = current
                if !delta.isEmpty {
                    emit(encodeJSON(StreamDeltaEvent(text: delta)))
                }
            }
            emit(encodeJSON(StreamDoneEvent(text: previous, finish: "stop")))
        } catch {
            emit(encodeJSON(StreamErrorEvent(error: ErrorBody(
                kind: "generation", message: String(describing: error)))))
        }
    }
    semaphore.wait()
}

/// Delta between cumulative snapshots. Normally `current` extends
/// `previous`; when a snapshot revises earlier text instead, fall back to
/// everything after the longest common prefix (best effort — the `done`
/// event carries the authoritative full text).
private func suffixDelta(previous: String, current: String) -> String {
    if current.hasPrefix(previous) {
        return String(current.dropFirst(previous.count))
    }
    let common = zip(previous, current).prefix { $0 == $1 }.count
    return String(current.dropFirst(common))
}
#endif
