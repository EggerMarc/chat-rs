// Shared session plumbing for the complete and stream paths: model
// construction (base or LoRA-adapted), availability gating, prompt
// rendering, and generation options.

import Foundation
#if canImport(FoundationModels)
import FoundationModels

/// Build the model (applying a `.fmadapter` LoRA when requested) and a
/// session over it. Errors come back as an `ErrorBody` so each caller
/// can wrap them in its own reply shape.
@available(macOS 26.0, *)
func makeSession(for request: CompleteRequest) -> Result<LanguageModelSession, ErrorBody> {
    let model: SystemLanguageModel
    if let loraPath = request.lora {
        do {
            let adapter = try SystemLanguageModel.Adapter(
                fileURL: URL(fileURLWithPath: loraPath))
            model = SystemLanguageModel(adapter: adapter)
        } catch {
            return .failure(
                ErrorBody(
                    kind: "adapter",
                    message: "could not load .fmadapter at \(loraPath): \(error)"))
        }
    } else {
        model = SystemLanguageModel.default
    }

    guard case .available = model.availability else {
        return .failure(
            ErrorBody(
                kind: "unavailable",
                message: "the on-device model is not available on this machine"))
    }

    if let instructions = request.instructions {
        return .success(LanguageModelSession(model: model, instructions: instructions))
    }
    return .success(LanguageModelSession(model: model))
}

/// Single user turn → pass the text straight through. Multi-turn → render
/// a role-tagged transcript (v1 flattening; native Transcript
/// reconstruction arrives with tool support).
func renderPrompt(_ messages: [WireMessage]) -> String {
    if messages.count == 1, let only = messages.first {
        return only.text
    }
    return messages.map { message in
        let tag = message.role == "assistant" ? "Assistant" : "User"
        return "\(tag): \(message.text)"
    }.joined(separator: "\n\n")
}

@available(macOS 26.0, *)
func generationOptions(_ options: WireOptions?) -> GenerationOptions {
    guard let options else { return GenerationOptions() }

    var sampling: GenerationOptions.SamplingMode?
    if options.greedy == true {
        sampling = .greedy
    } else if let k = options.top_k {
        sampling = .random(top: k, seed: options.seed)
    } else if let p = options.top_p {
        sampling = .random(probabilityThreshold: p, seed: options.seed)
    }

    return GenerationOptions(
        sampling: sampling,
        temperature: options.temperature,
        maximumResponseTokens: options.max_tokens
    )
}
#endif
