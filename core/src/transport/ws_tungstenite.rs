use std::sync::{Arc, Mutex};

use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket, connect};
use tungstenite::stream::MaybeTlsStream;

use super::types::{EventStream, Request, Response, TransportError};
use super::ws_common::{frame_to_event, is_terminal_event, wrap_ws_body, ws_url};
use super::Transport;

type WsConn = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

/// Sync WebSocket transport backed by [`tungstenite`].
///
/// Uses `tokio::task::spawn_blocking` to bridge the sync WebSocket
/// into the async `Transport` trait. Connection is lazily established
/// and reused across calls.
///
/// # Message wrapping
///
/// When `message_type` is set (e.g. `"response.create"` for OpenAI),
/// outgoing bodies are wrapped: the `type` field is injected and the
/// `stream` field is removed.
pub struct WsTransport {
    conn: Arc<Mutex<Option<WsConn>>>,
    headers: Vec<(String, String)>,
    message_type: Option<String>,
}

impl WsTransport {
    pub fn new() -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
            headers: Vec::new(),
            message_type: None,
        }
    }

    /// Set headers to include on the WebSocket handshake (e.g. Authorization).
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Set the message type to inject into outgoing frames.
    ///
    /// For OpenAI's Responses API over WebSocket, use `"response.create"`.
    pub fn with_message_type(mut self, message_type: impl Into<String>) -> Self {
        self.message_type = Some(message_type.into());
        self
    }
}

impl Default for WsTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn connect_ws(
    url: &str,
    headers: &[(String, String)],
) -> Result<WsConn, TransportError> {
    let mut req = url
        .into_client_request()
        .map_err(|e| TransportError::Connection(e.to_string()))?;

    use tungstenite::http::header::{HeaderName, HeaderValue};
    for (k, v) in headers {
        req.headers_mut().insert(
            HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| TransportError::Connection(e.to_string()))?,
            HeaderValue::from_str(v)
                .map_err(|e| TransportError::Connection(e.to_string()))?,
        );
    }

    let (socket, _response) =
        connect(req).map_err(|e| TransportError::Connection(e.to_string()))?;
    Ok(socket)
}

fn ensure_connected(
    conn: &Mutex<Option<WsConn>>,
    url: &str,
    headers: &[(String, String)],
) -> Result<(), TransportError> {
    let mut guard = conn.lock().map_err(|e| TransportError::Connection(e.to_string()))?;
    if guard.is_none() {
        *guard = Some(connect_ws(url, headers)?);
    }
    Ok(())
}

fn send_and_collect(
    conn: &Mutex<Option<WsConn>>,
    text: String,
) -> Result<Response, TransportError> {
    let mut guard = conn.lock().map_err(|e| TransportError::Connection(e.to_string()))?;
    let ws = guard.as_mut().ok_or_else(|| {
        TransportError::Connection("WebSocket not connected".to_string())
    })?;

    ws.send(Message::Text(text.into()))
        .map_err(|e| TransportError::Request { status: None, message: e.to_string() })?;

    let mut last_frame = String::new();

    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let text = text.to_string();
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    if event_type == "error" {
                        let msg = v.get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("WebSocket error frame");
                        let status = v.get("status").and_then(|s| s.as_u64()).map(|s| s as u16);
                        return Err(TransportError::Request { status, message: msg.to_string() });
                    }

                    last_frame = text;

                    if is_terminal_event(event_type) {
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => { let _ = ws.send(Message::Pong(data)); }
            Ok(_) => {}
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(e) => {
                *guard = None;
                return Err(TransportError::Stream(e.to_string()));
            }
        }
    }

    Ok(Response {
        status: 200,
        headers: vec![],
        body: last_frame.into_bytes(),
    })
}

fn read_frames(
    conn: &Mutex<Option<WsConn>>,
    text: String,
) -> Result<Vec<String>, TransportError> {
    let mut guard = conn.lock().map_err(|e| TransportError::Connection(e.to_string()))?;
    let ws = guard.as_mut().ok_or_else(|| {
        TransportError::Connection("WebSocket not connected".to_string())
    })?;

    ws.send(Message::Text(text.into()))
        .map_err(|e| TransportError::Request { status: None, message: e.to_string() })?;

    let mut frames = Vec::new();
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let text = text.to_string();
                let is_done = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v.get("type")?.as_str().map(|s| is_terminal_event(s)))
                    .unwrap_or(false);
                frames.push(text);
                if is_done {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => { let _ = ws.send(Message::Pong(data)); }
            Ok(_) => {}
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(e) => {
                *guard = None;
                return Err(TransportError::Stream(e.to_string()));
            }
        }
    }
    Ok(frames)
}

impl Transport for WsTransport {
    async fn send(&self, req: Request) -> Result<Response, TransportError> {
        let url = ws_url(&req.host, &req.path);
        let mut all_headers = self.headers.clone();
        all_headers.extend(req.headers);
        ensure_connected(&self.conn, &url, &all_headers)?;

        let text = wrap_ws_body(req.body, self.message_type.as_deref())?;

        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || send_and_collect(&conn, text))
            .await
            .map_err(|e| TransportError::Connection(format!("spawn_blocking failed: {e}")))?
    }

    async fn stream(&self, req: Request) -> Result<EventStream, TransportError> {
        let url = ws_url(&req.host, &req.path);
        let mut all_headers = self.headers.clone();
        all_headers.extend(req.headers);
        ensure_connected(&self.conn, &url, &all_headers)?;

        let text = wrap_ws_body(req.body, self.message_type.as_deref())?;

        let conn = Arc::clone(&self.conn);
        let frames = tokio::task::spawn_blocking(move || read_frames(&conn, text))
            .await
            .map_err(|e| TransportError::Connection(format!("spawn_blocking failed: {e}")))??;

        let events: Vec<Result<super::types::Event, TransportError>> =
            frames.iter().map(|f| frame_to_event(f)).collect();

        Ok(Box::pin(futures::stream::iter(events)))
    }
}
