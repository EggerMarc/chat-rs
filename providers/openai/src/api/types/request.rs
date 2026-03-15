use crate::tools::OpenAINativeTool;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{Messages, content::RoleEnum, file::File, parts::PartEnum},
        options::ChatOptions,
    },
};
use schemars::Schema;
use serde::Serialize;
use serde_json::{Value, json};
use tools_rs::ToolCollection;

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
                PartEnum::File(File::Bytes(b)) => {
                    let b64 = STANDARD.encode(&b.bytes);
                    let uri = format!("data:{};base64,{}", b.mimetype, b64);
                    parts.push(json!(uri));
                }
                PartEnum::File(File::Url(u)) => {
                    parts.push(json!(u.url.to_string()));
                }
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

#[derive(Debug, Serialize, Default)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OpenAIReasoning>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct OpenAIReasoning {
    pub effort: String,
    /// Controls whether reasoning tokens are returned in the response.
    /// Must be set to "auto", "concise", or "detailed" to see reasoning_content.
    pub summary: String,
}

#[derive(Debug, Serialize, Default)]
pub struct OpenAIMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl OpenAIRequest {
    pub fn from_core(
        model_name: &str,
        messages: &Messages,
        custom_tools: Option<&ToolCollection>,
        native_tools: &[Box<dyn OpenAINativeTool>],
        reasoning_effort: Option<String>,
        options: Option<&ChatOptions>,
        output_shape: Option<&Schema>,
    ) -> Result<Self, ChatError> {
        let mut req = Self {
            model: model_name.to_string(),
            reasoning: reasoning_effort.map(|effort| OpenAIReasoning {
                effort,
                summary: "auto".to_string(),
            }),
            ..Default::default()
        };

        if let Some(opts) = options {
            req.temperature = opts.temperature;
            req.top_p = opts.top_p;
            req.max_completion_tokens = opts.max_tokens;
        }

        if let Some(schema) = output_shape {
            req.response_format = Some(json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_output",
                    "strict": false,
                    "schema": schema
                }
            }));
        }

        let mut tools_list = Vec::new();
        if let Some(ct) = custom_tools {
            // 1. Bind the owned Value to a variable so it lives long enough
            let decls_value = ct.json().map_err(|e| ChatError::Other(e.to_string()))?;

            // 2. Destructure the Value directly into its inner Vec (no lifetimes to worry about!)
            if let serde_json::Value::Array(declarations) = decls_value {
                for declaration in declarations {
                    // Ownership of `declaration` is moved directly into the new JSON object
                    tools_list.push(json!({ "type": "function", "function": declaration }));
                }
            } else {
                return Err(ChatError::Other(
                    "Expected tools-rs to output a JSON array".to_string(),
                ));
            }
        }
        for tool in native_tools {
            tools_list.push(tool.to_tool_declaration());
        }
        if !tools_list.is_empty() {
            req.tools = Some(tools_list);
        }

        let mut oai_messages = Vec::new();
        for content in &messages.0 {
            let role_str = match content.role {
                RoleEnum::System => "system",
                RoleEnum::User => "user",
                RoleEnum::Model => "assistant",
            };

            let mut text_parts = Vec::new();
            let mut vision_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut tool_responses = Vec::new();

            for part in &content.parts.0 {
                match part {
                    PartEnum::Text(t) => text_parts.push(t.0.clone()),
                    PartEnum::Reasoning(r) => text_parts.push(r.text.clone()),
                    PartEnum::FunctionCall(fc) => tool_calls.push(fc),
                    PartEnum::FunctionResponse(fr) => tool_responses.push(fr),
                    PartEnum::File(File::Url(u)) => {
                        vision_parts.push(json!({
                            "type": "image_url",
                            "image_url": { "url": u.url.to_string() }
                        }));
                    }
                    PartEnum::File(File::Bytes(b)) => {
                        let b64 = STANDARD.encode(&b.bytes);
                        let uri = format!("data:{};base64,{}", b.mimetype, b64);
                        vision_parts.push(json!({
                            "type": "image_url",
                            "image_url": { "url": uri }
                        }));
                    }
                    _ => {}
                }
            }

            if !text_parts.is_empty() || !vision_parts.is_empty() || !tool_calls.is_empty() {
                let mut msg = OpenAIMessage {
                    role: role_str.to_string(),
                    ..Default::default()
                };

                if !vision_parts.is_empty() {
                    for text in text_parts {
                        vision_parts.insert(0, json!({ "type": "text", "text": text }));
                    }
                    msg.content = Some(Value::Array(vision_parts));
                } else if !text_parts.is_empty() {
                    msg.content = Some(Value::String(text_parts.join("\n")));
                }

                if !tool_calls.is_empty() {
                    msg.tool_calls = Some(tool_calls.into_iter().map(|fc| {
                        json!({
                            "id": fc.id.clone().map(Into::into).unwrap_or("call_unknown".to_string()),
                            "type": "function",
                            "function": {
                                "name": fc.name,
                                "arguments": serde_json::to_string(&fc.arguments).unwrap_or_default()
                            }
                        })
                    }).collect());
                }
                oai_messages.push(msg);
            }

            for fr in tool_responses {
                let content_str = if fr.result.is_string() {
                    fr.result.as_str().unwrap().to_string()
                } else {
                    fr.result.to_string()
                };

                oai_messages.push(OpenAIMessage {
                    role: "tool".to_string(),
                    content: Some(Value::String(content_str)),
                    tool_call_id: Some(
                        fr.id
                            .clone()
                            .map(Into::into)
                            .unwrap_or("call_unknown".to_string()),
                    ),
                    name: Some(fr.name.clone()),
                    ..Default::default()
                });
            }
        }

        req.messages = oai_messages;
        Ok(req)
    }
}
