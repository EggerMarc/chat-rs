use futures::stream::BoxStream;

/// An outbound request to be sent by a transport.
///
/// Providers build this from their wire-format types; the transport
/// decides *how* to deliver it (HTTP POST, WebSocket frame, gRPC call, …).
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// The response returned by a transport for a unary (non-streaming) call.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// A normalized streaming event: `(event_type, data)`.
///
/// - **HTTP/SSE transports** parse `event:` / `data:` lines and yield these.
/// - **WebSocket transports** extract the `type` field from each JSON frame
///   and yield the full frame JSON as `data`.
///
/// Providers consume these uniformly regardless of the underlying protocol.
pub type Event = (String, String);

/// A boxed stream of transport events, used by [`Transport::stream`].
pub type EventStream = BoxStream<'static, Result<Event, TransportError>>;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("connection error: {0}")]
    Connection(String),

    #[error("request error: {message}")]
    Request {
        /// HTTP status code, when available.
        status: Option<u16>,
        message: String,
    },

    #[error("stream error: {0}")]
    Stream(String),
}
