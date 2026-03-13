use chat_core::error::{ChatError, ChatFailure};
use reqwest::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GeminiErrorResponse {
    pub error: GeminiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct GeminiErrorDetail {
    pub code: Option<i32>,
    pub message: String,
    pub status: Option<String>,
}

pub async fn handle_gemini_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status();

    if status.is_success() {
        return Ok(res);
    }

    if status.as_u16() == 429 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    let err_text = res.text().await.unwrap_or_default();

    if let Ok(gemini_err) = serde_json::from_str::<GeminiErrorResponse>(&err_text) {
        let error_msg = format!(
            "Gemini API Error[{}] ({}): {}",
            gemini_err.error.code.unwrap_or(status.as_u16() as i32),
            gemini_err.error.status.as_deref().unwrap_or("UNKNOWN"),
            gemini_err.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {} Error: {}",
        status.as_u16(),
        err_text
    ))))
}
