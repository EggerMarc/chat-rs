use crate::tools::OpenAINativeTool;
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
    /// Tool declarations view from `Chat`. The builder calls `.json()`
    /// on it to obtain the JSON array of function decls.
    pub tool_declarations: Option<&'a dyn ToolDeclarations>,
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
        if let Some(decls) = tool_declarations {
            let value = decls.json().map_err(|e| ChatError::Other(e.to_string()))?;
            if let Value::Array(funcs) = value {
                for func in funcs {
                    let mut func = func;
                    func["type"] = json!("function");
                    tools_list.push(func);
                }
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

            // OpenAI's stored response already contains everything up to
            // and including the last Model message. We need to send any
            // *new* information since then: tool results that resolve
            // function_calls in that boundary Model message, plus any
            // user/system content the caller appended afterwards.
            let boundary = messages.0.iter().rposition(|c| c.role == RoleEnum::Model);

            let mut input = Vec::new();

            if let Some(idx) = boundary {
                // Boundary Model message: emit only function_call_output
                // items for tools that now have a response. Skip
                // function_call items themselves — those live in the
                // stored response and re-sending them confuses the API.
                for part in &messages.0[idx].parts.0 {
                    if let PartEnum::Tool(tool) = part {
                        let (_fc, maybe_fr) = tool.to_tuple();
                        if let Some(fr) = maybe_fr {
                            let output = if fr.result.is_string() {
                                fr.result.as_str().unwrap().to_string()
                            } else {
                                fr.result.to_string()
                            };
                            input.push(json!({
                                "type": "function_call_output",
                                "call_id": fr.id.clone().map(String::from).unwrap_or_default(),
                                "output": output,
                            }));
                        }
                    }
                }
            }

            let tail_start = boundary.map(|i| i + 1).unwrap_or(0);
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
                let part_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
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
            PartEnum::File(file) => {
                // OpenAI accepts file_id references via meta["openai_file_id"].
                // When present we short-circuit — the source payload (if any)
                // is redundant because the provider already has the bytes.
                let file_id = file
                    .meta
                    .get("openai_file_id")
                    .and_then(|v| v.as_str());
                let part_type = if file.is_image() {
                    "input_image"
                } else {
                    "input_file"
                };

                if let Some(id) = file_id {
                    message_parts.push(json!({ "type": part_type, "file_id": id }));
                    continue;
                }

                match &file.source {
                    FileSource::Url(url) if file.is_image() => {
                        message_parts.push(json!({
                            "type": "input_image",
                            "image_url": url.to_string(),
                        }));
                    }
                    FileSource::Bytes(bytes) if file.is_image() => {
                        let b64 = STANDARD.encode(bytes);
                        let uri = format!("data:{};base64,{}", file.mime, b64);
                        message_parts.push(json!({
                            "type": "input_image",
                            "image_url": uri,
                        }));
                    }
                    FileSource::Url(url) => {
                        message_parts.push(json!({
                            "type": "input_file",
                            "file_url": url.to_string(),
                        }));
                    }
                    FileSource::Bytes(bytes) => {
                        let b64 = STANDARD.encode(bytes);
                        message_parts.push(json!({
                            "type": "input_file",
                            "filename": "file",
                            "file_data": format!("data:{};base64,{}", file.mime, b64),
                        }));
                    }
                }
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
