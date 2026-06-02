use std::sync::Arc;

use async_stream::try_stream;
use futures::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use super::common::{frame_to_event, is_terminal_event, wrap_ws_body, ws_url};
use crate::transport::{EventStream, Request, Response, Transport, TransportError};

type WsConn = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Async WebSocket transport backed by [`tokio_tungstenite`].
///
/// Fully async — no blocking. Connection is lazily established
/// and reused across calls (including across `stream()` calls).
/// Uses `tokio::sync::Mutex` for interior mutability.
///
/// # Message wrapping
///
/// When `message_type` is set (e.g. `"response.create"` for OpenAI),
/// outgoing bodies are wrapped: the `type` field is injected and the
/// `stream` field is removed. This adapts the HTTP request body to the
/// WebSocket message format without requiring provider changes.
pub struct AsyncWsTransport {
    conn: Arc<Mutex<Option<WsConn>>>,
    headers: Vec<(String, String)>,
    message_type: Option<String>,
}

impl AsyncWsTransport {
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

impl Default for AsyncWsTransport {
    fn default() -> Self {
        Self::new()
    }
}

async fn connect_ws(url: &str, headers: &[(String, String)]) -> Result<WsConn, TransportError> {
    let mut req = url
        .into_client_request()
        .map_err(|e| TransportError::Connection(e.to_string()))?;

    use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
    for (k, v) in headers {
        req.headers_mut().insert(
            HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| TransportError::Connection(e.to_string()))?,
            HeaderValue::from_str(v).map_err(|e| TransportError::Connection(e.to_string()))?,
        );
    }

    let (socket, _response) = connect_async(req)
        .await
        .map_err(|e| TransportError::Connection(e.to_string()))?;
    Ok(socket)
}

async fn ensure_connected(
    conn: &Mutex<Option<WsConn>>,
    url: &str,
    headers: &[(String, String)],
) -> Result<(), TransportError> {
    let mut guard = conn.lock().await;
    if guard.is_none() {
        *guard = Some(connect_ws(url, headers).await?);
    }
    Ok(())
}

impl Transport for AsyncWsTransport {
    async fn send(&self, req: Request) -> Result<Response, TransportError> {
        let url = ws_url(&req.host, &req.path);
        let mut all_headers = self.headers.clone();
        all_headers.extend(req.headers);
        ensure_connected(&self.conn, &url, &all_headers).await?;

        let text = wrap_ws_body(req.body, self.message_type.as_deref())?;

        let mut guard = self.conn.lock().await;
        let ws = guard
            .as_mut()
            .ok_or_else(|| TransportError::Connection("WebSocket not connected".to_string()))?;

        ws.send(Message::Text(text.into()))
            .await
            .map_err(|e| TransportError::Request {
                status: None,
                message: e.to_string(),
            })?;

        let mut last_frame = String::new();

        while let Some(msg) = ws.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let text = text.to_string();
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

                        if event_type == "error" {
                            let msg = v
                                .get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("WebSocket error frame");
                            let status = v.get("status").and_then(|s| s.as_u64()).map(|s| s as u16);
                            return Err(TransportError::Request {
                                status,
                                message: msg.to_string(),
                            });
                        }

                        last_frame = text;

                        if is_terminal_event(event_type) {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    *guard = None;
                    return Err(TransportError::Stream(
                        "WebSocket closed before terminal event".to_string(),
                    ));
                }
                Ok(Message::Ping(data)) => {
                    let _ = ws.send(Message::Pong(data)).await;
                }
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
            body: last_frame.into_bytes(),
        })
    }

    async fn stream(&self, req: Request) -> Result<EventStream, TransportError> {
        let url = ws_url(&req.host, &req.path);
        let mut all_headers = self.headers.clone();
        all_headers.extend(req.headers);
        ensure_connected(&self.conn, &url, &all_headers).await?;

        let text = wrap_ws_body(req.body, self.message_type.as_deref())?;

        let mut guard = self.conn.lock().await;
        let mut ws = guard
            .take()
            .ok_or_else(|| TransportError::Connection("WebSocket not connected".to_string()))?;
        drop(guard);

        ws.send(Message::Text(text.into()))
            .await
            .map_err(|e| TransportError::Request {
                status: None,
                message: e.to_string(),
            })?;

        let conn = Arc::clone(&self.conn);

        let stream = try_stream! {
            while let Some(msg) = ws.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let event = frame_to_event(&text)?;

                        if event.0 == "error" {
                            let (status, msg) = serde_json::from_str::<serde_json::Value>(&event.1)
                                .ok()
                                .map(|v| {
                                    let status = v.get("status").and_then(|s| s.as_u64()).map(|s| s as u16);
                                    let msg = v.get("error")
                                        .and_then(|e| e.get("message"))
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("WebSocket error frame")
                                        .to_string();
                                    (status, msg)
                                })
                                .unwrap_or((None, "WebSocket error frame".to_string()));
                            Err(TransportError::Request { status, message: msg })?;
                        }

                        let is_done = is_terminal_event(&event.0);
                        yield event;
                        if is_done {
                            let mut guard = conn.lock().await;
                            *guard = Some(ws);
                            return;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        Err(TransportError::Stream(
                            "WebSocket closed before terminal event".to_string(),
                        ))?;
                    }
                    Ok(Message::Ping(data)) => {
                        let _ = ws.send(Message::Pong(data)).await;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        Err(TransportError::Stream(e.to_string()))?;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
