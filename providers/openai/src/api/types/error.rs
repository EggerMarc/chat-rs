use chat_core::error::{ChatError, ChatFailure};
use chat_core::transport::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OpenAIErrorResponse {
    pub error: OpenAIErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIErrorDetail {
    pub code: Option<String>,
    pub message: String,
    pub status: Option<String>,
}

pub fn handle_openai_error(res: Response) -> Result<Response, ChatFailure> {
    let status = res.status;

    if (200..300).contains(&status) {
        return Ok(res);
    }

    let err_text = String::from_utf8_lossy(&res.body).into_owned();

    if status == 429 {
        return Err(ChatFailure::from_err(ChatError::RateLimited));
    }

    if let Ok(openai_err) = serde_json::from_str::<OpenAIErrorResponse>(&err_text) {
        let code = openai_err.error.code.unwrap_or_else(|| status.to_string());
        let error_msg = format!(
            "OpenAI API Error[{code}] ({}): {}",
            openai_err.error.status.as_deref().unwrap_or("UNKNOWN"),
            openai_err.error.message
        );
        return Err(ChatFailure::from_err(ChatError::Provider(error_msg)));
    }

    Err(ChatFailure::from_err(ChatError::Provider(format!(
        "HTTP {status} Error: {err_text}",
    ))))
}
