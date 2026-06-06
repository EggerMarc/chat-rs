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
    // Base model, or the LoRA-adapted variant when a .fmadapter is given.
    let model: SystemLanguageModel
    if let loraPath = request.lora {
        do {
            let adapter = try SystemLanguageModel.Adapter(
                fileURL: URL(fileURLWithPath: loraPath))
            model = SystemLanguageModel(adapter: adapter)
        } catch {
            return errorJSON(
                "adapter", "could not load .fmadapter at \(loraPath): \(error)")
        }
    } else {
        model = SystemLanguageModel.default
    }

    guard case .available = model.availability else {
        return errorJSON("unavailable", "the on-device model is not available on this machine")
    }

    let session: LanguageModelSession
    if let instructions = request.instructions {
        session = LanguageModelSession(model: model, instructions: instructions)
    } else {
        session = LanguageModelSession(model: model)
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

/// Single user turn → pass the text straight through. Multi-turn → render
/// a role-tagged transcript (v1 flattening; native Transcript
/// reconstruction arrives with tool support).
private func renderPrompt(_ messages: [WireMessage]) -> String {
    if messages.count == 1, let only = messages.first {
        return only.text
    }
    return messages.map { message in
        let tag = message.role == "assistant" ? "Assistant" : "User"
        return "\(tag): \(message.text)"
    }.joined(separator: "\n\n")
}

@available(macOS 26.0, *)
private func generationOptions(_ options: WireOptions?) -> GenerationOptions {
    guard let options else { return GenerationOptions() }
    return GenerationOptions(
        sampling: options.greedy == true ? .greedy : nil,
        temperature: options.temperature,
        maximumResponseTokens: options.max_tokens
    )
}
#endif
