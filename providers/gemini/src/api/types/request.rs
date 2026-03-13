use chat_core::{
    error::ChatError,
    types::{
        messages::{Messages, content::RoleEnum, file::File, parts::PartEnum},
        options::ChatOptions,
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use tools_rs::ToolCollection;

use crate::tools::GeminiNativeTool;
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Default, Clone)]
pub enum EmbeddingsTask {
    SemanticSimilarity,
    Classification,
    Clustering,
    RetrievalDocument,
    RetrievalQuery,
    #[default]
    Embed,
}

#[derive(Clone, Default)]
pub(crate) struct GeminiEmbeddingsConfig {
    pub dimensions: Option<usize>,
    pub task: EmbeddingsTask,
}
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeminiThinkingConfig {
    pub include_thoughts: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
    pub thought: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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
    pub data: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<GeminiThinkingConfig>,
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
        native_tools: Option<&[Box<dyn GeminiNativeTool>]>,
        function_config: Option<&GeminiFunctionCallingConfig>,
        options: Option<&ChatOptions>,
        output_shape: Option<&schemars::Schema>,
        include_thoughts: bool,
    ) -> Result<Self, ChatError> {
        let mut req = Self::default();

        let mut gemini_contents = Vec::new();
        let mut system_parts = Vec::new();

        for content in &messages.0 {
            let mut parts = Vec::new();

            for core_part in &content.parts.0 {
                let mut gemini_part = GeminiPart::default();
                match core_part {
                    PartEnum::Text(t) => {
                        gemini_part.text = Some(t.0.clone());
                    }
                    PartEnum::Reasoning(r) => {
                        gemini_part.text = Some(r.text.clone());
                        gemini_part.thought = true;
                        gemini_part.thought_signature = r.signature.clone();
                    }
                    PartEnum::FunctionCall(fc) => {
                        gemini_part.function_call = Some(GeminiFunctionCall {
                            name: fc.name.clone(),
                            args: fc.arguments.clone(),
                            id: fc.id.clone().map(Into::into),
                        });
                        gemini_part.thought_signature = fc.id.clone().map(Into::into);
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
                                mime_type: Some(raw_data.mimetype.to_string()),
                                data: encoded_data,
                            });
                        }
                        File::Url(url_data) => {
                            gemini_part.file_data = Some(GeminiFileData {
                                file_uri: url_data.url.to_string(),
                                mime_type: url_data.mimetype.as_ref().map(|m| m.to_string()),
                            });
                        }
                    },
                    PartEnum::Structured(json_val) => {
                        gemini_part.text = Some(json_val.to_string());
                    }
                    PartEnum::Embeddings(_) => {
                        println!("Skipping Embeddings part in Gemini completion request.");
                        continue;
                    }
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

        if include_thoughts {
            gen_config.thinking_config = Some(GeminiThinkingConfig {
                include_thoughts: true,
            });
        }

        if let Some(opts) = options {
            gen_config.temperature = opts.temperature;
            gen_config.top_p = opts.top_p;
            gen_config.max_output_tokens = opts.max_tokens;
            gen_config.stop_sequences = opts
                .metadata
                .get("stop_sequences")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<String>>()
                });
        }

        if let Some(schema) = output_shape {
            gen_config.response_mime_type = Some("application/json".to_string());
            let mut clean_schema = serde_json::to_value(schema)
                .map_err(|e| ChatError::Other(format!("Schema error: {}", e)))?;
            sanitize_schema_for_gemini(&mut clean_schema);
            gen_config.response_schema = Some(clean_schema);
        }

        if !serde_json::to_value(&gen_config)
            .unwrap()
            .as_object()
            .unwrap()
            .is_empty()
        {
            req.generation_config = Some(gen_config);
        }

        let mut tools_list = Vec::new();
        let mut tool_config_extras = serde_json::Map::new();

        if let Some(ct) = custom_tools {
            let decls = ct.json().map_err(|e| ChatError::Other(e.to_string()))?;
            tools_list.push(json!({ "functionDeclarations": decls }));
        }
        if let Some(tools) = native_tools {
            for tool in tools {
                tools_list.push(tool.to_tool_declaration());
                if let Some((k, v)) = tool.to_tool_config() {
                    tool_config_extras.insert(k, v);
                }
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
                mode: fc.mode.clone(),
                allowed_function_names: fc.allowed_function_names.clone(),
            });
        }

        if has_config {
            req.tool_config = Some(req_tool_config);
        }

        Ok(req)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiEmbeddingRequest {
    pub content: GeminiContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<&'static str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dimensionality: Option<usize>,
}

impl GeminiEmbeddingRequest {
    pub fn from_core(
        messages: &Messages,
        config: Option<&GeminiEmbeddingsConfig>,
    ) -> Result<Self, ChatError> {
        let last_content = messages
            .0
            .last()
            .ok_or_else(|| ChatError::InvalidResponse("Sent empty content to embed".to_string()))?;

        let mut parts = Vec::new();
        for part in &last_content.parts.0 {
            match part {
                PartEnum::Text(t) => parts.push(GeminiPart {
                    text: Some(t.0.clone()),
                    ..Default::default()
                }),
                PartEnum::Reasoning(r) => parts.push(GeminiPart {
                    text: Some(r.text.clone()),
                    ..Default::default()
                }),
                _ => {
                    return Err(ChatError::InvalidResponse(
                        "Embeddings require text-like parts".to_string(),
                    ));
                }
            }
        }

        if parts.is_empty() {
            return Err(ChatError::InvalidResponse(
                "Sent empty content to embed".to_string(),
            ));
        }

        let content = GeminiContent {
            role: "user".to_string(),
            parts,
        };

        let mut req = Self {
            content,
            task_type: None,
            output_dimensionality: None,
        };

        if let Some(cfg) = config {
            req.task_type = cfg.task.as_str();
            req.output_dimensionality = cfg.dimensions;
        }

        Ok(req)
    }
}

impl EmbeddingsTask {
    pub fn as_str(&self) -> Option<&'static str> {
        match self {
            EmbeddingsTask::SemanticSimilarity => Some("SEMANTIC_SIMILARITY"),
            EmbeddingsTask::Classification => Some("CLASSIFICATION"),
            EmbeddingsTask::Clustering => Some("CLUSTERING"),
            EmbeddingsTask::RetrievalDocument => Some("RETRIEVAL_DOCUMENT"),
            EmbeddingsTask::RetrievalQuery => Some("RETRIEVAL_QUERY"),
            EmbeddingsTask::Embed => None, // "TASK_TYPE_UNSPECIFIED"
        }
    }
}

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
