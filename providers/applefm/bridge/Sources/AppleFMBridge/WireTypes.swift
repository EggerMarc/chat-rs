// Codable mirrors of the JSON wire protocol defined in
// `src/api/types/mod.rs`. Keep the two in sync — this boundary is the
// provider's equivalent of an HTTP provider's wire format.

import Foundation

struct WireOptions: Codable, Sendable {
    var temperature: Double?
    var max_tokens: Int?
    // Sampling mode, flattened. Picked as greedy > top_k > top_p.
    var greedy: Bool?
    var top_k: Int?
    var top_p: Double?
    var seed: UInt64?
}

struct WireMessage: Codable, Sendable {
    /// "user" | "assistant"
    var role: String
    var text: String
}

struct CompleteRequest: Codable, Sendable {
    var instructions: String?
    /// Filesystem path to a `.fmadapter` package (a trained LoRA for the
    /// on-device base model). Absent → the plain base model.
    var lora: String?
    var messages: [WireMessage]
    var options: WireOptions?
}

struct CompleteReply: Codable, Sendable {
    var text: String
    /// "stop" | "max_tokens"
    var finish: String
}

struct ErrorBody: Codable, Sendable, Error {
    var kind: String
    var message: String
}

struct ErrorReply: Codable, Sendable {
    var error: ErrorBody
}

// Stream events, discriminated by `type`.

struct StreamDeltaEvent: Codable, Sendable {
    var type = "delta"
    var text: String
}

struct StreamDoneEvent: Codable, Sendable {
    var type = "done"
    var text: String
    var finish: String
}

struct StreamErrorEvent: Codable, Sendable {
    var type = "error"
    var error: ErrorBody
}

func encodeJSON<T: Encodable>(_ value: T) -> String {
    guard let data = try? JSONEncoder().encode(value),
        let s = String(data: data, encoding: .utf8)
    else {
        return #"{"error":{"kind":"internal","message":"reply encoding failed"}}"#
    }
    return s
}

func errorJSON(_ kind: String, _ message: String) -> String {
    encodeJSON(ErrorReply(error: ErrorBody(kind: kind, message: message)))
}
