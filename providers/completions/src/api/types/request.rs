use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{
            Messages,
            content::{Content, RoleEnum},
            file::FileSource,
            parts::PartEnum,
        },
        options::ChatOptions,
        tools::ToolDeclarations,
    },
};
use schemars::Schema;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
}

pub struct CompletionsRequestConfig<'a> {
    pub model_name: &'a str,
    pub messages: &'a Messages,
    pub tool_declarations: Option<&'a dyn ToolDeclarations>,
    pub options: Option<&'a ChatOptions>,
    pub output_shape: Option<&'a Schema>,
}

impl ChatCompletionsRequest {
    pub fn from_core(config: CompletionsRequestConfig<'_>) -> Result<Self, ChatError> {
        let CompletionsRequestConfig {
            model_name,
            messages,
            tool_declarations,
            options,
            output_shape,
        } = config;

        let mut req = Self {
            model: model_name.to_string(),
            messages: Vec::new(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            tools: None,
            response_format: None,
            stream: None,
            stream_options: None,
        };

        if let Some(opts) = options {
            req.temperature = opts.temperature;
            req.top_p = opts.top_p;
            req.max_tokens = opts.max_tokens;
        }

        if let Some(schema) = output_shape {
            req.response_format = Some(json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_output",
                    "schema": schema,
                    "strict": false,
                }
            }));
        }

        // Tools (function calling)
        if let Some(decls) = tool_declarations {
            let value = decls.json().map_err(|e| ChatError::Other(e.to_string()))?;
            if let Value::Array(arr) = value {
                let mut tools_list = Vec::with_capacity(arr.len());
                for decl in arr {
                    let name = decl
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let description = decl
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let parameters = decl
                        .get("parameters")
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object"}));

                    tools_list.push(json!({
                        "type": "function",
                        "function": {
                            "name": name,
                            "description": description,
                            "parameters": parameters,
                        }
                    }));
                }
                if !tools_list.is_empty() {
                    req.tools = Some(tools_list);
                }
            }
        }

        for content in &messages.0 {
            push_content(content, &mut req.messages);
        }

        Ok(req)
    }
}

fn push_content(content: &Content, out: &mut Vec<Value>) {
    let role = match content.role {
        RoleEnum::User => "user",
        RoleEnum::Model => "assistant",
        RoleEnum::System => "system",
    };

    // Assistant tool calls are emitted as a single assistant message
    // with `tool_calls`. Tool results follow as separate `role: "tool"`
    // messages keyed by `tool_call_id`. Other parts (text, images,
    // files) live in the message's `content` array.
    let mut content_parts: Vec<Value> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    let mut tool_results: Vec<Value> = Vec::new();

    for part in &content.parts.0 {
        match part {
            PartEnum::Text(t) => {
                content_parts.push(json!({"type": "text", "text": t.0}));
            }
            PartEnum::Reasoning(r) => {
                // Chat Completions has no native reasoning channel; surface
                // the summary text inline so it isn't silently dropped.
                content_parts.push(json!({"type": "text", "text": r.text}));
            }
            PartEnum::Tool(tool) => {
                let (fc, maybe_fr) = tool.to_tuple();
                tool_calls.push(json!({
                    "id": fc.id.clone().map(String::from).unwrap_or_default(),
                    "type": "function",
                    "function": {
                        "name": fc.name,
                        "arguments": serde_json::to_string(&fc.arguments).unwrap_or_default(),
                    }
                }));
                if let Some(fr) = maybe_fr {
                    let output = if fr.result.is_string() {
                        fr.result.as_str().unwrap().to_string()
                    } else {
                        fr.result.to_string()
                    };
                    tool_results.push(json!({
                        "role": "tool",
                        "tool_call_id": fr.id.clone().map(String::from).unwrap_or_default(),
                        "content": output,
                    }));
                }
            }
            PartEnum::File(file) => {
                if file.is_image() {
                    let url = match &file.source {
                        FileSource::Url(u) => u.to_string(),
                        FileSource::Bytes(bytes) => {
                            let b64 = STANDARD.encode(bytes);
                            format!("data:{};base64,{}", file.mime, b64)
                        }
                    };
                    content_parts.push(json!({
                        "type": "image_url",
                        "image_url": {"url": url}
                    }));
                }
                // Non-image files have no portable encoding in the Chat
                // Completions spec — providers diverge. Skip silently.
            }
            PartEnum::Structured(_) | PartEnum::Embeddings(_) => {}
        }
    }

    let message_emitted =
        !content_parts.is_empty() || !tool_calls.is_empty() || role == "assistant";

    if message_emitted {
        let mut message = serde_json::Map::new();
        message.insert("role".into(), json!(role));

        if content_parts.is_empty() {
            // Assistant turn with only tool calls — content must be null
            // per spec (some servers reject empty arrays).
            message.insert("content".into(), Value::Null);
        } else if content_parts.len() == 1
            && let Some(t) = content_parts[0].get("text").and_then(|v| v.as_str())
            && content_parts[0].get("type").and_then(|v| v.as_str()) == Some("text")
        {
            // Collapse single text part to string form — broadest server support.
            message.insert("content".into(), json!(t));
        } else {
            message.insert("content".into(), Value::Array(content_parts));
        }

        if !tool_calls.is_empty() {
            message.insert("tool_calls".into(), Value::Array(tool_calls));
        }

        out.push(Value::Object(message));
    }

    out.extend(tool_results);
}

// ---------------------------------------------------------------------------
// Embeddings request
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ChatCompletionsEmbeddingRequest {
    pub model: String,
    pub input: Value,
}

impl ChatCompletionsEmbeddingRequest {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chat_core::types::messages;

    #[test]
    fn user_message_serializes_to_string_content() {
        let msgs = messages::from_user(vec!["hello"]);
        let req = ChatCompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: "llama3",
            messages: &msgs,
            tool_declarations: None,
            options: None,
            output_shape: None,
        })
        .unwrap();

        let val = serde_json::to_value(&req).unwrap();
        assert_eq!(val["model"], "llama3");
        assert_eq!(val["messages"][0]["role"], "user");
        assert_eq!(val["messages"][0]["content"], "hello");
    }

    #[test]
    fn system_message_uses_system_role() {
        let mut msgs = messages::Messages::default();
        msgs.0.push(messages::content::from_system(vec!["you are helpful"]));
        msgs.0.push(messages::content::from_user(vec!["hi"]));
        let req = ChatCompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: "m",
            messages: &msgs,
            tool_declarations: None,
            options: None,
            output_shape: None,
        })
        .unwrap();

        let val = serde_json::to_value(&req).unwrap();
        assert_eq!(val["messages"][0]["role"], "system");
        assert_eq!(val["messages"][0]["content"], "you are helpful");
        assert_eq!(val["messages"][1]["role"], "user");
    }
}
