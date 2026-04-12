use async_stream::try_stream;
use futures::StreamExt;

use chat_core::transport::{
    EventStream, Request, Response, Transport, TransportError,
    sse::SseParser,
};

/// HTTP transport backed by [`reqwest`].
///
/// Sends requests as HTTP POST and parses SSE for streaming.
/// Wraps a [`reqwest::Client`] internally and is [`Clone`]-able —
/// cloning shares the underlying connection pool.
#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl From<reqwest::Client> for ReqwestTransport {
    fn from(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Transport for ReqwestTransport {
    async fn send(&mut self, req: Request) -> Result<Response, TransportError> {
        let mut builder = self.client.post(&req.url);
        for (key, value) in &req.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        builder = builder.body(req.body);

        let res = builder
            .send()
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))?;

        let status = res.status().as_u16();

        let headers = res
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();

        let body = res
            .bytes()
            .await
            .map_err(|e| TransportError::Request {
                status: None,
                message: e.to_string(),
            })?
            .to_vec();

        Ok(Response {
            status,
            headers,
            body,
        })
    }

    async fn stream(&mut self, req: Request) -> Result<EventStream, TransportError> {
        let mut builder = self.client.post(&req.url);
        for (key, value) in &req.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        builder = builder.body(req.body);

        let res = builder
            .send()
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(TransportError::Request {
                status: Some(status),
                message: format!("HTTP {status}: {body}"),
            });
        }

        let stream = try_stream! {
            let mut byte_stream = res.bytes_stream();
            let mut parser = SseParser::default();

            while let Some(chunk_res) = byte_stream.next().await {
                let chunk = chunk_res.map_err(|e| TransportError::Stream(e.to_string()))?;
                parser.push(&chunk);

                while let Some(event) = parser.next_event() {
                    yield event;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
