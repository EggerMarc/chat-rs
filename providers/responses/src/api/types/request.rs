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

fn mime_to_ext(mime: &str) -> &'static str {
    match mime {
        "application/pdf" => "pdf",
        "application/json" => "json",
        "application/zip" => "zip",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "text/plain" => "txt",
        "text/csv" => "csv",
        "text/html" => "html",
        "text/markdown" => "md",
        _ => "bin",
    }
}

#[derive(Debug, Serialize)]
pub struct ReasoningConfig {
    pub effort: String,
    pub summary: String,
}

#[derive(Debug, Serialize, Default)]
pub struct ResponsesRequest {
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
    pub reasoning: Option<ReasoningConfig>,

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
    /// Pre-materialized provider-specific tool declarations (e.g.
    /// OpenAI's `web_search`, `image_generation`). The wire layer
    /// drops these into `tools[]` opaquely.
    pub extra_tool_declarations: &'a [Value],
    pub reasoning_effort: Option<String>,
    pub options: Option<&'a ChatOptions>,
    pub output_shape: Option<&'a Schema>,
    pub previous_response_id: Option<String>,
    pub store: Option<bool>,
}

impl ResponsesRequest {
    pub fn from_core(config: ResponsesRequestConfig<'_>) -> Result<Self, ChatError> {
        let ResponsesRequestConfig {
            model_name,
            messages,
            tool_declarations,
            extra_tool_declarations,
            reasoning_effort,
            options,
            output_shape,
            previous_response_id,
            store,
        } = config;
        let mut req = Self {
            model: model_name.to_string(),
            reasoning: reasoning_effort.map(|effort| ReasoningConfig {
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
        for decl in extra_tool_declarations {
            tools_list.push(decl.clone());
        }
        if !tools_list.is_empty() {
            req.tools = Some(tools_list);
        }

        if let Some(prev_id) = previous_response_id {
            req.previous_response_id = Some(prev_id);

            let boundary = messages.0.iter().rposition(|c| c.role == RoleEnum::Model);

            let mut input = Vec::new();

            if let Some(idx) = boundary {
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
                let file_id = file.meta.get("openai_file_id").and_then(|v| v.as_str());
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
                        let filename = file
                            .meta
                            .get("filename")
                            .and_then(|v| v.as_str())
                            .map(str::to_owned)
                            .unwrap_or_else(|| {
                                let ext = mime_to_ext(file.mime.as_str());
                                format!("file.{ext}")
                            });
                        message_parts.push(json!({
                            "type": "input_file",
                            "filename": filename,
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
