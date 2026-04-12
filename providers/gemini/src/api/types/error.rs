use chat_core::error::{ChatError, ChatFailure};
use chat_core::transport::Response;
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

pub fn handle_gemini_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status;

    if (200..300).contains(&status) {
        return Ok(res);
    }

    if status == 429 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    let err_text = String::from_utf8_lossy(&res.body).into_owned();

    if let Ok(gemini_err) = serde_json::from_str::<GeminiErrorResponse>(&err_text) {
        let error_msg = format!(
            "Gemini API Error[{}] ({}): {}",
            gemini_err.error.code.unwrap_or(status as i32),
            gemini_err.error.status.as_deref().unwrap_or("UNKNOWN"),
            gemini_err.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {status} Error: {err_text}",
    ))))
}
