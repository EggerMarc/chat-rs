use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::messages::{Messages, file::FileSource, parts::PartEnum},
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
pub struct OpenAIEmbeddingRequest {
    pub model: String,
    pub input: Value,
}

impl OpenAIEmbeddingRequest {
    pub fn from_core(model_name: &str, messages: &Messages) -> Result<Self, ChatError> {
        let last_content = messages
            .0
            .last()
            .ok_or_else(|| ChatError::InvalidResponse("Sent empty content to embed".to_string()))?;

        let mut parts = Vec::new();
        for part in &last_content.parts.0 {
            match part {
                PartEnum::Text(t) => parts.push(json!(t.0)),
                PartEnum::Reasoning(r) => parts.push(json!(r.text)),
                PartEnum::File(file) => match &file.source {
                    FileSource::Bytes(bytes) => {
                        let b64 = STANDARD.encode(bytes);
                        let uri = format!("data:{};base64,{}", file.mime, b64);
                        parts.push(json!(uri));
                    }
                    FileSource::Url(url) => {
                        parts.push(json!(url.to_string()));
                    }
                },
                _ => {}
            }
        }

        if parts.is_empty() {
            return Err(ChatError::InvalidResponse(
                "Sent empty content to embed".to_string(),
            ));
        }

        let input = if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Value::Array(parts)
        };

        Ok(Self {
            model: model_name.to_string(),
            input,
        })
    }
}
