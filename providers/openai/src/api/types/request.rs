use crate::tools::OpenAINativeTool;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{Messages, file::File, parts::PartEnum},
        options::ChatOptions,
    },
};
use schemars::Schema;
use serde::Serialize;
use serde_json::{Value, json};
use tools_rs::ToolCollection;

use super::parts::{OpenAIContent, OpenAIWireMessage};

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
    pub messages: Vec<OpenAIWireMessage>,

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
            let decls_value = ct.json().map_err(|e| ChatError::Other(e.to_string()))?;

            if let serde_json::Value::Array(declarations) = decls_value {
                for declaration in declarations {
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

        // Convert each core Content into OpenAIContent, then serialize to wire messages.
        let mut oai_messages = Vec::new();
        for content in &messages.0 {
            let oai_content = OpenAIContent::from(content);
            oai_messages.extend(oai_content.to_wire_messages());
        }
        req.messages = oai_messages;

        Ok(req)
    }
}
