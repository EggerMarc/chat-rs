use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            embeddings::Embeddings,
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

#[derive(Debug, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Option<CompletionsUsage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    #[serde(default)]
    pub content: Option<Value>,
    /// Thinking-model channel used by Qwen3, DeepSeek-R1 and clones.
    #[serde(default, alias = "reasoning")]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseToolCall {
    pub id: Option<String>,
    pub function: ResponseToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseToolCallFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompletionsUsage {
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
    pub total_tokens: Option<usize>,
}

impl CompletionsUsage {
    // Consumes a wire usage into the core type; renaming to `into_` would be a
    // public surface change.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_core(self) -> Usage {
        Usage {
            input_tokens: self.prompt_tokens.unwrap_or(0),
            output_tokens: self.completion_tokens.unwrap_or(0),
            total_tokens: self.total_tokens.unwrap_or(0),
        }
    }
}

/// Maps a Chat Completions `finish_reason` string to the core enum.
///
/// `tool_calls` is reported when the model emits one or more function
/// calls. `length` means the model hit its max-token budget. `stop` and
/// the like indicate a normal stop.
pub fn finish_reason_to_core(reason: Option<&str>, had_tool_calls: bool) -> CompleteReasonEnum {
    if had_tool_calls {
        return CompleteReasonEnum::ToolCall;
    }
    match reason {
        Some("stop") => CompleteReasonEnum::Stop,
        Some("length") => CompleteReasonEnum::MaxTokens,
        Some("tool_calls") | Some("function_call") => CompleteReasonEnum::ToolCall,
        Some(other) => CompleteReasonEnum::Other(other.to_string()),
        None => CompleteReasonEnum::None,
    }
}

/// Appends parts produced by a single response message into the parts buffer.
/// Returns whether any tool calls were emitted.
pub fn message_to_parts(msg: &ResponseMessage, parts: &mut Parts) -> bool {
    // Reasoning first so renderers can show it before the answer.
    if let Some(reasoning) = &msg.reasoning_content
        && !reasoning.is_empty()
    {
        parts.push(PartEnum::Reasoning(Reasoning::new(reasoning.clone())));
    }

    // Content can be string, array of typed parts, or null.
    if let Some(content) = &msg.content {
        append_content_value(content, parts);
    }

    let mut had_tool_calls = false;
    if let Some(calls) = &msg.tool_calls {
        for call in calls {
            had_tool_calls = true;
            let arguments: Value = call
                .function
                .arguments
                .as_deref()
                .map(|s| serde_json::from_str(s).unwrap_or(Value::Null))
                .unwrap_or(Value::Null);
            parts.push(PartEnum::from_function_call(FunctionCall {
                id: call.id.clone().map(Into::into),
                name: call.function.name.clone().unwrap_or_default(),
                arguments,
            }));
        }
    }
    had_tool_calls
}

fn append_content_value(value: &Value, parts: &mut Parts) {
    match value {
        Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(s)
                && (parsed.is_object() || parsed.is_array())
            {
                parts.push(PartEnum::Structured(parsed));
                return;
            }
            if !s.is_empty() {
                parts.push(PartEnum::Text(Text::new(s.clone())));
            }
        }
        Value::Array(arr) => {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    let ty = obj.get("type").and_then(|v| v.as_str());
                    if ty == Some("text")
                        && let Some(t) = obj.get("text").and_then(|v| v.as_str())
                    {
                        parts.push(PartEnum::Text(Text::new(t.to_string())));
                    }
                }
            }
        }
        _ => {}
    }
}

impl ChatCompletionsResponse {
    pub fn into_core_chat_response(self) -> Result<ChatResponse, ChatError> {
        let choice = self
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ChatError::InvalidResponse("response had no choices".into()))?;

        let mut parts = Parts::default();
        let had_tool_calls = message_to_parts(&choice.message, &mut parts);
        let complete_reason =
            finish_reason_to_core(choice.finish_reason.as_deref(), had_tool_calls);

        let metadata = Metadata {
            id: self.id,
            model_slug: self.model,
            usage: self
                .usage
                .map(CompletionsUsage::to_core)
                .unwrap_or_default(),
            ..Default::default()
        };

        Ok(ChatResponse {
            content: Content {
                role: RoleEnum::Model,
                parts,
                complete_reason,
            },
            metadata: Some(metadata),
        })
    }
}

// ---------------------------------------------------------------------------
// Embedding response
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChatCompletionsEmbeddingResponse {
    pub data: Vec<EmbeddingData>,
    pub model: Option<String>,
    pub usage: Option<CompletionsUsage>,
}

#[derive(Debug, Deserialize)]
pub struct EmbeddingData {
    pub embedding: Vec<f32>,
}

impl ChatCompletionsEmbeddingResponse {
    pub fn into_core_embeddings_response(self) -> Result<EmbeddingsResponse, ChatError> {
        let mut data = self.data.into_iter();
        let first = data
            .next()
            .ok_or_else(|| ChatError::InvalidResponse("No embedding data returned".into()))?;
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
                .map(CompletionsUsage::to_core)
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

    #[test]
    fn parses_simple_assistant_response() {
        let body = r#"{
            "id": "chatcmpl-1",
            "model": "llama3",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hi there"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}
        }"#;
        let resp: ChatCompletionsResponse = serde_json::from_str(body).unwrap();
        let core = resp.into_core_chat_response().unwrap();
        assert_eq!(core.content.complete_reason, CompleteReasonEnum::Stop);
        let txt = core
            .content
            .parts
            .text_response()
            .unwrap()
            .as_str()
            .to_string();
        assert_eq!(txt, "hi there");
    }

    #[test]
    fn parses_tool_call_response() {
        let body = r#"{
            "id": "x",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"Paris\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;
        let resp: ChatCompletionsResponse = serde_json::from_str(body).unwrap();
        let core = resp.into_core_chat_response().unwrap();
        assert_eq!(core.content.complete_reason, CompleteReasonEnum::ToolCall);
        let tool = core.content.parts.tools().next().expect("expected a tool");
        let (fc, _) = tool.to_tuple();
        assert_eq!(fc.name, "get_weather");
        assert_eq!(fc.arguments["city"], "Paris");
    }
}
