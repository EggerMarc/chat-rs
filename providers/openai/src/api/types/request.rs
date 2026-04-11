use crate::tools::OpenAINativeTool;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{
            Messages,
            content::{Content, RoleEnum},
            file::File,
            parts::PartEnum,
        },
        options::ChatOptions,
    },
};
use schemars::Schema;
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

#[derive(Debug, Serialize)]
pub struct OpenAIReasoning {
    pub effort: String,
    pub summary: String,
}

#[derive(Debug, Serialize, Default)]
pub struct OpenAIResponsesRequest {
    pub model: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<OpenAIReasoning>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
}

pub struct ResponsesRequestConfig<'a> {
    pub model_name: &'a str,
    pub messages: &'a Messages,
    /// Pre-computed tool declarations from `Chat`. Already in the
    /// `ToolCollection::json()` shape (a JSON array of function decls).
    pub tool_declarations: Option<&'a Value>,
    pub native_tools: &'a [Box<dyn OpenAINativeTool>],
    pub reasoning_effort: Option<String>,
    pub options: Option<&'a ChatOptions>,
    pub output_shape: Option<&'a Schema>,
    pub previous_response_id: Option<String>,
    pub store: Option<bool>,
}

impl OpenAIResponsesRequest {
    pub fn from_core(config: ResponsesRequestConfig<'_>) -> Result<Self, ChatError> {
        let ResponsesRequestConfig {
            model_name,
            messages,
            tool_declarations,
            native_tools,
            reasoning_effort,
            options,
            output_shape,
            previous_response_id,
            store,
        } = config;
        let mut req = Self {
            model: model_name.to_string(),
            reasoning: reasoning_effort.map(|effort| OpenAIReasoning {
                effort,
                summary: "auto".to_string(),
            }),
            store,
            ..Default::default()
        };

        if let Some(opts) = options {
            req.temperature = opts.temperature;
            req.top_p = opts.top_p;
            req.max_output_tokens = opts.max_tokens;
        }

        if let Some(schema) = output_shape {
            req.text = Some(json!({
                "format": {
                    "type": "json_schema",
                    "name": "structured_output",
                    "strict": false,
                    "schema": schema
                }
            }));
        }

        // Build tools list
        let mut tools_list = Vec::new();
        if let Some(decls) = tool_declarations
            && let Value::Array(funcs) = decls
        {
            for func in funcs {
                let mut func = func.clone();
                func["type"] = json!("function");
                tools_list.push(func);
            }
        }
        for tool in native_tools {
            tools_list.push(tool.to_tool_declaration());
        }
        if !tools_list.is_empty() {
            req.tools = Some(tools_list);
        }

        // Build input items
        if let Some(prev_id) = previous_response_id {
            req.previous_response_id = Some(prev_id);

            // Only send messages after the last model response
            let tail_start = messages
                .0
                .iter()
                .rposition(|c| c.role == RoleEnum::Model)
                .map(|i| i + 1)
                .unwrap_or(0);

            let mut input = Vec::new();
            for content in &messages.0[tail_start..] {
                content_to_input_items(content, &mut input);
            }
            req.input = Some(input);
        } else {
            let mut input = Vec::new();
            let mut instructions = Vec::new();

            for content in &messages.0 {
                if content.role == RoleEnum::System {
                    for part in &content.parts.0 {
                        if let PartEnum::Text(t) = part {
                            instructions.push(t.0.clone());
                        }
                    }
                } else {
                    content_to_input_items(content, &mut input);
                }
            }

            if !instructions.is_empty() {
                req.instructions = Some(instructions.join("\n"));
            }
            req.input = Some(input);
        }

        Ok(req)
    }
}

fn content_to_input_items(content: &Content, items: &mut Vec<Value>) {
    let role = match content.role {
        RoleEnum::User => "user",
        RoleEnum::Model => "assistant",
        RoleEnum::System => "system",
    };

    let mut message_parts: Vec<Value> = Vec::new();

    for part in &content.parts.0 {
        match part {
            PartEnum::Text(t) => {
                let part_type = if role == "assistant" { "output_text" } else { "input_text" };
                message_parts.push(json!({ "type": part_type, "text": t.0 }));
            }
            PartEnum::Reasoning(r) => {
                message_parts.push(json!({ "type": "input_text", "text": r.text }));
            }
            PartEnum::Tool(tool) => {
                let (fc, maybe_fr) = tool.to_tuple();
                items.push(json!({
                    "type": "function_call",
                    "call_id": fc.id.clone().map(String::from).unwrap_or_default(),
                    "name": fc.name,
                    "arguments": serde_json::to_string(&fc.arguments).unwrap_or_default(),
                }));
                if let Some(fr) = maybe_fr {
                    let output = if fr.result.is_string() {
                        fr.result.as_str().unwrap().to_string()
                    } else {
                        fr.result.to_string()
                    };
                    items.push(json!({
                        "type": "function_call_output",
                        "call_id": fr.id.clone().map(String::from).unwrap_or_default(),
                        "output": output,
                    }));
                }
            }
            PartEnum::File(File::Url(u)) => {
                message_parts.push(json!({
                    "type": "input_image",
                    "image_url": u.url.to_string(),
                }));
            }
            PartEnum::File(File::Bytes(b)) => {
                let b64 = STANDARD.encode(&b.bytes);
                let uri = format!("data:{};base64,{}", b.mimetype, b64);
                message_parts.push(json!({
                    "type": "input_image",
                    "image_url": uri,
                }));
            }
            PartEnum::Structured(_) | PartEnum::Embeddings(_) => {}
        }
    }

    if !message_parts.is_empty() {
        items.push(json!({
            "role": role,
            "content": message_parts,
        }));
    }
}
