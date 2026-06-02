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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_modalities: Option<Vec<String>>,
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
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

impl GeminiRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn from_core(
        messages: &Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        native_tools: Option<&[Box<dyn GeminiNativeTool>]>,
        function_config: Option<&GeminiFunctionCallingConfig>,
        options: Option<&ChatOptions>,
        output_shape: Option<&schemars::Schema>,
        include_thoughts: bool,
        response_modalities: Option<&[String]>,
    ) -> Result<Self, ChatError> {
        let mut req = Self::default();

        let mut gemini_contents: Vec<GeminiContent> = Vec::new();
        let mut system_parts = Vec::new();

        for content in &messages.0 {
            let mut assistant_parts: Vec<GeminiPart> = Vec::new();
            let mut function_parts: Vec<GeminiPart> = Vec::new();

            for core_part in &content.parts.0 {
                match core_part {
                    PartEnum::Text(t) => {
                        assistant_parts.push(GeminiPart {
                            text: Some(t.0.clone()),
                            ..Default::default()
                        });
                    }
                    PartEnum::Reasoning(r) => {
                        assistant_parts.push(GeminiPart {
                            text: Some(r.text.clone()),
                            thought: true,
                            thought_signature: r.signature.clone(),
                            ..Default::default()
                        });
                    }
                    PartEnum::Tool(tool) => {
                        let (fc, maybe_fr) = tool.to_tuple();
                        assistant_parts.push(GeminiPart {
                            function_call: Some(GeminiFunctionCall {
                                name: fc.name.clone(),
                                args: fc.arguments.clone(),
                                id: fc.id.clone().map(Into::into),
                            }),
                            thought_signature: fc.id.clone().map(Into::into),
                            ..Default::default()
                        });

                        if let Some(fr) = maybe_fr {
                            function_parts.push(GeminiPart {
                                function_response: Some(GeminiFunctionResponse {
                                    name: fr.name.clone(),
                                    response: if fr.result.is_object() {
                                        fr.result.clone()
                                    } else {
                                        json!({ "content": fr.result })
                                    },
                                }),
                                ..Default::default()
                            });
                        }
                    }
                    PartEnum::File(file) => {
                        let mut gp = GeminiPart::default();
                        match &file.source {
                            FileSource::Bytes(bytes) => {
                                let encoded_data = STANDARD.encode(bytes);
                                gp.inline_data = Some(GeminiInlineData {
                                    mime_type: Some(file.mime.to_string()),
                                    data: encoded_data,
                                });
                            }
                            FileSource::Url(url) => {
                                gp.file_data = Some(GeminiFileData {
                                    file_uri: url.to_string(),
                                    mime_type: Some(file.mime.to_string()),
                                });
                            }
                        }
                        assistant_parts.push(gp);
                    }
                    PartEnum::Structured(json_val) => {
                        assistant_parts.push(GeminiPart {
                            text: Some(json_val.to_string()),
                            ..Default::default()
                        });
                    }
                    PartEnum::Embeddings(_) => {
                        println!("Skipping Embeddings part in Gemini completion request.");
                    }
                }
            }

            if content.role == RoleEnum::System {
                system_parts.extend(assistant_parts);
                continue;
            }

            let assistant_role = match content.role {
                RoleEnum::User => "user",
                _ => "model",
            };

            let push_entry =
                |contents: &mut Vec<GeminiContent>, role: &str, parts: Vec<GeminiPart>| {
                    if parts.is_empty() {
                        return;
                    }
                    if let Some(last) = contents.last_mut()
                        && last.role == role
                    {
                        last.parts.extend(parts);
                        return;
                    }
                    contents.push(GeminiContent {
                        role: role.to_string(),
                        parts,
                    });
                };

            push_entry(&mut gemini_contents, assistant_role, assistant_parts);
            push_entry(&mut gemini_contents, "function", function_parts);
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

        if let Some(modalities) = response_modalities {
            gen_config.response_modalities = Some(modalities.to_vec());
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

        if let Some(decls) = tool_declarations {
            let value = decls.json().map_err(|e| ChatError::Other(e.to_string()))?;
            tools_list.push(json!({ "functionDeclarations": value }));
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
            EmbeddingsTask::Embed => None,
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
