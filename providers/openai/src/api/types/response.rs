use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            embeddings::Embeddings,
            file::File,
            parts::{PartEnum, Parts},
            reasoning::Reasoning,
            text::Text,
        },
        metadata::{Metadata, usage::Usage},
        response::{ChatResponse, EmbeddingsResponse},
    },
};
use serde::Deserialize;
use serde_json::Value;
use tools_rs::FunctionCall;

// ---------------------------------------------------------------------------
// Responses API types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ResponsesApiResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub output: Vec<ResponsesOutputItem>,
    pub usage: Option<OpenAIUsage>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message(ResponsesMessage),
    #[serde(rename = "function_call")]
    FunctionCall(ResponsesFunctionCall),
    #[serde(rename = "reasoning")]
    Reasoning(ResponsesReasoning),
    #[serde(rename = "web_search_call")]
    WebSearchCall(ResponsesWebSearchCall),
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall(ResponsesImageGenerationCall),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesImageGenerationCall {
    /// Base64-encoded image payload. Present when the model has produced a
    /// finalised image; absent during intermediate status frames.
    pub result: Option<String>,
    /// Output format hint (e.g. `"png"`, `"jpeg"`). Used to synthesise a
    /// mimetype when present.
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesMessage {
    pub content: Vec<ResponsesContentPart>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesContentPart {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "output_image")]
    OutputImage { image_url: Option<String> },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesFunctionCall {
    pub call_id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesReasoning {
    pub summary: Option<Vec<ResponsesSummaryPart>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesSummaryPart {
    #[serde(rename = "summary_text")]
    SummaryText { text: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesWebSearchCall {}

// ---------------------------------------------------------------------------
// Usage (supports both completions and responses field names)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OpenAIUsage {
    #[serde(alias = "prompt_tokens")]
    pub input_tokens: Option<usize>,
    #[serde(alias = "completion_tokens")]
    pub output_tokens: Option<usize>,
    pub total_tokens: Option<usize>,
}

// ---------------------------------------------------------------------------
// Conversion to core types
// ---------------------------------------------------------------------------

/// Appends a single content part to the parts collection.
fn append_content_part(parts: &mut Parts, content_part: &ResponsesContentPart) {
    match content_part {
        ResponsesContentPart::OutputText { text } => {
            if let Ok(value) = serde_json::from_str::<Value>(text)
                && (value.is_object() || value.is_array())
            {
                parts.push(PartEnum::Structured(value));
                return;
            }
            parts.push(PartEnum::Text(Text::new(text.clone())));
        }
        ResponsesContentPart::OutputImage { image_url } => {
            if let Some(url_str) = image_url
                && let Ok(file) = File::from_url(url_str, None)
            {
                parts.push(PartEnum::File(file));
            }
        }
        ResponsesContentPart::Unknown => {}
    }
}

/// Converts a slice of OpenAI output items into core parts.
/// Returns the parts and whether any function calls were present.
pub fn output_items_to_parts(output: &[ResponsesOutputItem]) -> (Parts, bool) {
    let mut parts = Parts::default();
    let mut has_function_call = false;

    for item in output {
        match item {
            ResponsesOutputItem::Message(msg) => {
                for content_part in &msg.content {
                    append_content_part(&mut parts, content_part);
                }
            }
            ResponsesOutputItem::FunctionCall(fc) => {
                has_function_call = true;
                let arguments: Value = fc
                    .arguments
                    .as_deref()
                    .map(|s| serde_json::from_str(s).unwrap_or_default())
                    .unwrap_or_default();

                parts.push(PartEnum::from_function_call(FunctionCall {
                    id: fc.call_id.clone().map(Into::into),
                    name: fc.name.clone().unwrap_or_default(),
                    arguments,
                }));
            }
            ResponsesOutputItem::Reasoning(r) => {
                if let Some(summary) = &r.summary {
                    for sp in summary {
                        let ResponsesSummaryPart::SummaryText { text } = sp;
                        parts.push(PartEnum::Reasoning(Reasoning::new(text.clone())));
                    }
                }
            }
            ResponsesOutputItem::ImageGenerationCall(call) => {
                if let Some(b64) = &call.result
                    && let Ok(bytes) = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        b64,
                    )
                {
                    let mime = call
                        .output_format
                        .as_deref()
                        .map(|fmt| format!("image/{fmt}"))
                        .unwrap_or_else(|| "image/png".to_string());
                    parts.push(PartEnum::File(File::from_bytes_with_mime(bytes, mime)));
                }
            }
            ResponsesOutputItem::WebSearchCall(_) | ResponsesOutputItem::Unknown => {}
        }
    }

    (parts, has_function_call)
}

impl ResponsesApiResponse {
    pub fn into_core_chat_response(self) -> Result<(ChatResponse, Option<String>), ChatError> {
        let response_id = self.id.clone();
        let (parts, has_function_call) = output_items_to_parts(&self.output);

        let complete_reason = if has_function_call {
            CompleteReasonEnum::ToolCall
        } else {
            match self.status.as_deref() {
                Some("completed") => CompleteReasonEnum::Stop,
                Some("incomplete") => CompleteReasonEnum::MaxTokens,
                Some(other) => CompleteReasonEnum::Other(other.to_string()),
                None => CompleteReasonEnum::None,
            }
        };

        let metadata = Metadata {
            id: self.id,
            model_slug: self.model,
            usage: self
                .usage
                .map(|u| Usage {
                    input_tokens: u.input_tokens.unwrap_or(0),
                    output_tokens: u.output_tokens.unwrap_or(0),
                    total_tokens: u.total_tokens.unwrap_or(0),
                })
                .unwrap_or_default(),
            ..Default::default()
        };

        Ok((
            ChatResponse {
                content: Content {
                    role: RoleEnum::Model,
                    parts,
                    complete_reason,
                },
                metadata: Some(metadata),
            },
            response_id,
        ))
    }
}

// ---------------------------------------------------------------------------
// Embedding response types (unchanged)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OpenAIEmbeddingResponse {
    pub data: Vec<OpenAIEmbeddingData>,
    pub model: Option<String>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIEmbeddingData {
    pub embedding: Vec<f32>,
}

impl OpenAIEmbeddingResponse {
    pub fn into_core_embeddings_response(self) -> Result<EmbeddingsResponse, ChatError> {
        let mut data = self.data.into_iter();
        let first = data.next().ok_or_else(|| {
            ChatError::InvalidResponse("No embedding data returned by OpenAI".into())
        })?;
        if data.next().is_some() {
            return Err(ChatError::InvalidResponse(
                "Expected a single embedding result".into(),
            ));
        }

        let dimension = first.embedding.len();

        let metadata = Metadata {
            model_slug: self.model,
            usage: self
                .usage
                .map(|u| Usage {
                    input_tokens: u.input_tokens.unwrap_or(0),
                    output_tokens: u.output_tokens.unwrap_or(0),
                    total_tokens: u.total_tokens.unwrap_or(0),
                })
                .unwrap_or_default(),
            ..Default::default()
        };

        Ok(EmbeddingsResponse {
            embeddings: Embeddings {
                content: first.embedding,
                dimension,
            },
            metadata: Some(metadata),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_core::types::messages::file::FileSource;

    #[test]
    fn image_generation_call_decoded_to_file_image() {
        // Tiny valid base64 payload ("hi").
        let body = r#"{
            "output": [
                {
                    "type": "image_generation_call",
                    "id": "ig_1",
                    "result": "aGk=",
                    "output_format": "png",
                    "status": "completed"
                }
            ]
        }"#;

        let resp: ResponsesApiResponse = serde_json::from_str(body).unwrap();
        let (parts, _) = output_items_to_parts(&resp.output);

        let file = parts
            .into_iter()
            .find_map(|p| match p {
                PartEnum::File(f) => Some(f),
                _ => None,
            })
            .expect("expected a File part");

        assert!(file.is_image());
        assert_eq!(file.mime.as_str(), "image/png");
        match file.source {
            FileSource::Bytes(bytes) => assert_eq!(bytes, b"hi"),
            other => panic!("expected Bytes source, got {other:?}"),
        }
    }

    #[test]
    fn image_generation_call_without_result_is_skipped() {
        let body = r#"{
            "output": [
                { "type": "image_generation_call", "id": "ig_1", "status": "in_progress" }
            ]
        }"#;
        let resp: ResponsesApiResponse = serde_json::from_str(body).unwrap();
        let (parts, _) = output_items_to_parts(&resp.output);
        assert!(parts.is_empty());
    }
}
