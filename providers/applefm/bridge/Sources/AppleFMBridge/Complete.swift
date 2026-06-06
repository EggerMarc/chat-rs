// One blocking turn against a stored session.
//
// The session keeps conversation history inside the OS, so each turn
// only prefills the new message — this is what makes multi-turn chat
// fast. Callers invoke this from a non-main thread (the Rust side uses
// spawn_blocking), so blocking on a semaphore here is safe.

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

func sessionCreateJSON(_ configJSON: String) -> String {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        guard let data = configJSON.data(using: .utf8),
            let config = try? JSONDecoder().decode(SessionConfig.self, from: data)
        else {
            return errorJSON("decode", "malformed session config JSON")
        }
        switch makeSession(for: config) {
        case .failure(let body):
            return encodeJSON(ErrorReply(error: body))
        case .success(let session):
            return encodeJSON(SessionCreated(session: SessionStore.shared.insert(session)))
        }
    } else {
        return errorJSON("unavailable", "macOS 26 or later is required")
    }
    #else
    return errorJSON("unavailable", "built against an SDK without the FoundationModels framework")
    #endif
}

func sessionFree(_ id: UInt64) {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        SessionStore.shared.remove(id)
    }
    #endif
}

/// Hint the OS to stage model resources so the next respond doesn't pay
/// warm-up. `id == 0` (or an unknown id) prewarms a throwaway default
/// session — asset staging is model-level, so it still helps the next
/// session created. Returns immediately; the work happens detached.
func sessionPrewarm(_ id: UInt64) {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        if let session = SessionStore.shared.get(id) {
            let box = SessionBox(session: session)
            Task.detached { box.session.prewarm() }
        } else {
            Task.detached { LanguageModelSession().prewarm() }
        }
    }
    #endif
}

func sessionRespondJSON(_ id: UInt64, _ requestJSON: String) -> String {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        guard let data = requestJSON.data(using: .utf8),
            let request = try? JSONDecoder().decode(TurnRequest.self, from: data)
        else {
            return errorJSON("decode", "malformed request JSON")
        }
        guard let session = SessionStore.shared.get(id) else {
            return errorJSON("internal", "unknown session \(id)")
        }
        return runRespond(SessionBox(session: session), request)
    } else {
        return errorJSON("unavailable", "macOS 26 or later is required")
    }
    #else
    return errorJSON("unavailable", "built against an SDK without the FoundationModels framework")
    #endif
}

#if canImport(FoundationModels)
@available(macOS 26.0, *)
private func runRespond(_ box: SessionBox, _ request: TurnRequest) -> String {
    final class Box: @unchecked Sendable { var reply = "" }
    let reply = Box()
    let semaphore = DispatchSemaphore(value: 0)

    Task.detached {
        do {
            let response = try await box.session.respond(
                to: request.message,
                options: generationOptions(request.options)
            )
            reply.reply = encodeJSON(CompleteReply(text: response.content, finish: "stop"))
        } catch {
            reply.reply = errorJSON("generation", String(describing: error))
        }
        semaphore.signal()
    }
    semaphore.wait()
    return reply.reply
}
#endif
