use chat_core::error::{ChatError, ChatFailure};
use chat_core::transport::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CompletionsErrorResponse {
    pub error: CompletionsErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct CompletionsErrorDetail {
    pub code: Option<serde_json::Value>,
    pub message: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

// `ChatFailure` is the engine-wide error type (carries metadata); its size is
// fixed by the trait surface, so boxing here would just diverge from it.
#[allow(clippy::result_large_err)]
pub fn handle_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status;

    if (200..300).contains(&status) {
        return Ok(res);
    }

    if status == 429 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    let err_text = String::from_utf8_lossy(&res.body).into_owned();

    if let Ok(parsed) = serde_json::from_str::<CompletionsErrorResponse>(&err_text) {
        let code = parsed
            .error
            .code
            .map(|v| match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            })
            .unwrap_or_else(|| status.to_string());
        let msg = format!(
            "Chat Completions API Error[{code}] ({}): {}",
            parsed.error.kind.as_deref().unwrap_or("UNKNOWN"),
            parsed.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {status} Error: {err_text}",
    ))))
}
