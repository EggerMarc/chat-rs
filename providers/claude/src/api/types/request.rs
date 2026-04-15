use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{Messages, content::RoleEnum, file::FileSource, parts::PartEnum},
        options::ChatOptions,
        tools::ToolDeclarations,
    },
};
use serde::Serialize;
use serde_json::{Value, json};

pub(crate) const STRUCTURED_OUTPUT_TOOL_NAME: &str = "__structured_output__";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Value>,
    pub messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ClaudeTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,
    pub budget_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct ClaudeTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl ClaudeRequest {
    pub fn from_core(
        model_name: &str,
        messages: &Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
        stream: bool,
        thinking_budget: Option<u32>,
    ) -> Result<Self, ChatError> {
        let mut claude_messages: Vec<ClaudeMessage> = Vec::new();
        let mut system_blocks: Vec<Value> = Vec::new();

        for content in &messages.0 {
            if content.role == RoleEnum::System {
                for part in &content.parts.0 {
                    match part {
                        PartEnum::Text(t) => {
                            system_blocks.push(json!({"type": "text", "text": t.0}));
                        }
                        PartEnum::Reasoning(r) => {
                            system_blocks.push(json!({"type": "text", "text": r.text}));
                        }
                        _ => continue,
                    }
                }
                continue;
            }

            let assistant_role = match content.role {
                RoleEnum::User => "user",
                RoleEnum::Model => "assistant",
                RoleEnum::System => unreachable!(),
            };

            // Claude requires tool_use blocks to live in assistant turns
            // and tool_result blocks to live in following user turns. A
            // single core Content can carry both — we split into two
            // sequential Claude messages.
            let mut assistant_blocks: Vec<Value> = Vec::new();
            let mut tool_result_blocks: Vec<Value> = Vec::new();

            for part in &content.parts.0 {
                match part {
                    PartEnum::Text(t) => {
                        assistant_blocks.push(json!({"type": "text", "text": t.0}));
                    }
                    PartEnum::Reasoning(r) => {
                        let mut block = json!({
                            "type": "thinking",
                            "thinking": r.text,
                        });
                        if let Some(sig) = &r.signature {
                            block
                                .as_object_mut()
                                .unwrap()
                                .insert("signature".to_string(), json!(sig));
                        }
                        assistant_blocks.push(block);
                    }
                    PartEnum::Tool(tool) => {
                        let (fc, maybe_fr) = tool.to_tuple();
                        let id = fc.id.as_ref().map(|id| id.to_string()).unwrap_or_default();
                        assistant_blocks.push(json!({
                            "type": "tool_use",
                            "id": id,
                            "name": fc.name,
                            "input": fc.arguments,
                        }));

                        if let Some(fr) = maybe_fr {
                            let tool_use_id =
                                fr.id.as_ref().map(|id| id.to_string()).unwrap_or_default();
                            let content_val = if fr.result.is_string() {
                                fr.result.as_str().unwrap().to_string()
                            } else {
                                fr.result.to_string()
                            };
                            tool_result_blocks.push(json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": content_val,
                            }));
                        }
                    }
                    PartEnum::File(file) => match &file.source {
                        FileSource::Bytes(bytes) => {
                            let encoded = STANDARD.encode(bytes);
                            assistant_blocks.push(json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": file.mime.to_string(),
                                    "data": encoded,
                                }
                            }));
                        }
                        FileSource::Url(url) => {
                            assistant_blocks.push(json!({
                                "type": "image",
                                "source": {
                                    "type": "url",
                                    "url": url.to_string(),
                                }
                            }));
                        }
                    },
                    PartEnum::Structured(v) => {
                        assistant_blocks.push(json!({"type": "text", "text": v.to_string()}));
                    }
                    PartEnum::Embeddings(_) => continue,
                }
            }

            let push_message = |msgs: &mut Vec<ClaudeMessage>, role: &str, blocks: Vec<Value>| {
                if blocks.is_empty() {
                    return;
                }
                if let Some(last) = msgs.last_mut()
                    && last.role == role
                {
                    last.content.extend(blocks);
                    return;
                }
                msgs.push(ClaudeMessage {
                    role: role.to_string(),
                    content: blocks,
                });
            };

            push_message(&mut claude_messages, assistant_role, assistant_blocks);
            push_message(&mut claude_messages, "user", tool_result_blocks);
        }

        // Build tools list
        let mut tools_list: Vec<ClaudeTool> = Vec::new();

        if let Some(decls) = tool_declarations {
            let value = decls.json().map_err(|e| ChatError::Other(e.to_string()))?;
            if let Value::Array(arr) = value {
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
                    let input_schema = decl
                        .get("parameters")
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object"}));

                    tools_list.push(ClaudeTool {
                        name,
                        description,
                        input_schema,
                    });
                }
            }
        }

        // Structured output via synthetic tool
        let mut tool_choice = None;
        if let Some(schema) = structured_output {
            let schema_val = serde_json::to_value(schema)
                .map_err(|e| ChatError::Other(format!("Schema error: {}", e)))?;

            tools_list.push(ClaudeTool {
                name: STRUCTURED_OUTPUT_TOOL_NAME.to_string(),
                description: "Return structured output matching the given schema.".to_string(),
                input_schema: schema_val,
            });

            tool_choice = Some(json!({
                "type": "tool",
                "name": STRUCTURED_OUTPUT_TOOL_NAME,
            }));
        }

        // Options
        let mut temperature = None;
        let mut top_p = None;
        let mut max_tokens = DEFAULT_MAX_TOKENS;

        if let Some(opts) = options {
            temperature = opts.temperature;
            top_p = opts.top_p;
            if let Some(mt) = opts.max_tokens {
                max_tokens = mt;
            }
        }

        let system = if system_blocks.is_empty() {
            None
        } else if system_blocks.len() == 1 {
            system_blocks[0].get("text").cloned()
        } else {
            Some(Value::Array(system_blocks))
        };

        // Thinking config — when enabled, temperature must be unset (Claude requires it)
        let thinking = thinking_budget.map(|budget| {
            temperature = None;
            ThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: budget,
            }
        });

        Ok(ClaudeRequest {
            model: model_name.to_string(),
            max_tokens,
            system,
            messages: claude_messages,
            tools: if tools_list.is_empty() {
                None
            } else {
                Some(tools_list)
            },
            tool_choice,
            temperature,
            top_p,
            stream: if stream { Some(true) } else { None },
            thinking,
        })
    }
}
