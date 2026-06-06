// One blocking completion against the on-device model.
//
// Stateless by design: every call builds the model (base or LoRA-adapted),
// a fresh session, and renders the full conversation — correctness first;
// session reuse is a later optimization. Callers invoke this from a
// non-main thread (the Rust side uses spawn_blocking), so blocking on a
// semaphore here is safe.

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

func completeJSON(_ requestJSON: String) -> String {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        guard let data = requestJSON.data(using: .utf8),
            let request = try? JSONDecoder().decode(CompleteRequest.self, from: data)
        else {
            return errorJSON("decode", "malformed request JSON")
        }
        return runComplete(request)
    } else {
        return errorJSON("unavailable", "macOS 26 or later is required")
    }
    #else
    return errorJSON("unavailable", "built against an SDK without the FoundationModels framework")
    #endif
}

#if canImport(FoundationModels)
@available(macOS 26.0, *)
private func runComplete(_ request: CompleteRequest) -> String {
    final class Box: @unchecked Sendable { var reply = "" }
    let box = Box()
    let semaphore = DispatchSemaphore(value: 0)

    Task.detached {
        box.reply = await respond(to: request)
        semaphore.signal()
    }
    semaphore.wait()
    return box.reply
}

@available(macOS 26.0, *)
private func respond(to request: CompleteRequest) async -> String {
    let session: LanguageModelSession
    switch makeSession(for: request) {
    case .failure(let body):
        return encodeJSON(ErrorReply(error: body))
    case .success(let s):
        session = s
    }

    do {
        let response = try await session.respond(
            to: renderPrompt(request.messages),
            options: generationOptions(request.options)
        )
        return encodeJSON(CompleteReply(text: response.content, finish: "stop"))
    } catch {
        return errorJSON("generation", String(describing: error))
    }
}
#endif
