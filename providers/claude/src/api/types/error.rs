use chat_core::error::{ChatError, ChatFailure};
use chat_core::transport::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ClaudeErrorResponse {
    pub error: ClaudeErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

pub fn handle_claude_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status;

    if (200..300).contains(&status) {
        return Ok(res);
    }

    if status == 429 || status == 529 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    let err_text = String::from_utf8_lossy(&res.body).into_owned();

    if let Ok(claude_err) = serde_json::from_str::<ClaudeErrorResponse>(&err_text) {
        let error_msg = format!(
            "Claude API Error ({}): {}",
            claude_err.error.error_type, claude_err.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {status} Error: {err_text}",
    ))))
}
