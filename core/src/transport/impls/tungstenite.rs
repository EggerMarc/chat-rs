use std::sync::{Arc, Mutex};

use futures::StreamExt;
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket, connect};
use tungstenite::stream::MaybeTlsStream;

use crate::transport::{EventStream, Request, Response, Transport, TransportError};
use super::common::{frame_to_event, is_terminal_event, wrap_ws_body, ws_url};

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

async fn ensure_connected(
    conn: &Arc<Mutex<Option<WsConn>>>,
    url: &str,
    headers: &[(String, String)],
) -> Result<(), TransportError> {
    {
        let guard = conn.lock().map_err(|e| TransportError::Connection(e.to_string()))?;
        if guard.is_some() {
            return Ok(());
        }
    }

    let url = url.to_string();
    let headers = headers.to_vec();
    let ws = tokio::task::spawn_blocking(move || connect_ws(&url, &headers))
        .await
        .map_err(|e| TransportError::Connection(format!("spawn_blocking failed: {e}")))?
        ?;

    let mut guard = conn.lock().map_err(|e| TransportError::Connection(e.to_string()))?;
    *guard = Some(ws);
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

    #[allow(unused_assignments)]
    let mut last_frame = None::<String>;

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

                    last_frame = Some(text);

                    if is_terminal_event(event_type) {
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => {
                *guard = None;
                return Err(TransportError::Stream(
                    "WebSocket closed before terminal event".to_string(),
                ));
            }
            Ok(Message::Ping(data)) => { let _ = ws.send(Message::Pong(data)); }
            Ok(_) => {}
            Err(e) => {
                *guard = None;
                return Err(TransportError::Stream(e.to_string()));
            }
        }
    }

    Ok(Response {
        status: 200,
        headers: vec![],
        body: last_frame.unwrap_or_default().into_bytes(),
    })
}

/// Blocking reader that sends each frame through a channel as it arrives.
/// Runs inside `spawn_blocking`.
fn stream_frames(
    conn: &Mutex<Option<WsConn>>,
    text: String,
    tx: tokio::sync::mpsc::UnboundedSender<Result<String, TransportError>>,
) {
    let send_err = |e: TransportError| { let _ = tx.send(Err(e)); };

    let mut guard = match conn.lock() {
        Ok(g) => g,
        Err(e) => { send_err(TransportError::Connection(e.to_string())); return; }
    };
    let ws = match guard.as_mut() {
        Some(ws) => ws,
        None => { send_err(TransportError::Connection("WebSocket not connected".to_string())); return; }
    };

    if let Err(e) = ws.send(Message::Text(text.into())) {
        send_err(TransportError::Request { status: None, message: e.to_string() });
        return;
    }

    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let text = text.to_string();
                let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
                let event_type = parsed.as_ref()
                    .and_then(|v| v.get("type")?.as_str())
                    .unwrap_or("");

                if event_type == "error" {
                    let status = parsed.as_ref().and_then(|v| v.get("status")?.as_u64()).map(|s| s as u16);
                    let msg = parsed.as_ref()
                        .and_then(|v| v.get("error")?.get("message")?.as_str())
                        .unwrap_or("WebSocket error frame")
                        .to_string();
                    *guard = None;
                    send_err(TransportError::Request { status, message: msg });
                    return;
                }

                let is_done = is_terminal_event(event_type);
                if tx.send(Ok(text)).is_err() {
                    return; // receiver dropped
                }
                if is_done {
                    return;
                }
            }
            Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => {
                *guard = None;
                send_err(TransportError::Stream(
                    "WebSocket closed before terminal event".to_string(),
                ));
                return;
            }
            Ok(Message::Ping(data)) => { let _ = ws.send(Message::Pong(data)); }
            Ok(_) => {}
            Err(e) => {
                *guard = None;
                send_err(TransportError::Stream(e.to_string()));
                return;
            }
        }
    }
}

impl Transport for WsTransport {
    async fn send(&self, req: Request) -> Result<Response, TransportError> {
        let url = ws_url(&req.host, &req.path);
        let mut all_headers = self.headers.clone();
        all_headers.extend(req.headers);
        ensure_connected(&self.conn, &url, &all_headers).await?;

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
        ensure_connected(&self.conn, &url, &all_headers).await?;

        let text = wrap_ws_body(req.body, self.message_type.as_deref())?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<String, TransportError>>();
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || stream_frames(&conn, text, tx));

        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
            .map(|result| match result {
                Ok(text) => frame_to_event(&text),
                Err(e) => Err(e),
            });

        Ok(Box::pin(stream))
    }
}
