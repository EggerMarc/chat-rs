use std::future::Future;

use super::types::{EventStream, Request, Response, TransportError};

/// A pluggable transport layer for sending requests and receiving
/// responses or event streams.
///
/// Implementations live in separate crates (e.g. `transport-reqwest`,
/// `transport-tungstenite`). Providers are generic over `T: Transport`
/// so users can swap transports without changing provider code.
///
/// # Contracts
///
/// - **`send`**: deliver a request and return a complete response.
///   HTTP transports do a POST; WebSocket transports send a frame and
///   collect response frames until the exchange is complete.
///
/// - **`stream`**: deliver a request and return a stream of
///   [`Event`](super::types::Event) tuples. HTTP transports parse SSE;
///   WebSocket transports yield individual frames.
pub trait Transport: Send + Sync {
    fn send(
        &mut self,
        req: Request,
    ) -> impl Future<Output = Result<Response, TransportError>> + Send;

    fn stream(
        &mut self,
        req: Request,
    ) -> impl Future<Output = Result<EventStream, TransportError>> + Send;
}
