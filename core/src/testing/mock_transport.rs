use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::transport::{EventStream, Request, Response, Transport, TransportError};

struct Inner {
    requests: Vec<Request>,
    responses: VecDeque<Result<Response, TransportError>>,
    stream_responses: VecDeque<Result<Vec<(String, String)>, TransportError>>,
}

/// A test transport that records outbound requests and returns pre-configured responses.
///
/// Created via [`MockTransport::new`], which returns a `(MockTransport, TransportInspector)` pair.
/// Configure responses through the inspector, then pass the transport to a provider builder
/// via `.with_transport(mock)`.
pub struct MockTransport {
    inner: Arc<Mutex<Inner>>,
}

/// Handle for configuring a [`MockTransport`] and inspecting recorded requests.
///
/// Use this to queue canned responses before calling `provider.complete()`,
/// then inspect what the provider sent on the wire.
pub struct TransportInspector {
    inner: Arc<Mutex<Inner>>,
}

impl MockTransport {
    /// Create a new mock transport and its inspector handle.
    pub fn new() -> (Self, TransportInspector) {
        let inner = Arc::new(Mutex::new(Inner {
            requests: Vec::new(),
            responses: VecDeque::new(),
            stream_responses: VecDeque::new(),
        }));
        (
            MockTransport {
                inner: inner.clone(),
            },
            TransportInspector { inner },
        )
    }
}

impl TransportInspector {
    /// Queue a single response that the next `send()` call will return.
    pub fn set_response(&self, status: u16, body: impl Into<Vec<u8>>) {
        let mut inner = self.inner.lock().unwrap();
        inner.responses.push_back(Ok(Response {
            status,
            headers: vec![],
            body: body.into(),
        }));
    }

    /// Queue multiple responses returned in order by successive `send()` calls.
    pub fn set_responses(&self, responses: Vec<(u16, Vec<u8>)>) {
        let mut inner = self.inner.lock().unwrap();
        for (status, body) in responses {
            inner.responses.push_back(Ok(Response {
                status,
                headers: vec![],
                body,
            }));
        }
    }

    /// Queue a transport error that the next `send()` call will return.
    pub fn set_error(&self, err: TransportError) {
        let mut inner = self.inner.lock().unwrap();
        inner.responses.push_back(Err(err));
    }

    /// Queue stream events that the next `stream()` call will return.
    pub fn set_stream_events(&self, events: Vec<(String, String)>) {
        let mut inner = self.inner.lock().unwrap();
        inner.stream_responses.push_back(Ok(events));
    }

    /// Queue a stream error that the next `stream()` call will return.
    pub fn set_stream_error(&self, err: TransportError) {
        let mut inner = self.inner.lock().unwrap();
        inner.stream_responses.push_back(Err(err));
    }

    /// Get the last request sent through the transport.
    pub fn last_request(&self) -> Option<Request> {
        let inner = self.inner.lock().unwrap();
        inner.requests.last().cloned()
    }

    /// Drain all recorded requests.
    pub fn take_requests(&self) -> Vec<Request> {
        let mut inner = self.inner.lock().unwrap();
        std::mem::take(&mut inner.requests)
    }

    /// Number of requests recorded so far.
    pub fn request_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.requests.len()
    }

    /// Clear all state: recorded requests, queued responses, queued stream events.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.requests.clear();
        inner.responses.clear();
        inner.stream_responses.clear();
    }
}

/// Parse a request body as JSON. Panics if the body is not valid JSON.
pub fn body_json(request: &Request) -> serde_json::Value {
    serde_json::from_slice(&request.body).expect("request body is not valid JSON")
}

impl Transport for MockTransport {
    fn send(
        &self,
        req: Request,
    ) -> impl std::future::Future<Output = Result<Response, TransportError>> + Send {
        let mut inner = self.inner.lock().unwrap();
        inner.requests.push(req);
        let response = inner.responses.pop_front().unwrap_or_else(|| {
            Ok(Response {
                status: 200,
                headers: vec![],
                body: vec![],
            })
        });
        async move { response }
    }

    fn stream(
        &self,
        req: Request,
    ) -> impl std::future::Future<Output = Result<EventStream, TransportError>> + Send {
        let mut inner = self.inner.lock().unwrap();
        inner.requests.push(req);
        let stream_result = inner.stream_responses.pop_front().unwrap_or_else(|| {
            Err(TransportError::Stream(
                "no stream response configured in MockTransport".into(),
            ))
        });
        async move {
            match stream_result {
                Ok(events) => {
                    let stream =
                        futures::stream::iter(events.into_iter().map(Ok::<_, TransportError>));
                    Ok(Box::pin(stream) as EventStream)
                }
                Err(e) => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_requests_and_returns_queued_responses() {
        let (transport, inspector) = MockTransport::new();
        inspector.set_response(200, b"{\"ok\": true}".to_vec());

        let req = Request {
            scheme: "https".into(),
            host: "api.example.com".into(),
            path: "/v1/test".into(),
            headers: vec![("Authorization".into(), "Bearer test".into())],
            body: b"{\"prompt\": \"hello\"}".to_vec(),
        };

        let resp = transport.send(req).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"{\"ok\": true}");

        assert_eq!(inspector.request_count(), 1);
        let sent = inspector.last_request().unwrap();
        assert_eq!(sent.host, "api.example.com");
        assert_eq!(sent.path, "/v1/test");

        let json = body_json(&sent);
        assert_eq!(json["prompt"], "hello");
    }

    #[tokio::test]
    async fn returns_default_200_when_no_response_queued() {
        let (transport, _inspector) = MockTransport::new();
        let req = Request {
            scheme: "https".into(),
            host: "example.com".into(),
            path: "/".into(),
            headers: vec![],
            body: vec![],
        };
        let resp = transport.send(req).await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.body.is_empty());
    }

    #[tokio::test]
    async fn returns_queued_error() {
        let (transport, inspector) = MockTransport::new();
        inspector.set_error(TransportError::Request {
            status: Some(429),
            message: "rate limited".into(),
        });

        let req = Request {
            scheme: "https".into(),
            host: "example.com".into(),
            path: "/".into(),
            headers: vec![],
            body: vec![],
        };
        let err = transport.send(req).await.unwrap_err();
        assert!(matches!(err, TransportError::Request { status: Some(429), .. }));
    }

    #[tokio::test]
    async fn queues_multiple_responses_in_order() {
        let (transport, inspector) = MockTransport::new();
        inspector.set_responses(vec![
            (200, b"first".to_vec()),
            (201, b"second".to_vec()),
        ]);

        let req = || Request {
            scheme: "https".into(),
            host: "example.com".into(),
            path: "/".into(),
            headers: vec![],
            body: vec![],
        };

        let r1 = transport.send(req()).await.unwrap();
        let r2 = transport.send(req()).await.unwrap();
        assert_eq!(r1.status, 200);
        assert_eq!(r1.body, b"first");
        assert_eq!(r2.status, 201);
        assert_eq!(r2.body, b"second");
        assert_eq!(inspector.request_count(), 2);
    }

    #[tokio::test]
    async fn reset_clears_all_state() {
        let (transport, inspector) = MockTransport::new();
        inspector.set_response(200, b"data".to_vec());

        let req = Request {
            scheme: "https".into(),
            host: "example.com".into(),
            path: "/".into(),
            headers: vec![],
            body: vec![],
        };
        let _ = transport.send(req).await;
        assert_eq!(inspector.request_count(), 1);

        inspector.reset();
        assert_eq!(inspector.request_count(), 0);
    }
}
