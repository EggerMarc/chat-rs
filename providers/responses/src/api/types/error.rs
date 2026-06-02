use chat_core::error::{ChatError, ChatFailure};
use chat_core::transport::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ResponsesErrorResponse {
    pub error: ResponsesErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesErrorDetail {
    pub code: Option<String>,
    pub message: String,
    pub status: Option<String>,
}

// `ChatFailure` is the engine-wide error type; its size is fixed by the trait
// surface, so boxing here would just diverge from it.
#[allow(clippy::result_large_err)]
/// Maps a raw transport response into a `ChatFailure`. Recognises the
/// `{ "error": { "code", "message", "status" } }` envelope used by
/// OpenAI and the providers (Groq, future) that adopt the Responses
/// wire — same shape as the Chat Completions envelope.
pub fn handle_responses_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status;

    if (200..300).contains(&status) {
        return Ok(res);
    }

    let err_text = String::from_utf8_lossy(&res.body).into_owned();

    if status == 429 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    if let Ok(parsed) = serde_json::from_str::<ResponsesErrorResponse>(&err_text) {
        let code = parsed.error.code.unwrap_or_else(|| status.to_string());
        let error_msg = format!(
            "Responses API Error[{code}] ({}): {}",
            parsed.error.status.as_deref().unwrap_or("UNKNOWN"),
            parsed.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {status} Error: {err_text}",
    ))))
}
