use chat_core::error::{ChatError, ChatFailure};
use reqwest::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OpenAIErrorResponse {
    pub error: OpenAIErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIErrorDetail {
    pub code: Option<i32>,
    pub message: String,
    pub status: Option<String>,
}

pub async fn handle_openai_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status();

    if status.is_success() {
        return Ok(res);
    }
    let err_text = res.text().await.unwrap_or_default();

    if status.as_u16() == 429 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    if let Ok(openai_err) = serde_json::from_str::<OpenAIErrorResponse>(&err_text) {
        let error_msg = format!(
            "OpenAI API Error[{}] ({}): {}",
            openai_err.error.code.unwrap_or(status.as_u16() as i32),
            openai_err.error.status.as_deref().unwrap_or("UNKNOWN"),
            openai_err.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {} Error: {}",
        status.as_u16(),
        err_text
    ))))
}
