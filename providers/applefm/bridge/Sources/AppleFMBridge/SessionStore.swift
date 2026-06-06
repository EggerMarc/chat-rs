// Live sessions, keyed by opaque handle.
//
// A chat-rs client holds one session for the lifetime of a conversation
// so each turn only prefills the new message — the OS session keeps the
// history. The Rust side serializes calls per session (a mutex held
// across each turn), so sessions are never used concurrently; the lock
// here only guards the table itself.

import Foundation
#if canImport(FoundationModels)
import FoundationModels

@available(macOS 26.0, *)
final class SessionStore: @unchecked Sendable {
    static let shared = SessionStore()

    private let lock = NSLock()
    private var sessions: [UInt64: LanguageModelSession] = [:]
    private var nextId: UInt64 = 1

    func insert(_ session: LanguageModelSession) -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        let id = nextId
        nextId += 1
        sessions[id] = session
        return id
    }

    func get(_ id: UInt64) -> LanguageModelSession? {
        lock.lock()
        defer { lock.unlock() }
        return sessions[id]
    }

    func remove(_ id: UInt64) {
        lock.lock()
        defer { lock.unlock() }
        sessions.removeValue(forKey: id)
    }
}

/// Allows moving a session into a detached task. Safe because the Rust
/// side guarantees one in-flight call per session.
@available(macOS 26.0, *)
struct SessionBox: @unchecked Sendable {
    let session: LanguageModelSession
}
#endif
