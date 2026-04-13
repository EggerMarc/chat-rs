use super::types::{Event, TransportError};

/// Parse a WebSocket text frame into a transport `Event`.
///
/// Extracts the `type` field from the JSON frame as the event type,
/// and passes the full JSON as the data. This matches the SSE
/// `(event_type, data)` contract used by HTTP transports.
pub(crate) fn frame_to_event(text: &str) -> Result<Event, TransportError> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        TransportError::Stream(format!("invalid JSON in WebSocket frame: {e}"))
    })?;
    let event_type = v
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    Ok((event_type, text.to_string()))
}

/// Build the WebSocket URL from a request, using `wss://` for standard
/// ports and `ws://` for localhost / explicit non-TLS hosts.
pub(crate) fn ws_url(host: &str, path: &str) -> String {
    let scheme = if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
        "ws"
    } else {
        "wss"
    };
    format!("{scheme}://{host}{path}")
}

/// Wrap a JSON body for WebSocket delivery.
///
/// If `message_type` is set, parses the body as JSON, injects
/// `"type": "<message_type>"` at the top level, removes the `"stream"`
/// field (transport-specific, not used over WS), and re-serializes.
///
/// If `message_type` is `None`, returns the body unchanged.
pub(crate) fn wrap_ws_body(
    body: Vec<u8>,
    message_type: Option<&str>,
) -> Result<String, TransportError> {
    let text = String::from_utf8(body)
        .map_err(|e| TransportError::Request { status: None, message: e.to_string() })?;

    let Some(msg_type) = message_type else {
        return Ok(text);
    };

    let mut obj: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&text).map_err(|e| TransportError::Request {
            status: None,
            message: format!("body is not a JSON object: {e}"),
        })?;

    obj.insert("type".to_string(), serde_json::Value::String(msg_type.to_string()));
    obj.remove("stream");

    serde_json::to_string(&obj).map_err(|e| TransportError::Request {
        status: None,
        message: e.to_string(),
    })
}

/// Whether an event type signals the end of a response exchange.
///
/// Over SSE, the HTTP connection closes to signal completion. Over
/// WebSocket, the connection stays open — so we detect completion
/// from the event type instead.
pub(crate) fn is_terminal_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "response.completed" | "response.failed" | "response.cancelled" | "error"
    )
}
