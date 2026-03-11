use chat_core::{
    error::ChatError,
    types::{
        messages::{content::RoleEnum, file::File, parts::PartEnum, Messages},
        options::ChatOptions,
    },
};
use schemars::Schema;
use serde::Serialize;
use serde_json::{json, Value};
use tools_rs::ToolCollection;

use crate::tools::GeminiNativeTool;
use base64::{engine::general_purpose::STANDARD, Engine as _};

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<GeminiToolConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<GeminiInlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<GeminiFileData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFileData {
    pub file_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiInlineData {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiToolConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_calling_config: Option<GeminiFunctionCallingConfig>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCallingConfig {
    pub mode: String, // "AUTO", "ANY", "NONE"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

impl GeminiRequest {
    pub fn from_core(
        messages: &Messages,
        custom_tools: Option<&ToolCollection>,
        native_tools: &[Box<dyn GeminiNativeTool>],
        function_config: Option<&GeminiFunctionCallingConfig>,
        options: Option<&ChatOptions>,
        output_shape: Option<&Schema>,
    ) -> Result<Self, ChatError> {
        let mut req = Self::default();

        // 1. MAP MESSAGES & SYSTEM PROMPT
        let mut gemini_contents = Vec::new();
        let mut system_parts = Vec::new();

        for content in &messages.0 {
            let mut parts = Vec::new();

            for core_part in &content.parts.0 {
                let mut gemini_part = GeminiPart::default();
                match core_part {
                    PartEnum::Text(t) => gemini_part.text = Some(t.0.clone()),
                    PartEnum::Reasoning(r) => gemini_part.text = Some(r.0.clone()),
                    PartEnum::FunctionCall(fc) => {
                        gemini_part.function_call = Some(GeminiFunctionCall {
                            name: fc.name.clone(),
                            args: fc.arguments.clone(),
                        });
                    }
                    PartEnum::FunctionResponse(fr) => {
                        gemini_part.function_response = Some(GeminiFunctionResponse {
                            name: fr.name.clone(),
                            response: if fr.result.is_object() {
                                fr.result.clone()
                            } else {
                                json!({ "content": fr.result })
                            },
                        });
                    }
                    PartEnum::File(file) => match file {
                        File::Bytes(raw_data) => {
                            let encoded_data = STANDARD.encode(&raw_data.bytes);
                            gemini_part.inline_data = Some(GeminiInlineData {
                                mime_type: raw_data.mimetype.to_string(),
                                file: encoded_data,
                            });
                        }
                        File::Url(url_data) => {
                            gemini_part.file_data = Some(GeminiFileData {
                                file_uri: url_data.url.to_string(),
                                mime_type: url_data.mimetype.as_ref().map(|m| m.to_string()),
                            });
                        }
                    },
                    _ => {}
                }
                parts.push(gemini_part);
            }

            if content.role == RoleEnum::System {
                system_parts.extend(parts);
            } else {
                let role_str = match content.role {
                    RoleEnum::User => "user",
                    _ => "model",
                };
                let is_func_response = content
                    .parts
                    .0
                    .iter()
                    .any(|p| matches!(p, PartEnum::FunctionResponse(_)));

                gemini_contents.push(GeminiContent {
                    role: if is_func_response {
                        "function".to_string()
                    } else {
                        role_str.to_string()
                    },
                    parts,
                });
            }
        }

        req.contents = gemini_contents;
        if !system_parts.is_empty() {
            req.system_instruction = Some(GeminiContent {
                role: "user".to_string(),
                parts: system_parts,
            });
        }

        let mut gen_config = GeminiGenerationConfig::default();
        if let Some(opts) = options {
            gen_config.temperature = opts.temperature;
            gen_config.top_p = opts.top_p;
            gen_config.max_output_tokens = opts.max_tokens;
            gen_config.stop_sequences = opts.stop_sequences.clone();
        }

        if let Some(schema) = output_shape {
            gen_config.response_mime_type = Some("application/json".to_string());
            let mut clean_schema = serde_json::to_value(schema)
                .map_err(|e| ChatError::Other(format!("Schema error: {}", e)))?;
            sanitize_schema_for_gemini(&mut clean_schema);
            gen_config.response_schema = Some(clean_schema);
        }

        if serde_json::to_value(&gen_config)
            .unwrap()
            .as_object()
            .unwrap()
            .len()
            > 0
        {
            req.generation_config = Some(gen_config);
        }

        let mut tools_list = Vec::new();
        let mut tool_config_extras = serde_json::Map::new();

        if let Some(ct) = custom_tools {
            let decls = ct.json().map_err(|e| ChatError::Other(e.to_string()))?;
            tools_list.push(json!({ "functionDeclarations": decls }));
        }

        for tool in native_tools {
            tools_list.push(tool.to_tool_declaration());
            if let Some((k, v)) = tool.to_tool_config() {
                tool_config_extras.insert(k, v);
            }
        }

        if !tools_list.is_empty() {
            req.tools = Some(tools_list);
        }

        let mut req_tool_config = GeminiToolConfig {
            extra: tool_config_extras,
            ..Default::default()
        };
        let mut has_config = !req_tool_config.extra.is_empty();

        if let Some(fc) = function_config {
            has_config = true;
            req_tool_config.function_calling_config = Some(GeminiFunctionCallingConfig {
                mode: fc.mode.clone().unwrap_or_else(|| "AUTO".to_string()),
                allowed_function_names: fc.allowed_function_names.clone(),
            });
        }

        if has_config {
            req.tool_config = Some(req_tool_config);
        }

        Ok(req)
    }
}

/// Recursively removes JSON Schema fields that Gemini rejects
fn sanitize_schema_for_gemini(schema: &mut Value) {
    if let Value::Object(map) = schema {
        map.remove("$schema");
        map.remove("title");
        map.remove("$id");
        map.remove("additionalProperties");
        map.remove("definitions");

        let keys: Vec<String> = map.keys().cloned().collect();
        for key in keys {
            if let Some(v) = map.get_mut(&key) {
                sanitize_schema_for_gemini(v);
            }
        }
    } else if let Value::Array(arr) = schema {
        for v in arr {
            sanitize_schema_for_gemini(v);
        }
    }
}
