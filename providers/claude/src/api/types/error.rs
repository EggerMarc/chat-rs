use chat_core::error::{ChatError, ChatFailure};
use reqwest::Response;
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

pub async fn handle_claude_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status();

    if status.is_success() {
        return Ok(res);
    }

    if status.as_u16() == 429 || status.as_u16() == 529 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    let err_text = res.text().await.unwrap_or_default();

    if let Ok(claude_err) = serde_json::from_str::<ClaudeErrorResponse>(&err_text) {
        let error_msg = format!(
            "Claude API Error ({}): {}",
            claude_err.error.error_type, claude_err.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {} Error: {}",
        status.as_u16(),
        err_text
    ))))
}
